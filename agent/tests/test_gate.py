"""The verify gate with no model (BUILD-ORDER.md Phase 1): no tokens, no network beyond ehr.

It checks both locks of SECURITY.md §4.9: which tools each node offers, and that ehr refuses
every chart and booking call until both factors pass. Then it walks a whole call through the
tool handlers, as the model would call them, and checks that each fixed line goes to the caller
word for word (the owner, 2026-11-11, fork 1A).
"""

from types import SimpleNamespace

from pipecat.flows import NO_RESPONSE
from pipecat.frames.frames import TTSSpeakFrame

from deets_agent.flow import FAKE_TODAY, CallFlow
from deets_agent.prompts import text

CHART_TOOLS = {"find_slots", "book", "find_visit"}
JORDAN = {"given_name": "Jordan", "family_name": "Ellis", "birth_date": "1984-03-12"}
CODE = {"code": "246810"}


class FakeWorker:
    """Stands in for the pipeline: keeps each frame a handler queues."""

    def __init__(self) -> None:
        self.frames = []

    async def queue_frame(self, frame) -> None:
        self.frames.append(frame)


def manager():
    return SimpleNamespace(worker=FakeWorker())


def spoken(fm) -> list[str]:
    return [f.text for f in fm.worker.frames if isinstance(f, TTSSpeakFrame)]


def tools(node) -> set[str]:
    return {f.name for f in node["functions"]}


def handler(node, name):
    return next(f for f in node["functions"] if f.name == name).handler


def new_flow(ehr) -> CallFlow:
    flow = CallFlow(ehr, today=FAKE_TODAY, preference_asked=True)
    # The caller's lines, as the screen feeds them, so the name guard lets these names through.
    flow.heard("Jordan Ellis, March 12, 1984")
    flow.heard("Nobody Here")
    return flow


async def identify(flow: CallFlow, args: dict, fm=None):
    """`identify` reads the details back; the caller says yes; `confirm_details` looks them up."""
    fm = fm or manager()
    _, nxt = await handler(flow.greet(), "identify")(args, fm)
    assert nxt is NO_RESPONSE, "the read-back waits for the caller"
    return await handler(flow.greet(), "confirm_details")({}, fm)


async def test_identify_reads_back_and_looks_up_nothing_until_yes(ehr):
    flow = new_flow(ehr)
    await flow.start()
    fm = manager()
    result, nxt = await handler(flow.greet(), "identify")(dict(JORDAN, birth_date="1984-03-21"), fm)
    assert nxt is NO_RESPONSE
    assert result["spoken_to_caller"] == "I have Jordan Ellis, born March 21, 1984. Is that right?"
    # The caller corrects the date: identify again, a new read-back, then yes.
    _, node = await identify(flow, JORDAN, fm)
    assert node["name"] == "verify"
    # Only one try went to ehr, and it matched: the code verifies.
    result, _ = await handler(node, "check_code")(CODE, manager())
    assert "verified" in result["spoken_to_caller"]
    await flow.close()


async def test_confirm_with_nothing_read_back_asks_for_details(ehr):
    flow = new_flow(ehr)
    await flow.start()
    result, nxt = await handler(flow.greet(), "confirm_details")({}, manager())
    assert nxt is NO_RESPONSE and "first name, last name and date of birth" in result["spoken_to_caller"]
    await flow.close()


async def test_the_name_guard_asks_again_for_a_name_never_said(ehr):
    flow = new_flow(ehr)
    await flow.start()
    fm = manager()
    result, nxt = await handler(flow.greet(), "identify")(dict(JORDAN, family_name="Smith"), fm)
    assert nxt is NO_RESPONSE and "full name" in result["spoken_to_caller"]
    # No try was used: the real name still verifies after it.
    _, node = await identify(flow, JORDAN)
    result, _ = await handler(node, "check_code")(CODE, manager())
    assert "verified" in result["spoken_to_caller"]
    await flow.close()


