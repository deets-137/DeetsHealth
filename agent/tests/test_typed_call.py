"""Whole typed calls through the real Flow, the real LLM service and the real ehr, replayed from
tests/replays/: no network to Anthropic, no tokens. A changed prompt or Flow misses the replay
and fails here; rerun the script with the harness in `record` mode to record it again.

The checks are on events, not the model's words (AGENT.md §4, step 3): the tools the model
called, ehr's answers (a booking, a code check), the staff flags, and ehr's own state after the
call (a fresh session for the same fake patient must see the visit the call booked). Words are
checked only where the code speaks a fixed line (`text(...)`, AGENT-PROMPT.md). Every call is
also checked for the model's words before a fixed-line tool and for values the model said that
no one gave it.

**Each draw writes its record** (call_record.py, origin `kept`): to `DEETS_KEPT_DIR` in a Test
run, with a line in `kept.jsonl` there (the test, the script, the record, the replies read), which
the dev server sends to the call store (DATABASE.md §8.3); to a temporary folder otherwise.
"""

import json
import os
import tempfile
from pathlib import Path
from typing import Any

from deets_agent import config, llm
from deets_agent.call import TypedCall
from deets_agent.call_record import CallRecord
from deets_agent.catalog import read_events
from deets_agent.flow import greeting, guesses_a_pronoun
from deets_agent.llm import Mode, build_llm
from deets_agent.prompts import text
from deets_agent.shielded import Shielded

SCRIPTS = Path(__file__).parent / "scripts"
# A Test run's folder for the draws' records (dev_server.py › record_and_test).
_KEPT = os.environ.get("DEETS_KEPT_DIR")
_SCRATCH = tempfile.TemporaryDirectory(prefix="deets-kept-") if not _KEPT else None


def script(name: str) -> list[str]:
    lines = (SCRIPTS / name).read_text(encoding="utf-8").splitlines()
    return [x.strip() for x in lines if x.strip() and not x.startswith("#")]


def assert_clean(call: TypedCall) -> None:
    assert not call.words_before_silent_tool, "the model spoke before a fixed-line tool"
    assert not call.invented, f"the model said a value no one gave it: {call.invented}"


class Draw:
    """One kept-script draw: the call, the replies, and the record's events."""

    def __init__(self, call: TypedCall, replies: list[str | None], events: list[dict[str, Any]]) -> None:
        self.call = call
        self.replies = replies
        self.events = events

    def tools(self) -> list[str]:
        """The tools the model called, in order."""
        return [e.get("name") for e in self.events if e.get("kind") == "tool_call"]

    def ehr(self, call: str) -> list[str | None]:
        """ehr's domain answers to one gRPC call, in order (`pass`, `booked`, ...)."""
        return [e.get("result") for e in self.events if e.get("kind") == "ehr" and e.get("call") == call]

    def booked(self) -> int:
        return self.ehr("Book").count("booked")

    def flags(self) -> list[str]:
        """The staff flags' kinds."""
        return [e.get("flag") for e in self.events if e.get("kind") == "staff_flag"]


def _test_name() -> str:
    current = os.environ.get("PYTEST_CURRENT_TEST", "")
    return current.split("::")[-1].split(" ")[0]


