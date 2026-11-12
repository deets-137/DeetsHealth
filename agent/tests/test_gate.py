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

CHART_TOOLS = {"find_slots", "book", "next_visit"}
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
    flow = CallFlow(ehr, today=FAKE_TODAY)
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
    assert {"find_slots", "next_visit"} <= tools(flow.menu())
    assert {"book", "find_slots"} <= tools(flow.schedule())


async def test_ehr_refuses_the_chart_even_if_the_flow_were_bypassed(ehr):
    flow = new_flow(ehr)
    await flow.start()
    # Call the menu's tools without verifying: ehr refuses, and the Flow hands off.
    _, next_node = await handler(flow.menu(), "next_visit")({}, manager())
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

    result, _ = await handler(node, "next_visit")({}, fm)
    assert "Tuesday, November 10, at 10 AM" in result["spoken_to_caller"]

    result, node = await handler(flow.menu(), "find_slots")({"reason": "a check-up"}, fm)
    assert node["name"] == "schedule" and result["options"] == [1, 2, 3]
    assert "Option 1: Monday, November 2, at 9 AM" in result["spoken_to_caller"]

    # Confirm before booking (the owner's choice A, 2026-11-12): the choice is read back first.
    result, nxt = await handler(node, "choose_option")({"option": 1}, fm)
    assert nxt is NO_RESPONSE and result["spoken_to_caller"].endswith("?")
    assert "Monday, November 2, at 9 AM" in result["spoken_to_caller"]
    result, node = await handler(node, "book")({}, fm)
    assert node["name"] == "menu" and result["spoken_to_caller"].startswith("You're booked for Monday, November 2")

    result, _ = await handler(node, "next_visit")({}, fm)
    assert "Monday, November 2" in result["spoken_to_caller"]

    # Every line the model was told about is a line the caller heard, word for word.
    assert len(spoken(fm)) == 8  # the read-back, then seven lines
    assert spoken(fm)[-1] == result["spoken_to_caller"]
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


async def test_book_needs_a_read_back_and_numbers_keep_their_time(ehr):
    """2026-11-12, after a double booking: `book` books nothing until a choice was read back,
    and an option number means one time for the whole call; a later search goes on from the
    last number."""
    flow = new_flow(ehr)
    await flow.start()
    _, node = await identify(flow, JORDAN)
    _, node = await handler(node, "check_code")(CODE, manager())
    first, node = await handler(node, "find_slots")({"reason": "a check-up"}, manager())
    assert first["options"] == [1, 2, 3]
    _, nxt = await handler(node, "book")({}, manager())
    assert nxt is NO_RESPONSE and flow.last_booked is None, "no booking without a read-back"

    later, _ = await handler(node, "find_slots")({"reason": "a check-up", "earliest_day": "2026-11-03"}, manager())
    # Tuesday at 10 AM was option 3 and stays option 3; the times not heard before go on from 4.
    assert later["options"][0] == 3 and all(n >= 4 for n in later["options"][1:])
    assert "Option 3: Tuesday, November 3, at 10 AM" in later["spoken_to_caller"]
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
    from deets_agent.flow import EMERGENCY_LINE
    from deets_agent.prompts import text

    flow = new_flow(ehr)
    remind = flow.safety_reminder_fn().handler
    fm = manager()
    await remind({}, fm)
    await remind({}, fm)
    assert spoken(fm) == [EMERGENCY_LINE, text("line.emergency_reminder")]
    # The full line raised one code flag; the reminder raised none.
    assert [(f.kind, f.source) for f in flow.flags] == [("possible_emergency", "code")]


async def test_a_flag_keeps_only_what_the_caller_said(ehr):
    """Fork 1A, 2026-11-12: the model's evidence is checked by code. A quote the caller said
    (punctuation and case aside) is kept; one they did not say is counted and dropped. Nothing is
    spoken (fork 2A)."""
    flow = CallFlow(ehr, today=FAKE_TODAY)
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
    flow = CallFlow(ehr, today=FAKE_TODAY)
    assert flow.raise_flag("made_up_kind", "model").kind == "other"
    assert flow.raise_flag("verification_failed", "code") is not None
    assert flow.raise_flag("verification_failed", "code") is None