async def test_no_chart_tool_exists_before_verification(ehr):
    flow = new_flow(ehr)
    assert not tools(flow.greet()) & CHART_TOOLS
    assert not tools(flow.verify()) & CHART_TOOLS
    assert {"find_slots", "find_visit"} <= tools(flow.menu())
    assert {"book", "find_slots"} <= tools(flow.schedule())


async def test_ehr_refuses_the_chart_even_if_the_flow_were_bypassed(ehr):
    flow = new_flow(ehr)
    await flow.start()
    # Call the menu's tools without verifying: ehr refuses, and the Flow hands off.
    _, next_node = await handler(flow.menu(), "find_visit")({}, manager())
    assert next_node["name"] == "hand_off"
    _, next_node = await handler(flow.menu(), "find_slots")({"reason": "a check-up"}, manager())
    assert next_node["name"] == "hand_off"
    await flow.close()


async def test_a_whole_call_through_the_handlers(ehr):
    flow = new_flow(ehr)
    fm = manager()
    await flow.start()
    result, node = await identify(flow, JORDAN, fm)
    assert node["name"] == "verify" and node["respond_immediately"] is False
    result, node = await handler(node, "check_code")(CODE, fm)
    assert node["name"] == "menu"

    result, nxt = await handler(node, "find_visit")({}, fm)
    assert "Tuesday, November 10, at 10 AM" in result["spoken_to_caller"]
    # What the visit is, from the record (FUTURE-SETTINGS.md §9), how many more are coming, and
    # the offer of its instructions (the owner's 2A, 2026-11-14): the line waits for a yes.
    # The hospital's spoken name for the record's "Office Visit" (Config › Hospital's Default).
    assert "It is listed as an office visit." in result["spoken_to_caller"]
    assert "The reason on record is Blood pressure follow-up." in result["spoken_to_caller"]
    assert "You have 2 more visits coming up after that." in result["spoken_to_caller"]
    assert result["spoken_to_caller"].endswith("Would you like to hear them?") and nxt is NO_RESPONSE
    assert (result["reason_for_visit"], result["instructions"], result["visit_number"]) == ("said", "offered", 1)
    assert "csn" not in result and "90000101" not in str(result), "the model never sees a CSN"
    result, _ = await handler(node, "read_instructions")({}, fm)
    assert result["spoken_to_caller"] == "The instructions for this visit say: Bring a list of the medicines you take."

    flow.heard("I'd like to book a check-up, please.")
    result, node = await handler(flow.menu(), "find_slots")({"reason": "a check-up"}, fm)
    assert node["name"] == "schedule" and result["options"] == [1, 2, 3]
    assert "Option 1: Monday, November 2, at 9 AM" in result["spoken_to_caller"]

    # A question with the choice (the owner's 1A, 2026-11-16): no read-back yet; the model is
    # told to answer, then to choose again. The second choose_option reads back.
    flow.heard("Option 1. Is that with Dr. Lin?")
    result, nxt = await handler(node, "choose_option")({"option": 1}, fm)
    assert nxt is None and "spoken_to_caller" not in result
    assert result["next_step"] == text("note.question_with_choice", number="1")
    # Confirm before booking (the owner's choice A, 2026-11-12): the choice is read back first.
    result, nxt = await handler(node, "choose_option")({"option": 1}, fm)
    assert nxt is NO_RESPONSE and result["spoken_to_caller"].endswith("?")
    assert "Monday, November 2, at 9 AM" in result["spoken_to_caller"]
    result, node = await handler(node, "book")({}, fm)
    assert node["name"] == "menu" and result["spoken_to_caller"].startswith("You're booked for Monday, November 2")

    # The model learns how the visit takes place and that its reason went with it (2026-11-14).
    assert (result["visit_is"], result["reason_for_visit"]) == ("in person", "said")

    # Asked again (the Priya Raman call, 2026-11-13): the time is not read out a second time; the
    # type and the caller's own reason are, from the booking.
    result, _ = await handler(node, "find_visit")({}, fm)
    assert result["spoken_to_caller"].startswith("That's the visit we just booked.")
    assert "Monday, November 2" not in result["spoken_to_caller"]
    assert "It is listed as an office visit." in result["spoken_to_caller"]
    assert "The reason on record is a check-up." in result["spoken_to_caller"]
    assert result["reason_for_visit"] == "said"

    # Every line the model was told about is a line the caller heard, word for word.
    assert len(spoken(fm)) == 9  # the read-back, then eight lines
    assert spoken(fm)[-1] == result["spoken_to_caller"]
    await flow.close()


