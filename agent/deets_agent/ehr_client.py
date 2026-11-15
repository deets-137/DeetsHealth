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
import base64
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
    # An online visit (from its visit type); the place is still the record's.
    online: bool = False


# How the caller wants to be seen (the owner's 1A, 2026-11-13): FindSlots's `visit_mode`.
VISIT_MODES = {
    "either": pb.VISIT_MODE_UNSPECIFIED,
    "in_person": pb.VISIT_MODE_IN_PERSON,
    "online": pb.VISIT_MODE_ONLINE,
}


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
    # How to prepare (Epic's `patientInstruction`), after the same screen; `instructions_private`:
    # the record held some and ehr dropped them (EPIC.md §7.8).
    instructions: Shielded[str] = field(default_factory=lambda: Shielded(""))
    instructions_private: bool = False
    # The visit's number at the hospital (Epic's CSN): for a staff flag only, never the model
    # (FLAGS.md §3.3). "" when unknown.
    csn: Shielded[str] = field(default_factory=lambda: Shielded(""))
    # "upcoming", "seen", "missed" or "cancelled".
    status: str = "upcoming"


class BookNotThrough(Exception):
    """The EHR's reply to a booking was lost, and a search found no such booking (the owner's
    V2A, EPIC.md §7.10). The caller is asked whether to try again; a retry with the same key
    books."""


@dataclass(frozen=True)
class FoundVisit:
    """FindVisit's answer (EPIC.md §7.10): one visit and how many others matched, or, when too
    many matched, the visit groups to ask the caller about."""

    visit: Visit | None = None
    more: int = 0
    # The hospital's group names; tied to this caller they say what their visits are.
    groups: tuple[Shielded[str], ...] = ()
    other_visits: bool = False
    # `more` counts the visits in the next this many days; 0: with no limit.
    more_within_days: int = 0


STATUSES = {
    pb.VISIT_STATUS_UPCOMING: "upcoming",
    pb.VISIT_STATUS_SEEN: "seen",
    pb.VISIT_STATUS_MISSED: "missed",
    pb.VISIT_STATUS_CANCELLED: "cancelled",
}


def _slot(s) -> Slot:
    return Slot(
        token=Shielded(s.token),
        starts_at=Shielded(s.starts_at),
        minutes=s.minutes,
        clinician=Shielded(s.clinician),
        place=Shielded(s.place),
        online=s.online,
    )


def _visit(v) -> Visit:
    return Visit(
        starts_at=Shielded(v.starts_at),
        minutes=v.minutes,
        clinician=Shielded(v.clinician),
        place=Shielded(v.place),
        visit_type=Shielded(v.visit_type),
        reason=Shielded(v.reason),
        online=v.online,
        reason_private=v.reason_private,
        instructions=Shielded(v.instructions),
        instructions_private=v.instructions_private,
        csn=Shielded(v.csn),
        status=STATUSES.get(v.status, "upcoming"),
    )


