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
import hashlib
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
    FoundSlots,
    FoundVisit,
    Slot,
    Visit,
)
from deets_agent.prompts import text
from deets_agent.shielded import Shielded

# The longest reason ehr takes (edge.rs › MAX_REASON).
MAX_REASON_CHARS = 500
# The longest place or kind of care sent to ehr (ehr's edge.rs › MAX_NAME).
MAX_PLACE_CHARS = 100
# FindVisit's day or month (EPIC.md §7.10).
WHEN = re.compile(r"\d{4}-\d{2}(-\d{2})?")
DAY = re.compile(r"\d{4}-\d{2}-\d{2}")
# The file hospital's times of day, by name (ehr/fixtures/hospital.json, the Default version of
# Config › Hospital): a call takes its own from OpenCall (`CallFlow.times_of_day`).
TIMES_OF_DAY = tuple(t["name"] for t in json.loads(config.HOSPITAL.read_text(encoding="utf-8"))["times_of_day"])
WEEKDAYS = ("mon", "tue", "wed", "thu", "fri", "sat", "sun")
# What a caller may ask of a place (place_info's `asks`), in the order the answer says them.
PLACE_ASKS = ("where", "phone", "hours")
# What a caller may say they want before they are verified (identify's `caller_wants`).
CALLER_WANTS = ("book", "visit", "both")

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


_DAY_NAMES = {
    "mon": "Monday",
    "tue": "Tuesday",
    "wed": "Wednesday",
    "thu": "Thursday",
    "fri": "Friday",
    "sat": "Saturday",
    "sun": "Sunday",
}


def _hours_words(hours: tuple[tuple[str, str, str], ...]) -> str:
    """(day, opens, closes) rows, Monday first -> 'Monday to Friday, 8 AM to 5 PM, and Saturday,
    8 AM to 12 PM'. Days in a row with the same hours are said together (BOOKING-RULES.md §3.7)."""

    def clock(hh_mm: str) -> str:
        h, m = hh_mm.split(":")
        return spoken_clock(int(h), int(m))

    runs: list[list[str]] = []  # [first day, last day, opens, closes]
    for day, opens, closes in hours:
        last = runs[-1] if runs else None
        follows = last is not None and WEEKDAYS.index(day) == WEEKDAYS.index(last[1]) + 1
        if follows and last[2:] == [opens, closes]:
            last[1] = day
        else:
            runs.append([day, day, opens, closes])
    parts = [
        f"{_DAY_NAMES[a]}{'' if a == b else f' to {_DAY_NAMES[b]}'}, {clock(o)} to {clock(c)}" for a, b, o, c in runs
    ]
    return parts[0] if len(parts) == 1 else f"{', '.join(parts[:-1])}, and {parts[-1]}"


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
    # The Booking rules (BOOKING-RULES.md §3.5), raised by code: a visit booked under Book and
    # flag, and a caller a rule sends to a person. Drafts for the owner to name.
    "referral_check",
    "booking_rule_hand_off",
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


# Words that only ask for a visit. A reason made of these alone says nothing about what the
# visit is for ("book an appointment", "I'd like to see the doctor").
_BOOKING_WORDS = {
    "a", "an", "the", "to", "i", "id", "im", "would", "like", "want", "need", "please", "can",
    "book", "booking", "make", "schedule", "set", "up", "get", "new", "appointment",
    "appointments", "visit", "visits", "see", "doctor", "dr", "with", "my", "me", "in", "come",
}


# The first words of a sentence that asks something ("Is this with my doctor", "Do you need").
_QUESTION_STARTS = {
    "is", "are", "was", "were", "do", "does", "did", "can", "could", "will", "would", "should",
    "may", "what", "whats", "when", "where", "who", "whos", "why", "how", "which",
}


def _asks_question(line: str) -> bool:
    """True when the caller's line holds a question: a sentence that ends in "?", or one that
    starts with a question word (speech-to-text may drop the mark). "Can I do option 2" counts:
    the model then just answers it."""
    for sentence in re.split(r"(?<=[.!?])\s+", line.strip()):
        words = sentence.split()
        if not words:
            continue
        if sentence.rstrip().endswith("?"):
            return True
        if "".join(c for c in words[0].lower() if c.isalpha()) in _QUESTION_STARTS:
            return True
    return False