async def test_a_caller_who_said_what_they_want_is_not_asked_again(ehr):
    """The Camila call of 2026-11-15: the caller said at the start that they wanted to check a
    visit and book one, and after the code heard the menu question again. Now identify keeps
    what they said, and the code's pass goes straight on, with no question."""
    flow = new_flow(ehr)
    fm = manager()
    await flow.start()
    _, node = await identify(flow, {**JORDAN, "caller_wants": "both"}, fm)
    result, nxt = await handler(node, "check_code")(CODE, fm)
    assert result["spoken_to_caller"] == text("line.verified_go")
    assert result["caller_wants"] == "both" and nxt["name"] == "menu" and nxt["respond_immediately"] is True
    await flow.close()
    # Nothing said, or a value off the list: the question, as before.
    flow = new_flow(ehr)
    await flow.start()
    _, node = await identify(flow, {**JORDAN, "caller_wants": "chat"}, fm)
    result, _ = await handler(node, "check_code")(CODE, fm)
    assert result["spoken_to_caller"] == text("line.verified")
    await flow.close()


async def test_more_times_go_on_numbering_and_only_listed_wishes_are_sent(ehr):
    """EPIC.md §7.10, build 2: the next times of the same search keep counting from the last
    option number; a time of day or a day the hospital's lists do not hold is dropped, never
    sent; an online time says the hospital's zone."""
    flow = new_flow(ehr)
    fm = manager()
    await flow.start()
    _, node = await identify(flow, JORDAN, fm)
    _, node = await handler(node, "check_code")(CODE, fm)
    flow.heard("I'd like a check-up, any time.")
    result, node = await handler(node, "find_slots")({"reason": "a check-up"}, fm)
    assert result["options"] == [1, 2, 3] and result["more_waiting"] is True
    result, _ = await handler(node, "find_slots")({"reason": "a check-up", "more": True}, fm)
    assert result["options"] == [4, 5, 6] and result["more_waiting"] is False
    assert "8:30 AM CST" in result["spoken_to_caller"], "an online time names the zone"
    result, _ = await handler(node, "find_slots")({"reason": "a check-up", "more": True}, fm)
    assert result["spoken_to_caller"] == text("line.no_more_slots")
    # "brunch" is not on the hospital's list: dropped, so the search runs without it.
    result, _ = await handler(node, "find_slots")(
        {"reason": "a check-up", "time_of_day": "brunch", "days": ["someday"]}, fm
    )
    assert result["options"] == [1, 2, 3]
    # A new search: a new list from Option 1 (the owner's N2, 2026-11-24).
    result, _ = await handler(node, "find_slots")({"reason": "a check-up", "time_of_day": "afternoon"}, fm)
    assert result["options"] == [1, 2]
    await flow.close()


async def test_a_wrong_identity_never_verifies_and_three_wrong_codes_hand_off(ehr):
    flow = new_flow(ehr)
    await flow.start()
    _, node = await identify(flow, {"given_name": "Nobody", "family_name": "Here", "birth_date": "2000-01-01"})
    # The same words as a match (SECURITY.md §4.4).
    assert node["name"] == "verify"
    check = handler(node, "check_code")
    for _ in range(2):
        result, nxt = await check(CODE, manager())
        assert nxt is NO_RESPONSE and "didn't match" in result["spoken_to_caller"]
    result, nxt = await check(CODE, manager())
    assert nxt["name"] == "hand_off" and nxt["pre_actions"] == [{"type": "end_conversation"}]
    await flow.close()


