"""The call's shape (VOICE-AGENT.md §8.1): each node has its own instructions and its own tools.

    greet ─► verify (name and birth date, then the code) ─► menu ─┬─► schedule (find, book)
                                                                  └─► next visit
    any node ─► hand off to a person            menu ─► goodbye

Phase 1 features (BUILD-ORDER.md): C1 greet, C2 + C3 verify (C3 with ehr's fake code), C4 hand
off, S1 find, S2 book, S5 next visit.

Two locks on the chart (SECURITY.md §4.9):
- **The Flow:** the chart and booking tools exist only in `menu` and `schedule`, which only a
  PASS from ehr's `CheckCode` leads to.
- **ehr:** every chart and booking call is refused until ehr itself has seen both factors. A
  tool takes no patient argument; the call's handle lives in this object, never in a prompt.

The words that carry a value are built here from ehr's reply (`say` in each tool result), and
the model is told to read them as written (SECURITY.md §4.9: "a template filled from the
record"). The agent's final words, name and voice are the owner's to decide (DESIGN.md §B,
Phase 3 at the latest); these are working words.

Nothing protected goes into Flows' `state` or a log: the handle and the offered slots live on
this object, as `Shielded`.
"""

from __future__ import annotations

import secrets
from dataclasses import dataclass, field
from datetime import date, datetime

from loguru import logger
from pipecat.flows import FlowManager, FlowsFunctionSchema, NodeConfig

from deets_agent.ehr_client import CheckOutcome, EhrClient, EhrError, ErrorKind, Slot, Visit
from deets_agent.shielded import Shielded

CLINIC = "Main Street Clinic"

# Phase 1 has a fixed "today", so a typed call's prompt is the same on every run and the replay
# cache holds (EHR-SERVICE.md §7). ehr's fake slots start the week after.
FAKE_TODAY = date(2026, 10, 26)

_MONTHS = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
]
_DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]


def spoken_day(d: date) -> str:
    return f"{_DAYS[d.weekday()]}, {_MONTHS[d.month - 1]} {d.day}"


def spoken_time(starts_at: str) -> str:
    """'2026-11-02T14:30' -> 'Monday, November 2, at 2:30 PM'."""
    t = datetime.strptime(starts_at, "%Y-%m-%dT%H:%M")
    hour = t.hour % 12 or 12
    half = "AM" if t.hour < 12 else "PM"
    minute = "" if t.minute == 0 else f":{t.minute:02d}"
    return f"{spoken_day(t.date())}, at {hour}{minute} {half}"


def _visit_words(v: Slot | Visit) -> str:
    """The one place a visit's values become words for the model (a reveal on purpose)."""
    return (
        f"{spoken_time(v.starts_at.reveal())}, with {v.clinician.reveal()} "
        f"at {v.place.reveal()}, for {v.minutes} minutes"
    )


ROLE = f"""You are the automated phone assistant for {CLINIC}. Today is {spoken_day(FAKE_TODAY)}, {FAKE_TODAY.year}.

How you speak: this is a phone call. Use one or two short, plain sentences at a time. No lists, \
no markdown, no emoji. Be warm and calm.

Rules you never break:
- You never know a caller's details until a tool tells you. Never guess or invent a date, a \
time, a name, a place or a number.
- When a tool result has a "say" field, say that text as written. You may add a short question \
after it.
- You cannot see or change records yourself. Only your tools can.
- If the caller asks for a person, is upset, or needs something your tools cannot do, use \
hand_off.
- If the caller describes an emergency, tell them to hang up and call 911 now, then use hand_off."""


