"""Per-tool latency (deets_agent/latency.py): the spans of each tool call, and the summary."""

from __future__ import annotations

from deets_agent.latency import percentile, summarize, tool_rows


def _start(schema: int) -> dict:
    return {"t_ms": 0, "kind": "call_start", "schema_version": schema, "model": "m", "source": "test"}


SCHEMA_4 = [
    _start(4),
    {"t_ms": 10.0, "kind": "node", "name": "verify"},
    {"t_ms": 100.0, "kind": "caller", "text": "a line"},
    {"t_ms": 900.0, "kind": "llm", "hit": False, "first_tool_ms": 750.5},
    {"t_ms": 900.4, "kind": "tool_call", "name": "confirm_details", "arguments": {}, "trigger_ms": 800.4},
    {"t_ms": 902.0, "kind": "ehr", "call": "Identify", "ms": 5.2, "outcome": "ok"},
    {"t_ms": 908.6, "kind": "tool_result", "name": "confirm_details", "result": {}, "ms": 8.2},
    {"t_ms": 1310.0, "kind": "agent", "text": "a fixed line", "first_ms": 905.0},
]


def test_each_span_of_an_exact_record():
    [r] = tool_rows("c", SCHEMA_4)
    assert (r["tool"], r["node"], r["seq"]) == ("confirm_details", "verify", 1)
    assert r["trigger_ms"] == 800.4, "the record's own exact span, not the t_ms difference"
    assert r["model_ms"] == 750.5
    assert r["confirm_ms"] == 8.2
    assert (r["ehr_ms"], r["ehr_calls"]) == (5.2, ["Identify"])
    assert r["answer_ms"] == 4.6, "from the call to the line's first frame, not to its late stamp"
    assert r["total_ms"] == 805.0
    assert r["exact"] and not r["replayed"] and not r["failed"]


def test_an_old_record_works_out_trigger_and_leaves_answer_empty():
    old = [dict(e) for e in SCHEMA_4]
    old[0]["schema_version"] = 2
    del old[4]["trigger_ms"]
    del old[7]["first_ms"]
    [r] = tool_rows("c", old)
    assert r["trigger_ms"] == 800.4  # 900.4 - 100.0
    assert r["answer_ms"] is None and r["total_ms"] is None, "a stamp after the grace is not a guess we make"
    assert not r["exact"]


def test_replayed_rows_stay_out_of_the_summary_unless_asked():
    replay = [dict(e) for e in SCHEMA_4]
    replay[3] = {**replay[3], "hit": True}
    rows = tool_rows("a", SCHEMA_4) + tool_rows("b", replay)
    assert [g["calls"] for g in summarize(rows)] == [1]
    assert [g["calls"] for g in summarize(rows, with_replayed=True)] == [2]


def test_two_tools_before_one_line_share_its_answer():
    events = [
        _start(4),
        {"t_ms": 0.0, "kind": "caller", "text": "x"},
        {"t_ms": 500.0, "kind": "tool_call", "name": "find_slots", "trigger_ms": 500.0},
        {"t_ms": 505.0, "kind": "tool_result", "name": "find_slots", "ms": 5.0},
        {"t_ms": 900.0, "kind": "tool_call", "name": "book", "trigger_ms": 900.0},
        {"t_ms": 904.0, "kind": "tool_result", "name": "book", "ms": 4.0, "error": "taken"},
        {"t_ms": 1400.0, "kind": "agent", "text": "y", "first_ms": 1000.0},
    ]
    a, b = tool_rows("c", events)
    assert (a["answer_ms"], b["answer_ms"]) == (500.0, 100.0)
    assert b["failed"]


def test_percentile_is_nearest_rank():
    vals = [float(v) for v in range(1, 11)]
    assert (percentile(vals, 50), percentile(vals, 90), percentile([7.0], 90)) == (5.0, 9.0, 7.0)
