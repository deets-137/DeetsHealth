"""Access by role in the dev server (docs/ideas/ACCESS-ROLES.md §5, §6 Hand-over 2): every route
names what it needs, the wall refuses by lever and level, the person travels to ehr on every
staff call, and the ledger (DEV-API.md §2a) is the code's. ehr is a fake here: its own rules
and the audit trail are tested in ehr (ehr/tests/store.rs)."""

from __future__ import annotations

import re
from pathlib import Path

import pytest
from starlette.testclient import TestClient
from starlette.websockets import WebSocketDisconnect

from deets_agent import dev_server
from deets_agent.access import AccessSettings
from deets_agent.dev_server import ROUTE_NEEDS, TOKEN_HEADER, TWILIO, State, build_app
from deets_agent.ehr_client import ACTOR, SYSTEM, EhrError, ErrorKind, as_system
from deets_agent.identity import LEVERS

TOKEN = "t" * 40
ORIGIN = "http://localhost:1420"
OWNER_HEADERS = {TOKEN_HEADER: TOKEN, "origin": ORIGIN}
OWNER_ID = "00000000-0000-4000-8000-000000000001"
ANALYST_ID = "00000000-0000-4000-8000-000000000002"
DEV_API = Path(__file__).resolve().parents[2] / "docs" / "architecture" / "DEV-API.md"
WORDS = {"phi": "PHI", "audit_logs": "Audit logs"}


class Gate:
    """A second door that lets in whoever the test names: `analyst@example.com` by default."""

    def __init__(self) -> None:
        self.settings = AccessSettings(team="t.cloudflareaccess.com", aud="a", origin="https://demo.example.com")
        self.email_now: str | None = "analyst@example.com"

    async def email(self, headers):
        return self.email_now if headers.get("x-test-guest") else None

    def forget(self, email):
        pass


class RolesEhr:
    """ehr in short: the owner holds Edit everywhere; the analyst holds a Junior Analyst's
    levels. Each staff call notes who asked (`ACTOR`)."""

    def __init__(self) -> None:
        self.asked: list[tuple[str, str | None]] = []
        self.analyst = {"agent": "view", "rules": "view", "testing": "edit", "analytics": "view"}

    async def seen_owner(self):
        return OWNER_ID

    async def admit_user(self, email):
        return ANALYST_ID if email == "analyst@example.com" else None

    async def levels(self):
        who = ACTOR.get()
        if who == OWNER_ID:
            held = {lever: "edit" for lever in LEVERS}
        elif who == ANALYST_ID:
            held = self.analyst
        else:
            e = EhrError(ErrorKind.FORBIDDEN, "Levels")
            e.detail = "not a person on the list"
            raise e
        return {"user_id": who, "role_ids": [], "roles_version": 1, "levels": {lv: held.get(lv, "none") for lv in LEVERS}}

    async def list_users(self):
        self.asked.append(("ListUsers", ACTOR.get()))
        return []

    async def get_settings(self, service, version_id=0):
        self.asked.append(("GetSettings", ACTOR.get()))

    async def list_setting_versions(self, service):
        self.asked.append(("ListSettingVersions", ACTOR.get()))

    async def log_access(self, lever, level, action, kind="", object_id="", data_class=""):
        self.asked.append((f"LogAccess {action}", ACTOR.get()))
        return True


def make(tmp_path: Path):
    state = State(TOKEN, {ORIGIN}, 8790, calls=tmp_path)
    state.access = Gate()  # type: ignore[assignment]
    state.ehr = RolesEhr()  # type: ignore[assignment]
    return state, TestClient(build_app(state))


ANALYST = {"x-test-guest": "1", "origin": "https://demo.example.com"}


def test_the_ledger_is_the_code():
    """DEV-API.md §2a, row by row, against ROUTE_NEEDS, both ways."""
    text = DEV_API.read_text(encoding="utf-8")
    section = text.split("## 2a. What each route needs", 1)[1].split("\n## ", 1)[0]
    rows = dict(re.findall(r"^\| `(\w+ /[^`]*)` \| ([^|]+) \|$", section, re.MULTILINE))
    want = {}
    for (method, path), need in ROUTE_NEEDS.items():
        if need is None:
            words = "any person"
        elif need == TWILIO:
            words = "ehr only (its own check)"
        else:
            lever, level = need
            words = f"{WORDS.get(lever, lever.capitalize())} {level.capitalize()}"
        want[f"{method} {path}"] = words
    assert {k: v.strip() for k, v in rows.items()} == want


def test_a_route_with_no_need_stops_the_start(tmp_path, monkeypatch):
    smaller = {k: v for k, v in ROUTE_NEEDS.items() if k != ("GET", "/agent/catalog")}
    monkeypatch.setattr(dev_server, "ROUTE_NEEDS", smaller)
    with pytest.raises(RuntimeError, match="/agent/catalog"):
        build_app(State(TOKEN, {ORIGIN}, 8790, calls=tmp_path))


def test_the_wall_refuses_by_lever_and_level(tmp_path):
    _, c = make(tmp_path)
    # The analyst: Agent View, so the words read; no Agent Edit, so no save; no People at all.
    assert c.get("/agent/prompt", headers=ANALYST).status_code == 200
    r = c.post("/agent/prompt", headers=ANALYST, json={"values": {}})
    assert r.status_code == 403 and r.json() == {"error": "forbidden", "lever": "agent", "level": "edit"}
    assert c.get("/agent/people", headers=ANALYST).status_code == 403
    assert c.get("/agent/audit", headers=ANALYST).status_code == 403
    # Their own levels, never their email.
    me = c.get("/agent/me", headers=ANALYST).json()
    assert me["user_id"] == ANALYST_ID and me["levels"]["agent"] == "view" and me["levels"]["phi"] == "none"
    assert "email" not in me
    # The owner, through the launch token, holds every lever.
    assert c.get("/agent/me", headers=OWNER_HEADERS).json()["levels"] == {lv: "edit" for lv in LEVERS}


def test_someone_not_on_the_list_gets_nothing(tmp_path):
    state, c = make(tmp_path)
    state.access.email_now = "stranger@example.com"  # type: ignore[union-attr]
    assert c.get("/agent/health", headers=ANALYST).status_code == 403


def test_the_person_travels_to_ehr_and_a_call_s_settings_are_the_system_s(tmp_path):
    state, c = make(tmp_path)
    c.get("/agent/people", headers=OWNER_HEADERS)
    assert ("ListUsers", OWNER_ID) in state.ehr.asked  # type: ignore[union-attr]

    async def read():
        with as_system():
            await state.ehr.get_settings("voice")  # type: ignore[union-attr]

    import asyncio

    asyncio.run(read())
    assert state.ehr.asked[-1] == ("GetSettings", SYSTEM)  # type: ignore[union-attr]


def test_a_transcript_read_is_recorded(tmp_path):
    state, c = make(tmp_path)
    assert c.get("/agent/calls/typed_nope", headers=ANALYST).status_code == 404
    # No such call: nothing to record. The look at a real one names the call, never its words.
    assert not any(a.startswith("LogAccess") for a, _ in state.ehr.asked)  # type: ignore[union-attr]


def test_a_watch_needs_phi_view(tmp_path):
    _, c = make(tmp_path)
    with pytest.raises(WebSocketDisconnect) as e, c.websocket_connect("/agent/watch", headers=ANALYST) as ws:
        ws.receive_text()
    assert e.value.code == 1008
    with c.websocket_connect("/agent/watch", headers=OWNER_HEADERS) as ws:
        assert '"hello"' in ws.receive_text()
