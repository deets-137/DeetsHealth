"""The call's shape (VOICE-AGENT.md §8.1): each node has its own instructions and its own tools.

    greet ─► verify (name and birth date, read back, then the code) ─► menu ─┬─► schedule (find, book)
                                                                             └─► next visit
    any node ─► hand off to a person            any node ─► goodbye

Phase 1 features (BUILD-ORDER.md): C1 greet, C2 + C3 verify (C3 with ehr's fake code), C4 hand
off, S1 find, S2 book, S5 next visit.

Two locks on the chart (SECURITY.md §4.9):
- **The Flow:** the chart and booking tools exist only in `menu` and `schedule`, which only a
  PASS from ehr's `CheckCode` leads to.
- **ehr:** every chart and booking call is refused until ehr itself has seen both factors. A
  tool takes no patient argument; the call's handle lives in this object, never in a prompt.

**Every word is in `agent/prompts/AGENT-PROMPT.md`** (the owner, 2026-11-11): the role prompt,
each node's task, each tool's description and every fixed line. This file holds the logic only;
`prompts.text(name, ...)` fetches the words.

**The fixed lines are spoken by the code, not the model** (the owner, 2026-11-11, fork 1A).
Every line a tool produces (`_say`) goes straight to the caller as a `TTSSpeakFrame`, word for
word: the template rule of SECURITY.md §4.9 is held by code, not by the prompt. The tool result
tells the model what the caller heard (`spoken_to_caller`). A line that ends in a question waits
for the caller; any other line lets the model go on. Every line ends in a question except where
the model must act next ("Let me look for others"), because a model that goes on after a fixed
line tends to say it again (2026-11-11).

**The read-back** (the owner, 2026-11-11, option A): `identify` does not look the caller up. It
reads the name and date of birth back as a fixed line and keeps them; only `confirm_details`,
after the caller says yes, sends them to ehr. A slip ("Ellis Jordan, 21 3 1984") is caught
before it costs a try or ends in a code that never comes. The read-back repeats only the
caller's own words, so it tells them nothing about the record.

**Today** is set per call (`today`), so the model's sense of "tomorrow" or "next Tuesday" is
right on a real call. A typed call pins it to `FAKE_TODAY`, because ehr's fake slots are fixed
to the weeks after it and the replay cache needs the same prompt on every run. A call that
reads Epic's sandbox (`ehr_source="epic"`) uses the real date: Epic's visits are real dates,
and ehr moves its stand-in slots to the weekdays after today (EPIC.md §5.2).

Nothing protected goes into Flows' `state` or a log: the handle, the details waiting for the
caller's yes and the offered slots live on this object, as `Shielded`.
"""

from __future__ import annotations

import functools
import json
import re
import secrets
import time
from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import date, datetime
from typing import Any

from loguru import logger
from pipecat.flows import NO_RESPONSE, FlowManager, FlowsFunctionSchema, NodeConfig
from pipecat.frames.frames import TTSSpeakFrame

from deets_agent import config, prompts
from deets_agent.ehr_client import (
    VISIT_MODES,
    BookNotThrough,
    CheckOutcome,
    EhrClient,
    EhrError,
    ErrorKind,
    FoundVisit,
    Slot,
    Visit,
)
from deets_agent.prompts import text
from deets_agent.shielded import Shielded

# The longest reason ehr takes (edge.rs › MAX_REASON).
MAX_REASON_CHARS = 500
# FindVisit's day or month (EPIC.md §7.10).
WHEN = re.compile(r"\d{4}-\d{2}(-\d{2})?")
DAY = re.compile(r"\d{4}-\d{2}-\d{2}")
# The hospital's times of day, by name: the only values find_slots's `time_of_day` may take
# (ehr/fixtures/hospital.json, which ehr reads too; FUTURE-SETTINGS.md §13.1).
TIMES_OF_DAY = tuple(t["name"] for t in json.loads(config.HOSPITAL.read_text(encoding="utf-8"))["times_of_day"])
WEEKDAYS = ("mon", "tue", "wed", "thu", "fri", "sat", "sun")

# The clinic's name is the System variable `{clinic}` (agent/prompts/VARIABLES.md; SYSTEM-PAGE.md
# §3.1): `prompts.text` fills it in any section that holds it.

# ehr's fake slots start the week after this day (ehr/src/scheduler.rs). A typed call and the
# tests use it as "today" so the fakes are in the future and the replay cache holds.
FAKE_TODAY = date(2026, 10, 26)
# The time a typed or test call starts at, for the same reason (the call-start note, 1A).
FAKE_CLOCK = "9 AM CST"
# The earliest birth date ehr accepts (ehr/src/edge.rs).
EARLIEST_BIRTH = date(1900, 1, 1)

_MONTHS = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
]
_DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]


def spoken_day(d: date) -> str:
    return f"{_DAYS[d.weekday()]}, {_MONTHS[d.month - 1]} {d.day}"


def spoken_date(d: date) -> str:
    """'1984-03-21' -> 'March 21, 1984' (a birth date, read back)."""
    return f"{_MONTHS[d.month - 1]} {d.day}, {d.year}"


def spoken_clock(hour: int, minute: int) -> str:
    """14, 30 -> '2:30 PM'; 9, 0 -> '9 AM'."""
    return f"{hour % 12 or 12}{'' if minute == 0 else f':{minute:02d}'} {'AM' if hour < 12 else 'PM'}"


def spoken_time(starts_at: str) -> str:
    """'2026-11-02T14:30' -> 'Monday, November 2, at 2:30 PM'."""
    t = datetime.strptime(starts_at, "%Y-%m-%dT%H:%M")
    hour = t.hour % 12 or 12
    half = "AM" if t.hour < 12 else "PM"
    minute = "" if t.minute == 0 else f":{t.minute:02d}"
    return f"{spoken_day(t.date())}, at {hour}{minute} {half}"