async def test_offer_staff_asks_and_the_call_goes_on(ehr):
    """The owner's option A (2026-11-13): a fixed offer that waits for the caller; the node does
    not change, so a "no" goes on with the call. Private has its own words."""
    flow = new_flow(ehr)
    fm = manager()
    menu = flow.menu()
    offer = next(f for f in flow_globals(flow) if f.name == "offer_staff").handler
    result, nxt = await offer({"why": "cannot_help"}, fm)
    assert nxt is NO_RESPONSE and result["spoken_to_caller"].endswith("?")
    assert "member of our staff" in result["spoken_to_caller"]
    private, _ = await offer({"why": "private"}, fm)
    assert private["spoken_to_caller"] != result["spoken_to_caller"]
    assert menu["name"] == "menu"


async def test_a_hand_off_on_the_reply_to_an_offer_says_a_person_will_answer(ehr):
    """The owner's 2C (2026-11-16): the caller asks again what the agent cannot answer; the
    hand-off goes on, and the caller hears that a person will answer it. A later hand-off, after
    more of the call, has the plain line."""
    flow = new_flow(ehr)
    fm = manager()
    offer, hand_off = (next(f for f in flow_globals(flow) if f.name == n).handler for n in ("offer_staff", "hand_off"))
    flow.heard("Is this with my primary care doctor?")
    await offer({"why": "cannot_help"}, fm)
    flow.heard("Is this with my primary care doctor?")
    result, _ = await hand_off({"kind": "other", "summary": "The caller asked about their doctor."}, fm)
    assert result["spoken_to_caller"] == text("line.hand_off_answer")

    flow = new_flow(ehr)
    offer, hand_off = (next(f for f in flow_globals(flow) if f.name == n).handler for n in ("offer_staff", "hand_off"))
    await offer({"why": "cannot_help"}, fm)
    for line in ("No thanks.", "Can I book a visit?", "Actually, get me a person."):
        flow.heard(line)
    result, _ = await hand_off({"kind": "other", "summary": "The caller asked for a person."}, fm)
    assert result["spoken_to_caller"] == text("line.hand_off")


def test_a_question_in_the_caller_s_line_is_found():
    from deets_agent.flow import _asks_question

    for asked in [
        "I'll take option 2. Do you need a reason for the appointment? Is this with my primary care doctor?",
        "Option 2, is that with Dr. Lin?",
        "Two. what time do I arrive",
    ]:
        assert _asks_question(asked), asked
    # Known limit: with no mark and no sentence break, a question inside the line is not found
    # ("option 2 is this with doctor lin"); the task's words still ask the model to answer it.
    assert not _asks_question("option 2 is this with doctor lin")
    for plain in ["I'll take option 2.", "Option three please", "The second one, thanks!"]:
        assert not _asks_question(plain), plain


def flow_globals(flow: CallFlow):
    return [flow.hand_off_fn(), flow.offer_staff_fn(), flow.end_call_fn()]


async def test_an_impossible_birth_date_costs_no_try(ehr):
    # The 2026-11-11 desk test: "2056" sent to identify used one of the three tries.
    flow = new_flow(ehr)
    await flow.start()
    read_back = handler(flow.greet(), "identify")
    for _ in range(4):
        result, nxt = await read_back(dict(JORDAN, birth_date="2056-02-27"), manager())
        assert nxt is NO_RESPONSE and "doesn't sound right" in result["spoken_to_caller"]
    # All three tries are still there: the real date still verifies.
    _, node = await identify(flow, JORDAN)
    _, node = await handler(node, "check_code")(CODE, manager())
    assert node["name"] == "menu"
    await flow.close()


async def test_the_model_never_sees_a_slot_token(ehr):
    flow = new_flow(ehr)
    await flow.start()
    _, node = await identify(flow, JORDAN)
    _, node = await handler(node, "check_code")(CODE, manager())
    result, _ = await handler(node, "find_slots")({"reason": "a check-up"}, manager())
    for slot in flow.offered.values():
        assert slot.token.reveal() not in str(result)
    await flow.close()


