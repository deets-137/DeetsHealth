"""The code checks that need no model: the emergency phrase screen (option 2A), the name guard,
and the invented-value check (2026-11-11)."""

from deets_agent.call import invented_values
from deets_agent.flow import CallFlow
from deets_agent.screen import emergency_match


def test_emergency_phrases_match():
    for line in [
        "Option 1. Am I dying?",
        "I have chest pain",
        "I can’t breathe",
        "my dad is having trouble breathing",
        "I think I overdosed",
        "I want to kill myself",
        "she passed out",
        "should I call 911",
    ]:
        assert emergency_match(line), line


def test_ordinary_lines_do_not_match():
    for line in [
        "I'm dying to get in this week",
        "Jordan Ellis, March 12, 1984",
        "Just a routine check-up, any day is fine",
        "I have a bad headache",
    ]:
        assert not emergency_match(line), line


def _flow(*lines: str) -> CallFlow:
    flow = CallFlow(ehr=None)  # the name guard never calls ehr
    for line in lines:
        flow.heard(line)
    return flow


def test_name_guard_takes_names_the_caller_said():
    flow = _flow("hi my name is casey", "morgan")
    assert flow._was_said("Casey") and flow._was_said("Morgan")
    assert _flow("I'm Mary-Ann O'Brien")._was_said("Mary-Ann")
    assert _flow("I'm Mary-Ann O'Brien")._was_said("O'Brien")


def test_name_guard_refuses_a_filled_in_name():
    flow = _flow("i am billy bob, born february 27th 1990")
    assert flow._was_said("Billy")
    assert not flow._was_said("Smith"), "a last name the caller never said"
    assert not flow._was_said(""), "an empty name"


def test_invented_values():
    sources = ["Option 1: Monday, November 2, at 9 AM", "Jordan Ellis, March 12, 1984"]
    assert invented_values("Option 1 is on Monday.", sources) == []
    assert invented_values("I can also do Tuesday at 3:30.", sources) == ["3:30", "Tuesday"]
