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


@pytest.mark.parametrize(
    "path",
    ["/agent/health", "/agent/catalog", "/agent/sandbox", "/agent/latency", "/agent/calls/a", "/agent/resources", "/agent/cartesia"],
)
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


def test_the_voice_seat_has_the_wall_and_waits_for_ehr(client):
    offer = {"type": "offer", "sdp": "v=0"}
    assert client.post("/agent/voice", json=offer).status_code == 403
    assert client.post("/agent/voice", json=offer, headers={TOKEN_HEADER: TOKEN, "origin": "https://evil.example"}).status_code == 403
    r = client.post("/agent/voice", json=offer, headers=OK)
    assert r.status_code == 409 and r.json()["detail"] == "ehr is not up yet"


def test_the_resources_count_this_cycles_voice_calls(client, tmp_path):
    voice = [
        {"t_ms": 0, "kind": "call_start", "schema_version": 7, "seat": "voice"},
        {"t_ms": 9, "kind": "call_end", "reason": "x", "cartesia": {"ink_s": 100.0, "sonic_chars": 500, "credits": 800}},
    ]
    from datetime import date

    stamp = date.today().strftime("%Y%m%d")
    (tmp_path / f"{stamp}-120000-test.jsonl").write_text("\n".join(json.dumps(e) for e in voice) + "\n", encoding="utf-8")
    (tmp_path / "19990101-120000-test.jsonl").write_text("\n".join(json.dumps(e) for e in voice) + "\n", encoding="utf-8")
    c = client.get("/agent/resources", headers=OK).json()["cartesia"]
    assert (c["used"], c["calls"], c["ink_s"], c["sonic_chars"]) == (800, 1, 100.0, 500)
    assert c["left"] == c["plan"] - 800 and c["live"] is None


def test_the_resources_give_claudes_dollars_and_one_calls_use(tmp_path):
    from datetime import date

    stamp = date.today().strftime("%Y%m%d")
    voice = [
        {"t_ms": 0, "kind": "call_start", "schema_version": 7, "seat": "voice"},
        {"t_ms": 9, "kind": "call_end", "reason": "x", "cost_usd": 0.032, "cartesia": {"credits": 1750}},
    ]
    text = [
        {"t_ms": 0, "kind": "call_start", "schema_version": 7, "seat": "text"},
        {"t_ms": 9, "kind": "call_end", "reason": "x", "cost_usd": 0.01},
    ]
    for name, events in ((f"{stamp}-120000-test", voice), (f"{stamp}-130000-test", text)):
        (tmp_path / f"{name}.jsonl").write_text("\n".join(json.dumps(e) for e in events) + "\n", encoding="utf-8")
    # The catalog reads the folder when the state is made, so the records go in first.
    client = TestClient(build_app(State(TOKEN, {ORIGIN}, 8790, calls=tmp_path)))
    r = client.get("/agent/resources", headers=OK).json()
    a = r["anthropic"]
    assert (a["calls"], a["spent"]) == (2, 0.042) and a["left"] == round(a["budget"] - a["offset"] - 0.042, 4)
    assert r["call"] is None
    one = client.get(f"/agent/resources?call={stamp}-120000-test", headers=OK).json()["call"]
    assert one == {"seat": "voice", "usd": 0.032, "credits": 1750, "live": False}
    typed = client.get(f"/agent/resources?call={stamp}-130000-test", headers=OK).json()["call"]
    assert typed == {"seat": "text", "usd": 0.01, "credits": None, "live": False}
    assert client.get("/agent/resources?call=../secret", headers=OK).json()["call"] is None


def test_the_cartesia_view_shows_the_levers_and_never_the_key(client):
    c = client.get("/agent/cartesia", headers=OK).json()
    assert c["tts"]["model"] == "sonic-3.6" and c["stt"]["model"] == "ink-2"
    # No ehr here: no store, no versions, the defaults shown and active.
    assert (c["store"], c["versions"], c["version"]) == (False, [], None)
    v = c["values"]
    assert v["eager_end_of_turn"] is False and v["barge_in"] is True
    assert (v["idle_s"], v["max_s"], v["offset"]) == (60, 360, 0)
    assert c["active"] == v
    assert isinstance(c["key"], bool)
    from deets_agent import config

    secret = config.cartesia_key()
    assert secret is None or secret not in json.dumps(c)


def test_a_save_waits_for_ehr_and_needs_values(client):
    assert client.post("/agent/cartesia", json={"values": {}}).status_code == 403
    assert client.post("/agent/cartesia", headers=OK, json={"x": 1}).status_code == 400
    r = client.post("/agent/cartesia", headers=OK, json={"values": {"speed": 1.0}})
    assert r.status_code == 409 and r.json()["detail"] == "ehr is not up yet"