def _visit_words(v: Slot | Visit, say_online: bool = True, zone: str = "") -> str:
    """The one place a visit's values become words (a reveal on purpose). An online visit says
    so after its place (2026-11-14), so the caller knows before choosing; an in-person one reads
    as it always did. `say_online=False` where a line of its own says it (`line.visit_online`).
    `zone`: the hospital's zone, said after an online visit's time, since the caller may be
    elsewhere (EPIC.md §7.10)."""
    how = ", online," if v.online and say_online else ","
    when = spoken_time(v.starts_at.reveal())
    if v.online and zone:
        when = f"{when} {zone}"
    return f"{when}, with {v.clinician.reveal()} at {v.place.reveal()}{how} for {v.minutes} minutes"


def _reason_status(v: Visit) -> str:
    """For the model only: whether the visit's reason is known, never its words. "private": ehr's
    sensitive screen dropped them (FUTURE-SETTINGS.md §9)."""
    if v.reason_private:
        return "private"
    return "said" if v.reason else "none on record"


def _same_visit(a: Slot | Visit, b: Slot | Visit) -> bool:
    """The same time with the same clinician at the same place."""
    return (a.starts_at.reveal(), a.clinician.reveal(), a.place.reveal()) == (
        b.starts_at.reveal(),
        b.clinician.reveal(),
        b.place.reveal(),
    )


# Words a staff summary must not hold (the owner's option A, 2026-11-14, FLAGS.md §3.2): a
# pronoun guessed from a name. The summary names the caller or says "the caller".
_GUESSED_PRONOUNS = re.compile(r"\b(he|she|him|her|his|hers|himself|herself)\b", re.IGNORECASE)


def guesses_a_pronoun(summary: str) -> bool:
    return bool(_GUESSED_PRONOUNS.search(summary))


# The fixed 911 line (the owner's option A, 2026-11-11). Spoken by the `emergency` tool and by
# the phrase screen (screen.py). The hospital sets its own words later (FUTURE-SETTINGS.md §1).
def emergency_line() -> str:
    """The 911 line in this call's words (prompts.current): read when spoken, never at import, so
    a saved version reaches the next call."""
    return text("line.emergency")


def emergency_frame() -> TTSSpeakFrame:
    """The 911 line as a frame that no barge-in can cut: a caller who keeps talking still hears
    all of it (CARTESIA.md §9). Interruptions drop every other line, as Pipecat does."""
    frame = TTSSpeakFrame(text=emergency_line(), append_to_context=False)
    frame.interruptible = False
    return frame


# Why the agent offers a person (offer_staff): it cannot do or see what was asked, or the record
# holds it but it is private (ehr's sensitive screen, FUTURE-SETTINGS.md §9).
OFFER_REASONS = ("cannot_help", "private")


# The kinds of staff flag (FLAGS.md §3; a draft list, the owner's to edit). A flag carries its
# kind out of the wall; its summary and evidence stay inside it.
FLAG_KINDS = (
    "possible_emergency",
    "verification_failed",
    "caller_distressed",
    "calling_for_someone_else",
    "safety_concern",
    "clinical_question_waiting",
    "booking_change",
    # A reason that does not read as a health need: booked as said, for clinical staff to
    # review and call back (the owner's 1A, 2026-11-14, FLAGS.md §3.1).
    "visit_reason_review",
    "other",
)


def _plain_words(value: str) -> list[str]:
    """Lowercase words, apostrophes kept, punctuation dropped: how a quote is matched."""
    return re.findall(r"[a-z0-9']+", value.lower().replace("’", "'"))


@dataclass
class StaffFlag:
    """One flag for staff (the owner, 2026-11-12, forks 1A and 2A): silent to the caller. The
    model's summary, and the caller's own words as its evidence, each checked by code against
    what the caller said. Protected data: Shielded; sealed in the store (FLAGS.md §4)."""

    kind: str
    source: str  # "code" or "model"
    t: float
    summary: Shielded[str] | None = None
    evidence: list[Shielded[str]] = field(default_factory=list)
    # Quotes the model gave that the caller never said. Counted, never kept: a quote the
    # caller did not say is the model's words, not evidence.
    unverified: int = 0
    # The CSN of the visit the flag is about, so staff find it at once (FLAGS.md §3.3). Code
    # picks the visit; the model never sees a CSN.
    csn: Shielded[str] | None = None

    def test_record(self) -> dict[str, Any]:
        """The fields for the dev call record. Test mode keeps full values (REGULATIONS.md §8);
        the store will seal the summary and the evidence."""
        return {
            "flag": self.kind,
            "source": self.source,
            "summary": self.summary.reveal() if self.summary is not None else None,
            "evidence": [q.reveal() for q in self.evidence],
            "unverified": self.unverified,
            "csn": self.csn.reveal() if self.csn is not None else None,
        }


def greeting() -> str:
    """The first words of every call, with the disclosures. The hospital sets its own later
    (FUTURE-SETTINGS.md §4)."""
    return text("line.greeting")


def _words(value: str) -> set[str]:
    """Lowercase words with only their letters and digits ("O'Brien," -> "obrien")."""
    return {w for w in ("".join(c for c in part if c.isalnum()) for part in value.lower().split()) if w}


def role(today: date) -> str:
    """The role prompt. `today` is its only part that changes, once a day, so the prompt cache
    misses once a day per clinic, not once a call."""
    return text("role", today=f"{spoken_day(today)}, {today.year}")


def _tool(name: str, handler, args: dict[str, str] | None = None, required: list[str] | None = None,
          types: dict[str, str] | None = None, schemas: dict[str, dict] | None = None) -> FlowsFunctionSchema:
    """A tool whose description and argument descriptions come from the prompt file. `schemas`
    gives an argument a full JSON schema (an enum, an array) in place of a plain type."""
    args = args or {}
    properties = {
        a: {
            **(schemas or {}).get(a, {"type": (types or {}).get(a, "string")}),
            "description": text(f"arg.{name}.{a}"),
        }
        for a in args
    }
    return FlowsFunctionSchema(
        name=name,
        description=text(f"tool.{name}"),
        properties=properties,
        required=required if required is not None else list(args),
        handler=handler,
    )


