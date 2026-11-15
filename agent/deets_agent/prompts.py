"""The agent's words, loaded from `agent/prompts/AGENT-PROMPT.md` (the owner, 2026-11-11: one file
he can read and edit by hand).

The file holds every word the model reads (the role prompt, each step's task, each tool's and
each argument's description) and every fixed line the caller hears. `text(name, **values)`
returns one section with its `{placeholders}` filled.

The file is checked when this module loads, so a bad edit stops the agent at start-up, not in
the middle of a call: every section in `PLACEHOLDERS` must be there, no other section may be,
each section must hold the code values listed for it, and any other `{name}` must be a System
variable of `agent/prompts/VARIABLES.md` (SYSTEM-PAGE.md §3). `text` fills both.

Joining rules (so the owner can wrap lines as he likes): lines next to each other join with a
space; a blank line starts a new paragraph; a line that starts with "- " starts a new line.
"""

from __future__ import annotations

import hashlib
import json
import re
import string
import unicodedata
from collections.abc import Mapping
from collections.abc import Set as AbstractSet
from contextvars import ContextVar, Token
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

PROMPT_FILE = Path(__file__).resolve().parents[1] / "prompts" / "AGENT-PROMPT.md"
VARIABLES_FILE = PROMPT_FILE.with_name("VARIABLES.md")