def _no_reason(given: str) -> bool:
    """True when `given` holds no reason: empty, or only words that ask for a visit."""
    return _words(given) <= _BOOKING_WORDS


def _spelled(line: str) -> set[str]:
    """The names a caller spelled in `line`, letter by letter: each run of two or more single
    letters, joined ("D E R R I C K. space L I N." -> {"derrick", "lin"}). Any word that is not
    one letter ("space", "the") ends a run."""
    runs: set[str] = set()
    run = ""
    for part in line.lower().split():
        token = "".join(c for c in part if c.isalpha())
        if len(token) == 1:
            run += token
            continue
        if len(run) >= 2:
            runs.add(run)
        run = ""
    if len(run) >= 2:
        runs.add(run)
    return runs


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

# The notes that belong to no tool: the call-start note (1A) and the repeat guard's (call.py).
NOTES_WITHOUT_TOOL = frozenset({"note.call_start", "note.repeat_dropped"})
# A note goes to the model in the result of one tool, so it is read in that tool's steps.
NOTE_TOOL = {
    "note.after_find_visit": "find_visit",
    "note.after_offer_instructions": "find_visit",
    "note.after_which_visit": "find_visit",
    "note.after_reminder": "safety_reminder",
    "note.after_verified": "check_code",
    "note.after_flag": "flag_for_staff",
    "note.after_ask_reason": "find_slots",
    "note.question_with_choice": "choose_option",
    "note.after_ask_preference": "find_slots",
    # The place questions (BOOKING-RULES.md §3.1, §3.2). The unknown place's note is also sent by
    # find_slots and find_visit; place_info is in every step, so its scope holds theirs.
    "note.after_ask_place": "find_slots",
    "note.after_referral_needed": "find_slots",
    "note.after_place_unknown": "place_info",
    "note.after_place_info": "place_info",
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
    # The names `find_slots` may send as `time_of_day`: the call's own hospital version, from
    # OpenCall (`start`); the file's until then (HOSPITAL-CONFIG.md).
    times_of_day: tuple[str, ...] = TIMES_OF_DAY
    # Whether the call can book both in person and online (OpenCall): only then is the caller
    # asked which (the owner's 1A, 2026-11-15). And whether the preference question was asked:
    # once a call, before the first search.
    books_both_modes: bool = True
    preference_asked: bool = False
    # Whether the caller was asked what the visit is for: once a call, before the preference
    # question and the first search (the owner, 2026-11-16: "ask for a reason before presenting
    # options (gives time for 911 flags, specialty care, etc)").
    reason_asked: bool = False
    # Whether the caller was asked where (BOOKING-RULES.md §3.2, 1B): once a call. A search after
    # it that names no place looks anywhere, so the question is never asked twice.
    place_asked: bool = False
    # The caller line and option of a choice that came with a question: noted once, so the
    # next choose_option reads back (the owner's 1A, 2026-11-16).
    questioned_choice: tuple[int, int] | None = None
    # How many caller lines there were when the agent offered a person (offer_staff): a hand-off
    # on the caller's next line says it is for what the agent could not answer (the owner's 2C,
    # 2026-11-16).
    staff_offered_at: int | None = None
    # The options of the list the caller heard last, by number. Each new search starts again at
    # Option 1 and the earlier times are no longer offered; "more" goes on from the last number
    # (the owner's N2, 2026-11-24). The read-back before booking (choose_option) says the time,
    # so a number never books a time the caller did not hear last (the double booking of
    # 2026-11-12 came before the read-back).
    offered: dict[int, Slot] = field(default_factory=dict)
    # One idempotency key per time the caller chose, by the time itself, not its number: a retry
    # never books twice, even after a new list gave the same time another number.
    book_keys: dict[str, str] = field(default_factory=dict)
    # The option read back and waiting for the caller's yes (confirm before booking, the owner's
    # choice A, 2026-11-12), and the visit booked last on this call.
    chosen: int | None = None
    last_booked: Visit | None = None
    # Every visit the caller has heard on this call, by its number, and the last one (for
    # read_instructions and change_visit). The model names a visit by its number only.
    visits_heard: dict[int, Visit] = field(default_factory=dict)
    # What the caller said they want, before verification (identify's `caller_wants`): after the
    # code passes, the agent goes on to it and does not ask again (the owner, 2026-11-15).
    wants: str = ""
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
        self.times_of_day = self.ehr.call_times(self.handle) or self.times_of_day
        both = self.ehr.call_books_both(self.handle)
        if both is not None:
            self.books_both_modes = both
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
            self.place_info_fn(),
        ]

    def place_info_fn(self) -> FlowsFunctionSchema:
        # Where to go (BOOKING-RULES.md §3.7): the address, phone and hours of a place, from the
        # hospital's lists only, so it works before verification. With no place named, the place
        # of the visit the caller just heard or booked (only a verified caller has heard one).
        async def handler(args, flow_manager: FlowManager):
            place = str(args.get("place", "") or "").strip()[:MAX_PLACE_CHARS]
            if not place:
                heard = self.last_booked or self.last_visit
                place = heard.place.reveal() if heard is not None else ""
            if not place:
                return await self._say(flow_manager, text("line.place_info_which"))
            try:
                found = await self._once_more(lambda: self.ehr.place_info(self.handle, Shielded(place)), "place_info")
            except EhrError as e:
                return await self._ehr_trouble(flow_manager, e, "place_info")
            logger.info("place_info: {}", found.outcome)
            if found.outcome == "unknown":
                return await self._say(
                    flow_manager, text("line.unknown_place"), next_step=text("note.after_place_unknown")
                )
            if found.outcome == "which":
                items = [p.reveal() for p in found.places]
                if found.other_places:
                    items.append(text("line.another_place"))
                return await self._say(
                    flow_manager, text("line.which_place", places=f" {text('line.choice_or')} ".join(items))
                )
            name = found.name.reveal()
            # Only what the caller asked about (a desk test, 2026-11-22: an hours question heard
            # "I don't have the address … I don't have a phone number" first). Nothing named: all.
            raw = args.get("asks") or []
            asks = {a for a in (raw if isinstance(raw, list) else [raw]) if a in PLACE_ASKS} or set(PLACE_ASKS)
            if asks == {"hours"} and not found.hours:
                asks = set(PLACE_ASKS)
            words = []
            if "where" in asks:
                words.append(
                    text("line.place_address", place=name, address=found.address.reveal())
                    if found.address.reveal()
                    else text("line.place_no_address", place=name)
                )
            if "phone" in asks:
                words.append(
                    text("line.place_phone", phone=found.phone.reveal())
                    if found.phone.reveal()
                    else text("line.place_no_phone", place=name)
                )
            if "hours" in asks and found.hours:
                words.append(text("line.place_hours", hours=_hours_words(found.hours)))
            logger.info("place_info: asked {}", ",".join(a for a in PLACE_ASKS if a in asks))
            return await self._say(
                flow_manager,
                " ".join(words),
                closed_days=[{"date": d, "name": n} for d, n in found.closed_days],
                next_step=text("note.after_place_info"),
            )

        return _tool(
            "place_info",
            handler,
            {"place": "", "asks": ""},
            required=[],
            schemas={"asks": {"type": "array", "items": {"type": "string", "enum": list(PLACE_ASKS)}}},
        )

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
            # On the caller's reply to offer_staff (a yes, or the question asked again), the line
            # says the person is for what the agent could not answer (the owner's 2C, 2026-11-16).
            if line == "line.hand_off" and self.staff_offered_at is not None and len(self.caller_lines) - self.staff_offered_at <= 1:
                line = "line.hand_off_answer"
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
        """True when every word of `name` is a word the caller said on this call, or is in a
        name the caller spelled letter by letter ("D E R R I C K, space, L I N"; the call of
        2026-11-15, 22:38: speech-to-text heard "Derek Lynn", the caller spelled it, and the
        guard refused the spelled name)."""
        words = _words(name)
        if not words:
            return False
        said: set[str] = set()
        spelled: set[str] = set()
        for line in self.caller_lines:
            said |= _words(line.reveal())
            spelled |= _spelled(line.reveal())
        return all(w in said or any(w in run for run in spelled) for w in words)

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
            wants = str(args.get("caller_wants", "") or "").strip()
            if wants in CALLER_WANTS and not self.wants:
                self.wants = wants
            logger.info("identify: details read back")
            line = text(
                "line.read_back",
                given=given_text,
                family=family_text,
                birth=spoken_date(date.fromisoformat(birth_text)),
            )
            return await self._say(flow_manager, line)

        return _tool(
            "identify",
            handler,
            {"given_name": "", "family_name": "", "birth_date": "", "caller_wants": ""},
            required=["given_name", "family_name", "birth_date"],
            schemas={"caller_wants": {"type": "string", "enum": ["", *CALLER_WANTS]}},
        )

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
                if self.wants:
                    # The caller already said what they need: no question; the model goes on to it.
                    return await self._say(
                        flow_manager,
                        text("line.verified_go"),
                        self.menu(),
                        caller_wants=self.wants,
                        next_step=text("note.after_verified"),
                    )
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
            given = str(args.get("reason", "") or "")
            # The reason question (the owner, 2026-11-16): once a call, before anything else the
            # search needs, when the caller has given no reason. "Book an appointment" is no
            # reason (the call of 2026-11-15, 22:36, filed it as the visit's note).
            if not more and not self.reason_asked and _no_reason(given):
                self.reason_asked = True
                logger.info("find_slots: reason asked")
                return await self._say(flow_manager, text("line.ask_reason"), next_step=text("note.after_ask_reason"))
            self.reason_asked = True
            reason = Shielded(self._reason_words(given))
            day = str(args.get("earliest_day", "") or "").strip()
            last = str(args.get("latest_day", "") or "").strip()
            said_mode = str(args.get("visit_mode", "") or "").strip()
            said_mode = said_mode if said_mode in VISIT_MODES else ""
            mode = said_mode or "either"
            # Only values the hospital's lists hold; anything else is dropped, never sent.
            when = str(args.get("time_of_day", "") or "").strip().lower()
            when = when if when in self.times_of_day else ""
            days = tuple(d for d in (args.get("days") or []) if isinstance(d, str) and d in WEEKDAYS)
            who = str(args.get("clinician", "") or "").strip()
            any_time = args.get("any_time") is True or str(args.get("any_time", "")).lower() == "true"
            any_place = args.get("any_place") is True or str(args.get("any_place", "")).lower() == "true"
            # The place and the kind of care, in the caller's words (BOOKING-RULES.md §3.1, the
            # owner's 2A: a specialty only from the caller's words); ehr matches them.
            place = str(args.get("place", "") or "").strip()[:MAX_PLACE_CHARS]
            specialty = str(args.get("specialty", "") or "").strip()[:MAX_PLACE_CHARS]
            # Asked where once: a search that still names no place looks anywhere.
            if self.place_asked and not place:
                any_place = True

            def search(plan_only: bool = False):
                return self.ehr.find_slots(
                    self.handle,
                    reason,
                    Shielded(day) if DAY.fullmatch(day) else None,
                    mode,
                    latest_day=Shielded(last) if DAY.fullmatch(last) else None,
                    time_of_day=when,
                    days=days,
                    clinician=Shielded(who) if who else None,
                    more=more,
                    any_place=any_place,
                    place=Shielded(place) if place else None,
                    specialty=Shielded(specialty) if specialty else None,
                    plan_only=plan_only,
                )

            # The preference question (the owner's 1A, 2026-11-15): once a call, before the first
            # search, the caller is asked what they have not said: in person or online (only when
            # the hospital books both), and a day or time. No search runs until they answer. When
            # ehr would ask where (1B), the same question asks it too.
            if not more and not self.preference_asked:
                ask_mode = self.books_both_modes and not said_mode
                ask_when = not (any_time or DAY.fullmatch(day) or DAY.fullmatch(last) or when or days)
                if ask_mode or ask_when:
                    self.preference_asked = True
                    key = "both" if ask_mode and ask_when else "mode" if ask_mode else "when"
                    logger.info("find_slots: preference asked ({})", key)
                    words, note = text(f"line.ask_preference_{key}"), text("note.after_ask_preference")
                    try:
                        plan = await search(plan_only=True)
                    except EhrError:
                        plan = None
                    # A Booking rule that offers no time answers now: no preference to ask.
                    if plan is not None and plan.rule:
                        self.preference_asked = False
                        return await self._rule_answer(flow_manager, plan.rule, reason)
                    if plan is not None and plan.place_ask in ("where", "which"):
                        self.place_asked = True
                        words = f"{words} {self._place_question(plan)}"
                        note = f"{note} {text('note.after_ask_place')}"
                    return await self._say(flow_manager, words, next_step=note)
                self.preference_asked = True
            try:
                found = await self._once_more(search, "find_slots")
            except EhrError as e:
                return await self._ehr_trouble(flow_manager, e, "find_slots")
            # A Booking rule held back every time (§3.5): its outcome is the answer.
            if found.rule:
                return await self._rule_answer(flow_manager, found.rule, reason)
            # No search ran: a question about the place or the kind of care (§3.1).
            if found.place_ask:
                logger.info("find_slots: place asked ({})", found.place_ask)
                if found.place_ask in ("where", "which"):
                    self.place_asked = True
                    return await self._say(
                        flow_manager, self._place_question(found), next_step=text("note.after_ask_place")
                    )
                return await self._say(
                    flow_manager, text(f"line.{found.place_ask}"), next_step=text("note.after_place_unknown")
                )
            slots, waiting, usual = found.slots, found.more, found.usual
            logger.info("find_slots: {} found", len(slots))
            self.chosen = None
            if not slots:
                return await self._say(flow_manager, text("line.no_more_slots" if more else "line.no_slots"))
            # A new search: a new list from Option 1 (N2). More times: the list goes on.
            if not more:
                self.offered.clear()
            numbers = [self._number_for(s) for s in slots]
            options = " ".join(
                text("line.slot_option", number=str(n), visit=_visit_words(s, zone=self.zone))
                for n, s in zip(numbers, slots)
            )
            # The usual place, named with the times so the caller can ask for another
            # (BOOKING-RULES.md §3.2), and the one sooner time with another clinician (§3.3, P-A).
            if usual.reveal():
                options = f"{text('line.slots_usual_place', place=usual.reveal())} {options}"
            for n, s in zip(numbers, slots):
                if s.sooner:
                    options = f"{options} {text('line.slot_sooner', number=str(n))}"
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
                "any_time": "",
                "more": "",
                "any_place": "",
                "place": "",
                "specialty": "",
            },
            required=["reason"],
            types={"more": "boolean", "any_time": "boolean", "any_place": "boolean"},
            schemas={
                "visit_mode": {"type": "string", "enum": ["", *VISIT_MODES]},
                "time_of_day": {"type": "string", "enum": ["", *self.times_of_day]},
                "days": {"type": "array", "items": {"type": "string", "enum": list(WEEKDAYS)}},
            },
        )

    @staticmethod
    def _slot_id(slot: Slot) -> str:
        """A time's identity on this call (its start, clinician and place), as a hash: the
        booking key's index, never the words themselves."""
        same = f"{slot.starts_at.reveal()}\x1f{slot.clinician.reveal()}\x1f{slot.place.reveal()}"
        return hashlib.sha256(same.encode()).hexdigest()

    def _number_for(self, slot: Slot) -> int:
        """The slot's option number in the list the caller hears: its number if this list (with
        its "more") already gave this time, or the next number."""
        same = self._slot_id(slot)
        for n, s in self.offered.items():
            if self._slot_id(s) == same:
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
            # A question asked with the choice (the owner's 1A, 2026-11-16; the call of 2026-11-15,
            # 22:36, dropped two): the read-back waits for the caller's answer, so the model
            # answers first. Once per choice; the next choose_option reads back.
            latest = self.caller_lines[-1].reveal() if self.caller_lines else ""
            if self.questioned_choice != (len(self.caller_lines), option) and _asks_question(latest):
                self.questioned_choice = (len(self.caller_lines), option)
                logger.info("choose_option: a question came with the choice; answered first")
                return {"next_step": text("note.question_with_choice", number=str(option))}, None
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
            key = self.book_keys.setdefault(self._slot_id(self.offered[option]), secrets.token_urlsafe(24))
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
            # Book and flag (BOOKING-RULES.md §3.5): staff check the referral before the visit.
            # Silent to the caller; the flag names the kind and carries the visit's CSN.
            if booked.check_referral:
                self.raise_flag("referral_check", "code", csn=booked.csn)
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
            place = str(args.get("place", "") or "").strip()[:MAX_PLACE_CHARS]
            past = args.get("past") is True or str(args.get("past", "")).lower() == "true"
            try:
                found = await self._once_more(
                    lambda: self.ehr.find_visit(
                        self.handle,
                        Shielded(when) if when else None,
                        Shielded(who) if who else None,
                        past,
                        Shielded(group) if group else None,
                        Shielded(place) if place else None,
                    ),
                    "find_visit",
                )
            except EhrError as e:
                return await self._ehr_trouble(flow_manager, e, "find_visit")
            if found.unknown_place:
                return await self._say(flow_manager, text("line.unknown_place"), next_step=text("note.after_place_unknown"))
            asked = bool(when or who or group or place)
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
                days = str(found.more_within_days)
                if found.more_within_days:
                    words.append(
                        text("line.one_more_visit_within", days=days)
                        if found.more == 1
                        else text("line.more_visits_within", count=str(found.more), days=days)
                    )
                else:
                    words.append(
                        text("line.one_more_visit")
                        if found.more == 1
                        else text("line.more_visits", count=str(found.more))
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
            {"when": "", "clinician": "", "past": "", "group": "", "place": ""},
            required=[],
            types={"past": "boolean"},
        )

    async def _rule_answer(self, flow_manager: FlowManager, rule: str, reason: Shielded[str]):
        """A Booking rule that offers no time (BOOKING-RULES.md §3.5). Referral needed: the caller
        hears why and is offered a primary care visit or a person. Hand off: a person books it;
        staff get a flag with the caller's own words for the reason."""
        logger.info("find_slots: booking rule ({})", rule)
        if rule == "referral_needed":
            return await self._say(
                flow_manager, text("line.referral_needed"), next_step=text("note.after_referral_needed")
            )
        self.raise_flag("booking_rule_hand_off", "code", quotes=[reason.reveal()])
        return await self._say(flow_manager, text("line.rule_hand_off"), self.handed_off())

    def _place_question(self, found: FoundSlots) -> str:
        """Where the caller would like the visit (BOOKING-RULES.md §3.2, 1B): the places by name
        when one question can name them, with "another place" for the rest; else the open
        question."""
        if found.place_ask != "which" or not found.places:
            return text("line.ask_place")
        items = [p.reveal() for p in found.places]
        if found.other_places:
            items.append(text("line.another_place"))
        return text("line.which_place", places=f" {text('line.choice_or')} ".join(items))

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
        # The owner's 2C (2026-11-16): the same question asked again is hand_off too, and the
        # line says why (`line.hand_off_answer`). The words are a hospital setting
        # (FUTURE-SETTINGS.md §10).
        async def handler(args, flow_manager: FlowManager):
            why = args.get("why")
            self.staff_offered_at = len(self.caller_lines)
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

    async def _once_more(self, call: Callable[[], Any], name: str) -> Any:
        """A read that timed out or found ehr down is sent once more, quietly, before the caller
        hears the system is down (the Camila call of 2026-11-15: one slow Epic search ended the
        call). Reads only: a booking is never sent twice (V2A)."""
        try:
            return await call()
        except EhrError as e:
            if e.kind is not ErrorKind.UNAVAILABLE:
                raise
            logger.info("ehr {}: no answer in time; sent once more", name)
            return await call()

    async def _ehr_trouble(self, flow_manager: FlowManager, e: EhrError, call: str):
        logger.warning("ehr {} failed: {}", call, e.kind.value)
        if e.kind in (ErrorKind.NOT_VERIFIED, ErrorKind.NO_SUCH_CALL):
            return await self._say(flow_manager, text("line.not_on_this_call"), self.handed_off())
        if e.kind is ErrorKind.NOT_ALLOWED_NOW:
            return await self._say(flow_manager, text("line.not_now"), self.handed_off())
        if e.kind is ErrorKind.BAD_INPUT:
            return await self._say(flow_manager, text("line.say_again"))
        return await self._say(flow_manager, text("line.ehr_down"), self.handed_off())
