"""Teams › People in the dev server (DEMO.md §3.5): the routes, Cloudflare's group kept as the
list, the guest check, and the start-up sync. No ehr and no Cloudflare: both are fakes here, so
no request leaves this machine and no database is touched (ehr's own store test covers the
table: ehr/tests/store.rs)."""

from __future__ import annotations

from pathlib import Path

import pytest
from starlette.testclient import TestClient

from deets_agent.access import AccessGate, AccessSettings
from deets_agent.cloudflare import AccessGroup, CloudflareError, CloudflareSettings
from deets_agent.dev_server import TOKEN_HEADER, State, build_app, sync_people
from deets_agent.ehr_client import EhrError, ErrorKind

TOKEN = "t" * 40
ORIGIN = "http://localhost:1420"
OK = {TOKEN_HEADER: TOKEN, "origin": ORIGIN}
OWNER_EMAIL = "owner@example.com"


class FakeEhr:
    """users.rs's rules, in memory."""

    def __init__(self) -> None:
        self.rows: dict[str, dict] = {}
        self.n = 0

    def _new(self, email: str, name: str, owner: bool = False, by: str | None = None) -> str:
        self.n += 1
        uid = f"00000000-0000-4000-8000-{self.n:012d}"
        self.rows[uid] = {"id": uid, "email": email, "name": name, "owner": owner, "added_by": by,
                          "added_at": "2026-11-14T00:00:00Z", "last_seen_at": None, "removed": False}
        return uid

    def _active(self):
        return [r for r in self.rows.values() if not r["removed"]]

    async def list_users(self):
        return [{k: v for k, v in r.items() if k != "removed"} for r in sorted(self._active(), key=lambda r: not r["owner"])]

    async def add_user(self, email, name, by):
        email = email.strip().lower()
        if "@" not in email:
            e = EhrError(ErrorKind.BAD_INPUT, "AddUser")
            e.detail = "email: not an email address"
            raise e
        if any(r["email"] == email for r in self._active()):
            e = EhrError(ErrorKind.OTHER, "AddUser")
            e.detail = "that email is already on the list"
            raise e
        return self._new(email, name.strip(), by=by or "owner")

    async def remove_user(self, uid):
        r = self.rows.get(uid)
        if r is None or r["removed"]:
            e = EhrError(ErrorKind.OTHER, "RemoveUser")
            e.detail = "no such person on the list"
            raise e
        if r["owner"]:
            e = EhrError(ErrorKind.NOT_VERIFIED, "RemoveUser")
            e.detail = "the owner cannot be removed"
            raise e
        r["removed"] = True
        return r["email"]

    async def admit_user(self, email):
        return any(r["email"] == email for r in self._active())

    async def seed_users(self, emails):
        known = {r["email"] for r in self.rows.values()}
        new = [e for e in dict.fromkeys(emails) if e not in known]
        owner = not self.rows and len(new) == 1
        for e in new:
            self._new(e, e, owner=owner)
        return len(new)


class FakeCloudflare:
    def __init__(self, emails: list[str], fail: bool = False) -> None:
        self.group = {"name": "DeetsHealth demo guests", "include": [{"email": {"email": e}} for e in emails],
                      "exclude": [], "require": []}
        self.ended: list[str] = []
        self.fail = fail
        self.calls: list[tuple[str, str]] = []

    async def __call__(self, method, url, headers, body):
        assert headers["Authorization"] == "Bearer cf-token"
        self.calls.append((method, url.rsplit("/access/", 1)[1]))
        if self.fail:
            return 500, {"success": False}
        if method == "GET":
            return 200, {"success": True, "result": self.group}
        if method == "PUT":
            self.group = body
            return 200, {"success": True, "result": body}
        self.ended.append(body["email"])
        return 200, {"success": True, "result": None}

    def emails(self) -> list[str]:
        return [r["email"]["email"] for r in self.group["include"]]


SETTINGS = CloudflareSettings(token="cf-token", account="a" * 32, group="0" * 8 + "-0000-4000-8000-" + "0" * 12)


def make(tmp_path: Path, cf: FakeCloudflare | None):
    group = AccessGroup(SETTINGS, request=cf) if cf is not None else None
    access = AccessGate(AccessSettings(team="t.cloudflareaccess.com", aud="a", origin="https://demo.example.com"))
    state = State(TOKEN, {ORIGIN}, 8790, calls=tmp_path, access=access, cloudflare=group)
    state.ehr = FakeEhr()  # type: ignore[assignment]
    return state, TestClient(build_app(state))