# Every section the code uses, and the code values each one must hold: the `{name}`s the code
# fills, each in its own section only (SYSTEM-PAGE.md §3.1). ehr's settings.rs › PROMPT is the
# same list.
PLACEHOLDERS: dict[str, set[str]] = {
    "role": {"today"},
    # The hospital's time when the call began, in the call's own messages (the owner's 1A).
    "note.call_start": {"now"},
    # Sent when the repeat guard dropped a reply that only said a fixed line again (call.py).
    "note.repeat_dropped": set(),
    "task.greet": set(),
    "task.verify": set(),
    "task.menu": set(),
    "task.schedule": set(),
    "tool.identify": set(),
    "arg.identify.given_name": set(),
    "arg.identify.family_name": set(),
    "arg.identify.birth_date": set(),
    # What the caller already said they want (the owner, 2026-11-15: no second question).
    "arg.identify.caller_wants": set(),
    "tool.confirm_details": set(),
    "tool.check_code": set(),
    "arg.check_code.code": set(),
    "tool.resend_code": set(),
    "tool.find_slots": set(),
    "arg.find_slots.reason": set(),
    "arg.find_slots.earliest_day": set(),
    # In person, online or either (the owner's 1A, 2026-11-13).
    "arg.find_slots.visit_mode": set(),
    # The caller's other wishes, and the next slots of the same search (EPIC.md §7.10).
    "arg.find_slots.latest_day": set(),
    "arg.find_slots.time_of_day": set(),
    "arg.find_slots.days": set(),
    "arg.find_slots.clinician": set(),
    "arg.find_slots.any_time": set(),
    "arg.find_slots.more": set(),
    "tool.choose_option": set(),
    "arg.choose_option.option": set(),
    "tool.book": set(),
    "tool.change_visit": set(),
    "arg.change_visit.visit": set(),
    # One of the caller's visits (EPIC.md §7.10).
    "tool.find_visit": set(),
    "arg.find_visit.when": set(),
    "arg.find_visit.clinician": set(),
    "arg.find_visit.past": set(),
    "arg.find_visit.group": set(),
    "tool.read_instructions": set(),
    "tool.end_call": set(),
    "tool.hand_off": set(),
    "tool.emergency": set(),
    "tool.safety_reminder": set(),
    "tool.flag_for_staff": set(),
    "arg.hand_off.kind": set(),
    "arg.hand_off.summary": set(),
    "arg.hand_off.evidence": set(),
    "arg.flag_for_staff.kind": set(),
    "arg.flag_for_staff.summary": set(),
    "arg.flag_for_staff.evidence": set(),
    "tool.medical_question": set(),
    "line.greeting": set(),
    "line.read_back": {"given", "family", "birth"},
    "line.ask_details": set(),
    "line.birth_not_possible": set(),
    "line.name_not_said": set(),
    # The text message with the code. ehr sends it (ehr/src/texts.rs reads this section);
    # it is checked here too, so a bad edit fails at start-up on both sides.
    "text.code": {"code"},
    "line.code_sent": set(),
    "line.code_length": set(),
    "line.verified": set(),
    "line.verified_go": set(),
    "note.after_verified": set(),
    "line.locked": set(),
    "line.code_wrong": set(),
    "line.no_more_codes": set(),
    "line.code_resent": set(),
    # The preference question before the first search (the owner's 1A, 2026-11-15).
    "line.ask_preference_both": set(),
    "line.ask_preference_mode": set(),
    "line.ask_preference_when": set(),
    "note.after_ask_preference": set(),
    "line.no_slots": set(),
    "line.no_more_slots": set(),
    "line.slots": {"options"},
    "line.slot_option": {"number", "visit"},
    "line.which_option": set(),
    # The holding line when a reply is cut off by the token cap (llm.py, call.py).
    "line.one_moment": set(),
    "line.confirm_slot": {"number", "visit"},
    "line.change_booked": {"visit"},
    "line.change_visit": set(),
    "line.slot_taken": set(),
    # A lost booking reply, and no booking found (the owner's V2A).
    "line.book_not_through": set(),
    "line.booked": {"visit"},
    "line.no_visits": set(),
    "line.next_visit": {"visit"},
    # In place of line.next_visit when the next visit is the one this call just booked.
    "line.booked_visit_asked": set(),
    # What the visit is, as the record says (FUTURE-SETTINGS.md §9; the owner, 2026-11-13).
    "line.visit_online": set(),
    "line.visit_type": {"visit_type"},
    "line.visit_reason": {"reason"},
    # A visit the caller named, a past one, the count of the others (EPIC.md §7.10).
    "line.visit_found": {"visit"},
    "line.past_seen": {"visit"},
    "line.past_missed": {"visit"},
    "line.past_cancelled": {"visit"},
    "line.one_more_visit": set(),
    "line.more_visits": {"count"},
    # The count in the window the search found the visit in (7, 30 or 90 days).
    "line.one_more_visit_within": {"days"},
    "line.more_visits_within": {"count", "days"},
    "line.no_visit_match": set(),
    "line.no_past_visits": set(),
    # The narrowing question (the owner's V1A): "Is this about your imaging visit or ...?"
    "line.which_visit": {"groups"},
    "line.visit_group": {"group"},
    "line.another_visit": set(),
    "line.choice_or": set(),
    # How to prepare (the owner's 2A, 2026-11-14).
    "line.offer_instructions": set(),
    "line.visit_instructions": {"instructions"},
    "line.no_instructions": set(),
    "note.after_find_visit": set(),
    "note.after_offer_instructions": set(),
    "note.after_which_visit": set(),
    # When the agent cannot help: it says so and offers a person (FUTURE-SETTINGS.md §10).
    "tool.offer_staff": set(),
    "arg.offer_staff.why": set(),
    "line.offer_staff": set(),
    "line.offer_staff_private": set(),
    "line.goodbye": set(),
    "line.hand_off": set(),
    # The hand-off after the code did not work (FUTURE-SETTINGS.md §7, the owner, 2026-11-12).
    "line.hand_off_code": set(),
    "line.emergency": set(),
    "line.emergency_reminder": set(),
    # A note in a tool result, for the model only: never spoken.
    "note.after_reminder": set(),
    "note.after_flag": set(),
    # A staff summary that guessed a pronoun goes back once (FLAGS.md §3.2).
    "note.summary_pronoun": set(),
    "line.medical_question": set(),
    "line.not_on_this_call": set(),
    "line.not_now": set(),
    "line.say_again": set(),
    "line.ehr_down": set(),
}


CODE_VALUES: frozenset[str] = frozenset().union(*PLACEHOLDERS.values())

# The System variables (SYSTEM-PAGE.md §3.2): the built-in ones and the limits. ehr's
# settings.rs holds the same.
BUILT_IN = ("clinic",)
NAME_MAX = 32
MEANING_MAX = 200
VALUE_MAX = 300
VARIABLES_MAX = 100
_NAME = re.compile(r"[a-z][a-z0-9_]*")


def _control(text: str) -> bool:
    """A control character (a line break, a tab), as Rust's `char::is_control` reads one."""
    return any(unicodedata.category(c) == "Cc" for c in text)