async def test_book_needs_a_read_back_and_each_search_starts_at_option_1(ehr):
    """2026-11-12, after a double booking: `book` books nothing until a choice was read back.
    The owner's N2 (2026-11-24): each new search starts again at Option 1 and the earlier times
    are no longer offered; the booking key follows the time, not its number."""
    flow = new_flow(ehr)
    await flow.start()
    _, node = await identify(flow, JORDAN)
    _, node = await handler(node, "check_code")(CODE, manager())
    first, node = await handler(node, "find_slots")({"reason": "a check-up"}, manager())
    assert first["options"] == [1, 2, 3]
    _, nxt = await handler(node, "book")({}, manager())
    assert nxt is NO_RESPONSE and flow.last_booked is None, "no booking without a read-back"
    tuesday = flow._slot_id(flow.offered[3])

    later, _ = await handler(node, "find_slots")({"reason": "a check-up", "earliest_day": "2026-11-03"}, manager())
    # Tuesday at 10 AM was option 3 and is now option 1 of the new list.
    assert later["options"] == list(range(1, len(later["options"]) + 1))
    assert "Option 1: Tuesday, November 3, at 10 AM" in later["spoken_to_caller"]
    assert set(flow.offered) == set(later["options"]), "the first list's times are no longer offered"
    await handler(node, "choose_option")({"option": 1}, manager())
    assert flow._slot_id(flow.offered[1]) == tuesday
    await flow.close()


async def test_a_change_after_booking_names_the_visit_flags_and_hands_off(ehr):
    flow = new_flow(ehr)
    await flow.start()
    _, node = await identify(flow, JORDAN)
    _, node = await handler(node, "check_code")(CODE, manager())
    _, node = await handler(node, "find_slots")({"reason": "a check-up"}, manager())
    await handler(node, "choose_option")({"option": 2}, manager())
    _, node = await handler(node, "book")({}, manager())
    flow.heard("Actually, could I take option 3 instead?")
    result, nxt = await handler(node, "change_visit")({}, manager())
    assert nxt["name"] == "hand_off"
    assert "Monday, November 2, at 2:30 PM" in result["spoken_to_caller"], "the caller hears what stays booked"
    flag = flow.flags[-1]
    assert flag.kind == "booking_change" and flag.evidence[0].reveal().startswith("Actually")
    await flow.close()


async def test_safety_reminder_holds_the_safe_side(ehr):
    """The owner, 2026-11-12: the reminder only after the caller heard the full 911 line. Before
    that, the code speaks the full line, whatever the model chose."""
    from deets_agent.flow import emergency_line
    from deets_agent.prompts import text

    flow = new_flow(ehr)
    remind = flow.safety_reminder_fn().handler
    fm = manager()
    await remind({}, fm)
    await remind({}, fm)
    assert spoken(fm) == [emergency_line(), text("line.emergency_reminder")]
    # The full line raised one code flag; the reminder raised none.
    assert [(f.kind, f.source) for f in flow.flags] == [("possible_emergency", "code")]


async def test_a_fixed_line_leaves_from_the_llm_before_the_tool_result(ehr):
    """2026-11-13: a fixed line queued at the pipeline's top raced the model's next reply and once
    came after it. With the call's LLM set, `_say` pushes the line out of the LLM itself, before
    the handler returns its result, and nothing goes to the pipeline's top."""
    from deets_agent.prompts import text

    pushed = []

    class FakeLlm:
        async def push_frame(self, frame) -> None:
            pushed.append(frame)

    flow = new_flow(ehr)
    flow.llm = FakeLlm()
    fm = manager()
    await flow.end_call_fn().handler({}, fm)
    assert [f.text for f in pushed] == [text("line.goodbye")]
    assert fm.worker.frames == [], "nothing queued at the top"


