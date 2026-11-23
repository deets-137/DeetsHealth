"""The agent's side of the wire to ehr (EHR-SERVICE.md): mTLS, a deadline on every call, and
the edge.

The edge: no code past this file sees a wire message. Each reply becomes the agent's own
types, and each marked field becomes a `Shielded`. A request is built here from `Shielded`
values, which is one of the few places that may `reveal()` them.

Errors become an `EhrError` with a kind. The server's error text is fixed and PHI-free, but it
is dropped anyway: the agent acts on the kind only.

**Who is asking** (docs/architecture/ACCESS-ROLES.md §5; EHR-SERVICE.md §3). Every staff call carries
the person behind it in the gRPC metadata `deets-actor`: their user id, from `ACTOR`, which the
dev server sets for each request (identity.py). Code that acts on its own, with no person
behind it (the start-up sync, a call reading its settings), runs inside `as_system()`. ehr
reads the person's levels itself; a call RPC (OpenCall … FileCall) carries no person.
"""

from __future__ import annotations

import asyncio
import base64
import enum
import json
import time
from collections.abc import Iterator
from contextlib import contextmanager
from contextvars import ContextVar
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Any

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
# An import of the hospital's places (HOSPITAL-CONFIG.md §8): no caller waits on it. ehr caps
# every request at 10 s (main.rs › REQUEST_LIMIT), so this is that cap.
IMPORT_DEADLINE_S = 10.0


# The person behind the staff calls of this request or task: a user id, `SYSTEM`, or None
# (no person: ehr refuses every staff call but the settings reads).
ACTOR: ContextVar[str | None] = ContextVar("deets_actor", default=None)
ACTOR_HEADER = "deets-actor"
SYSTEM = "system"


@contextmanager
def as_system() -> Iterator[None]:
    """The agent acting on its own inside this block (ehr's `Need::Settings` and
    `LeverOrSystem`): the start-up sync, a call reading its settings."""
    token = ACTOR.set(SYSTEM)
    try:
        yield
    finally:
        ACTOR.reset(token)


def _meta() -> tuple[tuple[str, str], ...] | None:
    actor = ACTOR.get()
    return ((ACTOR_HEADER, actor),) if actor else None


class ErrorKind(enum.Enum):
    NO_SUCH_CALL = "no_such_call"
    NOT_VERIFIED = "not_verified"
    BAD_INPUT = "bad_input"
    NOT_ALLOWED_NOW = "not_allowed_now"
    UNAVAILABLE = "unavailable"
    # A staff call ehr refused for the person's access (access.rs); `detail` names the rule.
    FORBIDDEN = "forbidden"
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


def _staff_error(e: grpc.aio.AioRpcError, name: str) -> EhrError:
    """A staff call's error. ehr's reasons on these calls are fixed words (a field's name, a
    rule), never a value: safe to show. PERMISSION_DENIED is the person's access."""
    kind = ErrorKind.FORBIDDEN if e.code() == grpc.StatusCode.PERMISSION_DENIED else _KINDS.get(e.code(), ErrorKind.OTHER)
    error = EhrError(kind, name)
    error.detail = e.details() or ""
    return error


def _plain(message) -> dict[str, Any]:
    """A reply message as plain values, every field present: numbers stay numbers (protobuf's own
    JSON form writes an int64 as a string), an unset message is None."""
    out: dict[str, Any] = {}
    for f in message.DESCRIPTOR.fields:
        value = getattr(message, f.name)
        if f.message_type is not None:
            if f.is_repeated:
                out[f.name] = [_plain(v) for v in value]
            else:
                out[f.name] = _plain(value) if message.HasField(f.name) else None
        else:
            out[f.name] = list(value) if f.is_repeated else value
    return out


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
    # With the caller's PCP; the one sooner time with another clinician (BOOKING-RULES.md §3.3).
    with_pcp: bool = False
    sooner: bool = False


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
    # A visit this call just booked whose Booking rule says Book and flag: staff check the
    # referral (BOOKING-RULES.md §3.5). False for every other visit.
    check_referral: bool = False


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
    # The place the caller named is on no list (BOOKING-RULES.md §3.1).
    unknown_place: bool = False


