"""The agent's dev server over HTTP (dev_server.py; DEV-API.md): the wall on every route, and the
annotation gate's answers. No ehr and no Claude: the app runs without its lifespan, so ehr is
never started and `/agent/call` refuses."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from starlette.testclient import TestClient
from starlette.websockets import WebSocketDisconnect

from deets_agent.dev_server import TOKEN_HEADER, State, build_app

TOKEN = "t" * 40
ORIGIN = "http://localhost:1420"
OK = {TOKEN_HEADER: TOKEN, "origin": ORIGIN}

CALL = [
    {"t_ms": 0, "kind": "call_start", "schema_version": 5, "call_id": "a", "source": "test",
     "mode": "live", "model": "m", "started": "2026-11-12T10:00:00"},
    {"t_ms": 900, "kind": "call_end", "reason": "the agent ended it", "cost_usd": 0.0},
]


@pytest.fixture
def client(tmp_path: Path) -> TestClient:
    (tmp_path / "a.jsonl").write_text("\n".join(json.dumps(e) for e in CALL) + "\n", encoding="utf-8")
    state = State(TOKEN, {ORIGIN}, 8790, calls=tmp_path)
    return TestClient(build_app(state))  # not entered: no lifespan, no ehr


@pytest.mark.parametrize("path", ["/agent/health", "/agent/catalog", "/agent/sandbox", "/agent/latency", "/agent/calls/a"])
def test_the_wall_refuses_without_the_token_or_from_another_origin(client, path):
    assert client.get(path).status_code == 403
    assert client.get(path, headers={TOKEN_HEADER: "x" * 40, "origin": ORIGIN}).status_code == 403
    assert client.get(path, headers={TOKEN_HEADER: TOKEN, "origin": "https://evil.example"}).status_code == 403
    assert client.get(path, headers=OK).status_code == 200


@pytest.mark.parametrize("path", ["/agent/watch", "/agent/call"])
def test_the_sockets_close_1008_without_the_token(client, path):
    with pytest.raises(WebSocketDisconnect) as e, client.websocket_connect(path) as ws:
        ws.receive_text()
    assert e.value.code == 1008


def test_a_call_waits_for_ehr(client):
    with pytest.raises(WebSocketDisconnect) as e, client.websocket_connect("/agent/call", headers=OK) as ws:
        ws.receive_text()
    assert e.value.code == 1013


def test_the_watch_stream_says_hello(client):
    with client.websocket_connect("/agent/watch", headers=OK) as ws:
        assert json.loads(ws.receive_text()) == {"type": "hello", "live": None}


def test_a_missing_call_is_404(client):
    assert client.get("/agent/calls/nope", headers=OK).status_code == 404


def test_an_annotation_is_stamped_by_the_catalog(client, tmp_path):
    r = client.post("/agent/annotations", headers=OK, json={
        "kind": "rating", "call": "a", "grader_kind": "human", "grader": "owner",
        "dimension": "phone_style", "verdict": "pass", "score": 4,
    })
    assert r.status_code == 200
    line = json.loads((tmp_path / "annotations.jsonl").read_text(encoding="utf-8"))
    assert line["at"] and line["rubric_version"]
    rating = client.get("/agent/catalog", headers=OK).json()["calls"][0]["ratings"][0]
    assert (rating["verdict"], rating["score"]) == ("pass", 4)


@pytest.mark.parametrize(
    ("body", "reason"),
    [
        ({"kind": "keep", "call": "a", "value": True, "extra": "x"}, "extra: not a field of keep"),
        ({"kind": "label", "call": "a", "text": "x", "at": "2020-01-01"}, "at: not a field of label"),
        ({"kind": "rating", "call": "a", "grader_kind": "human", "grader": "owner", "dimension": "phone_style",
          "verdict": "pass", "rubric_version": "old"}, "rubric_version: not a field of rating"),
        ({"kind": "label", "call": "a", "text": "x", "Some Name": 1}, "a field: not a field of label"),
        ({"kind": "keep", "call": "a", "value": "yes"}, "value: not true or false"),
        ({"kind": "label", "call": "nope", "text": "x"}, "call: no such call"),
    ],
)
def test_a_refused_annotation_says_which_field(client, tmp_path, body, reason):
    r = client.post("/agent/annotations", headers=OK, json=body)
    assert r.status_code == 400
    assert r.json() == {"detail": reason}
    assert not (tmp_path / "annotations.jsonl").exists()


def test_an_annotation_that_is_not_an_object_is_400(client):
    assert client.post("/agent/annotations", headers=OK, json=[1]).status_code == 400
    assert client.post("/agent/annotations", headers={**OK, "content-type": "application/json"},
                       content=b"{").status_code == 400
