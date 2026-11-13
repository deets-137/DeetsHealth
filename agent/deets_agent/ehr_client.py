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
import json
import time
from dataclasses import dataclass, field
from pathlib import Path

import grpc

from deets_agent.proto import pb, pb_grpc
from deets_agent.shielded import Shielded

# The longest the agent waits for one ehr call. A slow EHR must not hold the caller in silence
# (EHR-SERVICE.md §2); ehr stops its work when this passes.
DEADLINE_S = 3.0
# Filing a whole record after the call: no caller waits on it, so a longer deadline.
FILE_DEADLINE_S = 15.0
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
        # SaveSettings only: which setting did not fit (settings.rs names it, never a value).
        self.detail = ""


class _NoStore(Exception):
    """ehr runs without a store: the settings fall back to the code's defaults."""


@dataclass(frozen=True)
class SettingVersion:
    """One saved version of a service's settings (CONSOLE.md §7.3). No value here is protected
    data."""

    id: int
    day: str  # YYYY-MM-DD
    seq: int
    created_at: str
    created_by: str
    active: bool

    @property
    def label(self) -> str:
        """The owner's label: "MM-DD | seq", or "Default" for number 0 (the code's defaults)."""
        return "Default" if self.seq == 0 else f"{self.day[5:]} | {self.seq}"