# Every step of the Flow, in the order a call meets them, and the ones that end the call. The
# Chat page's Step panel draws the steps still ahead from this (CHAT.md §11). A new node goes
# here too.
STEPS = ("greet", "verify", "menu", "schedule", "goodbye", "hand_off")
CLOSING_STEPS = ("goodbye", "hand_off")
# The steps where the model runs, each with tools of its own.
TOOL_STEPS = ("greet", "verify", "menu", "schedule")

# A note goes to the model in the result of one tool, so it is read in that tool's steps.
NOTE_TOOL = {
    "note.after_find_visit": "find_visit",
    "note.after_offer_instructions": "find_visit",
    "note.after_which_visit": "find_visit",
    "note.after_reminder": "safety_reminder",
    "note.after_flag": "flag_for_staff",
    # Also sent back by hand_off; both are global tools, so the scope is the same.
    "note.summary_pronoun": "flag_for_staff",
}


@functools.cache
def tool_steps() -> dict[str, tuple[str, ...]]:
    """Each tool and the steps that offer it, read from the nodes themselves; "every" for the
    tools of every step (Config › System, SYSTEM-PAGE.md §3.6)."""
    f = CallFlow(ehr=None)  # type: ignore[arg-type]  # builds nodes only; never opens a call
    out: dict[str, list[str]] = {fn.name: ["every"] for fn in f.global_functions()}
    for step in TOOL_STEPS:
        for fn in getattr(f, step)()["functions"]:
            out.setdefault(fn.name, []).append(step)
    return {k: tuple(v) for k, v in out.items()}


@functools.cache
def tool_scope() -> dict[str, frozenset[str]]:
    """The tools each section may name (SYSTEM-PAGE.md §3.5): the tools of the steps where the
    model reads it. A line or the text message: none, since the caller hears or reads it."""
    steps = tool_steps()
    every = frozenset(t for t, s in steps.items() if s == ("every",))
    own = {step: frozenset(t for t, s in steps.items() if step in s) for step in TOOL_STEPS}

    def of_tool(tool: str) -> frozenset[str]:
        where = [s for s in steps.get(tool, ()) if s != "every"]
        if not where:
            return every
        return every | frozenset.intersection(*(own[s] for s in where))

    out: dict[str, frozenset[str]] = {}
    for key in prompts.PLACEHOLDERS:
        kind, _, rest = key.partition(".")
        if key == "role":
            out[key] = every
        elif kind == "task":
            out[key] = every | own.get(rest, frozenset())
        elif kind in ("tool", "arg"):
            out[key] = of_tool(rest.split(".")[0])
        elif kind == "note":
            out[key] = of_tool(NOTE_TOOL.get(key, ""))
        else:
            out[key] = frozenset()
    return out