@dataclass
class CallFlow:
    """One call's Flow. Holds the call's handle and offered slots; nothing else holds them."""

    ehr: EhrClient
    handle: Shielded[str] | None = None
    offered: list[Slot] = field(default_factory=list)
    # One idempotency key per option the caller chose, so a retry never books twice.
    book_keys: dict[int, str] = field(default_factory=dict)
    # Set when the Flow moves to a node that ends the call (goodbye, hand off).
    closing: bool = False

    async def start(self) -> None:
        self.handle = await self.ehr.open_call()

    async def close(self) -> None:
        if self.handle is not None:
            try:
                await self.ehr.close_call(self.handle)
            except EhrError:
                pass
            self.handle = None

    # ---- nodes ----

    def greet(self) -> NodeConfig:
        return {
            "name": "greet",
            "role_message": ROLE,
            "task_messages": [
                {
                    "role": "developer",
                    "content": (
                        f"Greet the caller: say you are {CLINIC}'s automated assistant and ask how "
                        "you can help. To book a visit or hear about their own visits, the caller "
                        "must first confirm who they are: ask for their first name, last name and "
                        "date of birth, then use identify."
                    ),
                }
            ],
            "functions": [self._identify_fn()],
        }

    def verify(self) -> NodeConfig:
        return {
            "name": "verify",
            "task_messages": [
                {
                    "role": "developer",
                    "content": (
                        "Ask the caller to read the 6-digit code from the text message, then use "
                        "check_code. If they got no text, offer to send it again with resend_code, "
                        "or to try their name and date of birth again with identify."
                    ),
                }
            ],
            "functions": [self._check_code_fn(), self._resend_code_fn(), self._identify_fn()],
        }

    def menu(self) -> NodeConfig:
        return {
            "name": "menu",
            "task_messages": [
                {
                    "role": "developer",
                    "content": (
                        "The caller is verified. Ask what they need: to book a visit (find_slots), "
                        "or to hear when their next visit is (next_visit). When they are done, use "
                        "end_call."
                    ),
                }
            ],
            "functions": [self._find_slots_fn(), self._next_visit_fn(), self._end_call_fn()],
        }

    def schedule(self) -> NodeConfig:
        return {
            "name": "schedule",
            "task_messages": [
                {
                    "role": "developer",
                    "content": (
                        "Read the options as written and ask which one the caller wants. When they "
                        "choose, confirm it back in one sentence and use book with the option "
                        "number. If none suits them, use find_slots with another day."
                    ),
                }
            ],
            "functions": [self._book_fn(), self._find_slots_fn(), self._end_call_fn()],
        }

    def goodbye(self) -> NodeConfig:
        self.closing = True
        return {
            "name": "goodbye",
            "task_messages": [
                {"role": "developer", "content": "Thank the caller and say goodbye in one sentence."}
            ],
            "functions": [],
            "post_actions": [{"type": "end_conversation"}],
        }

    def handed_off(self) -> NodeConfig:
        self.closing = True
        return {
            "name": "hand_off",
            "task_messages": [
                {
                    "role": "developer",
                    "content": "Say you are connecting them with a person now, in one sentence.",
                }
            ],
            "functions": [],
            "post_actions": [{"type": "end_conversation"}],
        }

    # ---- global ----

    def hand_off_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            logger.info("hand off requested")
            # Phase 1 has no line to transfer to; the call ends with the words only.
            return {"say": "Of course. I'm connecting you with a member of our staff now."}, self.handed_off()

        return FlowsFunctionSchema(
            name="hand_off",
            description="Connect the caller with a person. Use when they ask for one, or you cannot help.",
            properties={},
            required=[],
            handler=handler,
        )

    # ---- verification ----

    def _identify_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            given = Shielded(str(args.get("given_name", "")).strip())
            family = Shielded(str(args.get("family_name", "")).strip())
            birth = Shielded(str(args.get("birth_date", "")).strip())
            try:
                left = await self.ehr.identify(self.handle, given, family, birth)
            except EhrError as e:
                return self._ehr_trouble(e, "identify")
            logger.info("identify sent, {} tries left", left)
            # The same words whether or not a patient matched (SECURITY.md §4.4).
            return {
                "say": (
                    "Thank you. If those details match our records, we've just sent a 6-digit code "
                    "by text to the phone number we have on file. Could you read it to me?"
                ),
            }, self.verify()

        return FlowsFunctionSchema(
            name="identify",
            description="Look up the caller by first name, last name and date of birth, and text them a code.",
            properties={
                "given_name": {"type": "string", "description": "The caller's first name, as they said it."},
                "family_name": {"type": "string", "description": "The caller's last name, as they said it."},
                "birth_date": {"type": "string", "description": "Date of birth as YYYY-MM-DD."},
            },
            required=["given_name", "family_name", "birth_date"],
            handler=handler,
        )

    def _check_code_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            digits = "".join(c for c in str(args.get("code", "")) if c.isdigit())
            if len(digits) != 6:
                return {"say": "The code has 6 digits. Could you read it again?"}, None
            try:
                outcome, left = await self.ehr.check_code(self.handle, Shielded(digits))
            except EhrError as e:
                return self._ehr_trouble(e, "check_code")
            logger.info("code check: {}", outcome.value)
            if outcome is CheckOutcome.PASS:
                return {"say": "Thank you, you're verified."}, self.menu()
            if outcome is CheckOutcome.LOCKED:
                return {
                    "say": "I'm sorry, I can't confirm your identity on this call. Let me connect you with a person."
                }, self.handed_off()
            return {
                "say": "That code didn't match. If no text came, the details may not match our records.",
                "tries_left": left,
            }, None

        return FlowsFunctionSchema(
            name="check_code",
            description="Check the 6-digit code the caller read from their text message.",
            properties={"code": {"type": "string", "description": "The 6 digits the caller read."}},
            required=["code"],
            handler=handler,
        )

    def _resend_code_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            try:
                left = await self.ehr.resend_code(self.handle)
            except EhrError as e:
                if e.kind is ErrorKind.NOT_ALLOWED_NOW:
                    return {"say": "I can't send another code on this call. Let me connect you with a person."}, self.handed_off()
                return self._ehr_trouble(e, "resend_code")
            return {"say": "I've sent the code again. It can take a minute to arrive.", "resends_left": left}, None

        return FlowsFunctionSchema(
            name="resend_code",
            description="Send the text code again to the number on file.",
            properties={},
            required=[],
            handler=handler,
        )

    # ---- the chart and booking (only in menu and schedule) ----

    def _find_slots_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            reason = Shielded(str(args.get("reason", "")).strip() or "a visit")
            day = str(args.get("earliest_day", "")).strip()
            try:
                slots = await self.ehr.find_slots(self.handle, reason, Shielded(day) if day else None)
            except EhrError as e:
                return self._ehr_trouble(e, "find_slots")
            logger.info("find_slots: {} offered", len(slots))
            self.offered = slots
            self.book_keys.clear()
            if not slots:
                return {"say": "I don't see any open times then. Would another day work?"}, None
            options = [f"Option {i}: {_visit_words(s)}." for i, s in enumerate(slots, 1)]
            return {"options": len(slots), "say": "Here are the times I have. " + " ".join(options)}, self.schedule()

        return FlowsFunctionSchema(
            name="find_slots",
            description="Find open visit times for the caller.",
            properties={
                "reason": {"type": "string", "description": "What the visit is for, in the caller's words."},
                "earliest_day": {
                    "type": "string",
                    "description": "The earliest day they can come, YYYY-MM-DD. Leave empty for the next open day.",
                },
            },
            required=["reason"],
            handler=handler,
        )

    def _book_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            try:
                option = int(args.get("option", 0))
            except (TypeError, ValueError):
                option = 0
            if not 1 <= option <= len(self.offered):
                return {"say": "Which option would you like?"}, None
            key = self.book_keys.setdefault(option, secrets.token_urlsafe(24))
            chosen = self.offered[option - 1]
            try:
                booked = await self.ehr.book(self.handle, chosen.token, key)
            except EhrError as e:
                return self._ehr_trouble(e, "book")
            if booked is None:
                logger.info("book: slot taken")
                return {"say": "I'm sorry, that time was just taken. Let me look for others."}, None
            logger.info("book: booked")
            self.offered = []
            return {"say": f"You're booked for {_visit_words(booked)}. Is there anything else?"}, self.menu()

        return FlowsFunctionSchema(
            name="book",
            description="Book the option the caller chose.",
            properties={"option": {"type": "integer", "description": "The option number the caller chose."}},
            required=["option"],
            handler=handler,
        )

    def _next_visit_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            try:
                visit = await self.ehr.next_visit(self.handle)
            except EhrError as e:
                return self._ehr_trouble(e, "next_visit")
            if visit is None:
                return {"say": "I don't see any visits coming up for you."}, None
            return {"say": f"Your next visit is {_visit_words(visit)}."}, None

        return FlowsFunctionSchema(
            name="next_visit",
            description="Tell the caller when their next visit is.",
            properties={},
            required=[],
            handler=handler,
        )

    def _end_call_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            return {"say": "Thank you for calling. Goodbye."}, self.goodbye()

        return FlowsFunctionSchema(
            name="end_call",
            description="End the call when the caller has nothing else.",
            properties={},
            required=[],
            handler=handler,
        )

    # ---- trouble ----

    def _ehr_trouble(self, e: EhrError, call: str):
        logger.warning("ehr {} failed: {}", call, e.kind.value)
        if e.kind in (ErrorKind.NOT_VERIFIED, ErrorKind.NO_SUCH_CALL):
            return {"say": "I'm sorry, I can't do that on this call. Let me connect you with a person."}, self.handed_off()
        if e.kind is ErrorKind.NOT_ALLOWED_NOW:
            return {"say": "I can't do that right now. Let me connect you with a person."}, self.handed_off()
        if e.kind is ErrorKind.BAD_INPUT:
            return {"say": "I didn't catch that clearly. Could you say it again?"}, None
        return {"say": "Our scheduling system isn't answering right now. Let me connect you with a person."}, self.handed_off()
