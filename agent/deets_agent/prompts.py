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

import re
import string
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
    "tool.book": set(),
    "arg.book.option": set(),
    "tool.next_visit": set(),
    "tool.end_call": set(),
    "tool.hand_off": set(),
    "tool.emergency": set(),
    "tool.medical_question": set(),
    "line.greeting": {"clinic"},
    "line.read_back": {"given", "family", "birth"},
    "line.ask_details": set(),
    "line.birth_not_possible": set(),
    "line.name_not_said": set(),
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
    "line.slot_taken": set(),
    "line.booked": {"visit"},
    "line.no_visits": set(),
    "line.next_visit": {"visit"},
    "line.goodbye": set(),
    "line.hand_off": set(),
    "line.emergency": set(),
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


def text(name: str, **values: str) -> str:
    """One section of the prompt file, with its placeholders filled."""
    return SECTIONS[name].format(**values)
