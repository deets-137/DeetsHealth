"""Whole typed calls through the real Flow, the real LLM service and the real ehr, replayed from
tests/replays/: no network to Anthropic, no tokens. A changed prompt or Flow misses the replay
and fails here; rerun the script with the harness in `record` mode to record it again.

The check is ehr's state, not the model's words: after the call, a fresh session for the same
fake patient must see the visit the call booked. Every call is also checked for the model's
words before a fixed-line tool and for values the model said that no one gave it.
"""

from pathlib import Path

from deets_agent.call import TypedCall
from deets_agent.flow import greeting, guesses_a_pronoun
from deets_agent.llm import Mode, build_llm
from deets_agent.prompts import text
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
    # hang_up() in `finally`: a reply that is not recorded fails the test at once, and the
    # pipeline is stopped, so the run goes on to the next test (it hung before, 2026-11-12).
    try:
        # The disclosures are a fixed line: every call starts with them, word for word.
        assert await call.start() == greeting()
        replies = [await call.say(line) for line in script(name)]
        assert_clean(call)
    finally:
        await call.hang_up()
    return call, replies


async def assert_booked_9am(ehr) -> None:
    """Ask ehr directly, as a new call by the same fake patient."""
    handle = await ehr.open_call()
    await ehr.identify(handle, Shielded("Jordan"), Shielded("Ellis"), Shielded("1984-03-12"))
    await ehr.check_code(handle, Shielded("246810"))
    visit = (await ehr.find_visit(handle)).visit
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


async def test_emergency_reminder(ehr):
    """The owner, 2026-11-12: a caller who said they are safe and mentions the same danger again
    hears the one-sentence reminder, not the full 911 line; and details the caller already gave
    are used, never asked for again."""
    call, replies = await run(ehr, "emergency_reminder.txt")
    full = [r for r in replies if r and "call 911 now" in r]
    assert len(full) == 1, f"the full 911 line should play once, not {len(full)} times"
    assert any(r and r.startswith(text("line.emergency_reminder")) for r in replies)
    assert not any(r and "last name" in r for r in replies), "the details were asked for again"
    assert call.ended
    await assert_booked_9am(ehr)


async def test_flag_for_someone_else(ehr):
    """Forks 1A and 2A, 2026-11-12: a caller for someone else gets a model flag whose evidence is
    the caller's own words; the caller never hears of it."""
    call, replies = await run(ehr, "flag_for_someone_else.txt")
    model_flags = [f for f in call.flow.flags if f.source == "model"]
    assert model_flags, "the model should raise a staff flag"
    assert model_flags[0].kind == "calling_for_someone_else"
    assert model_flags[0].evidence, "the flag should carry the caller's own words"
    assert model_flags[0].unverified == 0, "every quote should be the caller's"
    assert not any(r and "flag" in r.lower() for r in replies), "the caller is not told"
    # Only the patient can verify, so a caller for someone else goes to a person (2026-11-12).
    assert not any(r and "last name" in r for r in replies), "the caller's own details were asked for"
    assert call.ended and "member of our staff now" in (replies[-1] or "")


async def test_change_after_booking(ehr):
    """The owner's choice A, 2026-11-12, after a double booking: the time is read back before
    it is booked, and a change after booking goes to a person, who hears which visit stays
    booked. Only one visit is ever booked."""
    call, replies = await run(ehr, "change_after_booking.txt")
    assert any(r and "Shall I book it?" in r for r in replies), "the choice should be read back"
    assert sum(1 for r in replies if r and "You're booked for" in r) == 1, "exactly one booking"
    last = replies[-1] or ""
    assert "is still booked" in last and "member of our staff now" in last
    assert any(f.kind == "booking_change" for f in call.flow.flags)
    assert call.ended


async def test_code_confusion(ehr):
    """The owner, 2026-11-12: a caller confused after a failed code goes to a person at once,
    not to more tries. Two near-identical records on one date mean no code went out (twins)."""
    call, replies = await run(ehr, "code_confusion.txt")
    assert any(r and "didn't match" in r for r in replies), "the first code should fail"
    assert replies[-1] == text("line.hand_off_code"), "the confused caller goes to a person"
    assert call.ended