async def test_a_flag_keeps_only_what_the_caller_said(ehr):
    """Fork 1A, 2026-11-12: the model's evidence is checked by code. A quote the caller said
    (punctuation and case aside) is kept; one they did not say is counted and dropped. Nothing is
    spoken (fork 2A)."""
    flow = CallFlow(ehr, today=FAKE_TODAY, preference_asked=True)
    flow.heard("I'm calling for my mom. She's been really confused lately, I'm worried.")
    seen = []
    flow.on_flag = seen.append
    fm = manager()
    result, nxt = await flow.flag_for_staff_fn().handler(
        {
            "kind": "calling_for_someone_else",
            "summary": "The caller is calling for their mother, who has been confused.",
            "evidence": ["calling for my mom", "she's been REALLY confused lately", "she fell yesterday"],
        },
        fm,
    )
    assert spoken(fm) == [], "the caller hears nothing"
    assert nxt is None, "the model goes on"
    flag = seen[0]
    assert flag.kind == "calling_for_someone_else" and flag.source == "model"
    assert [q.reveal() for q in flag.evidence] == ["calling for my mom", "she's been REALLY confused lately"]
    assert flag.unverified == 1
    assert result["evidence_kept"] == 2


async def test_an_unknown_flag_kind_becomes_other_and_code_flags_raise_once(ehr):
    flow = CallFlow(ehr, today=FAKE_TODAY, preference_asked=True)
    assert flow.raise_flag("made_up_kind", "model").kind == "other"
    assert flow.raise_flag("verification_failed", "code") is not None
    assert flow.raise_flag("verification_failed", "code") is None


async def test_a_summary_that_guesses_a_pronoun_goes_back_once(ehr):
    """The owner's option A (2026-11-14, FLAGS.md §3.2), after the Priya Raman call's "She
    asks…": the first such summary goes back to the model with nothing spoken and no flag; a
    second one is kept as it is. Evidence (the caller's own words) is never checked for it."""
    from deets_agent.prompts import text

    flow = CallFlow(ehr, today=FAKE_TODAY, preference_asked=True)
    flow.heard("What was the reason on the visit again? Is it online?")
    seen = []
    flow.on_flag = seen.append
    fm = manager()
    args = {"kind": "other", "summary": "She asks what the visit is for.", "evidence": ["Is it online?"]}
    result, nxt = await flow.hand_off_fn().handler(args, fm)
    assert result == {"summary_refused": text("note.summary_pronoun")} and nxt is None
    assert spoken(fm) == [] and seen == [], "the caller hears nothing; no flag yet"
    named = {**args, "summary": "Priya Raman asks what the visit is for."}
    result, node = await flow.hand_off_fn().handler(named, fm)
    assert node["name"] == "hand_off" and seen[0].summary.reveal() == named["summary"]
    # flag_for_staff gets its own one chance; after it, a pronoun is kept.
    result, _ = await flow.flag_for_staff_fn().handler(args, manager())
    assert "summary_refused" in result
    result, _ = await flow.flag_for_staff_fn().handler(args, manager())
    assert result["flag_raised"] == "other" and seen[-1].summary.reveal() == args["summary"]


def test_the_reason_sent_is_the_callers_own_words(ehr):
    """The visit's note is the caller's words, never the model's (2026-11-14): the model's reason
    is kept only if the caller said it, as for a flag's evidence; else the caller's latest line."""
    flow = CallFlow(ehr, today=FAKE_TODAY, preference_asked=True)
    flow.heard("Can I book a visit for my strawberry water distillation?")
    assert flow._reason_words("strawberry water distillation") == "strawberry water distillation"
    assert flow._reason_words("Strawberry-water distillation.") == "Strawberry-water distillation."
    line = "Can I book a visit for my strawberry water distillation?"
    assert flow._reason_words("distilling water at home") == line
    assert flow._reason_words("  ") == "a visit"


