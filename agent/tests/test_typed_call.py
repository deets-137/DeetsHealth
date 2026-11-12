"""Whole typed calls through the real Flow, the real LLM service and the real ehr, replayed from
tests/replays/: no network to Anthropic, no tokens. A changed prompt or Flow misses the replay
and fails here; rerun the script with the harness in `record` mode to record it again.

The check is ehr's state, not the model's words: after the call, a fresh session for the same
fake patient must see the visit the call booked. Every call is also checked for the model's
words before a fixed-line tool and for values the model said that no one gave it.
"""

from pathlib import Path

from deets_agent.call import TypedCall
from deets_agent.flow import greeting
from deets_agent.llm import Mode, build_llm
from deets_agent.shielded import Shielded

SCRIPTS = Path(__file__).parent / "scripts"


def script(name: str) -> list[str]:
    lines = (SCRIPTS / name).read_text(encoding="utf-8").splitlines()
    return [x.strip() for x in lines if x.strip() and not x.startswith("#")]


def assert_clean(call: TypedCall) -> None:
    assert not call.words_before_silent_tool, "the model spoke before a fixed-line tool"
    assert not call.invented, f"the model said a value no one gave it: {call.invented}"


async def run(ehr, name: str) -> tuple[TypedCall, list[str | None]]:
    call = TypedCall(ehr, build_llm(Mode.REPLAY))
    # The disclosures are a fixed line: every call starts with them, word for word.
    assert await call.start() == greeting()
    replies = [await call.say(line) for line in script(name)]
    assert_clean(call)
    await call.hang_up()
    return call, replies


async def assert_booked_9am(ehr) -> None:
    """Ask ehr directly, as a new call by the same fake patient."""
    handle = await ehr.open_call()
    await ehr.identify(handle, Shielded("Jordan"), Shielded("Ellis"), Shielded("1984-03-12"))
    await ehr.check_code(handle, Shielded("246810"))
    visit = await ehr.next_visit(handle)
    assert visit is not None and visit.starts_at.reveal() == "2026-11-02T09:00"


async def test_book_a_visit(ehr):
    call, _ = await run(ehr, "book_a_visit.txt")
    assert call.ended, "the call should end after the goodbye"
    await assert_booked_9am(ehr)


async def _leave(ehr, name: str) -> None:
    """A caller who wants to hang up hears the goodbye line and the call ends, at any step."""
    call, replies = await run(ehr, name)
    last = replies[-1]
    assert call.ended, "the call should end when the caller says goodbye"
    assert last is not None and last.endswith("Thank you for calling. Goodbye.")
    assert "staff" not in last, "a caller who only hangs up is not handed off"


async def test_leave_at_greet(ehr):
    await _leave(ehr, "leave_at_greet.txt")


async def test_leave_at_code(ehr):
    await _leave(ehr, "leave_at_code.txt")


async def test_emergency_then_book(ehr):
    """Option A (2026-11-11): the 911 line is fixed, the call stays open, and the booking goes on.
    "Am I dying?" is on the screen's list (option 2A), so the code speaks the line itself."""
    call, replies = await run(ehr, "emergency_then_book.txt")
    warned = [i for i, r in enumerate(replies) if r and "call 911 now" in r]
    assert warned, "the caller should hear the fixed 911 line"
    assert warned[0] < len(replies) - 1, "the call should stay open after the 911 line"
    assert call.screen_fired, "the phrase screen should have spoken the line"
    assert call.ended
    await assert_booked_9am(ehr)


async def test_read_back_correction(ehr):
    """Option A (2026-11-11): the details are read back before the lookup, the caller corrects
    the name order and the date, and the code then verifies."""
    call, replies = await run(ehr, "read_back_correction.txt")
    read_backs = [r for r in replies if r and r.startswith("I have ")]
    assert len(read_backs) == 2, "one read-back for the slip, one for the correction"
    assert "Jordan Ellis, born March 12, 1984" in read_backs[1]
    assert any(r and "you're verified" in r for r in replies)
    assert call.ended


async def test_off_topic(ehr):
    """The off-topic rule (2026-11-11): no answers to arithmetic or how-to; the call ends on goodbye.
    `run` fails on an invented value, so a sum such as "23" fails here."""
    call, replies = await run(ehr, "off_topic.txt")
    assert not any(r and "print(" in r for r in replies), "the model gave a Python answer"
    assert call.ended


async def test_code_confusion(ehr):
    """The owner, 2026-11-12: a caller confused after a failed code goes to a person at once,
    not to more tries. Two near-identical records on one date mean no code went out (twins)."""
    call, replies = await run(ehr, "code_confusion.txt")
    assert any(r and "didn't match" in r for r in replies), "the first code should fail"
    last = replies[-1]
    assert last is not None and "member of our staff now" in last, "the confused caller goes to a person"
    assert call.ended


async def test_medical_question(ehr):
    """Option 1A (2026-11-11): a medical question gets the fixed line, and the booking goes on."""
    call, replies = await run(ehr, "medical_question.txt")
    assert any(r and "can't answer medical questions" in r for r in replies)
    assert call.ended
    await assert_booked_9am(ehr)