async def draw(ehr, name: str) -> Draw:
    folder = Path(_KEPT) if _KEPT else Path(_SCRATCH.name)
    record = CallRecord("replay", config.MODEL, folder=folder, source="kept", scenario=Path(name).stem)
    llm.SEEN = []
    call = TypedCall(ehr, build_llm(Mode.REPLAY), record)
    reason = "the agent ended it"
    replies: list[str | None] = []
    # hang_up() in `finally`: a reply that is not recorded fails the test at once, and the
    # pipeline is stopped, so the run goes on to the next test (it hung before, 2026-11-12).
    try:
        # The disclosures are a fixed line: every call starts with them, word for word.
        assert await call.start() == greeting()
        for line in script(name):
            replies.append(await call.say(line))
    except BaseException as e:
        reason = f"the harness failed ({type(e).__name__})"
        raise
    finally:
        await call.hang_up()
        record.close(reason if call.ended else "hung up")
        # The ehr client outlives the call (the checks below ask ehr again): it writes no more
        # to this record.
        ehr.record = None
        keys, llm.SEEN = llm.SEEN or [], None
        if _KEPT:
            line = {"test": _test_name(), "script": Path(name).stem, "record": record.path.name, "keys": sorted(set(keys))}
            with (Path(_KEPT) / "kept.jsonl").open("a", encoding="utf-8") as f:
                f.write(json.dumps(line) + "\n")
    assert_clean(call)
    return Draw(call, replies, read_events(record.path))


async def run(ehr, name: str) -> tuple[TypedCall, list[str | None]]:
    d = await draw(ehr, name)
    return d.call, d.replies


async def assert_booked_9am(ehr) -> None:
    """Ask ehr directly, as a new call by the same fake patient."""
    handle = await ehr.open_call()
    await ehr.identify(handle, Shielded("Jordan"), Shielded("Ellis"), Shielded("1984-03-12"))
    await ehr.check_code(handle, Shielded("246810"))
    visit = (await ehr.find_visit(handle)).visit
    assert visit is not None and visit.starts_at.reveal() == "2026-11-02T09:00"


async def test_book_a_visit(ehr):
    d = await draw(ehr, "book_a_visit.txt")
    assert d.ehr("CheckCode") == ["pass"], "verified once, before the booking"
    assert d.booked() == 1, "exactly one booking"
    assert d.tools()[-1] == "end_call" and d.call.ended, "the call should end after the goodbye"
    await assert_booked_9am(ehr)


async def _leave(ehr, name: str) -> None:
    """A caller who wants to hang up hears the goodbye line and the call ends, at any step."""
    d = await draw(ehr, name)
    last = d.replies[-1]
    assert d.call.ended and d.tools()[-1] == "end_call", "the call should end when the caller says goodbye"
    assert last is not None and last.endswith("Thank you for calling. Goodbye.")
    assert "hand_off" not in d.tools(), "a caller who only hangs up is not handed off"
    assert d.booked() == 0


async def test_leave_at_greet(ehr):
    await _leave(ehr, "leave_at_greet.txt")


async def test_leave_at_code(ehr):
    await _leave(ehr, "leave_at_code.txt")


async def test_emergency_then_book(ehr):
    """Option A (2026-11-11): the 911 line is fixed, the call stays open, and the booking goes on.
    "Am I dying?" is on the screen's list (option 2A), so the code speaks the line itself."""
    d = await draw(ehr, "emergency_then_book.txt")
    warned = [i for i, r in enumerate(d.replies) if r and "call 911 now" in r]
    assert warned, "the caller should hear the fixed 911 line"
    assert warned[0] < len(d.replies) - 1, "the call should stay open after the 911 line"
    assert d.call.screen_fired, "the phrase screen should have spoken the line"
    assert "possible_emergency" in d.flags(), "the emergency is flagged for staff"
    assert d.booked() == 1 and d.call.ended
    await assert_booked_9am(ehr)


async def test_read_back_correction(ehr):
    """Option A (2026-11-11): the details are read back before the lookup, the caller corrects
    the name order and the date, and the code then verifies."""
    d = await draw(ehr, "read_back_correction.txt")
    read_backs = [r for r in d.replies if r and r.startswith("I have ")]
    assert len(read_backs) == 2, "one read-back for the slip, one for the correction"
    assert "Jordan Ellis, born March 12, 1984" in read_backs[1]
    assert d.tools().count("identify") == 2 and d.tools().count("confirm_details") == 1, "confirmed once, after the fix"
    assert d.ehr("CheckCode") == ["pass"], "the code verifies"
    assert d.call.ended


