"""The prompt file (agent/prompts/AGENT-PROMPT.md): a bad hand edit must stop the agent at
start-up with a message that says what to fix, never in the middle of a call."""

import pytest

from deets_agent.prompts import PLACEHOLDERS, PromptFileError, SECTIONS, check, parse


def test_the_file_has_every_section_the_code_needs():
    assert set(SECTIONS) == set(PLACEHOLDERS)


def test_lines_join_and_list_items_stay_apart():
    parsed = parse("notes\n## role\nOne\ntwo.\n\nRules:\n- a\nb\n- c\n")
    assert parsed["role"] == "One two.\n\nRules:\n- a b\n- c"


def _sections(**changes: str) -> dict[str, str]:
    sections = dict(SECTIONS)
    sections.update(changes)
    return sections


def test_a_missing_section_is_named():
    sections = dict(SECTIONS)
    del sections["line.goodbye"]
    with pytest.raises(PromptFileError, match="missing section '## line.goodbye'"):
        check(sections)


def test_a_lost_placeholder_is_named():
    with pytest.raises(PromptFileError, match="line.booked"):
        check(_sections(**{"line.booked": "You're booked. Is there anything else?"}))


def test_a_stray_brace_is_named():
    with pytest.raises(PromptFileError, match="line.goodbye"):
        check(_sections(**{"line.goodbye": "Thank you for calling {. Goodbye."}))


def test_an_unknown_section_is_named():
    with pytest.raises(PromptFileError, match="unknown section"):
        check(_sections(**{"line.typo": "Hello?"}))