async def test_the_start_sync_brings_in_the_group_and_makes_the_owner(tmp_path):
    cf = FakeCloudflare([OWNER_EMAIL])
    state, _ = make(tmp_path, cf)
    await sync_people(state)
    people = await state.ehr.list_users()
    assert [(p["email"], p["owner"]) for p in people] == [(OWNER_EMAIL, True)]
    assert cf.emails() == [OWNER_EMAIL]
    # From now on the gate asks the list.
    assert await state.access._guests(OWNER_EMAIL) is True
    assert await state.access._guests("stranger@example.com") is False


async def test_without_a_cloudflare_link_access_alone_decides(tmp_path):
    state, _ = make(tmp_path, None)
    await sync_people(state)
    assert state.access._guests is None


async def test_a_cloudflare_failure_at_start_leaves_access_alone(tmp_path):
    state, _ = make(tmp_path, FakeCloudflare([OWNER_EMAIL], fail=True))
    await sync_people(state)
    assert state.access._guests is None


async def test_add_and_remove_keep_the_group_the_list(tmp_path):
    cf = FakeCloudflare([OWNER_EMAIL])
    state, c = make(tmp_path, cf)
    await sync_people(state)

    r = c.post("/agent/people", headers=OK, json={"email": "Priya@Example.com", "name": "Priya Shah"})
    assert r.status_code == 200 and r.json()["warning"] is None
    assert sorted(cf.emails()) == [OWNER_EMAIL, "priya@example.com"]
    view = c.get("/agent/people", headers=OK).json()
    assert view["linked"] and view["store"] and view["you"] == "owner"
    assert view["invite"] == "https://demo.example.com/demo/"
    priya = next(p for p in view["people"] if p["email"] == "priya@example.com")

    assert c.post("/agent/people", headers=OK, json={"email": "priya@example.com", "name": "x"}).status_code == 409
    assert c.post("/agent/people", headers=OK, json={"email": "nope", "name": "x"}).status_code == 400
    assert c.post("/agent/people", headers=OK, json={"email": 1}).status_code == 400

    owner = view["people"][0]
    assert c.delete(f"/agent/people/{owner['id']}", headers=OK).status_code == 403
    r = c.delete(f"/agent/people/{priya['id']}", headers=OK)
    assert r.status_code == 200 and r.json()["warning"] is None
    assert cf.emails() == [OWNER_EMAIL]
    assert cf.ended == ["priya@example.com"]
    assert c.delete(f"/agent/people/{priya['id']}", headers=OK).status_code == 404


async def test_a_cloudflare_failure_after_a_save_is_a_warning(tmp_path):
    cf = FakeCloudflare([OWNER_EMAIL])
    state, c = make(tmp_path, cf)
    await sync_people(state)
    cf.fail = True
    r = c.post("/agent/people", headers=OK, json={"email": "sam@example.com", "name": "Sam"})
    assert r.status_code == 200
    assert "Cloudflare answered 500" in r.json()["warning"]
    assert "sam@example.com" not in r.json()["warning"]


def test_the_routes_have_the_wall(tmp_path):
    _, c = make(tmp_path, None)
    assert c.get("/agent/people").status_code == 403
    assert c.post("/agent/people", json={"email": "a@b.co", "name": "A"}).status_code == 403
    assert c.delete("/agent/people/x").status_code == 403


async def test_the_group_is_never_written_empty():
    group = AccessGroup(SETTINGS, request=FakeCloudflare([OWNER_EMAIL]))
    with pytest.raises(CloudflareError):
        await group.set_emails([])


def test_settings_come_from_env(monkeypatch):
    for n in ("DEETS_CF_API_TOKEN", "DEETS_CF_ACCOUNT_ID", "DEETS_ACCESS_GROUP_ID"):
        monkeypatch.delenv(n, raising=False)
    assert CloudflareSettings.from_env() is None
    monkeypatch.setenv("DEETS_CF_API_TOKEN", "cf-token")
    monkeypatch.setenv("DEETS_CF_ACCOUNT_ID", "a" * 32)
    monkeypatch.setenv("DEETS_ACCESS_GROUP_ID", SETTINGS.group)
    assert CloudflareSettings.from_env() == SETTINGS
    monkeypatch.setenv("DEETS_CF_ACCOUNT_ID", "../x")
    with pytest.raises(ValueError):
        CloudflareSettings.from_env()
