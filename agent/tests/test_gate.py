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
    assert node["name"] == "schedule" and result["options"] == 3
    assert "Option 1: Monday, November 2, at 9 AM" in result["spoken_to_caller"]

    result, node = await handler(node, "book")({"option": 1}, fm)
    assert node["name"] == "menu" and result["spoken_to_caller"].startswith("You're booked for Monday, November 2")

    result, _ = await handler(node, "next_visit")({}, fm)
    assert "Monday, November 2" in result["spoken_to_caller"]

    # Every line the model was told about is a line the caller heard, word for word.
    assert len(spoken(fm)) == 7  # the read-back, then six lines
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
    for slot in flow.offered:
        assert slot.token.reveal() not in str(result)
    await flow.close()
