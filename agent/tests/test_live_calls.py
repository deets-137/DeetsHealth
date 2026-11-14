"""Several live calls at once (dev_server.py; docs/ops/DEMO.md §3.6): the live set, the watch
stream's hello and started, the label typed beside Start, who started each call, the limit, and
the fake Twilio's text going to the call that asked. No ehr, no Claude: records are opened by
hand in a temporary folder."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from starlette.testclient import TestClient

from deets_agent import config
from deets_agent.dev_server import (
    MAX_LABEL,
    MAX_LIVE_CALLS,
    OWNER,
    TOKEN_HEADER,
    State,
    _label,
    _open_record,
    _refuse_call,
    _tag,
    build_app,
)
from deets_agent.prompts import FILE_WORDS

TOKEN = "t" * 40
ORIGIN = "http://localhost:1420"
OK = {TOKEN_HEADER: TOKEN, "origin": ORIGIN}
LLM = {**config.llm_defaults()}


@pytest.fixture
def state(tmp_path: Path) -> State:
    s = State(TOKEN, {ORIGIN}, 8790, calls=tmp_path)
    s.ehr = object()  # type: ignore[assignment]  # "up", for _refuse_call; never called here
    s.has_key = True
    return s


def open_call(state: State, who: str = OWNER, label: str | None = None, tag: str | None = None):
    return _open_record(state, "local", "text", LLM, FILE_WORDS, who, label, tag)


def start_line(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8").splitlines()[0])


def test_two_calls_are_live_at_once(state):
    a = open_call(state)
    b = open_call(state, "guest@example.com")
    assert list(state.lives) == [a.record.call_id, b.record.call_id]
    assert a.record.call_id != b.record.call_id
    assert _refuse_call(state, "local") is None


def test_the_limit_holds(state):
    for _ in range(MAX_LIVE_CALLS):
        open_call(state)
    assert "calls are running" in (_refuse_call(state, "local") or "")


def test_the_record_names_owner_or_guest_never_the_email(state):
    a = open_call(state, label="reschedule on Epic")
    g = open_call(state, "guest@example.com")
    assert start_line(a.record.path)["started_by"] == "owner"
    assert start_line(a.record.path)["label"] == "reschedule on Epic"
    first = start_line(g.record.path)
    assert first["started_by"] == "guest" and "label" not in first
    assert "guest@example.com" not in g.record.path.read_text(encoding="utf-8")
    assert g.info()["by"] == "guest@example.com"  # in memory, for the Caller rail's list


def test_hello_lists_every_live_call_and_its_events(state):
    a = open_call(state, label="first", tag="tab-1")
    b = open_call(state, "guest@example.com", tag="tab-2")
    c = TestClient(build_app(state))
    with c.websocket_connect("/agent/watch", headers=OK) as ws:
        hello = json.loads(ws.receive_text())
        assert hello["live"] == b.record.call_id
        assert [x["call"] for x in hello["calls"]] == [a.record.call_id, b.record.call_id]
        assert hello["calls"][0]["label"] == "first" and hello["calls"][0]["by"] == OWNER
        assert hello["calls"][1]["tag"] == "tab-2"
        backlog = {json.loads(ws.receive_text())["call"] for _ in range(2)}
        assert backlog == {a.record.call_id, b.record.call_id}


def test_started_carries_the_call_info(state):
    seen: list[dict] = []
    q = type("Q", (), {"qsize": lambda self: 0, "put_nowait": lambda self, t: seen.append(json.loads(t))})()
    state.watchers.add(q)  # type: ignore[arg-type]
    live = open_call(state, label="x", tag="t1")
    started = next(m for m in seen if m["type"] == "started")
    assert started["call"] == live.record.call_id
    assert started["info"]["tag"] == "t1" and started["info"]["seat"] == "text"


def test_the_text_goes_to_the_call_that_called_a_tool_last(state):
    a = open_call(state)
    b = open_call(state)
    a.record.event("tool_call", name="identify")
    b.record.event("tool_call", name="identify")
    c = TestClient(build_app(state), client=("127.0.0.1", 5000))
    import base64

    tw = state.twilio
    auth = base64.b64encode(f"{tw.account_sid}:{tw.auth_token}".encode()).decode()
    form = {"To": "+15550100", "Body": "Your code is 123456", "MessagingServiceSid": tw.messaging_service}
    r = c.post(tw.path(), headers={"authorization": f"Basic {auth}"}, data=form)
    assert r.status_code == 201
    assert '"kind": "text"' in b.record.path.read_text(encoding="utf-8")
    assert '"kind": "text"' not in a.record.path.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "raw,want",
    [
        (None, None),
        ("", None),
        ("   ", None),
        ("reschedule\non Epic\t ok", "reschedule on Epic ok"),
        ("x" * 500, "x" * MAX_LABEL),
    ],
)
def test_a_label_is_one_trimmed_line(raw, want):
    assert _label(raw) == want


@pytest.mark.parametrize("raw,ok", [("tab-1a", True), ("a" * 41, False), ("<script>", False), (None, False)])
def test_a_tag_is_a_short_plain_mark(raw, ok):
    assert (_tag(raw) is not None) == ok