@dataclass
class CallFlow:
    """One call's Flow. Holds the call's handle and offered slots; nothing else holds them."""

    ehr: EhrClient
    today: date = field(default_factory=date.today)
    # The hospital's time as the call starts, for the model (the owner's 1A, 2026-11-14): in
    # the call's own messages, not in `role`, so the shared prompt cache holds. An Epic call
    # takes it, and today, from ehr's clock in the hospital's time zone (`start`).
    clock: str = FAKE_CLOCK
    # The hospital's time zone's short name ("CST"), said after an online slot's time.
    zone: str = FAKE_CLOCK.split()[-1]
    # The EHR this call reads: `local` (ehr's fakes) or `epic` (Epic's public sandbox, dev only).
    ehr_source: str = "local"
    handle: Shielded[str] | None = None
    # Every option the caller has heard on this call, by its number. A number keeps its time for
    # the whole call, and a new search goes on from the last number, so "option 3" never means
    # two times (the owner, 2026-11-12, after a double booking).
    offered: dict[int, Slot] = field(default_factory=dict)
    # One idempotency key per option the caller chose, so a retry never books twice.
    book_keys: dict[int, str] = field(default_factory=dict)
    # The option read back and waiting for the caller's yes (confirm before booking, the owner's
    # choice A, 2026-11-12), and the visit booked last on this call.
    chosen: int | None = None
    last_booked: Visit | None = None
    # Every visit the caller has heard on this call, by its number, and the last one (for
    # read_instructions and change_visit). The model names a visit by its number only.
    visits_heard: dict[int, Visit] = field(default_factory=dict)
    last_visit: Visit | None = None
    # Set when the Flow moves to a node that ends the call (goodbye, hand off).
    closing: bool = False
    # Every caller line on this call, for the name guard. Protected data: Shielded, call-long.
    caller_lines: list[Shielded[str]] = field(default_factory=list)
    # The details read back and waiting for the caller's yes: (given, family, birth).
    pending: tuple[Shielded[str], Shielded[str], Shielded[str]] | None = None
    # True once the caller has heard the full 911 line on this call (from the `emergency` tool or
    # the phrase screen). Until then, `safety_reminder` speaks the full line, not the reminder.
    warned: bool = False
    # The staff flags raised on this call, and who hears of each (the dev call record today;
    # the store and the rules engine later, FLAGS.md §5).
    flags: list[StaffFlag] = field(default_factory=list)
    on_flag: Callable[[StaffFlag], None] | None = None
    # The tools (`hand_off`, `flag_for_staff`) whose summary went back to the model once for a
    # guessed pronoun (FLAGS.md §3.2). A second summary is kept as it is.
    summary_retries: set[str] = field(default_factory=set)
    # Told each time the Flow moves to a node: its name and its own tools (the dev call record's
    # `step` event; the Chat page's Step panel, CHAT.md §11).
    on_step: Callable[[str, list[str]], None] | None = None
    # The call's LLM service: `_say` pushes each fixed line out of it, so the line is always
    # spoken before the model's next words (call.py sets it). None: the pipeline's worker.
    llm: Any = None

    async def start(self) -> None:
        # The call's prompt version: ehr takes the text message's words from it (CONSOLE.md §9).
        words = prompts.current()
        self.handle = await self.ehr.open_call(self.ehr_source, words.version_id, words.system_id)
        got = self.ehr.call_clock(self.handle) if self.ehr_source == "epic" else None
        if got and got[0]:
            # ehr holds the hospital's one time zone, so "today" is the hospital's, not this PC's.
            at = datetime.strptime(got[0], "%Y-%m-%dT%H:%M")
            self.today = at.date()
            self.clock = f"{spoken_clock(at.hour, at.minute)} {got[1]}".strip()
            self.zone = got[1]

    def clock_note(self) -> str:
        """The call-start note: the hospital's time when the call began (1A)."""
        return text("note.call_start", now=self.clock)

    async def close(self) -> None:
        if self.handle is not None:
            try:
                await self.ehr.close_call(self.handle)
            except EhrError:
                pass
            self.handle = None

    # ---- speaking ----

    async def _say(
        self, flow_manager: FlowManager, line: str, next_node: NodeConfig | None = None, **extra: Any
    ):
        """Speak `line` to the caller word for word, and return the tool's (result, next node).

        A line that ends in a question waits for the caller; any other line lets the model go on.
        """
        frame = emergency_frame() if line == emergency_line() else TTSSpeakFrame(text=line, append_to_context=False)
        if self.llm is not None:
            # Out of the LLM, downstream, now: ahead of this tool's result, and so ahead of
            # whatever the model says next. Queued at the pipeline's top instead, the line raced
            # the model's next reply and once came after it (2026-11-13, `next_visit`: 1 of 562).
            await self.llm.push_frame(frame)
        else:
            await flow_manager.worker.queue_frame(frame)
        result = {"spoken_to_caller": line, **extra}
        wait = line.rstrip().endswith("?")
        if next_node is None:
            return result, (NO_RESPONSE if wait else None)
        if not next_node.get("closing"):
            next_node["respond_immediately"] = not wait
        next_node.pop("closing", None)
        return result, next_node

    # ---- nodes ----

    @staticmethod
    def _task(name: str, **values: str) -> list[dict[str, str]]:
        return [{"role": "developer", "content": text(f"task.{name}", **values)}]

    def _step(self, node: NodeConfig) -> NodeConfig:
        """Report the move to `node`. Each builder below is called only at the moment the Flow
        moves (a tool returns it, or the call starts), so this is when the step begins. Not a
        Flow pre-action: a "function" action runs only when its frame reaches the pipeline's
        end, and it holds up the actions after it."""
        if self.on_step is not None:
            self.on_step(node["name"], [f.name for f in node.get("functions", [])])
        return node

    def greet(self) -> NodeConfig:
        # The greeting carries the disclosures (REGULATIONS.md §6, the owner, 2026-11-12): an
        # automated assistant, the call is transcribed, a person at any time. The law wants
        # those words exact, so the code speaks them as a fixed line before the model runs, and
        # the model waits for the caller. The task tells the model what the caller heard; the
        # line stays out of the context, so the model's first message is the caller's.
        return self._step({
            "name": "greet",
            "role_message": role(self.today),
            "task_messages": [*self._task("greet"), {"role": "developer", "content": self.clock_note()}],
            "functions": [self._identify_fn(), self._confirm_details_fn()],
            "pre_actions": [
                {"type": "tts_say", "text": greeting(), "append_text_to_context": False},
            ],
            "respond_immediately": False,
        })

    def verify(self) -> NodeConfig:
        return self._step({
            "name": "verify",
            "task_messages": self._task("verify"),
            "functions": [
                self._check_code_fn(),
                self._resend_code_fn(),
                self._identify_fn(),
                self._confirm_details_fn(),
            ],
        })

    def menu(self) -> NodeConfig:
        return self._step({
            "name": "menu",
            "task_messages": self._task("menu"),
            "functions": [
                self._find_slots_fn(),
                self._find_visit_fn(),
                self._read_instructions_fn(),
                self._change_visit_fn(),
            ],
        })

    def schedule(self) -> NodeConfig:
        return self._step({
            "name": "schedule",
            "task_messages": self._task("schedule"),
            "functions": [
                self._choose_option_fn(),
                self._book_fn(),
                self._find_slots_fn(),
                self._change_visit_fn(),
            ],
        })

    def _closing_node(self, name: str) -> NodeConfig:
        # The last words were spoken by `_say`; the model does not run again, and the call ends.
        self.closing = True
        return self._step({
            "name": name,
            "task_messages": [],
            "functions": [],
            "respond_immediately": False,
            "pre_actions": [{"type": "end_conversation"}],
            "closing": True,
        })

    def goodbye(self) -> NodeConfig:
        return self._closing_node("goodbye")

    def handed_off(self) -> NodeConfig:
        return self._closing_node("hand_off")

    def global_functions(self) -> list[FlowsFunctionSchema]:
        """The tools of every step (call.py gives them to the FlowManager)."""
        return [
            self.hand_off_fn(),
            self.offer_staff_fn(),
            self.end_call_fn(),
            self.emergency_fn(),
            self.safety_reminder_fn(),
            self.medical_question_fn(),
            self.flag_for_staff_fn(),
        ]

    # ---- staff flags (FLAGS.md) ----

    def _caller_said(self, quote: str) -> bool:
        """True when `quote`'s words appear, in order and together, in one caller line."""
        words = _plain_words(quote)
        if not words:
            return False
        for line in self.caller_lines:
            said = _plain_words(line.reveal())
            if any(said[i : i + len(words)] == words for i in range(len(said) - len(words) + 1)):
                return True
        return False

    def raise_flag(
        self,
        kind: str,
        source: str,
        summary: str | None = None,
        quotes: list[str] | tuple[str, ...] = (),
        csn: Shielded[str] | None = None,
    ) -> StaffFlag | None:
        """Raise a flag. A code flag of one kind is raised once per call; the model may add its
        own of any kind. Each quote is kept only if the caller said it (fork 1A)."""
        if kind not in FLAG_KINDS:
            kind = "other"
        if source == "code" and any(f.kind == kind and f.source == "code" for f in self.flags):
            return None
        kept = [Shielded(q.strip()) for q in quotes if self._caller_said(q)]
        flag = StaffFlag(
            kind=kind,
            source=source,
            t=time.time(),
            summary=Shielded(summary.strip()) if summary and summary.strip() else None,
            evidence=kept,
            unverified=len([q for q in quotes if q.strip()]) - len(kept),
            csn=csn if csn is not None and csn.reveal() else None,
        )
        self.flags.append(flag)
        logger.info("staff flag raised: {} by {}, {} quotes kept", kind, source, len(kept))
        if self.on_flag is not None:
            self.on_flag(flag)
        return flag

    def _summary_back(self, tool: str, summary: str) -> dict[str, str] | None:
        """A summary that guesses a pronoun goes back to the model once, to be written again
        with the caller's name or "the caller" (FLAGS.md §3.2). None: keep it. The caller hears
        nothing: neither tool has spoken yet."""
        if tool in self.summary_retries or not guesses_a_pronoun(summary):
            return None
        self.summary_retries.add(tool)
        logger.info("{}: summary sent back once for a guessed pronoun", tool)
        return {"summary_refused": text("note.summary_pronoun")}

    def flag_for_staff_fn(self) -> FlowsFunctionSchema:
        # The owner, 2026-11-12: the model raises a flag with its notes and its evidence; the
        # caller is not told (2A). Nothing is spoken, so the result tells the model to go on.
        async def handler(args, flow_manager: FlowManager):
            back = self._summary_back("flag_for_staff", str(args.get("summary", "")))
            if back is not None:
                return back, None
            quotes = args.get("evidence") or []
            if isinstance(quotes, str):
                quotes = [quotes]
            flag = self.raise_flag(
                str(args.get("kind", "other")),
                "model",
                str(args.get("summary", "")),
                [str(q) for q in quotes],
            )
            result = {
                "flag_raised": flag.kind,
                "evidence_kept": len(flag.evidence),
                "next_step": text("note.after_flag"),
            }
            return result, None

        return _tool(
            "flag_for_staff",
            handler,
            {"kind": "", "summary": "", "evidence": ""},
            schemas={
                "kind": {"type": "string", "enum": list(FLAG_KINDS)},
                "evidence": {"type": "array", "items": {"type": "string"}},
            },
        )

    # ---- global (every node has these) ----

    def medical_question_fn(self) -> FlowsFunctionSchema:
        # The owner's option 1A (2026-11-11): a fixed line, so no medical wording of the model's
        # own reaches a patient. A tool, so the hospital can set the words and what follows (a
        # nurse line) later (FUTURE-SETTINGS.md §2). It ends in a question, like every fixed line:
        # a statement let the model speak next, and it said the line again (2026-11-11).
        async def handler(args, flow_manager: FlowManager):
            logger.info("medical question line spoken")
            return await self._say(flow_manager, text("line.medical_question"))

        return _tool("medical_question", handler)

    def emergency_fn(self) -> FlowsFunctionSchema:
        # The owner's option A (2026-11-11): the 911 line is fixed, and the call stays open, so a
        # caller who is only worried keeps their place (a chosen option stays offered). What
        # follows the line is a hospital setting later (FUTURE-SETTINGS.md): a nurse line, or
        # an end to the call.
        async def handler(args, flow_manager: FlowManager):
            logger.info("emergency line spoken")
            self.warned = True
            self.raise_flag("possible_emergency", "code")
            return await self._say(flow_manager, emergency_line())

        return _tool("emergency", handler)

    def safety_reminder_fn(self) -> FlowsFunctionSchema:
        # The owner, 2026-11-12: over-warning is fine, but a caller who has said they are safe to
        # go on should not be asked the same question again. The reminder is a statement, so the
        # model goes on with the task after it. The model chooses it; the code holds the safe
        # side: before the caller has heard the full line, the full line is spoken instead.
        async def handler(args, flow_manager: FlowManager):
            if not self.warned:
                logger.info("safety reminder asked before any warning; full 911 line spoken")
                self.warned = True
                self.raise_flag("possible_emergency", "code")
                return await self._say(flow_manager, emergency_line())
            logger.info("safety reminder spoken")
            # A statement lets the model go on, and a model that goes on after a fixed line tends
            # to say it again (AGENT.md §3); it did on 2026-11-12. The result says what comes next.
            return await self._say(
                flow_manager, text("line.emergency_reminder"), next_step=text("note.after_reminder")
            )

        return _tool("safety_reminder", handler)

    def end_call_fn(self) -> FlowsFunctionSchema:
        # Global, so a caller can leave at any step, verified or not; before, "end call" in the
        # verify step had only hand_off, and "goodbye" in greet had no tool at all.
        async def handler(args, flow_manager: FlowManager):
            return await self._say(flow_manager, text("line.goodbye"), self.goodbye())

        return _tool("end_call", handler)

    def hand_off_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            logger.info("hand off requested")
            back = self._summary_back("hand_off", str(args.get("summary") or ""))
            if back is not None:
                return back, None
            # A hand-off carries its own flag (FLAGS.md §2): the staff member learns why the
            # caller is coming to them. A hand-off ends the call, so a separate flag_for_staff
            # in a later reply would never run.
            if args.get("kind") or args.get("summary"):
                quotes = args.get("evidence") or []
                self.raise_flag(
                    str(args.get("kind") or "other"),
                    "model",
                    str(args.get("summary") or ""),
                    [quotes] if isinstance(quotes, str) else [str(q) for q in quotes],
                )
            # Phase 1 has no line to transfer to; the call ends with the words only. After a code
            # that did not work, the caller hears an apology (the owner, 2026-11-12). The same
            # words for every caller, matched or not, so they reveal nothing (SECURITY.md §4.4).
            line = "line.hand_off_code" if args.get("kind") == "verification_failed" else "line.hand_off"
            return await self._say(flow_manager, text(line), self.handed_off())

        return _tool(
            "hand_off",
            handler,
            {"kind": "", "summary": "", "evidence": ""},
            required=[],
            schemas={
                "kind": {"type": "string", "enum": list(FLAG_KINDS)},
                "evidence": {"type": "array", "items": {"type": "string"}},
            },
        )

    # ---- verification ----

    def heard(self, line: str) -> None:
        """Keep a caller line for the name guard (screen.py feeds every line). Protected data:
        held as Shielded, for this call only."""
        self.caller_lines.append(Shielded(line))

    def _was_said(self, name: str) -> bool:
        """True when every word of `name` is a word the caller said on this call."""
        words = _words(name)
        if not words:
            return False
        said: set[str] = set()
        for line in self.caller_lines:
            said |= _words(line.reveal())
        return words <= said

    def _possible_birth(self, value: str) -> bool:
        try:
            born = date.fromisoformat(value)
        except ValueError:
            return False
        return EARLIEST_BIRTH <= born <= self.today

    def _identify_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            birth_text = str(args.get("birth_date", "")).strip()
            if not self._possible_birth(birth_text):
                # Caught here, so a slip of the tongue costs no try (ehr refuses it too).
                logger.info("identify: birth date not possible; asked again")
                return await self._say(flow_manager, text("line.birth_not_possible"))
            given_text = str(args.get("given_name", "")).strip()
            family_text = str(args.get("family_name", "")).strip()
            if not (self._was_said(given_text) and self._was_said(family_text)):
                # The name guard (2026-11-11): the model may only send names the caller said. A
                # filled-in or guessed name ("Billy Bob" as both names) is caught here, costs no
                # try, and the caller is asked again.
                logger.info("identify: a name the caller did not say; asked again")
                return await self._say(flow_manager, text("line.name_not_said"))
            # The read-back (option A): nothing goes to ehr until the caller says yes.
            self.pending = (Shielded(given_text), Shielded(family_text), Shielded(birth_text))
            logger.info("identify: details read back")
            line = text(
                "line.read_back",
                given=given_text,
                family=family_text,
                birth=spoken_date(date.fromisoformat(birth_text)),
            )
            return await self._say(flow_manager, line)

        return _tool("identify", handler, {"given_name": "", "family_name": "", "birth_date": ""})

    def _confirm_details_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            if self.pending is None:
                logger.info("confirm_details: nothing read back yet; asked for details")
                return await self._say(flow_manager, text("line.ask_details"))
            given, family, birth = self.pending
            self.pending = None
            try:
                left = await self.ehr.identify(self.handle, given, family, birth)
            except EhrError as e:
                return await self._ehr_trouble(flow_manager, e, "identify")
            logger.info("identify sent, {} tries left", left)
            # The same words whether or not a patient matched (SECURITY.md §4.4).
            return await self._say(flow_manager, text("line.code_sent"), self.verify())

        return _tool("confirm_details", handler)

    def _check_code_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            digits = "".join(c for c in str(args.get("code", "")) if c.isdigit())
            if len(digits) != 6:
                return await self._say(flow_manager, text("line.code_length"))
            try:
                outcome, left = await self.ehr.check_code(self.handle, Shielded(digits))
            except EhrError as e:
                return await self._ehr_trouble(flow_manager, e, "check_code")
            logger.info("code check: {}", outcome.value)
            if outcome is CheckOutcome.PASS:
                return await self._say(flow_manager, text("line.verified"), self.menu())
            if outcome is CheckOutcome.LOCKED:
                self.raise_flag("verification_failed", "code")
                return await self._say(flow_manager, text("line.locked"), self.handed_off())
            return await self._say(flow_manager, text("line.code_wrong"), tries_left=left)

        return _tool("check_code", handler, {"code": ""})

    def _resend_code_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            try:
                left = await self.ehr.resend_code(self.handle)
            except EhrError as e:
                if e.kind is ErrorKind.NOT_ALLOWED_NOW:
                    self.raise_flag("verification_failed", "code")
                    return await self._say(flow_manager, text("line.no_more_codes"), self.handed_off())
                return await self._ehr_trouble(flow_manager, e, "resend_code")
            return await self._say(flow_manager, text("line.code_resent"), resends_left=left)

        return _tool("resend_code", handler)

    # ---- the chart and booking (only in menu and schedule) ----

    def _reason_words(self, given: str) -> str:
        """The reason that goes to the scheduler as the visit's note: the caller's own words, never
        the model's (2026-11-14). The model's words are kept only if the caller said them, as for
        a flag's evidence; otherwise the caller's latest line goes, whole. No reason at all:
        "a visit"."""
        given = given.strip()
        if not given:
            return "a visit"
        if self._caller_said(given):
            return given
        logger.info("find_slots: the reason was not the caller's words; their latest line is sent")
        latest = self.caller_lines[-1].reveal().strip() if self.caller_lines else ""
        return latest[:MAX_REASON_CHARS] or "a visit"

    def _find_slots_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            more = args.get("more") is True or str(args.get("more", "")).lower() == "true"
            reason = Shielded(self._reason_words(str(args.get("reason", ""))))
            day = str(args.get("earliest_day", "") or "").strip()
            last = str(args.get("latest_day", "") or "").strip()
            mode = str(args.get("visit_mode", "") or "either").strip()
            if mode not in VISIT_MODES:
                mode = "either"
            # Only values the hospital's lists hold; anything else is dropped, never sent.
            when = str(args.get("time_of_day", "") or "").strip().lower()
            when = when if when in TIMES_OF_DAY else ""
            days = tuple(d for d in (args.get("days") or []) if isinstance(d, str) and d in WEEKDAYS)
            who = str(args.get("clinician", "") or "").strip()
            try:
                slots, waiting = await self.ehr.find_slots(
                    self.handle,
                    reason,
                    Shielded(day) if DAY.fullmatch(day) else None,
                    mode,
                    latest_day=Shielded(last) if DAY.fullmatch(last) else None,
                    time_of_day=when,
                    days=days,
                    clinician=Shielded(who) if who else None,
                    more=more,
                )
            except EhrError as e:
                return await self._ehr_trouble(flow_manager, e, "find_slots")
            logger.info("find_slots: {} found", len(slots))
            self.chosen = None
            if not slots:
                return await self._say(flow_manager, text("line.no_more_slots" if more else "line.no_slots"))
            numbers = [self._number_for(s) for s in slots]
            options = " ".join(
                text("line.slot_option", number=str(n), visit=_visit_words(s, zone=self.zone))
                for n, s in zip(numbers, slots)
            )
            return await self._say(
                flow_manager,
                text("line.slots", options=options),
                self.schedule(),
                options=numbers,
                more_waiting=waiting,
            )

        return _tool(
            "find_slots",
            handler,
            {
                "reason": "",
                "earliest_day": "",
                "latest_day": "",
                "visit_mode": "",
                "time_of_day": "",
                "days": "",
                "clinician": "",
                "more": "",
            },
            required=["reason"],
            types={"more": "boolean"},
            schemas={
                "visit_mode": {"type": "string", "enum": list(VISIT_MODES)},
                "time_of_day": {"type": "string", "enum": ["", *TIMES_OF_DAY]},
                "days": {"type": "array", "items": {"type": "string", "enum": list(WEEKDAYS)}},
            },
        )

    def _number_for(self, slot: Slot) -> int:
        """The slot's option number on this call: its old number if the caller heard this time
        before, or the next number after every one they have heard."""
        same = (slot.starts_at.reveal(), slot.clinician.reveal(), slot.place.reveal())
        for n, s in self.offered.items():
            if (s.starts_at.reveal(), s.clinician.reveal(), s.place.reveal()) == same:
                self.offered[n] = slot  # the newest token for the same time
                return n
        n = max(self.offered, default=0) + 1
        self.offered[n] = slot
        return n

    def _choose_option_fn(self) -> FlowsFunctionSchema:
        # Confirm before booking (the owner's choice A, 2026-11-12): the agent cannot undo a
        # booking (moving and cancelling go to staff, BUILD-ORDER.md), so the chosen time is read
        # back first, as the details are before the lookup.
        async def handler(args, flow_manager: FlowManager):
            try:
                option = int(args.get("option", 0))
            except (TypeError, ValueError):
                option = 0
            if option not in self.offered:
                return await self._say(flow_manager, text("line.which_option"))
            self.chosen = option
            logger.info("choose_option: read back")
            said = _visit_words(self.offered[option], zone=self.zone)
            line = text("line.confirm_slot", number=str(option), visit=said)
            return await self._say(flow_manager, line)

        return _tool("choose_option", handler, {"option": ""}, types={"option": "integer"})

    def _book_fn(self) -> FlowsFunctionSchema:
        async def handler(args, flow_manager: FlowManager):
            option = self.chosen
            if option is None or option not in self.offered:
                logger.info("book: nothing read back yet")
                return await self._say(flow_manager, text("line.which_option"))
            key = self.book_keys.setdefault(option, secrets.token_urlsafe(24))
            try:
                booked = await self.ehr.book(self.handle, self.offered[option].token, key)
            except BookNotThrough:
                # The owner's V2A: never a second booking without the caller's yes. The choice
                # stays, so a yes books the same time with the same key.
                logger.info("book: the reply was lost and no booking was found")
                return await self._say(flow_manager, text("line.book_not_through"))
            except EhrError as e:
                return await self._ehr_trouble(flow_manager, e, "book")
            self.chosen = None
            if booked is None:
                logger.info("book: slot taken")
                del self.offered[option]
                return await self._say(flow_manager, text("line.slot_taken"))
            logger.info("book: booked")
            self.last_booked = booked
            # The caller hears the visit as booked, "online" in it when it is (2026-11-14). For the
            # model only: how the visit takes place and whether its reason went with it, so a
            # question about this visit is answered, not handed to a person (the Priya Raman
            # call, 2026-11-13).
            return await self._say(
                flow_manager,
                text("line.booked", visit=_visit_words(booked, zone=self.zone)),
                self.menu(),
                visit_is="online" if booked.online else "in person",
                reason_for_visit=_reason_status(booked),
            )

        return _tool("book", handler)

    def _change_visit_fn(self) -> FlowsFunctionSchema:
        # The agent cannot move or cancel a visit (BUILD-ORDER.md: Epic has no API for it), so a
        # change goes to staff. The caller hears which visit stays booked, and staff get a flag
        # with the caller's own words (FLAGS.md).
        async def handler(args, flow_manager: FlowManager):
            asked = [self.caller_lines[-1].reveal()] if self.caller_lines else []
            # The visit the change is about, so its CSN goes with the flag (FLAGS.md §3.3): the
            # number the model gave, else the one booked on this call, else the last one heard.
            try:
                number = int(args.get("visit", 0) or 0)
            except (TypeError, ValueError):
                number = 0
            about = self.visits_heard.get(number) or self.last_booked or self.last_visit
            self.raise_flag("booking_change", "code", quotes=asked, csn=about.csn if about else None)
            if self.last_booked is not None:
                line = text("line.change_booked", visit=_visit_words(self.last_booked, zone=self.zone))
            else:
                line = text("line.change_visit")
            return await self._say(flow_manager, line, self.handed_off())

        return _tool("change_visit", handler, {"visit": ""}, required=[], types={"visit": "integer"})

    def _visit_number(self, visit: Visit) -> int:
        """The visit's number on this call: its old number if the caller heard it before."""
        for n, v in self.visits_heard.items():
            if _same_visit(v, visit):
                self.visits_heard[n] = visit
                return n
        n = max(self.visits_heard, default=0) + 1
        self.visits_heard[n] = visit
        return n

    def _find_visit_fn(self) -> FlowsFunctionSchema:
        # One of the caller's visits (EPIC.md §7.10; the owner's V1A, 2026-11-14): the soonest to
        # come, or one the caller names. ehr matches the words and narrows; the model never sees
        # a CSN or an id, only the visit's number on this call.
        async def handler(args, flow_manager: FlowManager):
            when = str(args.get("when", "") or "").strip()
            if when and not WHEN.fullmatch(when):
                when = ""
            who = str(args.get("clinician", "") or "").strip()
            group = str(args.get("group", "") or "").strip()
            past = args.get("past") is True or str(args.get("past", "")).lower() == "true"
            try:
                found = await self.ehr.find_visit(
                    self.handle,
                    Shielded(when) if when else None,
                    Shielded(who) if who else None,
                    past,
                    Shielded(group) if group else None,
                )
            except EhrError as e:
                return await self._ehr_trouble(flow_manager, e, "find_visit")
            asked = bool(when or who or group)
            if found.groups:
                return await self._say(
                    flow_manager, self._which_visit(found), next_step=text("note.after_which_visit")
                )
            visit = found.visit
            if visit is None:
                if past and not asked:
                    return await self._say(flow_manager, text("line.no_past_visits"))
                return await self._say(flow_manager, text("line.no_visit_match" if asked else "line.no_visits"))
            number = self._visit_number(visit)
            self.last_visit = visit
            words = self._visit_lines(visit, asked)
            if found.more and not asked and not past:
                words.append(
                    text("line.one_more_visit") if found.more == 1 else text("line.more_visits", count=str(found.more))
                )
            # The owner's 2A (2026-11-14): instructions are offered first; read at once when the
            # caller asked about this visit by its day, clinician or group. Never for a visit
            # that is over.
            instructions = "none on record"
            if visit.instructions_private:
                instructions = "private"
            elif visit.instructions.reveal() and visit.status == "upcoming":
                if asked:
                    words.append(text("line.visit_instructions", instructions=visit.instructions.reveal()))
                    instructions = "said"
                else:
                    words.append(text("line.offer_instructions"))
                    instructions = "offered"
            note = "note.after_offer_instructions" if instructions == "offered" else "note.after_find_visit"
            return await self._say(
                flow_manager,
                " ".join(words),
                visit_number=number,
                visit_status=visit.status,
                reason_for_visit=_reason_status(visit),
                instructions=instructions,
                next_step=text(note),
            )

        return _tool(
            "find_visit",
            handler,
            {"when": "", "clinician": "", "past": "", "group": ""},
            required=[],
            types={"past": "boolean"},
        )

    def _which_visit(self, found: FoundVisit) -> str:
        """The narrowing question: the groups by name, and "another visit" for the rest."""
        items = [text("line.visit_group", group=g.reveal()) for g in found.groups]
        if found.other_visits:
            items.append(text("line.another_visit"))
        return text("line.which_visit", groups=f" {text('line.choice_or')} ".join(items))

    def _visit_lines(self, visit: Visit, asked: bool) -> list[str]:
        """The visit's time, then what it is, as the record says (the owner, 2026-11-13: an online
        visit keeps its place). The line ends without a question, so the model goes on and answers
        the rest of what the caller asked (Camila's call, 2026-11-13). The visit this call just
        booked is not read out again: the caller heard it a moment ago (the Priya Raman call)."""
        said = _visit_words(visit, say_online=False, zone=self.zone)
        just_booked = self.last_booked is not None and _same_visit(visit, self.last_booked)
        if visit.status == "seen":
            words = [text("line.past_seen", visit=said)]
        elif visit.status == "missed":
            words = [text("line.past_missed", visit=said)]
        elif visit.status == "cancelled":
            words = [text("line.past_cancelled", visit=said)]
        elif just_booked:
            words = [text("line.booked_visit_asked")]
        elif asked:
            words = [text("line.visit_found", visit=said)]
        else:
            words = [text("line.next_visit", visit=said)]
        if visit.online:
            words.append(text("line.visit_online"))
        elif visit.visit_type:
            words.append(text("line.visit_type", visit_type=visit.visit_type.reveal()))
        if visit.reason:
            words.append(text("line.visit_reason", reason=visit.reason.reveal()))
        return words

    def _read_instructions_fn(self) -> FlowsFunctionSchema:
        # The caller said yes to hearing the instructions (2A), or asked how to prepare.
        async def handler(args, flow_manager: FlowManager):
            visit = self.last_visit
            if visit is None or not visit.instructions.reveal():
                return await self._say(flow_manager, text("line.no_instructions"))
            return await self._say(
                flow_manager, text("line.visit_instructions", instructions=visit.instructions.reveal())
            )

        return _tool("read_instructions", handler)

    def offer_staff_fn(self) -> FlowsFunctionSchema:
        # The owner's option A (2026-11-13), after Camila's call: when the agent cannot do what the
        # caller asks, it says so and offers a person; the caller chooses. A yes is hand_off; a no,
        # the call goes on. A caller who asks for a person, or whose code fails, still goes at once.
        # The words are a hospital setting (FUTURE-SETTINGS.md §10).
        async def handler(args, flow_manager: FlowManager):
            why = args.get("why")
            logger.info("staff offered ({})", why if why in OFFER_REASONS else "cannot_help")
            line = "line.offer_staff_private" if why == "private" else "line.offer_staff"
            return await self._say(flow_manager, text(line))

        return _tool(
            "offer_staff",
            handler,
            {"why": ""},
            schemas={"why": {"type": "string", "enum": list(OFFER_REASONS)}},
        )

    # ---- trouble ----

    async def _ehr_trouble(self, flow_manager: FlowManager, e: EhrError, call: str):
        logger.warning("ehr {} failed: {}", call, e.kind.value)
        if e.kind in (ErrorKind.NOT_VERIFIED, ErrorKind.NO_SUCH_CALL):
            return await self._say(flow_manager, text("line.not_on_this_call"), self.handed_off())
        if e.kind is ErrorKind.NOT_ALLOWED_NOW:
            return await self._say(flow_manager, text("line.not_now"), self.handed_off())
        if e.kind is ErrorKind.BAD_INPUT:
            return await self._say(flow_manager, text("line.say_again"))
        return await self._say(flow_manager, text("line.ehr_down"), self.handed_off())
