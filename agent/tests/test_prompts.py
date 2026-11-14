"""The prompt file (agent/prompts/AGENT-PROMPT.md): a bad hand edit must stop the agent at
start-up with a message that says what to fix, never in the middle of a call."""

import re

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


def test_a_section_may_hold_any_system_variable_and_only_its_own_code_values():
    """SYSTEM-PAGE.md §3.1: a section keeps its code values and may add a System variable."""
    check(_sections(**{"line.goodbye": "Thank you for calling {clinic}. Goodbye."}))
    with pytest.raises(PromptFileError, match=r"line.goodbye': \{parking\} is not a System variable"):
        check(_sections(**{"line.goodbye": "Park at {parking}."}))
    with pytest.raises(PromptFileError, match=r"\{given\} is filled only in its own section"):
        check(_sections(**{"line.goodbye": "Bye, {given}."}))
    check(_sections(**{"line.goodbye": "Park at {parking}."}), {"clinic", "parking"})


def test_a_call_fills_its_own_system_variables():
    from deets_agent import prompts

    v = {"clinic": prompts.Variable("The name.", "North Clinic"), "parking": prompts.Variable("Where.", "Lot B")}
    words = prompts.Words(_sections(**{"line.goodbye": "{clinic}: park at {parking}."}), 3, "x", v, 4, "y")
    token = prompts.use(words)
    try:
        assert prompts.text("line.goodbye") == "North Clinic: park at Lot B."
        assert prompts.text("line.booked", visit="Monday") == SECTIONS["line.booked"].format(visit="Monday")
    finally:
        prompts.reset(token)


def test_the_variables_are_checked():
    from deets_agent import prompts

    ok = {"clinic": prompts.Variable("The name.", "Main Street Clinic")}
    prompts.check_variables(ok)
    for bad, match in [
        ({}, "built in"),
        ({**ok, "Bad": prompts.Variable("", "x")}, "not a variable name"),
        ({**ok, "given": prompts.Variable("", "x")}, "filled by the code"),
        ({**ok, "x": prompts.Variable("", "{code}")}, "no { or }"),
        ({**ok, "x": prompts.Variable("", "a\nb")}, "one line"),
        ({**ok, "x": prompts.Variable("", " ")}, "no value"),
    ]:
        with pytest.raises(PromptFileError, match=re.escape(match)):
            prompts.check_variables(bad)


def test_the_variables_file_reads_back(tmp_path):
    from deets_agent import prompts

    file = tmp_path / "VARIABLES.md"
    source = prompts.VARIABLES_FILE.read_text(encoding="utf-8")
    file.write_text(source, encoding="utf-8")
    prompts.write_variables(prompts.load_variables(), file)
    assert file.read_text(encoding="utf-8") == source.replace("\r\n", "\n"), "no change, no diff"
    more = {**prompts.load_variables(), "parking": prompts.Variable("Where to park.", "Lot B")}
    prompts.write_variables(more, file)
    assert prompts.load_variables(file) == more


def test_ehr_reads_the_variables_file_the_same_way():
    """ehr compiles VARIABLES.md in for the text message (settings.rs › file_variables) and keeps
    the same limits."""
    from deets_agent import config, prompts

    rust = (config.REPO / "ehr" / "src" / "settings.rs").read_text(encoding="utf-8")
    for name, value in [
        ("VARIABLE_NAME_MAX", prompts.NAME_MAX),
        ("VARIABLE_MEANING_MAX", prompts.MEANING_MAX),
        ("VARIABLE_VALUE_MAX", prompts.VALUE_MAX),
        ("VARIABLES_MAX", prompts.VARIABLES_MAX),
    ]:
        assert f"pub const {name}: usize = {value};" in rust
    assert 'pub const BUILT_IN: &[&str] = &["clinic"];' in rust and prompts.BUILT_IN == ("clinic",)


def test_a_tool_is_named_in_backticks_and_only_where_its_step_offers_it():
    """SYSTEM-PAGE.md §3.5: the file passes; a plain word is not a mention; a tool outside its
    steps, or in a line the caller hears, is refused."""
    from deets_agent import flow, prompts

    scope, tools = flow.tool_scope(), set(flow.tool_steps())
    assert prompts.mention_problems(SECTIONS, scope, tools) == []
    assert prompts.mention_problems(_sections(**{"task.greet": "Then book a visit."}), scope, tools) == []
    assert prompts.mention_problems(_sections(**{"task.schedule": "Use `book`."}), scope, tools) == []
    assert prompts.mention_problems(_sections(**{"task.greet": "Use `book`."}), scope, tools) == [
        "task.greet: names `book`, but its step does not offer it"
    ]
    assert prompts.mention_problems(_sections(**{"line.goodbye": "Use `end_call`."}), scope, tools) == [
        "line.goodbye: names `end_call`, but the caller hears or reads this section"
    ]
    # A tool's words are read in every step that offers it: find_slots is in menu and schedule.
    assert "book" not in scope["tool.find_slots"] and "next_visit" in scope["tool.next_visit"]
    assert set(flow.NOTE_TOOL) == {k for k in SECTIONS if k.startswith("note.")}


def test_every_tool_has_its_reference_and_each_line_exists():
    """Config › System › Tools reads agent/prompts/TOOLS.md (SYSTEM-PAGE.md §3.6)."""
    from deets_agent import flow, prompts

    ref = prompts.load_tools()
    assert set(ref) == set(flow.tool_steps())
    for name, t in ref.items():
        assert t["does"] and t["record"] in ("reads", "writes", "none"), name
        assert all(line in SECTIONS for line in t["lines"]), name


def test_a_wrapped_line_never_starts_a_list_item_or_a_heading():
    from deets_agent import prompts

    line = "word " * 18 + "- not an item " + "word " * 10 + "## not a heading " + "word " * 10
    assert prompts.parse("## role\n" + "\n".join(prompts._wrap(line.strip())) + "\n")["role"] == line.strip()