def test_the_voice_defaults_name_every_setting_ehr_checks():
    """The agent's defaults and ehr's settings.rs › VOICE list the same settings."""
    import re

    from deets_agent import config

    rust = (config.REPO / "ehr" / "src" / "settings.rs").read_text(encoding="utf-8")
    block = rust.split("pub const VOICE: &[Setting] = &[", 1)[1].split("];", 1)[0]
    keys = re.findall(r's\("([a-z_]+)"', block)
    assert sorted(keys) == sorted(config.voice_defaults())


def test_the_llm_defaults_name_every_setting_ehr_checks():
    """The agent's llm defaults and ehr's settings.rs › LLM list the same settings, and the
    models and efforts agree."""
    import re

    from deets_agent import config

    rust = (config.REPO / "ehr" / "src" / "settings.rs").read_text(encoding="utf-8")
    block = rust.split("pub const LLM: &[Setting] = &[", 1)[1].split("];", 1)[0]
    assert sorted(re.findall(r's\("([a-z_]+)"', block)) == sorted(config.llm_defaults())
    models = rust.split("pub const MODELS: &[&str] = &[", 1)[1].split("];", 1)[0]
    assert re.findall(r'"([a-z0-9-]+)"', models) == list(config.LLM_MODELS)
    efforts = rust.split("pub const EFFORTS: &[&str] = &[", 1)[1].split("];", 1)[0]
    assert tuple(re.findall(r'"([a-z]+)"', efforts)) == config.EFFORTS


def test_the_hospital_keys_and_limits_are_ehr_s(client):
    """The agent's list of the hospital's settings is settings.rs › HOSPITAL and hospital.rs ›
    KEYS, in order; the file holds each; the limits are hospital.rs's."""
    import re

    from deets_agent import config

    rust = (config.REPO / "ehr" / "src" / "settings.rs").read_text(encoding="utf-8")
    block = rust.split("pub const HOSPITAL: &[Setting] = &[", 1)[1].split("];", 1)[0]
    assert tuple(re.findall(r's\("([a-z_]+)"', block)) == config.HOSPITAL_KEYS
    hospital = (config.REPO / "ehr" / "src" / "hospital.rs").read_text(encoding="utf-8")
    keys = hospital.split("pub const KEYS: &[&str] = &[", 1)[1].split("];", 1)[0]
    assert tuple(re.findall(r'"([a-z_]+)"', keys)) == config.HOSPITAL_KEYS
    for name, rs in (("cell", "CELL_MAX"), ("rows", "ROWS_MAX"), ("words", "WORDS_MAX"), ("times", "TIMES_MAX"), ("windows", "WINDOWS_MAX")):
        assert re.search(rf"pub const {rs}: usize = {config.HOSPITAL_LIMITS[name]};", hospital), rs
    assert set(config.hospital_defaults()) == set(config.HOSPITAL_KEYS)
    # Without a store: the file, and the limits for the page.
    c = client.get("/agent/hospital", headers=OK).json()
    assert c["store"] is False and c["keys"] == list(config.HOSPITAL_KEYS)
    assert c["values"]["time_zone"] == config.hospital_defaults()["time_zone"]
    assert client.get("/agent/hospital?version=-1", headers=OK).status_code == 400


def test_a_hospital_save_needs_every_list_and_waits_for_ehr(client):
    from deets_agent import config

    assert client.post("/agent/hospital", json={"values": {}}).status_code == 403
    assert client.post("/agent/hospital", headers=OK, json={"values": {"time_zone": "x"}}).status_code == 400
    r = client.post("/agent/hospital", headers=OK, json={"values": config.hospital_defaults()})
    assert r.status_code == 409 and r.json()["detail"] == "ehr is not up yet"


def test_the_anthropic_view_shows_the_models_and_never_the_key(client):
    c = client.get("/agent/anthropic", headers=OK).json()
    assert (c["store"], c["versions"], c["version"]) == (False, [], None)
    from deets_agent import config

    assert c["values"] == c["active"] == config.llm_defaults()
    assert [m["id"] for m in c["models"]] == list(config.LLM_MODELS)
    opus = next(m for m in c["models"] if m["id"] == "claude-opus-5-5")
    assert opus["thinking_off"] is False and opus["price"]["output"] == 20.0
    assert c["inference_geo"] == "us" and c["usage"]["budget"] == c["active"]["budget_usd"]
    assert isinstance(c["key"], bool)
    secret = config.anthropic_key()
    assert secret is None or secret not in json.dumps(c)


def test_an_anthropic_save_waits_for_ehr_and_needs_values(client):
    assert client.post("/agent/anthropic", json={"values": {}}).status_code == 403
    assert client.post("/agent/anthropic", headers=OK, json={"x": 1}).status_code == 400
    r = client.post("/agent/anthropic", headers=OK, json={"values": {"model": "claude-haiku-5-5"}})
    assert r.status_code == 409 and r.json()["detail"] == "ehr is not up yet"


