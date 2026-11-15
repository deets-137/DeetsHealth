"""The request log in the dev call record (schema 12, the owner, 2026-11-15): each request to
Claude keeps what was sent, as a delta, and ehr's requests to Epic come in as `epic` events.
Synthetic test calls only."""

import json

from deets_agent.call_record import CallRecord


def events(record: CallRecord) -> list[dict]:
    record._file.flush()
    return [json.loads(line) for line in record.path.read_text(encoding="utf-8").splitlines()]


def test_each_request_keeps_only_what_is_new(tmp_path):
    r = CallRecord(mode="replay", model="claude-haiku-5-5", folder=tmp_path, source="test")
    first = [{"role": "user", "content": [{"type": "text", "text": "Hi", "cache_control": {"type": "ephemeral"}}]}]
    r.llm_request("k1" * 8, {"system": "S", "messages": first}, hit=True, dropped=0).done({}, replayed=True)
    second = [
        {"role": "user", "content": [{"type": "text", "text": "Hi"}]},
        {"role": "assistant", "content": [{"type": "thinking", "thinking": "x"}, {"type": "text", "text": "Hello"}]},
        {"role": "user", "content": "Book a visit"},
    ]
    r.llm_request("k2" * 8, {"system": "S", "messages": second}, hit=True, dropped=0).done({}, replayed=True)
    r.epic({"rpc": "FindVisit", "what": "GET Appointment?date&patient", "status": 200, "ms": 512,
            "results": 71, "bytes": 90000, "at_ms": 40})
    llm = [e for e in events(r) if e["kind"] == "llm"]
    assert llm[0]["system"] == "S" and len(llm[0]["new_messages"]) == 1
    # The cache marker moved and a thinking block came: still a delta, not the whole context.
    assert "system" not in llm[1], "the system prompt did not change"
    assert [m["role"] for m in llm[1]["new_messages"]] == ["assistant", "user"]
    assert llm[1]["new_messages"][0]["content"] == [{"type": "text", "text": "Hello"}]
    epic = [e for e in events(r) if e["kind"] == "epic"]
    assert epic[0]["what"] == "GET Appointment?date&patient" and epic[0]["results"] == 71


def test_a_rewritten_context_is_kept_whole(tmp_path):
    r = CallRecord(mode="replay", model="claude-haiku-5-5", folder=tmp_path, source="test")
    r.llm_request("k1" * 8, {"messages": [{"role": "user", "content": "A"}]}, hit=True, dropped=0).done({}, replayed=True)
    r.llm_request("k2" * 8, {"messages": [{"role": "user", "content": "B"}]}, hit=True, dropped=0).done({}, replayed=True)
    llm = [e for e in events(r) if e["kind"] == "llm"]
    assert llm[1]["all_messages"] == [{"role": "user", "content": "B"}]