async def test_an_online_wish_offers_online_times_and_says_so(ehr):
    flow = new_flow(ehr)
    fm = manager()
    await flow.start()
    await identify(flow, JORDAN, fm)
    await handler(flow.verify(), "check_code")(CODE, fm)
    flow.heard("Could I do a video visit for my follow-up?")
    result, node = await handler(flow.menu(), "find_slots")(
        {"reason": "follow-up", "visit_mode": "online"}, fm
    )
    assert node["name"] == "schedule" and len(result["options"]) == 2
    assert result["spoken_to_caller"].count(", online, for") == 2
    flow.heard("The first one, please.")
    await handler(node, "choose_option")({"option": result["options"][0]}, fm)
    result, _ = await handler(node, "book")({}, fm)
    assert ", online, for" in result["spoken_to_caller"] and result["visit_is"] == "online"
    await flow.close()


async def test_the_reason_is_asked_once_before_the_preference(ehr):
    """The owner, 2026-11-16: ask for a reason before presenting options. A reason made only of
    words that ask for a visit ("book an appointment", the call of 2026-11-15, 22:36) is none.
    Asked once; then the preference question; then the search."""
    flow = CallFlow(ehr, today=FAKE_TODAY)
    flow.heard("Jordan Ellis, March 12, 1984")
    fm = manager()
    await flow.start()
    await identify(flow, JORDAN, fm)
    await handler(flow.verify(), "check_code")(CODE, fm)
    flow.heard("I'd like to book an appointment")
    result, node = await handler(flow.menu(), "find_slots")({"reason": "book an appointment"}, fm)
    assert result["spoken_to_caller"] == text("line.ask_reason")
    assert result["next_step"] == text("note.after_ask_reason") and "options" not in result
    flow.heard("My knee hurts when I run")
    result, _ = await handler(flow.menu(), "find_slots")({"reason": "my knee hurts when I run"}, fm)
    assert result["spoken_to_caller"] == text("line.ask_preference_both")
    # Asked once: an empty reason after that searches, with "a visit" as before.
    result, node = await handler(flow.menu(), "find_slots")({"reason": "", "any_time": True, "visit_mode": "either"}, fm)
    assert node["name"] == "schedule" and result["options"]
    await flow.close()


def test_a_reason_of_booking_words_only_is_no_reason():
    from deets_agent.flow import _no_reason

    for none in ["", "  ", "book an appointment", "I'd like to see the doctor", "schedule a visit please"]:
        assert _no_reason(none), none
    for some in ["a check-up", "my knee", "strawberry water distillation", "follow-up visit"]:
        assert not _no_reason(some), some


async def test_the_preference_is_asked_once_before_the_first_search(ehr):
    """The owner's 1A (2026-11-15): before the first search the caller is asked what they have
    not said, in one fixed line, and no search runs; the next find_slots searches whatever the
    answer. A caller who said both gets no question."""
    flow = CallFlow(ehr, today=FAKE_TODAY)
    flow.heard("Jordan Ellis, March 12, 1984")
    fm = manager()
    await flow.start()
    assert flow.books_both_modes, "the local fake books in person and online"
    await identify(flow, JORDAN, fm)
    await handler(flow.verify(), "check_code")(CODE, fm)
    flow.heard("I'd like a check-up")
    result, node = await handler(flow.menu(), "find_slots")({"reason": "a check-up"}, fm)
    assert result["spoken_to_caller"] == text("line.ask_preference_both")
    assert result["next_step"] == text("note.after_ask_preference") and "options" not in result
    # The answer gives only "online": no second question, the search runs.
    result, node = await handler(flow.menu(), "find_slots")({"reason": "a check-up", "visit_mode": "online"}, fm)
    assert node["name"] == "schedule" and result["options"]
    await flow.close()

    # Said when but not how: only "in person or online?".
    flow = CallFlow(ehr, today=FAKE_TODAY)
    flow.heard("Jordan Ellis, March 12, 1984")
    await flow.start()
    await identify(flow, JORDAN, fm)
    await handler(flow.verify(), "check_code")(CODE, fm)
    result, _ = await handler(flow.menu(), "find_slots")({"reason": "a check-up", "any_time": True}, fm)
    assert result["spoken_to_caller"] == text("line.ask_preference_mode")
    await flow.close()

    # Said both: no question. A hospital that books one kind never asks how.
    flow = CallFlow(ehr, today=FAKE_TODAY, books_both_modes=False)
    flow.heard("Jordan Ellis, March 12, 1984")
    await flow.start()
    flow.books_both_modes = False
    await identify(flow, JORDAN, fm)
    await handler(flow.verify(), "check_code")(CODE, fm)
    result, node = await handler(flow.menu(), "find_slots")(
        {"reason": "a check-up", "time_of_day": "afternoon"}, fm
    )
    assert node["name"] == "schedule" and result["options"]
    await flow.close()