class PromptFileError(Exception):
    """AGENT-PROMPT.md does not match what the code needs. The message says what to fix."""


@dataclass(frozen=True)
class Variable:
    meaning: str
    value: str


def check_variables(variables: dict[str, Variable]) -> None:
    """The System variables fit SYSTEM-PAGE.md §3.2. The message names a variable, never its
    value."""
    problems: list[str] = []
    if len(variables) > VARIABLES_MAX:
        problems.append(f"at most {VARIABLES_MAX} variables")
    for name, v in variables.items():
        if not _NAME.fullmatch(name) or len(name) > NAME_MAX:
            problems.append(f"'{name}' is not a variable name (a to z, 0 to 9 and _, a letter first)")
        elif name in CODE_VALUES:
            problems.append(f"{{{name}}} is filled by the code")
        if len(v.meaning) > MEANING_MAX or _control(v.meaning):
            problems.append(f"{{{name}}}'s meaning: one line, at most {MEANING_MAX} characters")
        if not v.value.strip():
            problems.append(f"{{{name}}} has no value")
        elif len(v.value) > VALUE_MAX or _control(v.value) or any(c in v.value for c in "{}"):
            problems.append(f"{{{name}}}'s value: one line, at most {VALUE_MAX} characters, no {{ or }}")
    problems += [f"{{{b}}} is built in and cannot be removed" for b in BUILT_IN if b not in variables]
    if problems:
        raise PromptFileError(f"{VARIABLES_FILE.name}:\n- " + "\n- ".join(problems))


def parse_variables(source: str) -> dict[str, Variable]:
    """The variables file: a `## name` heading, then `meaning:` and `value:` lines. Text above the
    first heading is ignored (ehr's settings.rs › file_variables reads it the same way)."""
    out: dict[str, dict[str, str]] = {}
    name: str | None = None
    for line in source.replace("\r\n", "\n").split("\n"):
        heading = re.match(r"^## (\S+)\s*$", line)
        if heading:
            name = heading.group(1)
            if name in out:
                raise PromptFileError(f"{VARIABLES_FILE.name}: {{{name}}} appears twice")
            out[name] = {"meaning": "", "value": ""}
        elif name is not None:
            for key in ("meaning", "value"):
                if line.startswith(f"{key}:"):
                    out[name][key] = line[len(key) + 1 :].strip()
    return {k: Variable(**v) for k, v in out.items()}


def load_variables(path: Path | None = None) -> dict[str, Variable]:
    variables = parse_variables((path or VARIABLES_FILE).read_text(encoding="utf-8"))
    check_variables(variables)
    return variables


def variables_to_store(variables: dict[str, Variable]) -> list[dict[str, str]]:
    """The System service's one setting, as ehr keeps it (settings.rs › Kind::Variables)."""
    return [{"name": k, "meaning": v.meaning, "value": v.value} for k, v in variables.items()]


def variables_from_store(raw: Any) -> dict[str, Variable]:
    if not isinstance(raw, list):
        raise PromptFileError("variables: not a list")
    try:
        return {str(r["name"]): Variable(str(r["meaning"]), str(r["value"])) for r in raw}
    except (KeyError, TypeError):
        raise PromptFileError("variables: each needs a name, a meaning and a value") from None


def render_variables(variables: dict[str, Variable], source: str) -> str:
    """The variables file with `variables` in place of its own; the notes above the first
    heading are kept."""
    head = re.split(r"(?m)^## ", source.replace("\r\n", "\n"), maxsplit=1)[0]
    body = "".join(f"\n## {k}\nmeaning: {v.meaning}\nvalue: {v.value}\n" for k, v in variables.items())
    return head.rstrip("\n") + "\n" + body


def write_variables(variables: dict[str, Variable], path: Path | None = None) -> None:
    """Write a saved System version into the variables file (SYSTEM-PAGE.md §3.4)."""
    path = path or VARIABLES_FILE
    check_variables(variables)
    path.write_text(render_variables(variables, path.read_text(encoding="utf-8")), encoding="utf-8")


def _join(lines: list[str]) -> str:
    paragraphs: list[str] = []
    current: list[str] = []
    for raw in lines:
        line = raw.strip()
        if not line:
            if current:
                paragraphs.append("\n".join(current))
                current = []
        elif line.startswith("- ") or not current:
            current.append(line)
        else:
            current[-1] += " " + line
    if current:
        paragraphs.append("\n".join(current))
    return "\n\n".join(paragraphs)