async def test_off_topic(ehr):
    """The off-topic rule (2026-11-11): no answers to arithmetic or how-to; the call ends on goodbye.
    `draw` fails on an invented value, so a sum such as "23" fails here."""
    d = await draw(ehr, "off_topic.txt")
    assert not any(r and "print(" in r for r in d.replies), "the model gave a Python answer"
    assert d.tools() == ["end_call"], "no tool but the goodbye: nothing off-topic was done"
    assert d.call.ended


async def test_emergency_reminder(ehr):
    """The owner, 2026-11-12: a caller who said they are safe and mentions the same danger again
    hears the one-sentence reminder, not the full 911 line; and details the caller already gave
    are used, never asked for again."""
    d = await draw(ehr, "emergency_reminder.txt")
    full = [r for r in d.replies if r and "call 911 now" in r]
    assert len(full) == 1, f"the full 911 line should play once, not {len(full)} times"
    assert any(r and r.startswith(text("line.emergency_reminder")) for r in d.replies)
    assert "safety_reminder" in d.tools(), "the reminder is the tool's fixed line"
    assert d.tools().count("identify") == 1, "the details were asked for again"
    assert d.booked() == 1 and d.call.ended
    await assert_booked_9am(ehr)


async def test_flag_for_someone_else(ehr):
    """Forks 1A and 2A, 2026-11-12: a caller for someone else gets a model flag whose evidence is
    the caller's own words; the caller never hears of it."""
    d = await draw(ehr, "flag_for_someone_else.txt")
    model_flags = [f for f in d.call.flow.flags if f.source == "model"]
    assert model_flags, "the model should raise a staff flag"
    assert model_flags[0].kind == "calling_for_someone_else"
    assert model_flags[0].evidence, "the flag should carry the caller's own words"
    assert model_flags[0].unverified == 0, "every quote should be the caller's"
    assert not any(r and "flag" in r.lower() for r in d.replies), "the caller is not told"
    # Only the patient can verify, so a caller for someone else goes to a person (2026-11-12).
    assert "identify" not in d.tools(), "the caller's own details were asked for"
    assert "hand_off" in d.tools() and d.call.ended
    assert "member of our staff now" in (d.replies[-1] or ""), "the hand-off's fixed line"


async def test_change_after_booking(ehr):
    """The owner's choice A, 2026-11-12, after a double booking: the time is read back before
    it is booked, and a change after booking goes to a person, who hears which visit stays
    booked. Only one visit is ever booked."""
    d = await draw(ehr, "change_after_booking.txt")
    assert any(r and "Shall I book it?" in r for r in d.replies), "the choice should be read back"
    assert d.booked() == 1, "exactly one booking"
    assert d.tools()[-1] == "change_visit", "the change goes to a person"
    last = d.replies[-1] or ""
    assert "is still booked" in last and "member of our staff now" in last
    assert "booking_change" in d.flags()
    assert d.call.ended


async def test_code_confusion(ehr):
    """The owner, 2026-11-12: a caller confused after a failed code goes to a person at once,
    not to more tries. Two near-identical records on one date mean no code went out (twins)."""
    d = await draw(ehr, "code_confusion.txt")
    assert d.ehr("CheckCode") == ["fail"], "one failed code, no more tries"
    assert d.replies[-1] == text("line.hand_off_code"), "the confused caller goes to a person"
    assert d.tools()[-1] == "hand_off" and "verification_failed" in d.flags()
    assert d.call.ended


async def test_no_code_after_a_resend(ehr):
    """The owner's test call, 2026-11-12: a twin gets no text and goes to a person with an
    apology (FUTURE-SETTINGS.md §7). No code was ever checked. "I didn't get a code, can you send
    it again?" goes to a person at once, with no new code (the owner's option A, 2026-11-14,
    FUTURE-SETTINGS.md §11): a resend cannot fix a mismatch or a missing mobile number."""
    d = await draw(ehr, "no_code_after_resend.txt")
    asked = next(i for i, line in enumerate(script("no_code_after_resend.txt")) if "send it again" in line)
    assert d.replies[asked] == text("line.hand_off_code"), "no text goes to a person at once"
    assert "resend_code" not in d.tools() and "check_code" not in d.tools(), "no new code, no code checked"
    assert d.tools()[-1] == "hand_off" and d.call.ended