class EhrClient:
    """One connection, opened when the agent starts, so no TLS handshake falls inside a call."""

    def __init__(self, channel: grpc.aio.Channel) -> None:
        self._channel = channel
        self._stub = pb_grpc.EhrStub(channel)
        # Each open call's clock from ehr, by handle, until its Flow takes it (`call_clock`).
        self._clocks: dict[str, tuple[str, str]] = {}
        # Each open call's time-of-day names, by its handle; dropped when it closes.
        self._times: dict[str, tuple[str, ...]] = {}
        # Each open call's "books both in person and online", by its handle.
        self._both: dict[str, bool] = {}
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

    async def open_call(self, ehr: str = "local", prompt_version: int = 0, system_version: int = 0) -> Shielded[str]:
        """`ehr`: `local` (ehr's fakes) or `epic` (Epic's public sandbox; dev only).
        `prompt_version`: the saved version of the agent's words the call runs with; 0 for the
        prompt file. ehr takes the text message's words from it. `system_version`: the saved
        System variables the call runs with (SYSTEM-PAGE.md §3.4); ehr fills them into the text."""
        source = {"local": pb.EHR_SOURCE_LOCAL, "epic": pb.EHR_SOURCE_EPIC_SANDBOX}[ehr]
        reply = await self._call(
            "OpenCall",
            pb.OpenCallRequest(ehr=source, prompt_version=prompt_version, system_version=system_version),
            left=lambda r: {"identify": r.identifies_left, "resend": r.resends_left, "code": r.code_tries_left},
        )
        self._clocks[reply.handle] = (reply.local_now, reply.zone)
        # The call's hospital (HOSPITAL-CONFIG.md): its time-of-day names for `find_slots`, read
        # once, and its version's label for the record. Not protected data.
        self._times[reply.handle] = tuple(reply.times_of_day)
        self._both[reply.handle] = reply.books_both_modes
        if self.record is not None:
            self.record.event("hospital", version=reply.hospital_version)
        return Shielded(reply.handle)

    def call_times(self, handle: Shielded[str]) -> tuple[str, ...] | None:
        """The names of the call's times of day ("morning"), from the hospital version it
        opened with; None for a handle this client did not open."""
        return self._times.get(handle.reveal())

    def call_books_both(self, handle: Shielded[str]) -> bool | None:
        """Whether the call can book both an in-person and an online visit (OpenCall), so the
        agent asks which; None for a handle this client did not open."""
        return self._both.get(handle.reveal())

    def call_clock(self, handle: Shielded[str]) -> tuple[str, str] | None:
        """The hospital's date and time when the call opened (YYYY-MM-DDTHH:MM) and its zone's
        short name, from ehr, which holds the one time zone (FUTURE-SETTINGS.md §13.1). Once."""
        return self._clocks.pop(handle.reveal(), None)

    async def close_call(self, handle: Shielded[str]) -> None:
        """Closes the call in ehr. Its EHR ledger (each request ehr sent the EHR: names, statuses,
        times and counts, never a value; DEBUGGING.md §3) goes into the dev call record."""
        self._times.pop(handle.reveal(), None)
        self._both.pop(handle.reveal(), None)
        reply = await self._call("CloseCall", pb.CloseCallRequest(handle=handle.reveal()))
        if self.record is not None:
            for e in reply.ledger:
                self.record.epic(
                    {
                        "rpc": e.rpc,
                        "what": e.what,
                        "status": e.status,
                        "ms": e.ms,
                        "results": e.results,
                        "bytes": e.bytes,
                        "at_ms": e.at_ms,
                    }
                )

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

    # ---- the People list (users.rs; docs/ops/DEMO.md §3.5). Staff identities, not patients',
    # but never logged: ehr.proto marks the fields.

    async def list_users(self) -> list[dict]:
        """Everyone on the list now, the owner first. Raises EhrError (NOT_ALLOWED_NOW without a
        store)."""
        reply = await self._users_call("ListUsers", pb.ListUsersRequest())
        return [
            {
                "id": u.id,
                "email": u.email,
                "name": u.name,
                "owner": u.owner,
                "added_by": u.added_by or None,
                "added_at": u.added_at,
                "last_seen_at": u.last_seen_at or None,
                "first_name": u.first_name,
                "last_name": u.last_name,
                "phone": u.phone or None,
                "phone_verified": u.phone_verified,
                "source": u.source,
                "photo_at": u.photo_at or None,
            }
            for u in reply.users
        ]

    async def update_user(self, user_id: str, first_name: str, last_name: str, phone: str, email: str) -> str | None:
        """Saves a person's card; the old email when the email changed ("" when the card had
        none yet), else None. Raises EhrError with ehr's fixed reason in `detail` (BAD_INPUT
        names the field)."""
        reply = await self._users_call(
            "UpdateUser",
            pb.UpdateUserRequest(id=user_id, first_name=first_name, last_name=last_name, phone=phone, email=email),
        )
        return reply.old_email if reply.email_changed else None

    async def user_photo(self, user_id: str) -> bytes | None:
        """A person's picture (WebP bytes), or None when they have none."""
        reply = await self._users_call("GetUserPhoto", pb.GetUserPhotoRequest(id=user_id))
        return base64.b64decode(reply.photo) if reply.photo else None

    async def set_user_photo(self, user_id: str, photo: bytes | None) -> None:
        """Saves a person's picture (a WebP); None clears it."""
        text = base64.b64encode(photo).decode() if photo else ""
        await self._users_call("SetUserPhoto", pb.SetUserPhotoRequest(id=user_id, photo=text))

    async def seen_owner(self) -> None:
        """The owner was seen on this PC (the launch token): ehr stamps last seen."""
        await self._users_call("SeenOwner", pb.SeenOwnerRequest())

    async def add_user(self, email: str, name: str, by: str) -> str:
        """Puts a person on the list; their id. `by`: the adder's email, "" for the owner's PC.
        Raises EhrError with ehr's fixed reason in `detail` (BAD_INPUT, OTHER for a duplicate)."""
        reply = await self._users_call("AddUser", pb.AddUserRequest(email=email, name=name, by=by))
        return reply.id

    async def remove_user(self, user_id: str) -> str:
        """Takes a person off the list; their email. Raises EhrError: NOT_VERIFIED for the owner,
        OTHER for no such person, each with ehr's fixed reason in `detail`."""
        reply = await self._users_call("RemoveUser", pb.RemoveUserRequest(id=user_id))
        return reply.email

    async def admit_user(self, email: str) -> bool:
        """Whether this email is on the list now; ehr stamps when it was last seen."""
        reply = await self._users_call("AdmitUser", pb.AdmitUserRequest(email=email))
        return reply.active

    async def seed_users(self, emails: list[str]) -> int:
        """Brings in the emails already in Cloudflare's group that the list lacks."""
        reply = await self._users_call(
            "SeedUsers", pb.SeedUsersRequest(emails=[pb.SeedEmail(email=e) for e in emails])
        )
        return reply.added

    async def _users_call(self, name: str, request):
        try:
            return await getattr(self._stub, name)(request, timeout=DEADLINE_S)
        except grpc.aio.AioRpcError as e:
            error = EhrError(_KINDS.get(e.code(), ErrorKind.OTHER), name)
            # users.rs's reasons are fixed words that name a field, never a value: safe to show.
            error.detail = e.details() or ""
            raise error from None

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
        self,
        handle: Shielded[str],
        reason: Shielded[str],
        earliest_day: Shielded[str] | None,
        visit_mode: str = "either",
        latest_day: Shielded[str] | None = None,
        time_of_day: str = "",
        days: tuple[str, ...] = (),
        clinician: Shielded[str] | None = None,
        more: bool = False,
    ) -> tuple[list[Slot], bool]:
        """The slots to offer now, and whether more of the same search wait. `reason`: the
        caller's own words (flow.py checks them); ehr keeps them for this call and Book sends
        them to the scheduler as the visit's note. `more`: the next slots of the last search;
        every other argument is ignored."""
        reply = await self._call(
            "FindSlots",
            pb.FindSlotsRequest(
                handle=handle.reveal(),
                reason=reason.reveal(),
                earliest_day=earliest_day.reveal() if earliest_day else "",
                visit_mode=VISIT_MODES.get(visit_mode, pb.VISIT_MODE_UNSPECIFIED),
                latest_day=latest_day.reveal() if latest_day else "",
                time_of_day=time_of_day,
                days=list(days),
                clinician=clinician.reveal() if clinician else "",
                more=more,
            ),
        )
        return [_slot(s) for s in reply.slots], reply.more

    async def book(
        self, handle: Shielded[str], slot_token: Shielded[str], idempotency_key: str
    ) -> Visit | None:
        """The booked visit (its type, whether it is online, the reason sent as its note), or
        None when someone else took it first."""
        reply = await self._call(
            "Book",
            pb.BookRequest(
                handle=handle.reveal(),
                slot_token=slot_token.reveal(),
                idempotency_key=idempotency_key,
            ),
            deadline=BOOK_DEADLINE_S,
            result=lambda r: {pb.BOOK_OUTCOME_BOOKED: "booked", pb.BOOK_OUTCOME_NOT_THROUGH: "not_through"}.get(
                r.outcome, "slot_taken"
            ),
        )
        if reply.outcome == pb.BOOK_OUTCOME_BOOKED and reply.HasField("visit"):
            return _visit(reply.visit)
        if reply.outcome == pb.BOOK_OUTCOME_NOT_THROUGH:
            raise BookNotThrough()
        return None

    async def find_visit(
        self,
        handle: Shielded[str],
        when: Shielded[str] | None = None,
        clinician: Shielded[str] | None = None,
        past: bool = False,
        group: Shielded[str] | None = None,
    ) -> FoundVisit:
        """One of the verified caller's visits: the soonest to come with no argument, else the
        one the caller named (a day or month YYYY-MM-DD / YYYY-MM, a clinician, the past, a
        visit group)."""
        reply = await self._call(
            "FindVisit",
            pb.FindVisitRequest(
                handle=handle.reveal(),
                when=when.reveal() if when else "",
                clinician=clinician.reveal() if clinician else "",
                past=past,
                group=group.reveal() if group else "",
            ),
            result=lambda r: "visit" if r.HasField("visit") else ("which" if r.groups else "none"),
        )
        return FoundVisit(
            visit=_visit(reply.visit) if reply.HasField("visit") else None,
            more=reply.more,
            groups=tuple(Shielded(g) for g in reply.groups),
            other_visits=reply.other_visits,
            more_within_days=reply.more_within_days,
        )