def parse(source: str) -> dict[str, str]:
    """The sections of a prompt file, by name. Text above the first `## ` is ignored."""
    sections: dict[str, list[str]] = {}
    name: str | None = None
    for line in source.replace("\r\n", "\n").split("\n"):
        heading = re.match(r"^## (\S+)\s*$", line)
        if heading:
            name = heading.group(1)
            if name in sections:
                raise PromptFileError(f"{PROMPT_FILE.name}: section '{name}' appears twice")
            sections[name] = []
        elif name is not None:
            sections[name].append(line)
    return {k: _join(v) for k, v in sections.items()}


def check(sections: dict[str, str], names: set[str] | None = None) -> None:
    """Every section is there and fits: it holds its own code values, and any other `{name}`
    is one of `names`, the System variables (the variables file's when None)."""
    if names is None:
        names = set(load_variables())
    missing = sorted(set(PLACEHOLDERS) - set(sections))
    unknown = sorted(set(sections) - set(PLACEHOLDERS))
    problems = [f"missing section '## {m}'" for m in missing]
    problems += [f"unknown section '## {u}' (the code does not use it)" for u in unknown]
    for name, text in sections.items():
        if name not in PLACEHOLDERS:
            continue
        if not text.strip():
            problems.append(f"section '{name}' is empty")
        try:
            found = {f for _, f, _, _ in string.Formatter().parse(text) if f is not None}
        except ValueError as e:
            problems.append(f"section '{name}': {e} (a stray {{ or }})")
            continue
        own = PLACEHOLDERS[name]
        if not own <= found:
            want = " ".join(f"{{{p}}}" for p in sorted(own))
            problems.append(f"section '{name}' needs {want}")
        for extra in sorted(found - own):
            if not _NAME.fullmatch(extra):
                problems.append(f"section '{name}' has a {{…}} that is not a variable name")
            elif extra in CODE_VALUES:
                problems.append(f"section '{name}': {{{extra}}} is filled only in its own section")
            elif extra not in names:
                problems.append(f"section '{name}': {{{extra}}} is not a System variable")
    if problems:
        raise PromptFileError(f"{PROMPT_FILE}:\n- " + "\n- ".join(problems))


def load(path: Path = PROMPT_FILE) -> dict[str, str]:
    sections = parse(path.read_text(encoding="utf-8"))
    check(sections)
    return sections


SECTIONS = load()
VARIABLES = load_variables()


@dataclass(frozen=True)
class Words:
    """The words one call runs with (CONSOLE.md §9): every section and the System variables
    (SYSTEM-PAGE.md §3.4), and the versions they came from. `version_id` 0 and `label` "file":
    the files, as the tests and the typed harness run."""

    sections: dict[str, str]
    version_id: int = 0
    label: str = "file"
    variables: dict[str, Variable] = field(default_factory=lambda: dict(VARIABLES))
    system_id: int = 0
    system_label: str = "file"


FILE_WORDS = Words(SECTIONS)

# The words of the call this code runs in. A test call sets it once, when it starts (dev_server.py
# › _run_call), and every task the call starts inherits it, so a save during the call changes
# nothing in it. Unset: the prompt file.
_CALL: ContextVar[Words | None] = ContextVar("deets_call_words", default=None)


def current() -> Words:
    return _CALL.get() or FILE_WORDS


def use(words: Words) -> Token:
    """Make `words` this call's words, until `reset` with the returned token."""
    return _CALL.set(words)


def reset(token: Token) -> None:
    _CALL.reset(token)


def text(name: str, **values: str) -> str:
    """One section of this call's words, filled: the System variables, then the code values the
    caller gives (`values`; a code value wins over a System variable of the same name, which the
    checks never allow)."""
    words = current()
    return words.sections[name].format(**{**{k: v.value for k, v in words.variables.items()}, **values})


# ---- writing a save back to the file (dev only; PROMPT-EDITOR.md §6.2)

WRAP = 96


