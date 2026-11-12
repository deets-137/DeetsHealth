"""Whole typed calls through the real Flow, the real LLM service and the real ehr, replayed from
tests/replays/: no network to Anthropic, no tokens. A changed prompt or Flow misses the replay
and fails here; rerun the script with the harness in `record` mode to record it again.

The check is ehr's state, not the model's words: after the call, a fresh session for the same
fake patient must see the visit the call booked.
"""

from pathlib import Path

from deets_agent.call import TypedCall
from deets_agent.llm import Mode, build_llm
from deets_agent.shielded import Shielded

SCRIPTS = Path(__file__).parent / "scripts"


def script(name: str) -> list[str]:
    lines = (SCRIPTS / name).read_text(encoding="utf-8").splitlines()
    return [x.strip() for x in lines if x.strip() and not x.startswith("#")]


async def test_book_a_visit(ehr):
    call = TypedCall(ehr, build_llm(Mode.REPLAY))
    replies = [await call.start()]
    for line in script("book_a_visit.txt"):
        replies.append(await call.say(line))
    assert call.ended, "the call should end after the goodbye"
    assert not call.words_before_closing, "the model spoke before the closing tool's fixed line"
    await call.hang_up()

    # Ask ehr directly, as a new call by the same fake patient.
    handle = await ehr.open_call()
    await ehr.identify(handle, Shielded("Jordan"), Shielded("Ellis"), Shielded("1984-03-12"))
    await ehr.check_code(handle, Shielded("246810"))
    visit = await ehr.next_visit(handle)
    assert visit is not None and visit.starts_at.reveal() == "2026-11-02T09:00"


async def _leave(ehr, name: str) -> None:
    """A caller who wants to hang up hears the goodbye line and the call ends, at any step."""
    call = TypedCall(ehr, build_llm(Mode.REPLAY))
    await call.start()
    last = None
    for line in script(name):
        last = await call.say(line)
    assert call.ended, "the call should end when the caller says goodbye"
    assert last is not None and last.endswith("Thank you for calling. Goodbye.")
    assert "staff" not in last, "a caller who only hangs up is not handed off"
    assert not call.words_before_closing, "the model spoke before the closing tool's fixed line"
    await call.hang_up()


async def test_leave_at_greet(ehr):
    await _leave(ehr, "leave_at_greet.txt")


async def test_leave_at_code(ehr):
    await _leave(ehr, "leave_at_code.txt")
