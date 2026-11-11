"""The agent's side of the wire to ehr (EHR-SERVICE.md): mTLS, a deadline on every call, and
the edge.

The edge: no code past this file sees a wire message. Each reply becomes the agent's own
types, and each marked field becomes a `Shielded`. A request is built here from `Shielded`
values, which is one of the few places that may `reveal()` them.

Errors become an `EhrError` with a kind. The server's error text is fixed and PHI-free, but it
is dropped anyway: the agent acts on the kind only.
"""

from __future__ import annotations

import asyncio
import enum
import time
from dataclasses import dataclass
from pathlib import Path

import grpc

from deets_agent.proto import pb, pb_grpc
from deets_agent.shielded import Shielded

# The longest the agent waits for one ehr call. A slow EHR must not hold the caller in silence
# (EHR-SERVICE.md §2); ehr stops its work when this passes.
DEADLINE_S = 3.0
# A booking may take longer at a real EHR. It runs to its end in ehr even if this passes.
BOOK_DEADLINE_S = 8.0


class ErrorKind(enum.Enum):
    NO_SUCH_CALL = "no_such_call"
    NOT_VERIFIED = "not_verified"
    BAD_INPUT = "bad_input"
    NOT_ALLOWED_NOW = "not_allowed_now"
    UNAVAILABLE = "unavailable"
    OTHER = "other"


_KINDS = {
    grpc.StatusCode.UNAUTHENTICATED: ErrorKind.NO_SUCH_CALL,
    grpc.StatusCode.PERMISSION_DENIED: ErrorKind.NOT_VERIFIED,
    grpc.StatusCode.INVALID_ARGUMENT: ErrorKind.BAD_INPUT,
    grpc.StatusCode.FAILED_PRECONDITION: ErrorKind.NOT_ALLOWED_NOW,
    grpc.StatusCode.UNAVAILABLE: ErrorKind.UNAVAILABLE,
    grpc.StatusCode.DEADLINE_EXCEEDED: ErrorKind.UNAVAILABLE,
}


class EhrError(Exception):
    def __init__(self, kind: ErrorKind, call: str) -> None:
        super().__init__(f"{call}: {kind.value}")
        self.kind = kind


class CheckOutcome(enum.Enum):
    PASS = "pass"
    FAIL = "fail"
    LOCKED = "locked"


@dataclass(frozen=True)
class Slot:
    """An open slot, or a booked one. The token is for ehr only; the model never sees it."""

    token: Shielded[str]
    starts_at: Shielded[str]
    minutes: int
    clinician: Shielded[str]
    place: Shielded[str]


@dataclass(frozen=True)
class Visit:
    starts_at: Shielded[str]
    minutes: int
    clinician: Shielded[str]
    place: Shielded[str]


def _slot(s) -> Slot:
    return Slot(
        token=Shielded(s.token),
        starts_at=Shielded(s.starts_at),
        minutes=s.minutes,
        clinician=Shielded(s.clinician),
        place=Shielded(s.place),
    )


class EhrClient:
    """One connection, opened when the agent starts, so no TLS handshake falls inside a call."""

    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel
        self._stub = pb_grpc.EhrStub(channel)
        # The dev call record (call_record.py), set by the typed harness only. It gets each
        # call's name, time and outcome; never a value.
        self.record = None

    @classmethod
    async def connect(cls, target: str, certs: Path, ready_s: float = 5.0) -> EhrClient:
        """mTLS with the agent's certificate; only a server the CA signed as `ehr` is trusted."""
        creds = grpc.ssl_channel_credentials(
            root_certificates=(certs / "ca.pem").read_bytes(),
            private_key=(certs / "agent.key").read_bytes(),
            certificate_chain=(certs / "agent.pem").read_bytes(),
        )
        channel = grpc.aio.secure_channel(
            target, creds, options=[("grpc.ssl_target_name_override", "ehr")]
        )
        try:
            await asyncio.wait_for(channel.channel_ready(), timeout=ready_s)
        except TimeoutError:
            await channel.close()
            raise EhrError(ErrorKind.UNAVAILABLE, "connect") from None
        return cls(channel)

    async def close(self) -> None:
        await self._channel.close()

    async def _call(self, name: str, request, deadline: float = DEADLINE_S):
        start = time.perf_counter()
        outcome = "ok"
        try:
            return await getattr(self._stub, name)(request, timeout=deadline)
        except grpc.aio.AioRpcError as e:
            outcome = e.code().name
            raise EhrError(_KINDS.get(e.code(), ErrorKind.OTHER), name) from None
        finally:
            if self.record is not None:
                self.record.ehr(name, round((time.perf_counter() - start) * 1000), outcome)

    # ---- the calls ----

    async def open_call(self) -> Shielded[str]:
        reply = await self._call("OpenCall", pb.OpenCallRequest())
        return Shielded(reply.handle)

    async def close_call(self, handle: Shielded[str]) -> None:
        await self._call("CloseCall", pb.CloseCallRequest(handle=handle.reveal()))

    async def identify(
        self,
        handle: Shielded[str],
        given: Shielded[str],
        family: Shielded[str],
        birth_date: Shielded[str],
    ) -> int:
        """How many tries are left. The reply never says whether a patient matched."""
        reply = await self._call(
            "Identify",
            pb.IdentifyRequest(
                handle=handle.reveal(),
                given_name=given.reveal(),
                family_name=family.reveal(),
                birth_date=birth_date.reveal(),
            ),
        )
        return reply.tries_left

    async def resend_code(self, handle: Shielded[str]) -> int:
        reply = await self._call("ResendCode", pb.ResendCodeRequest(handle=handle.reveal()))
        return reply.resends_left

    async def check_code(
        self, handle: Shielded[str], code: Shielded[str]
    ) -> tuple[CheckOutcome, int]:
        reply = await self._call(
            "CheckCode", pb.CheckCodeRequest(handle=handle.reveal(), code=code.reveal())
        )
        outcome = {
            pb.CHECK_OUTCOME_PASS: CheckOutcome.PASS,
            pb.CHECK_OUTCOME_LOCKED: CheckOutcome.LOCKED,
        }.get(reply.outcome, CheckOutcome.FAIL)
        return outcome, reply.tries_left

    async def find_slots(
        self, handle: Shielded[str], reason: Shielded[str], earliest_day: Shielded[str] | None
    ) -> list[Slot]:
        reply = await self._call(
            "FindSlots",
            pb.FindSlotsRequest(
                handle=handle.reveal(),
                reason=reason.reveal(),
                earliest_day=earliest_day.reveal() if earliest_day else "",
            ),
        )
        return [_slot(s) for s in reply.slots]

    async def book(
        self, handle: Shielded[str], slot_token: Shielded[str], idempotency_key: str
    ) -> Slot | None:
        """The booked slot, or None when someone else took it first."""
        reply = await self._call(
            "Book",
            pb.BookRequest(
                handle=handle.reveal(),
                slot_token=slot_token.reveal(),
                idempotency_key=idempotency_key,
            ),
            deadline=BOOK_DEADLINE_S,
        )
        if reply.outcome == pb.BOOK_OUTCOME_BOOKED and reply.HasField("booked"):
            return _slot(reply.booked)
        return None

    async def next_visit(self, handle: Shielded[str]) -> Visit | None:
        reply = await self._call("NextVisit", pb.NextVisitRequest(handle=handle.reveal()))
        if not reply.HasField("visit"):
            return None
        v = reply.visit
        return Visit(
            starts_at=Shielded(v.starts_at),
            minutes=v.minutes,
            clinician=Shielded(v.clinician),
            place=Shielded(v.place),
        )