@dataclass(frozen=True)
class FoundSlots:
    """FindSlots's answer: the slots to offer now, whether more of the same search wait, the
    caller's usual place when the search was held to it ("" otherwise; BOOKING-RULES.md §3.2),
    and, when no search ran, the question to ask about the place or the kind of care (§3.1)."""

    slots: list[Slot]
    more: bool = False
    usual: Shielded[str] = field(default_factory=lambda: Shielded(""))
    # "", "where", "which", "unknown_place", "unknown_specialty" or "not_at_place".
    place_ask: str = ""
    # With "which": the places to name, in the hospital's words; tied to this caller's ask.
    places: tuple[Shielded[str], ...] = ()
    other_places: bool = False
    # A Booking rule held back every time (BOOKING-RULES.md §3.5): "referral_needed" or
    # "hand_off"; "" otherwise.
    rule: str = ""


@dataclass(frozen=True)
class PlaceInfo:
    """PlaceInfo's answer (BOOKING-RULES.md §3.7): the hospital's own facts about one place, or
    the places to ask about. An empty address or phone is not on file."""

    outcome: str  # "found", "unknown" or "which"
    name: Shielded[str] = field(default_factory=lambda: Shielded(""))
    address: Shielded[str] = field(default_factory=lambda: Shielded(""))
    phone: Shielded[str] = field(default_factory=lambda: Shielded(""))
    # (day "mon".."sun", opens HH:MM, closes HH:MM), Monday first.
    hours: tuple[tuple[str, str, str], ...] = ()
    # (YYYY-MM-DD, name): the hospital's next closed days.
    closed_days: tuple[tuple[str, str], ...] = ()
    places: tuple[Shielded[str], ...] = ()
    other_places: bool = False


RULE_ASKS = {
    pb.RULE_ASK_REFERRAL_NEEDED: "referral_needed",
    pb.RULE_ASK_HAND_OFF: "hand_off",
}