def _wrap(paragraph_line: str) -> list[str]:
    """One joined line, wrapped at `WRAP` columns so that `_join` makes it whole again: a
    continuation never starts with "- " (it would start a list item) or "#" (a heading)."""
    words = paragraph_line.split(" ")
    lines: list[str] = []
    current_line = ""
    for word in words:
        if current_line and len(current_line) + 1 + len(word) > WRAP and word != "-" and not word.startswith("#"):
            lines.append(current_line)
            current_line = word
        else:
            current_line = f"{current_line} {word}" if current_line else word
    lines.append(current_line)
    return lines


def _raw_sections(source: str) -> dict[str, list[str]]:
    """Each section's lines as written in the file."""
    out: dict[str, list[str]] = {}
    name: str | None = None
    for line in source.split("\n"):
        heading = re.match(r"^## (\S+)\s*$", line)
        if heading:
            name = heading.group(1)
            out[name] = []
        elif name is not None:
            out[name].append(line)
    return out


def render(sections: dict[str, str], source: str) -> str:
    """The prompt file with `sections` in place of its own: the notes above the first section and
    the sections' order are kept; each section's words are wrapped. Raises PromptFileError when
    the result would not read back as `sections`."""
    source = source.replace("\r\n", "\n")
    head = re.split(r"(?m)^## ", source, maxsplit=1)[0]
    raw = _raw_sections(source)
    order = list(raw) + [k for k in sections if k not in raw]
    out = [head.rstrip("\n") + "\n"]
    for name in order:
        if name not in sections:
            continue
        # A section whose words did not change keeps its own lines, so a save's diff shows only
        # what changed.
        if name in raw and _join(raw[name]) == sections[name]:
            body = raw[name]
            while body and not body[-1].strip():
                body = body[:-1]
        else:
            body = []
            for paragraph in sections[name].split("\n\n"):
                if body:
                    body.append("")
                for line in paragraph.split("\n"):
                    body.extend(_wrap(line))
        out.append(f"\n## {name}\n" + "\n".join(body) + "\n")
    rendered = "".join(out)
    if parse(rendered) != {k: sections[k] for k in order if k in sections}:
        raise PromptFileError("the saved words would not read back the same from the file")
    return rendered


def write_file(sections: dict[str, str], path: Path = PROMPT_FILE, names: set[str] | None = None) -> None:
    """Write a saved version into the prompt file, so git, the tests and edits by hand keep
    working (PROMPT-EDITOR.md §6.2)."""
    check(sections, names)
    path.write_text(render(sections, path.read_text(encoding="utf-8")), encoding="utf-8")


def digest(sections: dict[str, str], variables: dict[str, Variable]) -> str:
    """A hash of the words and the System variables' values (not their wrapping, not the
    meanings): what the kept recordings were made with. A changed value changes what the model
    reads, so it makes the recordings out of date (SYSTEM-PAGE.md §3.4)."""
    canon = json.dumps(
        {"sections": sections, "variables": {k: v.value for k, v in variables.items()}},
        sort_keys=True,
        ensure_ascii=False,
    )
    return hashlib.sha256(canon.encode()).hexdigest()


# ---- the reference for Config › System (SYSTEM-PAGE.md §3.6)

TOOLS_FILE = PROMPT_FILE.with_name("TOOLS.md")

# Each code value: what it is, where the code takes it from, and whether it is protected data
# (SECURITY.md §2). Shown on System › Variables, never with a value.
CODE_VALUE_INFO: dict[str, tuple[str, str, bool]] = {
    "today": ("Today's date, as the model reads it.", "The clock, when the call starts.", False),
    "given": ("The caller's given name, as they said it.", "The caller.", True),
    "family": ("The caller's family name, as they said it.", "The caller.", True),
    "birth": ("The caller's date of birth, spoken.", "The caller.", True),
    "code": ("The six-digit code in the text message.", "ehr, when the text goes out.", True),
    "options": ("Every offered time, each said with its number.", "The record's open slots.", True),
    "number": ("A time's option number on this call.", "The code, per call.", False),
    "visit": ("A visit's time, clinician, place and length, spoken.", "The record.", True),
    "visit_type": ("What kind of visit it is, as the record says.", "The record.", True),
    "reason": ("Why the visit is, as the record says.", "The record.", True),
    "now": ("The hospital's time when the call began, with its time zone.", "ehr's clock, in the hospital's time zone.", False),
    "count": ("How many more visits are coming.", "The record.", True),
    "days": ("How many days ahead the count of visits covers.", "ehr's search window.", False),
    "groups": ("The kinds of visit the caller is asked to choose from.", "The record and the hospital's visit groups.", True),
    "group": ("One kind of visit, by the hospital's name for it.", "The hospital's visit groups.", True),
    "instructions": ("How to prepare for the visit, as the record says.", "The record.", True),
}