async def test_no_code_after_a_resend(ehr):
    """The owner's test call, 2026-11-12: a twin gets no text and goes to a person with an
    apology (FUTURE-SETTINGS.md §7). No code was ever checked. "I didn't get a code, can you send
    it again?" goes to a person at once, with no new code (the owner's option A, 2026-11-14,
    FUTURE-SETTINGS.md §11): a resend cannot fix a mismatch or a missing mobile number."""
    call, replies = await run(ehr, "no_code_after_resend.txt")
    asked = next(i for i, line in enumerate(script("no_code_after_resend.txt")) if "send it again" in line)
    assert replies[asked] == text("line.hand_off_code"), "no text goes to a person at once"
    assert text("line.code_resent") not in replies, "no new code when a person is coming"
    assert call.ended


async def test_booked_reason_asked(ehr):
    """The Priya Raman call (2026-11-13), as fixed on 2026-11-14: an odd reason is booked as said
    and flagged for clinical review, silent to the caller (FLAGS.md §3.1); asked what the visit
    is for and whether it is online, the agent answers from the booking with no hand-off and
    without reading the time again; no staff summary guesses a pronoun (FLAGS.md §3.2)."""
    call, replies = await run(ehr, "booked_reason_asked.txt")
    assert any(r and r.startswith("You're booked for") for r in replies), "the visit should be booked"
    asked = next((r for r in replies if r and "That's the visit we just booked." in r), None)
    assert asked is not None, "the visit just booked should not be read out again"
    # The caller's own words, as the model passed them on ("my strawberry water distillation" is
    # theirs too): the code keeps only words the caller said (flow.py `_reason_words`).
    assert "The reason on record is" in asked and "strawberry water distillation." in asked
    assert "It is listed as Office Visit." in asked
    # The greeting and the code line also name "a member of our staff": look for the offer and
    # the hand-off's own lines.
    assert text("line.offer_staff") not in replies and text("line.hand_off") not in replies, "no offer or hand-off"
    reviews = [f for f in call.flow.flags if f.kind == "visit_reason_review"]
    assert reviews and reviews[0].source == "model", "the odd reason should be flagged for review"
    assert not any(r and "flag" in r.lower() for r in replies), "the caller is not told"
    assert not any(f.summary and guesses_a_pronoun(f.summary.reveal()) for f in call.flow.flags)
    assert call.ended and (replies[-1] or "").endswith("Thank you for calling. Goodbye.")


async def test_visit_reason_then_an_offer(ehr):
    """Camila's call (2026-11-13): the visit's type and reason are said in the same turn as its
    time; a request the agent cannot do gets the offer of a person (option A), not a hand-off;
    a "no" keeps the call going to the caller's own goodbye."""
    call, replies = await run(ehr, "visit_reason_offer.txt")
    assert any(r and "The reason on record is Blood pressure follow-up." in r for r in replies)
    assert text("line.offer_staff") in replies, "the agent should offer a person, not hand off"
    assert not any(r and "member of our staff now" in r for r in replies), "no hand-off"
    assert any(r and "Here are the times I have" in r for r in replies), "after a no, the call goes on"
    last = replies[-1] or ""
    assert last.endswith("Thank you for calling. Goodbye.") and call.ended


async def test_no_text_goes_to_a_person_at_once(ehr):
    """FUTURE-SETTINGS.md §11, option A (the owner, 2026-11-13, after Warren's call): the code
    line is true for every caller and promises a person if no text comes; "no text" is a
    hand-off at once, with no new code. The words never say whether the details matched."""
    call, replies = await run(ehr, "no_text_to_person.txt")
    assert text("line.code_sent") in replies
    assert text("line.code_resent") not in replies, "no new code unless the caller asks"
    assert replies[-1] == text("line.hand_off_code")
    assert call.ended


async def test_medical_question(ehr):
    """Option 1A (2026-11-11): a medical question gets the fixed line, and the booking goes on."""
    call, replies = await run(ehr, "medical_question.txt")
    assert any(r and "can't answer medical questions" in r for r in replies)
    assert call.ended
    await assert_booked_9am(ehr)