PLACE_ASKS = {
    pb.PLACE_ASK_WHERE: "where",
    pb.PLACE_ASK_WHICH: "which",
    pb.PLACE_ASK_UNKNOWN_PLACE: "unknown_place",
    pb.PLACE_ASK_UNKNOWN_SPECIALTY: "unknown_specialty",
    pb.PLACE_ASK_NOT_AT_PLACE: "not_at_place",
}


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
        with_pcp=s.with_pcp,
        sooner=s.sooner,
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
            # Every call this agent opens today is synthetic (DATABASE.md §7.5): ehr lets it reach
            # a sandbox only.
            pb.OpenCallRequest(
                ehr=source,
                prompt_version=prompt_version,
                system_version=system_version,
                data_class=pb.DATA_CLASS_SYNTHETIC,
            ),
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

    # ---- the kept scripts' runs and the annotations (DATABASE.md §8, §9) ----

    async def file_test_run(self, run: dict[str, Any]) -> dict[str, Any] | None:
        """File one run of the kept scripts. `run` holds FileTestRunRequest's fields, its
        `results` a list of TestRunResult's. Returns `run_id`, `needed` (the call ids whose
        records to send with `file_call`) and `pruned`; None when ehr runs without a store."""
        results = [pb.TestRunResult(**r) for r in run.get("results", [])]
        request = pb.FileTestRunRequest(**{k: v for k, v in run.items() if k != "results"}, results=results)
        try:
            reply = await self._settings_call("FileTestRun", request)
        except _NoStore:
            return None
        return {"run_id": reply.run_id, "needed": list(reply.needed), "pruned": reply.pruned}

    async def add_annotation(self, a: dict[str, Any]) -> bool | None:
        """File one checked and stamped annotation (catalog.py › check_annotation) in the store.
        True when filed, False when the same one was filed before, None without a store.
        EhrError BAD_INPUT for a call the store does not hold."""
        rating = a["kind"] == "rating"
        words = a.get("text") if a["kind"] in ("label", "note") else a.get("note") if rating else None
        request = pb.Annotation(
            call_id=a["call"],
            kind=a["kind"],
            at=a["at"],
            grader_kind=a.get("grader_kind") or "",
            grader=a.get("grader") or "",
            dimension=a.get("dimension") or "",
            verdict=a.get("verdict") or "",
            score=a["score"] if rating and isinstance(a.get("score"), int) else -1,
            rubric_version=(a.get("rubric_version") or "") if rating else "",
            keep=bool(a.get("value")) if a["kind"] == "keep" else False,
            words=words or "",
            has_words=words is not None,
        )
        try:
            reply = await self._stub.AddAnnotation(request, timeout=DEADLINE_S, metadata=_meta())
        except grpc.aio.AioRpcError as e:
            if e.code() == grpc.StatusCode.FAILED_PRECONDITION:
                return None
            if e.code() == grpc.StatusCode.PERMISSION_DENIED:
                raise _staff_error(e, "AddAnnotation") from None
            # NOT_FOUND: the store does not hold the call (not filed yet).
            kind = ErrorKind.BAD_INPUT if e.code() == grpc.StatusCode.NOT_FOUND else _KINDS.get(e.code(), ErrorKind.OTHER)
            raise EhrError(kind, "AddAnnotation") from None
        return reply.filed

    async def list_annotations(self) -> list[dict[str, Any]] | None:
        """Every annotation of the synthetic class, oldest first, in the catalog's line shape
        (the shape `check_annotation(..., stamped=True)` takes). None without a store."""
        try:
            reply = await self._settings_call("ListAnnotations", pb.ListAnnotationsRequest())
        except _NoStore:
            return None
        out: list[dict[str, Any]] = []
        for a in reply.annotations:
            line: dict[str, Any] = {"kind": a.kind, "call": a.call_id, "at": a.at}
            if a.kind in ("label", "note"):
                line["text"] = a.words if a.has_words else ""
            elif a.kind == "keep":
                line["value"] = a.keep
            else:
                line.update(grader_kind=a.grader_kind, grader=a.grader, dimension=a.dimension, verdict=a.verdict)
                line["score"] = a.score if a.score >= 0 else None
                line["rubric_version"] = a.rubric_version
                if a.has_words:
                    line["note"] = a.words
            out.append(line)
        return out

    # ---- the voice catalog (VOICE-VENDORS.md §2.1) ----

    async def voice_catalog(self) -> dict[str, Any]:
        """The voice catalog: every vendor, its levers and rates, and every voice setting's first
        value (`defaults`). The code's own data; no store needed."""
        reply = await self._users_call("GetVoiceCatalog", pb.GetVoiceCatalogRequest())
        return json.loads(reply.catalog)

    async def model_catalog(self) -> dict[str, Any]:
        """The model catalog (VOICE-VENDORS.md §10): each model vendor, its models, prices and
        rules, and every `llm` setting's first value (`defaults`). No store needed."""
        reply = await self._users_call("GetModelCatalog", pb.GetModelCatalogRequest())
        return json.loads(reply.catalog)

    async def price_model_use(self, request: dict[str, Any]) -> dict[str, Any]:
        """The model's cost from the vendors crate (VOICE-VENDORS.md §10): each call's dollars,
        the live calls', a list of uses, and the budget's cycle. `request`: `today`, `settings`
        (the llm settings), `calls` (`{id, day, usd | use}`), `live` and `uses` (uses)."""
        reply = await self._users_call("PriceModelUse", pb.PriceModelUseRequest(request=json.dumps(request)))
        return json.loads(reply.reply)

    async def price_voice_use(self, request: dict[str, Any]) -> dict[str, Any]:
        """The voice's cost from the vendors crate (VOICE-VENDORS.md §9): each call's credits by
        vendor, the live calls' credits, each chosen vendor's cycle. `request`: `today`,
        `settings`, `calls` (`{id, day, use}`), `live` (uses). Counts and dates only."""
        reply = await self._users_call("PriceVoiceUse", pb.PriceVoiceUseRequest(request=json.dumps(request)))
        return json.loads(reply.reply)

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
            reply = await self._stub.SaveSettings(request, timeout=DEADLINE_S, metadata=_meta())
        except grpc.aio.AioRpcError as e:
            # ehr's settings.rs names the setting or the rule only (a roles save refused by
            # access.rs comes as FAILED_PRECONDITION with its rule); safe to show.
            raise _staff_error(e, "SaveSettings") from None
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
                "role_ids": list(u.role_ids),
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

    async def seen_owner(self) -> str | None:
        """The owner was seen on this PC (the launch token): ehr stamps last seen and answers
        with the owner's id, the person the launch token speaks for (None: no owner yet)."""
        reply = await self._users_call("SeenOwner", pb.SeenOwnerRequest())
        return reply.user_id or None

    async def add_user(self, email: str, name: str) -> str:
        """Puts a person on the list, with no role; their id. The adder is `ACTOR`. Raises
        EhrError with ehr's fixed reason in `detail` (BAD_INPUT, OTHER for a duplicate)."""
        reply = await self._users_call("AddUser", pb.AddUserRequest(email=email, name=name))
        return reply.id

    async def remove_user(self, user_id: str) -> str:
        """Takes a person off the list and revokes their roles; their email. Raises EhrError:
        FORBIDDEN for someone above you or the last who can manage the console, OTHER for no such
        person, each with ehr's fixed reason in `detail`."""
        reply = await self._users_call("RemoveUser", pb.RemoveUserRequest(id=user_id))
        return reply.email

    async def admit_user(self, email: str) -> str | None:
        """The person's id when this email is on the list now (ehr stamps when they were last
        seen); else None."""
        reply = await self._users_call("AdmitUser", pb.AdmitUserRequest(email=email))
        return reply.user_id if reply.active else None

    async def seed_users(self, emails: list[str]) -> int:
        """Brings in the emails already in Cloudflare's group that the list lacks."""
        reply = await self._users_call(
            "SeedUsers", pb.SeedUsersRequest(emails=[pb.SeedEmail(email=e) for e in emails])
        )
        return reply.added

    async def import_places(self, addresses: list[str]) -> dict:
        """Config › Hospital › Places, Import from Epic (HOSPITAL-CONFIG.md §8): the hospital's
        facilities, departments and specialties from the EHR's directory, as the hospital's rows
        (`values`: facilities, locations, specialty_list, specialties), and the counts. Saves
        nothing. Raises EhrError with ehr's fixed reason in `detail`: BAD_INPUT for the
        addresses, NOT_ALLOWED_NOW when ehr serves no directory, UNAVAILABLE while it does not
        answer, OTHER when it returns more than an import holds. Facts about the hospital, never
        protected data."""
        try:
            reply = await self._stub.ImportPlaces(
                pb.ImportPlacesRequest(addresses=addresses), timeout=IMPORT_DEADLINE_S, metadata=_meta()
            )
        except grpc.aio.AioRpcError as e:
            # service.rs › import_places: fixed words, never a value.
            raise _staff_error(e, "ImportPlaces") from None
        return {
            "values": {v.key: json.loads(v.value) for v in reply.values},
            "others": reply.others,
            "other_specialty_lists": reply.other_specialty_lists,
            "reads": reply.reads,
        }

    async def _users_call(self, name: str, request):
        try:
            return await getattr(self._stub, name)(request, timeout=DEADLINE_S, metadata=_meta())
        except grpc.aio.AioRpcError as e:
            raise _staff_error(e, name) from None

    async def _settings_call(self, name: str, request):
        try:
            return await getattr(self._stub, name)(request, timeout=DEADLINE_S, metadata=_meta())
        except grpc.aio.AioRpcError as e:
            if e.code() == grpc.StatusCode.FAILED_PRECONDITION:
                raise _NoStore from None
            raise _staff_error(e, name) from None

    # ---- access by role and the audit trail (docs/architecture/ACCESS-ROLES.md §5, §5.1) ----

    async def levels(self) -> dict[str, Any]:
        """The person in `ACTOR`: their id, live role ids, the roles version and the level on
        each lever (`{lever: none|view|edit}`). Raises EhrError: FORBIDDEN for no such person,
        NOT_ALLOWED_NOW without a store."""
        reply = await self._users_call("Levels", pb.LevelsRequest())
        return {
            "user_id": reply.user_id,
            "role_ids": list(reply.role_ids),
            "roles_version": reply.roles_version,
            "levels": {lv.lever: lv.level for lv in reply.levels},
        }

    async def grant_role(self, user_id: str, role_id: str) -> None:
        """Grants a role (Roles Edit). EhrError FORBIDDEN names the rule in `detail`."""
        await self._users_call("GrantRole", pb.GrantRoleRequest(user_id=user_id, role_id=role_id))

    async def revoke_role(self, user_id: str, role_id: str) -> None:
        """Revokes a role (Roles Edit). EhrError FORBIDDEN names the rule in `detail`."""
        await self._users_call("RevokeRole", pb.RevokeRoleRequest(user_id=user_id, role_id=role_id))

    async def read_audit(self, before: int = 0, limit: int = 100, changes: bool = False) -> list[dict[str, Any]]:
        """The audit trail, newest first (Audit logs View): ids and field names only."""
        reply = await self._users_call("ReadAudit", pb.ReadAuditRequest(before=before, limit=limit, changes=changes))
        return [
            {
                "seq": r.seq,
                "at": r.at,
                "actor": r.actor or None,
                "source": r.source,
                "roles": list(r.roles),
                "roles_version": r.roles_version or None,
                "lever": r.lever or None,
                "level": r.level or None,
                "action": r.action,
                "object_kind": r.object_kind or None,
                "object_id": r.object_id or None,
                "data_class": r.data_class or None,
                "fields": list(r.fields),
                "before_version": r.before_version or None,
                "after_version": r.after_version or None,
                "allowed": r.allowed,
            }
            for r in reply.rows
        ]

    async def list_calls(self, since: str = "", until: str = "", limit: int = 2000) -> list[dict[str, Any]]:
        """Analytics › Logs (Analytics View; ANALYTICS.md §3.1): the synthetic calls started in
        [since, until), newest first. Names and numbers, and each call's own label. Raises
        EhrError: NOT_ALLOWED_NOW without a store."""
        reply = await self._users_call(
            "ListCalls",
            pb.ListCallsRequest(data_class=pb.DATA_CLASS_SYNTHETIC, since=since, until=until, limit=limit),
        )
        return [
            {
                "id": c.id,
                "started": c.started or None,
                "origin": c.origin,
                "seat": c.seat or None,
                "ehr": c.ehr or None,
                "mode": c.mode or None,
                "model": c.model or None,
                "prompt_version": c.prompt_version or None,
                "system_version": c.system_version or None,
                "hospital_version": c.hospital_version or None,
                "scenario": c.scenario or None,
                "outcome": c.outcome,
                "end_reason": c.end_reason or None,
                "reached": list(c.reached),
                "turns": c.turns,
                "duration_ms": c.duration_ms,
                "cost_usd": c.cost_usd,
                "turn_waits_ms": list(c.turn_waits_ms),
                "tools": list(c.tools),
                "tool_failed": list(c.tool_failed),
                "staff_flags": list(c.staff_flags),
                "review_flags": list(c.review_flags),
                "label": c.label or None,
                "ratings_passed": c.ratings_passed,
                "ratings_failed": c.ratings_failed,
                "requests_live": c.requests_live,
                "requests_replayed": c.requests_replayed,
            }
            for c in reply.calls
        ]

    async def call_stats(
        self, since: str, until: str, prev: tuple[str, str] | None, filters: dict[str, Any]
    ) -> dict[str, Any]:
        """Analytics' sums (Analytics View; ANALYTICS.md §3.2) for the synthetic calls in [since,
        until) that pass `filters` (stats.rs › Filter's names), and the totals of `prev`, the
        range before. Names, counts, times and dollars only."""
        request = pb.CallStatsRequest(
            data_class=pb.DATA_CLASS_SYNTHETIC,
            since=since,
            until=until,
            prev_since=prev[0] if prev else "",
            prev_until=prev[1] if prev else "",
            has_prev=prev is not None,
            **filters,
        )
        reply = await self._users_call("CallStats", request)
        return _plain(reply)

    async def read_flags(self, call_id: str) -> list[dict[str, Any]] | None:
        """One call's staff flags with their summary and evidence (PHI View; ANALYTICS.md §3.2).
        ehr writes the `flag.view` audit row. None for a call the store does not hold."""
        try:
            reply = await self._stub.ReadFlags(pb.ReadFlagsRequest(call_id=call_id), timeout=DEADLINE_S, metadata=_meta())
        except grpc.aio.AioRpcError as e:
            if e.code() == grpc.StatusCode.NOT_FOUND:
                return None
            raise _staff_error(e, "ReadFlags") from None
        return [_plain(f) for f in reply.flags]

    async def verify_audit(self) -> dict[str, Any]:
        """Walks the whole chain: `{ok, rows, first_bad}`."""
        reply = await self._users_call("VerifyAudit", pb.VerifyAuditRequest())
        return {"ok": reply.ok, "rows": reply.rows, "first_bad": reply.first_bad or None}

    async def log_access(
        self, lever: str, level: str, action: str, object_kind: str = "", object_id: str = "", data_class: str = ""
    ) -> bool:
        """Records a look that changes nothing (a page opened, a transcript viewed, a test run):
        ehr writes an allowed or a refused row and answers whether it was allowed."""
        reply = await self._users_call(
            "LogAccess",
            pb.LogAccessRequest(
                lever=lever, level=level, action=action, object_kind=object_kind, object_id=object_id, data_class=data_class
            ),
        )
        return reply.allowed

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
        any_place: bool = False,
        place: Shielded[str] | None = None,
        specialty: Shielded[str] | None = None,
        plan_only: bool = False,
    ) -> FoundSlots:
        """The slots to offer now, or the question to ask first (`FoundSlots`). `reason`: the
        caller's own words (flow.py checks them); ehr keeps them for this call and Book sends
        them to the scheduler as the visit's note. `more`: the next slots of the last search;
        every other argument is ignored. `any_place`: the caller wants any place, not their
        usual one. `place`, `specialty`: the caller's words, matched in ehr (BOOKING-RULES.md
        §3.1). `plan_only`: where the search would look, and no search."""
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
                any_place=any_place,
                place=place.reveal() if place else "",
                specialty=specialty.reveal() if specialty else "",
                plan_only=plan_only,
            ),
            result=lambda r: RULE_ASKS.get(r.rule) or PLACE_ASKS.get(r.place_ask, "slots"),
        )
        return FoundSlots(
            slots=[_slot(s) for s in reply.slots],
            more=reply.more,
            usual=Shielded(reply.place_default),
            place_ask=PLACE_ASKS.get(reply.place_ask, ""),
            places=tuple(Shielded(p) for p in reply.places),
            other_places=reply.other_places,
            rule=RULE_ASKS.get(reply.rule, ""),
        )

    async def place_info(self, handle: Shielded[str], place: Shielded[str]) -> PlaceInfo:
        """Where a place is, its phone and its hours, from the hospital's lists; open before
        verification (BOOKING-RULES.md §3.7)."""
        outcomes = {pb.PLACE_INFO_OUTCOME_FOUND: "found", pb.PLACE_INFO_OUTCOME_WHICH: "which"}
        reply = await self._call(
            "PlaceInfo",
            pb.PlaceInfoRequest(handle=handle.reveal(), place=place.reveal()),
            result=lambda r: outcomes.get(r.outcome, "unknown"),
        )
        return PlaceInfo(
            outcome=outcomes.get(reply.outcome, "unknown"),
            name=Shielded(reply.name),
            address=Shielded(reply.address),
            phone=Shielded(reply.phone),
            hours=tuple((h.day, h.opens, h.closes) for h in reply.hours),
            closed_days=tuple((d.date, d.name) for d in reply.closed_days),
            places=tuple(Shielded(p) for p in reply.places),
            other_places=reply.other_places,
        )

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
            return replace(_visit(reply.visit), check_referral=reply.check_referral)
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
        place: Shielded[str] | None = None,
    ) -> FoundVisit:
        """One of the verified caller's visits: the soonest to come with no argument, else the
        one the caller named (a day or month YYYY-MM-DD / YYYY-MM, a clinician, the past, a
        visit group, a place)."""
        reply = await self._call(
            "FindVisit",
            pb.FindVisitRequest(
                handle=handle.reveal(),
                when=when.reveal() if when else "",
                clinician=clinician.reveal() if clinician else "",
                past=past,
                group=group.reveal() if group else "",
                place=place.reveal() if place else "",
            ),
            result=lambda r: "visit"
            if r.HasField("visit")
            else ("which" if r.groups else ("unknown_place" if r.unknown_place else "none")),
        )
        return FoundVisit(
            visit=_visit(reply.visit) if reply.HasField("visit") else None,
            more=reply.more,
            groups=tuple(Shielded(g) for g in reply.groups),
            other_visits=reply.other_visits,
            more_within_days=reply.more_within_days,
            unknown_place=reply.unknown_place,
        )