def _version(v) -> SettingVersion:
    return SettingVersion(v.id, v.day, v.seq, v.created_at, v.created_by, v.active)


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
    # The record's visit type and reason ("" for none), after ehr's sensitive screen
    # (FUTURE-SETTINGS.md §9). `reason_private`: ehr dropped both, and the agent may say only
    # that it cannot share what the visit is for.
    visit_type: Shielded[str] = field(default_factory=lambda: Shielded(""))
    reason: Shielded[str] = field(default_factory=lambda: Shielded(""))
    online: bool = False
    reason_private: bool = False


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

    async def _call(self, name: str, request, deadline: float = DEADLINE_S, result=None, left=None):
        """`result`, when given, turns the reply into a domain word for the record (`booked`,
        `pass`): an enum's name, never a value. `left`, when given, reads the verification
        limits left from the reply, as counts (the Step panel, CHAT.md §11)."""
        start = time.perf_counter()
        outcome = "ok"
        word = None
        counts = None
        try:
            reply = await getattr(self._stub, name)(request, timeout=deadline)
            if result is not None:
                word = result(reply)
            if left is not None:
                counts = left(reply)
            return reply
        except grpc.aio.AioRpcError as e:
            outcome = e.code().name
            raise EhrError(_KINDS.get(e.code(), ErrorKind.OTHER), name) from None
        finally:
            if self.record is not None:
                ms = round((time.perf_counter() - start) * 1000, 1)
                self.record.ehr(name, ms, outcome, word, counts)

    # ---- the calls ----

    async def open_call(self, ehr: str = "local") -> Shielded[str]:
        """`ehr`: `local` (ehr's fakes) or `epic` (Epic's public sandbox; dev only)."""
        source = {"local": pb.EHR_SOURCE_LOCAL, "epic": pb.EHR_SOURCE_EPIC_SANDBOX}[ehr]
        reply = await self._call(
            "OpenCall",
            pb.OpenCallRequest(ehr=source),
            left=lambda r: {"identify": r.identifies_left, "resend": r.resends_left, "code": r.code_tries_left},
        )
        return Shielded(reply.handle)

    async def close_call(self, handle: Shielded[str]) -> None:
        await self._call("CloseCall", pb.CloseCallRequest(handle=handle.reveal()))

    async def file_call(self, call_id: str, record: Path) -> bool | None:
        """File one ended call's record into the call store (DATABASE.md §4a), one line per
        message. True when filed, False when the same record was filed before, None when ehr
        runs without a store (the next `ehr ingest` files it). The record is closed by now, so
        this is not written to it."""

        async def lines():
            for line in record.read_text(encoding="utf-8").splitlines():
                if line.strip():
                    yield pb.FileCallLine(call_id=call_id, line=line)

        try:
            reply = await self._stub.FileCall(lines(), timeout=FILE_DEADLINE_S)
        except grpc.aio.AioRpcError as e:
            if e.code() == grpc.StatusCode.FAILED_PRECONDITION:
                return None
            raise EhrError(_KINDS.get(e.code(), ErrorKind.OTHER), "FileCall") from None
        return reply.filed

    # ---- the settings store (CONSOLE.md §7.3) ----

    async def list_setting_versions(self, service: str) -> list[SettingVersion] | None:
        """Every saved version of a service's settings, newest first; None when ehr runs without
        a store."""
        try:
            reply = await self._settings_call("ListSettingVersions", pb.ListSettingVersionsRequest(service=service))
        except _NoStore:
            return None
        return [_version(v) for v in reply.versions]

    async def get_settings(self, service: str, version_id: int = 0) -> tuple[SettingVersion | None, dict] | None:
        """One version and its values (each parsed from JSON): the active one when `version_id`
        is 0. (None, {}) when there is no version yet; None when ehr runs without a store."""
        try:
            reply = await self._settings_call("GetSettings", pb.GetSettingsRequest(service=service, version_id=version_id))
        except _NoStore:
            return None
        version = _version(reply.version) if reply.HasField("version") else None
        return version, {v.key: json.loads(v.value) for v in reply.values}

    async def save_settings(self, service: str, values: dict, by: str, seed: bool = False) -> SettingVersion:
        """Save a whole set as a new, active version; with `seed`, the Default version (active
        only when nothing is). Raises EhrError: BAD_INPUT with the reason in `detail` (a
        setting's name, never a value); NOT_ALLOWED_NOW without a store; OTHER when the Default
        exists already."""
        request = pb.SaveSettingsRequest(
            service=service,
            values=[pb.SettingValue(key=k, value=json.dumps(v)) for k, v in values.items()],
            by=by,
            seed=seed,
        )
        try:
            reply = await self._stub.SaveSettings(request, timeout=DEADLINE_S)
        except grpc.aio.AioRpcError as e:
            error = EhrError(_KINDS.get(e.code(), ErrorKind.OTHER), "SaveSettings")
            if e.code() == grpc.StatusCode.INVALID_ARGUMENT:
                # ehr's settings.rs names the setting only; safe to show.
                error.detail = e.details() or ""
            raise error from None
        return _version(reply.version)

    async def _settings_call(self, name: str, request):
        try:
            return await getattr(self._stub, name)(request, timeout=DEADLINE_S)
        except grpc.aio.AioRpcError as e:
            if e.code() == grpc.StatusCode.FAILED_PRECONDITION:
                raise _NoStore from None
            raise EhrError(_KINDS.get(e.code(), ErrorKind.OTHER), name) from None

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
            left=lambda r: {"identify": r.tries_left},
        )
        return reply.tries_left

    async def resend_code(self, handle: Shielded[str]) -> int:
        reply = await self._call(
            "ResendCode",
            pb.ResendCodeRequest(handle=handle.reveal()),
            left=lambda r: {"resend": r.resends_left},
        )
        return reply.resends_left

    async def check_code(
        self, handle: Shielded[str], code: Shielded[str]
    ) -> tuple[CheckOutcome, int]:
        reply = await self._call(
            "CheckCode",
            pb.CheckCodeRequest(handle=handle.reveal(), code=code.reveal()),
            result=lambda r: {pb.CHECK_OUTCOME_PASS: "pass", pb.CHECK_OUTCOME_LOCKED: "locked"}.get(
                r.outcome, "fail"
            ),
            left=lambda r: {"code": r.tries_left},
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
            result=lambda r: "booked" if r.outcome == pb.BOOK_OUTCOME_BOOKED else "slot_taken",
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
            visit_type=Shielded(v.visit_type),
            reason=Shielded(v.reason),
            online=v.online,
            reason_private=v.reason_private,
        )
