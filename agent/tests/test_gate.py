"""The verify gate with no model (BUILD-ORDER.md Phase 1): no tokens, no network beyond ehr.

It checks both locks of SECURITY.md §4.9: which tools each node offers, and that ehr refuses
every chart and booking call until both factors pass. Then it walks a whole call through the
tool handlers, as the model would call them.
"""

from deets_agent.flow import CallFlow

CHART_TOOLS = {"find_slots", "book", "next_visit"}
JORDAN = {"given_name": "Jordan", "family_name": "Ellis", "birth_date": "1984-03-12"}
CODE = {"code": "246810"}


def tools(node) -> set[str]:
    return {f.name for f in node["functions"]}


def handler(node, name):
    return next(f for f in node["functions"] if f.name == name).handler


async def test_no_chart_tool_exists_before_verification(ehr):
    flow = CallFlow(ehr)
    assert not tools(flow.greet()) & CHART_TOOLS
    assert not tools(flow.verify()) & CHART_TOOLS
    assert {"find_slots", "next_visit"} <= tools(flow.menu())
    assert {"book", "find_slots"} <= tools(flow.schedule())


async def test_ehr_refuses_the_chart_even_if_the_flow_were_bypassed(ehr):
    flow = CallFlow(ehr)
    await flow.start()
    # Call the menu's tools without verifying: ehr refuses, and the Flow hands off.
    _, next_node = await handler(flow.menu(), "next_visit")({}, None)
    assert next_node["name"] == "hand_off"
    _, next_node = await handler(flow.menu(), "find_slots")({"reason": "a check-up"}, None)
    assert next_node["name"] == "hand_off"
    await flow.close()


async def test_a_whole_call_through_the_handlers(ehr):
    flow = CallFlow(ehr)
    await flow.start()
    result, node = await handler(flow.greet(), "identify")(JORDAN, None)
    assert node["name"] == "verify"
    result, node = await handler(node, "check_code")(CODE, None)
    assert node["name"] == "menu"

    result, _ = await handler(node, "next_visit")({}, None)
    assert "Tuesday, November 10, at 10 AM" in result["say"]

    result, node = await handler(flow.menu(), "find_slots")({"reason": "a check-up"}, None)
    assert node["name"] == "schedule" and result["options"] == 3
    assert "Option 1: Monday, November 2, at 9 AM" in result["say"]

    result, node = await handler(node, "book")({"option": 1}, None)
    assert node["name"] == "menu" and result["say"].startswith("You're booked for Monday, November 2")

    result, _ = await handler(node, "next_visit")({}, None)
    assert "Monday, November 2" in result["say"]
    await flow.close()


async def test_a_wrong_identity_never_verifies_and_three_wrong_codes_hand_off(ehr):
    flow = CallFlow(ehr)
    await flow.start()
    _, node = await handler(flow.greet(), "identify")(
        {"given_name": "Nobody", "family_name": "Here", "birth_date": "2000-01-01"}, None
    )
    # The same words as a match (SECURITY.md §4.4).
    assert node["name"] == "verify"
    check = handler(node, "check_code")
    for _ in range(2):
        result, nxt = await check(CODE, None)
        assert nxt is None and "didn't match" in result["say"]
    result, nxt = await check(CODE, None)
    assert nxt["name"] == "hand_off"
    await flow.close()


async def test_the_model_never_sees_a_slot_token(ehr):
    flow = CallFlow(ehr)
    await flow.start()
    _, node = await handler(flow.greet(), "identify")(JORDAN, None)
    _, node = await handler(node, "check_code")(CODE, None)
    result, _ = await handler(node, "find_slots")({"reason": "a check-up"}, None)
    for slot in flow.offered:
        assert slot.token.reveal() not in str(result)
    await flow.close()
