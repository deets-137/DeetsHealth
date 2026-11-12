"""The Flow's steps (flow.py `STEPS`, `CallFlow._step`): the Step panel's source (CHAT.md §11)."""

from __future__ import annotations

from deets_agent.flow import CLOSING_STEPS, STEPS, CallFlow


def _built(flow: CallFlow) -> list[dict]:
    return [flow.greet(), flow.verify(), flow.menu(), flow.schedule(), flow.goodbye(), flow.handed_off()]


def test_every_node_is_a_listed_step_and_reports_itself():
    seen: list[tuple[str, list[str]]] = []
    flow = CallFlow(ehr=None)  # building a node calls no ehr
    flow.on_step = lambda name, tools: seen.append((name, tools))
    nodes = _built(flow)
    assert [n["name"] for n in nodes] == [s for s, _ in seen]
    assert {n["name"] for n in nodes} == set(STEPS), "a node missing from STEPS, or a step with no node"
    assert set(CLOSING_STEPS) <= set(STEPS)
    tools = dict(seen)
    assert tools["verify"] and all(isinstance(t, str) for t in tools["verify"])
    assert tools["goodbye"] == [] and tools["hand_off"] == []


def test_no_hook_no_report():
    _built(CallFlow(ehr=None))  # on_step unset: building still works