async def test_booked_reason_asked(ehr):
    """The Priya Raman call (2026-11-13), as fixed on 2026-11-14: an odd reason is booked as said
    and flagged for clinical review, silent to the caller (FLAGS.md §3.1); asked what the visit
    is for and whether it is online, the agent answers from the booking with no hand-off and
    without reading the time again; no staff summary guesses a pronoun (FLAGS.md §3.2)."""
    d = await draw(ehr, "booked_reason_asked.txt")
    replies = d.replies
    assert d.booked() == 1, "the visit should be booked"
    asked = next((r for r in replies if r and "That's the visit we just booked." in r), None)
    assert asked is not None, "the visit just booked should not be read out again"
    # The caller's own words, as the model passed them on ("my strawberry water distillation" is
    # theirs too): the code keeps only words the caller said (flow.py `_reason_words`).
    assert "The reason on record is" in asked and "strawberry water distillation." in asked
    assert "It is listed as an office visit." in asked
    assert "offer_staff" not in d.tools() and "hand_off" not in d.tools(), "no offer or hand-off"
    assert "visit_reason_review" in d.flags(), "the odd reason should be flagged for review"
    reviews = [f for f in d.call.flow.flags if f.kind == "visit_reason_review"]
    assert reviews[0].source == "model"
    assert not any(r and "flag" in r.lower() for r in replies), "the caller is not told"
    assert not any(f.summary and guesses_a_pronoun(f.summary.reveal()) for f in d.call.flow.flags)
    assert d.tools()[-1] == "end_call" and d.call.ended
    assert (replies[-1] or "").endswith("Thank you for calling. Goodbye.")


async def test_visit_reason_then_an_offer(ehr):
    """Camila's call (2026-11-13): the visit's type and reason are said in the same turn as its
    time; a request the agent cannot do gets the offer of a person (option A), not a hand-off;
    a "no" keeps the call going to the caller's own goodbye."""
    d = await draw(ehr, "visit_reason_offer.txt")
    assert any(r and "The reason on record is Blood pressure follow-up." in r for r in d.replies)
    assert text("line.offer_staff") in d.replies and "offer_staff" in d.tools(), "the agent should offer a person"
    assert "hand_off" not in d.tools(), "no hand-off"
    assert d.tools().index("find_slots") > d.tools().index("offer_staff"), "after a no, the call goes on"
    last = d.replies[-1] or ""
    assert last.endswith("Thank you for calling. Goodbye.") and d.call.ended


async def test_no_text_goes_to_a_person_at_once(ehr):
    """FUTURE-SETTINGS.md §11, option A (the owner, 2026-11-13, after Warren's call): the code
    line is true for every caller and promises a person if no text comes; "no text" is a
    hand-off at once, with no new code. The words never say whether the details matched."""
    d = await draw(ehr, "no_text_to_person.txt")
    assert text("line.code_sent") in d.replies
    assert "resend_code" not in d.tools(), "no new code unless the caller asks"
    assert d.replies[-1] == text("line.hand_off_code")
    assert d.tools()[-1] == "hand_off" and d.call.ended


async def test_medical_question(ehr):
    """Option 1A (2026-11-11): a medical question gets the fixed line, and the booking goes on."""
    d = await draw(ehr, "medical_question.txt")
    assert any(r and "can't answer medical questions" in r for r in d.replies)
    assert "medical_question" in d.tools(), "the fixed line is the tool's"
    assert d.booked() == 1 and d.call.ended
    await assert_booked_9am(ehr)