# The code values that come from the caller or the record, during a call; the rest (`today`, `now`)
# are the code's own.
CALL_VALUES = frozenset(CODE_VALUES - {"today", "now"})


def load_tools(path: Path | None = None) -> dict[str, dict[str, Any]]:
    """The tool reference (TOOLS.md): per tool, what it does, what it does with the record, and
    the lines it can speak."""
    out: dict[str, dict[str, Any]] = {}
    name: str | None = None
    for line in (path or TOOLS_FILE).read_text(encoding="utf-8").replace("\r\n", "\n").split("\n"):
        heading = re.match(r"^## (\S+)\s*$", line)
        if heading:
            name = heading.group(1)
            out[name] = {"does": [], "record": "none", "lines": []}
        elif name is None or not line.strip():
            continue
        elif line.startswith("record:"):
            out[name]["record"] = line[7:].strip()
        elif line.startswith("lines:"):
            names = [x.strip() for x in line[6:].split(",")]
            out[name]["lines"] = [x for x in names if x and x != "none"]
        else:
            out[name]["does"].append(line.strip())
    for t in out.values():
        t["does"] = " ".join(t["does"])
    return out


GUIDE_FILE = PROMPT_FILE.with_name("GUIDE.md")
# Config › Agent's tabs (AGENT-PAGE.md §2), in the call's order; one word each (the owner).
GUIDE_TABS = ("Core", "Greet", "Verify", "Menu", "Schedule", "Anytime", "Failure")


def load_guide(path: Path | None = None) -> list[dict[str, str]]:
    """Config › Agent's guide (GUIDE.md): per section, in the page's order, its key, tab, name,
    the tool it sits under ("" for none) and when it plays."""
    out: list[dict[str, str]] = []
    for line in (path or GUIDE_FILE).read_text(encoding="utf-8").replace("\r\n", "\n").split("\n"):
        heading = re.match(r"^## (\S+)\s*$", line)
        if heading:
            out.append({"key": heading.group(1), "tab": "", "name": "", "under": "", "plays": ""})
        elif out:
            for field_name in ("tab", "name", "under", "plays"):
                if line.startswith(f"{field_name}:"):
                    value = line[len(field_name) + 1 :].strip()
                    out[-1][field_name] = "" if value == "-" else value
    return out


def file_digest() -> str:
    """The digest of the prompt file and the variables file."""
    return digest(load(), load_variables())


# ---- tool names in the words (SYSTEM-PAGE.md §3.5)

# A tool's name in backticks: the only form the check reads. A plain name is not a mention.
_MENTION = re.compile(r"`([a-z][a-z0-9_]*)`")


def mentions(text: str, tools: set[str]) -> set[str]:
    """The tools a section names: each known tool name in backticks."""
    return {m for m in _MENTION.findall(text) if m in tools}


def mention_problems(sections: dict[str, str], allowed: Mapping[str, AbstractSet[str]], tools: set[str]) -> list[str]:
    """Each section names only the tools of the steps where it is read (`allowed`, from
    flow.py › tool_scope). Each problem names the section and the tool."""
    problems = []
    for name, text in sections.items():
        for tool in sorted(mentions(text, tools) - allowed.get(name, set())):
            why = "the caller hears or reads this section" if not allowed.get(name) else "its step does not offer it"
            problems.append(f"{name}: names `{tool}`, but {why}")
    return problems


def check_mentions(sections: dict[str, str], allowed: Mapping[str, AbstractSet[str]], tools: set[str]) -> None:
    problems = mention_problems(sections, allowed, tools)
    if problems:
        raise PromptFileError(f"{PROMPT_FILE}:\n- " + "\n- ".join(problems))


def placeholders_in(text: str) -> set[str]:
    """The `{name}`s in a text; none when a stray brace makes it unreadable."""
    try:
        return {f.split(".")[0].split("[")[0] for _, f, _, _ in string.Formatter().parse(text) if f}
    except ValueError:
        return set()