def test_the_agent_view_shows_the_file_without_a_store(client):
    c = client.get("/agent/prompt", headers=OK).json()
    from deets_agent import prompts

    assert (c["store"], c["versions"], c["version"]) == (False, [], None)
    assert c["values"] == c["active"] == prompts.load()
    assert c["placeholders"]["line.booked"] == ["visit"] and len(c["placeholders"]) == 118
    assert c["recordings"]["scripts"] == 15 and isinstance(c["recordings"]["match"], bool)


def test_a_prompt_save_waits_for_ehr_and_needs_words(client):
    assert client.post("/agent/prompt", json={"values": {}}).status_code == 403
    assert client.post("/agent/prompt", headers=OK, json={"values": {"role": 3}}).status_code == 400
    r = client.post("/agent/prompt", headers=OK, json={"values": {"role": "Hi."}})
    assert r.status_code == 409 and r.json()["detail"] == "ehr is not up yet"


def test_the_kept_recordings_match_the_prompt_file():
    """tests/RECORDED-PROMPT.sha256 names the words the replays were made with. After an edit,
    press Record and test on Config › Agent (or record the scripts by hand and update it)."""
    from deets_agent.dev_server import recordings_match

    assert recordings_match()


def test_every_kept_script_has_a_cost_for_the_quote():
    """tests/RECORDED-COSTS.json: what recording each kept script cost (Record and test writes it
    after a passing run), so the dry run's quote has a number for every script."""
    from deets_agent.dev_server import RECORDED_COSTS, SCRIPTS

    costs = json.loads(RECORDED_COSTS.read_text(encoding="utf-8"))
    assert sorted(costs) == sorted(p.stem for p in SCRIPTS.glob("*.txt"))
    assert all(0 <= v < 1 for v in costs.values())


def test_the_spend_counts_this_cycles_records(tmp_path):
    from datetime import date

    from deets_agent.resources import anthropic_cycle

    end = {"t_ms": 9, "kind": "call_end", "reason": "x", "cost_usd": 0.25}
    stamp = date.today().strftime("%Y%m%d")
    (tmp_path / f"{stamp}-120000-test.jsonl").write_text(json.dumps(end) + "\n", encoding="utf-8")
    (tmp_path / "19990101-120000-test.jsonl").write_text(json.dumps(end) + "\n", encoding="utf-8")
    c = anthropic_cycle(tmp_path, live=0.05, budget=100, offset=10, day=1)
    assert (c["spent"], c["calls"], c["left"]) == (0.3, 1, 89.7)


def test_the_sandbox_list_takes_local_or_epic_only(client):
    local = client.get("/agent/sandbox", headers=OK).json()
    assert local["patients"] and local["more"] == {}
    assert client.get("/agent/sandbox?ehr=local", headers=OK).json() == local
    assert client.get("/agent/sandbox?ehr=cerner", headers=OK).status_code == 400


def test_epic_mode_is_refused_without_the_epic_files(tmp_path: Path):
    state = State(TOKEN, {ORIGIN}, 8790, calls=tmp_path)
    state.epic = False  # as on a machine with no epic-client.json or key
    c = TestClient(build_app(state))
    assert c.get("/agent/sandbox?ehr=epic", headers=OK).status_code == 503
    assert c.get("/agent/health", headers=OK).json()["epic"] is False


def test_the_watch_stream_says_hello(client):
    with client.websocket_connect("/agent/watch", headers=OK) as ws:
        assert json.loads(ws.receive_text()) == {"type": "hello", "live": None, "calls": []}


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


async def test_a_version_saved_before_a_release_keeps_its_words_and_gets_the_new_sections(tmp_path):
    """PROMPT-EDITOR.md §9 (2026-11-14): a hospital's saved words stay; a section added by a
    release takes the shipped words; a section the code no longer knows is dropped. Before, a
    missing section sent the whole call back to the files."""
    from types import SimpleNamespace

    from deets_agent import prompts
    from deets_agent.dev_server import active_words

    saved = {k: v for k, v in prompts.load().items() if k != "line.booked_visit_asked"}
    saved["line.goodbye"] = "Thanks for calling the clinic. Bye now."
    saved["line.retired"] = "A section from an older release."
    version = SimpleNamespace(id=7, label="11-14 | 1")

    class Ehr:
        async def list_setting_versions(self, service):
            return [SimpleNamespace(seq=0)]

        async def get_settings(self, service):
            return (version, saved) if service == "prompt" else None

    state = State(TOKEN, {ORIGIN}, 8790, calls=tmp_path)
    state.ehr = Ehr()
    words = await active_words(state)
    assert words.version_id == 7, "the saved version is used, not the files"
    assert words.sections["line.goodbye"] == "Thanks for calling the clinic. Bye now."
    assert words.sections["line.booked_visit_asked"] == prompts.load()["line.booked_visit_asked"]
    assert "line.retired" not in words.sections