async def test_with_no_usual_place_the_preference_question_also_asks_where(ehr):
    """BOOKING-RULES.md §3.2, 1B: Priya Raman has no usual place, so the preference question also
    names the two primary care places; her answer searches there, and a later search that names
    no place is never asked again. Words that name no place are said so."""
    flow = CallFlow(ehr, today=FAKE_TODAY)
    flow.heard("Priya Raman, November 30, 1971")
    fm = manager()
    await flow.start()
    priya = {"given_name": "Priya", "family_name": "Raman", "birth_date": "1971-11-30"}
    await identify(flow, priya, fm)
    await handler(flow.verify(), "check_code")(CODE, fm)
    flow.heard("I'd like a check-up")
    result, _ = await handler(flow.menu(), "find_slots")({"reason": "a check-up"}, fm)
    where = text("line.which_place", places=f"Main Street Clinic {text('line.choice_or')} the family medicine clinic")
    assert result["spoken_to_caller"] == f"{text('line.ask_preference_both')} {where}"
    assert text("note.after_ask_place") in result["next_step"]
    result, node = await handler(flow.menu(), "find_slots")(
        {"reason": "a check-up", "visit_mode": "in_person", "any_time": True, "place": "the family medicine clinic"}, fm
    )
    assert node["name"] == "schedule" and "Moreno" in result["spoken_to_caller"]
    result, _ = await handler(flow.menu(), "find_slots")({"reason": "a check-up", "place": "Chicago"}, fm)
    assert result["spoken_to_caller"] == text("line.unknown_place")
    await flow.close()


async def test_where_to_go_is_open_before_verification_and_reads_the_lists(ehr):
    """BOOKING-RULES.md §3.7: place_info is in every step and needs no verified caller. Its own
    hours are said with days in a row together; with no place and no visit heard, it asks which."""
    flow = CallFlow(ehr, today=FAKE_TODAY)
    fm = manager()
    await flow.start()
    place_info = next(fn for fn in flow.global_functions() if fn.name == "place_info").handler
    result, _ = await place_info({"place": "the x-ray place"}, fm)
    assert result["spoken_to_caller"] == " ".join(
        [
            text("line.place_address", place="EMH X-Ray Imaging", address="300 Lake Road, Fitchburg, WI 53711"),
            text("line.place_phone", phone="608-555-0140"),
            text("line.place_hours", hours="Monday to Friday, 7 AM to 7 PM, and Saturday, 8 AM to 12 PM"),
        ]
    )
    assert result["closed_days"][0]["name"] == "Thanksgiving" or result["closed_days"] == []
    # Only what was asked (a desk test, 2026-11-22): the hours alone, or the phone and the address.
    result, _ = await place_info({"place": "the x-ray place", "asks": ["hours"]}, fm)
    assert result["spoken_to_caller"] == text(
        "line.place_hours", hours="Monday to Friday, 7 AM to 7 PM, and Saturday, 8 AM to 12 PM"
    )
    result, _ = await place_info({"place": "the x-ray place", "asks": ["phone", "where"]}, fm)
    assert result["spoken_to_caller"] == " ".join(
        [
            text("line.place_address", place="EMH X-Ray Imaging", address="300 Lake Road, Fitchburg, WI 53711"),
            text("line.place_phone", phone="608-555-0140"),
        ]
    )
    result, _ = await place_info({}, fm)
    assert result["spoken_to_caller"] == text("line.place_info_which")
    await flow.close()
