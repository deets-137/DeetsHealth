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


def test_name_guard_takes_a_name_spelled_letter_by_letter():
    # The call of 2026-11-15, 22:38: speech-to-text heard "Derek Lynn"; the caller spelled it.
    flow = _flow("Hi, my name is Derek Lynn.", "It's D E R R I C K. space L I N.")
    assert flow._was_said("Derrick") and flow._was_said("Lin")
    assert flow._was_said("Derek"), "what the caller first said still counts"
    # Spelled with no break between the names: each name is in the run.
    run = _flow("D-E-R-R-I-C-K L I N", "d e r r i c k l i n")
    assert run._was_said("Derrick") and run._was_said("Lin")
    # Single letters alone are not a name, and a name never spelled is still refused.
    assert not _flow("plan A or B")._was_said("Ab")
    assert not flow._was_said("Smith")


def test_name_guard_refuses_a_filled_in_name():
    flow = _flow("i am billy bob, born february 27th 1990")
    assert flow._was_said("Billy")
    assert not flow._was_said("Smith"), "a last name the caller never said"
    assert not flow._was_said(""), "an empty name"


def test_invented_values():
    sources = ["Option 1: Monday, November 2, at 9 AM", "Jordan Ellis, March 12, 1984"]
    assert invented_values("Option 1 is on Monday.", sources) == []
    assert invented_values("I can also do Tuesday at 3:30.", sources) == ["3:30", "Tuesday"]
