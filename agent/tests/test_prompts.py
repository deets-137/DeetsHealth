"""The prompt file (agent/prompts/AGENT-PROMPT.md): a bad hand edit must stop the agent at
start-up with a message that says what to fix, never in the middle of a call."""

import pytest

from deets_agent.prompts import PLACEHOLDERS, SECTIONS, PromptFileError, check, parse


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


def test_ehr_checks_the_same_sections_and_placeholders():
    """ehr's settings.rs › PROMPT checks a save with the same list (CONSOLE.md §9)."""
    import re

    from deets_agent import config

    rust = (config.REPO / "ehr" / "src" / "settings.rs").read_text(encoding="utf-8")
    block = rust.split("pub const PROMPT: &[Setting] = &[", 1)[1].split("];", 1)[0]
    found = {k: set(re.findall(r'"([a-z_]+)"', ph)) for k, ph in re.findall(r'p\("([a-z_.]+)", &\[([^\]]*)\]\)', block)}
    assert found == PLACEHOLDERS


def test_the_agent_page_names_every_section_once():
    """Config › Agent (console/src/pages/agent.ts › SECTIONS) puts each section in one tab with
    a name; a section left out would be missing from the page."""
    import re

    from deets_agent import config

    ts = (config.REPO / "console" / "src" / "pages" / "agent.ts").read_text(encoding="utf-8")
    block = ts.split("const SECTIONS: [string, Tab, string][] = [", 1)[1].split("];", 1)[0]
    keys = re.findall(r'\["([a-z_.]+)", "', block)
    assert sorted(keys) == sorted(PLACEHOLDERS) and len(keys) == len(set(keys))


def test_each_call_uses_its_own_words():
    """A test call sets its words once (dev_server.py › _run_call); outside a call, the file."""
    from deets_agent import prompts

    assert prompts.current() is prompts.FILE_WORDS
    words = prompts.Words(_sections(**{"line.goodbye": "Bye now."}), 7, "11-13 | 2")
    token = prompts.use(words)
    try:
        assert prompts.text("line.goodbye") == "Bye now." and prompts.current().version_id == 7
    finally:
        prompts.reset(token)
    assert prompts.text("line.goodbye") == SECTIONS["line.goodbye"]


def test_a_save_writes_only_the_changed_sections(tmp_path):
    """A save writes the prompt file (PROMPT-EDITOR.md §6.2): the notes and every unchanged
    section keep their lines; the changed one is wrapped, and the file reads back the same."""
    from deets_agent import prompts

    file = tmp_path / "AGENT-PROMPT.md"
    source = prompts.PROMPT_FILE.read_text(encoding="utf-8")
    file.write_text(source, encoding="utf-8")
    prompts.write_file(dict(SECTIONS), file)
    assert file.read_text(encoding="utf-8") == source, "no change, no diff"

    long = "Thank you for calling. " * 12 + "Goodbye."
    prompts.write_file(_sections(**{"line.goodbye": long}), file)
    written = file.read_text(encoding="utf-8")
    assert prompts.parse(written)["line.goodbye"] == long
    assert max(len(line) for line in written.split("## line.goodbye\n", 1)[1].split("\n## ")[0].splitlines()) <= prompts.WRAP
    changed = [a for a, b in zip(source.splitlines(), written.splitlines(), strict=False) if a != b]
    assert changed and all("calling" in line or line.startswith("## ") or not line for line in changed[:1])


def test_a_wrapped_line_never_starts_a_list_item_or_a_heading():
    from deets_agent import prompts

    line = "word " * 18 + "- not an item " + "word " * 10 + "## not a heading " + "word " * 10
    assert prompts.parse("## role\n" + "\n".join(prompts._wrap(line.strip())) + "\n")["role"] == line.strip()
