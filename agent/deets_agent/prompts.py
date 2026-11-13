"""The agent's words, loaded from `agent/prompts/AGENT-PROMPT.md` (the owner, 2026-11-11: one file
he can read and edit by hand).

The file holds every word the model reads (the role prompt, each step's task, each tool's and
each argument's description) and every fixed line the caller hears. `text(name, **values)`
returns one section with its `{placeholders}` filled.

The file is checked when this module loads, so a bad edit stops the agent at start-up, not in
the middle of a call: every section in `PLACEHOLDERS` must be there, no other section may be,
and each section must use exactly the placeholders listed for it.

Joining rules (so the owner can wrap lines as he likes): lines next to each other join with a
space; a blank line starts a new paragraph; a line that starts with "- " starts a new line.
"""

from __future__ import annotations

import hashlib
import json
import re
import string
from contextvars import ContextVar, Token
from dataclasses import dataclass
from pathlib import Path

PROMPT_FILE = Path(__file__).resolve().parents[1] / "prompts" / "AGENT-PROMPT.md"

# Every section the code uses, and the placeholders each one must have.
PLACEHOLDERS: dict[str, set[str]] = {
    "role": {"clinic", "today"},
    "task.greet": {"clinic"},
    "task.verify": set(),
    "task.menu": set(),
    "task.schedule": set(),
    "tool.identify": set(),
    "arg.identify.given_name": set(),
    "arg.identify.family_name": set(),
    "arg.identify.birth_date": set(),
    "tool.confirm_details": set(),
    "tool.check_code": set(),
    "arg.check_code.code": set(),
    "tool.resend_code": set(),
    "tool.find_slots": set(),
    "arg.find_slots.reason": set(),
    "arg.find_slots.earliest_day": set(),
    "tool.choose_option": set(),
    "arg.choose_option.option": set(),
    "tool.book": set(),
    "tool.change_visit": set(),
    "tool.next_visit": set(),
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
    "line.greeting": {"clinic"},
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
    "line.locked": set(),
    "line.code_wrong": set(),
    "line.no_more_codes": set(),
    "line.code_resent": set(),
    "line.no_slots": set(),
    "line.slots": {"options"},
    "line.slot_option": {"number", "visit"},
    "line.which_option": set(),
    # The holding line when a reply is cut off by the token cap (llm.py, call.py).
    "line.one_moment": set(),
    "line.confirm_slot": {"number", "visit"},
    "line.change_booked": {"visit"},
    "line.change_visit": set(),
    "line.slot_taken": set(),
    "line.booked": {"visit"},
    "line.no_visits": set(),
    "line.next_visit": {"visit"},
    # What the visit is, as the record says (FUTURE-SETTINGS.md §9; the owner, 2026-11-13).
    "line.visit_online": set(),
    "line.visit_type": {"visit_type"},
    "line.visit_reason": {"reason"},
    "note.after_next_visit": set(),
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
    "line.medical_question": set(),
    "line.not_on_this_call": set(),
    "line.not_now": set(),
    "line.say_again": set(),
    "line.ehr_down": set(),
}


class PromptFileError(Exception):
    """AGENT-PROMPT.md does not match what the code needs. The message says what to fix."""


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


def check(sections: dict[str, str]) -> None:
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
        if found != PLACEHOLDERS[name]:
            want = ", ".join(sorted(PLACEHOLDERS[name])) or "none"
            have = ", ".join(sorted(found)) or "none"
            problems.append(f"section '{name}' has placeholders {have}; it needs {want}")
    if problems:
        raise PromptFileError(f"{PROMPT_FILE}:\n- " + "\n- ".join(problems))


def load(path: Path = PROMPT_FILE) -> dict[str, str]:
    sections = parse(path.read_text(encoding="utf-8"))
    check(sections)
    return sections


SECTIONS = load()


@dataclass(frozen=True)
class Words:
    """The words one call runs with (CONSOLE.md §9): every section, and the version they came
    from. `version_id` 0 and `label` "file": the prompt file, as the tests and the typed harness
    run."""

    sections: dict[str, str]
    version_id: int = 0
    label: str = "file"


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
    """One section of this call's words, with its placeholders filled."""
    return current().sections[name].format(**values)


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


def write_file(sections: dict[str, str], path: Path = PROMPT_FILE) -> None:
    """Write a saved version into the prompt file, so git, the tests and edits by hand keep
    working (PROMPT-EDITOR.md §6.2)."""
    check(sections)
    path.write_text(render(sections, path.read_text(encoding="utf-8")), encoding="utf-8")


def digest(sections: dict[str, str]) -> str:
    """A hash of the words alone (not their wrapping): what the kept recordings were made with."""
    canon = json.dumps(sections, sort_keys=True, ensure_ascii=False)
    return hashlib.sha256(canon.encode()).hexdigest()
