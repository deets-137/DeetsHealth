"""The demo's second door and its routes (dev_server.py; docs/ops/DEMO.md §3). No ehr, no
Cloudflare: the Access gate gets a key made here, and the app runs without its lifespan."""

from __future__ import annotations

import base64
import time
from pathlib import Path

import jwt
import pytest
from cryptography.hazmat.primitives.asymmetric import rsa
from starlette.testclient import TestClient
from starlette.websockets import WebSocketDisconnect

from deets_agent.access import TOKEN_HEADER as ACCESS_HEADER
from deets_agent.access import AccessGate, AccessSettings
from deets_agent.dev_server import TOKEN_HEADER, DemoPrefix, State, build_app
from deets_agent.ehr_client import ACTOR
from deets_agent.identity import LEVERS

TOKEN = "t" * 40
LOCAL = "http://localhost:1420"
DEMO = "https://deetshealth.example.com"
SETTINGS = AccessSettings(team="team.cloudflareaccess.com", aud="aud-tag", origin=DEMO)
KEY = rsa.generate_private_key(public_exponent=65537, key_size=2048)


class Keys:
    async def key_for(self, token: str):
        return KEY.public_key()


def access_token(**claims) -> str:
    now = int(time.time())
    body = {"aud": SETTINGS.aud, "iss": SETTINGS.issuer, "email": "guest@example.com"}
    body.update({"iat": now, "exp": now + 600, **claims})
    return jwt.encode(body, KEY, algorithm="RS256", headers={"kid": "k1"})


GUEST = {ACCESS_HEADER: access_token(), "origin": DEMO}
OWNER = {TOKEN_HEADER: TOKEN, "origin": LOCAL}


@pytest.fixture
def site(tmp_path: Path) -> Path:
    root = tmp_path / "site"
    (root / "assets").mkdir(parents=True)
    (root / "index.html").write_text(
        '<script type="module" crossorigin src="/demo/assets/index-aaa.js"></script>', encoding="utf-8"
    )
    (root / "assets" / "index-aaa.js").write_text("console.log(1)", encoding="utf-8")
    (root / ".secret").write_text("no", encoding="utf-8")
    (tmp_path / "outside.txt").write_text("no", encoding="utf-8")
    return root


GUEST_ID = "00000000-0000-4000-8000-000000000002"


class GuestEhr:
    """ehr in short: the guest is on the list and holds a role with Testing Edit and PHI View
    (they may test and watch); the launch token has no owner row yet (every lever)."""

    async def seen_owner(self):
        return None

    async def admit_user(self, email):
        return GUEST_ID if email == "guest@example.com" else None

    async def levels(self):
        assert ACTOR.get() == GUEST_ID
        held = {"testing": "edit", "phi": "view"}
        return {"user_id": GUEST_ID, "role_ids": ["tester"], "roles_version": 1,
                "levels": {lever: held.get(lever, "none") for lever in LEVERS}}

    async def log_access(self, *args):
        return True


def client_for(tmp_path: Path, site: Path | None, door: bool = True) -> TestClient:
    calls = tmp_path / "calls"
    calls.mkdir(exist_ok=True)
    access = AccessGate(SETTINGS, keys=Keys()) if door else None
    state = State(TOKEN, {LOCAL}, 8790, calls=calls, access=access, static=site)
    state.ehr = GuestEhr()  # type: ignore[assignment]
    return TestClient(DemoPrefix(build_app(state)))


@pytest.fixture
def client(tmp_path, site) -> TestClient:
    return client_for(tmp_path, site)


def test_a_guest_passes_the_second_door(client):
    assert client.get("/agent/health", headers=GUEST).status_code == 200
    assert client.get("/demo/agent/health", headers=GUEST).status_code == 200


@pytest.mark.parametrize(
    "headers",
    [
        {},
        {ACCESS_HEADER: access_token()},  # no origin is fine for a GET; see below
        {ACCESS_HEADER: access_token(), "origin": "https://evil.example"},
        {ACCESS_HEADER: access_token(), "origin": LOCAL},
        {ACCESS_HEADER: access_token(aud="other"), "origin": DEMO},
        {ACCESS_HEADER: access_token(exp=1), "origin": DEMO},
        {ACCESS_HEADER: TOKEN, "origin": DEMO},  # the launch token is not an Access token
    ],
)
def test_the_second_door_refuses_a_bad_guest(client, headers):
    want = 200 if headers.keys() == {ACCESS_HEADER} else 403
    assert client.get("/agent/health", headers=headers).status_code == want


def test_without_access_settings_there_is_no_second_door(tmp_path, site):
    c = client_for(tmp_path, site, door=False)
    assert c.get("/agent/health", headers=GUEST).status_code == 403
    assert c.get("/agent/health", headers=OWNER).status_code == 200


def test_a_guest_opens_the_sockets_and_a_stranger_does_not(client):
    with client.websocket_connect("/demo/agent/watch", headers=GUEST) as ws:
        assert '"hello"' in ws.receive_text()
    with pytest.raises(WebSocketDisconnect) as e, client.websocket_connect(
        "/demo/agent/watch", headers={"origin": DEMO}
    ) as ws:
        ws.receive_text()
    assert e.value.code == 1008


def test_the_fake_twilio_refuses_the_tunnel(tmp_path):
    """The tunnel's requests come from 127.0.0.1 too; Cloudflare's header marks them, so even
    the right Basic auth is refused."""
    state = State(TOKEN, {LOCAL}, 8790, calls=tmp_path, access=AccessGate(SETTINGS, keys=Keys()))
    c = TestClient(DemoPrefix(build_app(state)), client=("127.0.0.1", 5000))
    tw = state.twilio
    auth = {"authorization": "Basic " + base64.b64encode(f"{tw.account_sid}:{tw.auth_token}".encode()).decode()}
    r = c.post(tw.path(), headers={**auth, "cf-connecting-ip": "203.0.113.9"})
    assert r.status_code == 401


def test_the_built_console_is_served_under_demo(client):
    r = client.get("/demo/", headers=GUEST)
    assert r.status_code == 200 and "index-aaa.js" in r.text
    assert r.headers["cache-control"] == "no-store"
    a = client.get("/demo/assets/index-aaa.js", headers=GUEST)
    assert a.status_code == 200 and "immutable" in a.headers["cache-control"]
    assert client.get("/demo", headers=GUEST, follow_redirects=False).status_code == 308


def test_the_built_console_needs_the_door_too(client):
    assert client.get("/demo/").status_code == 403


@pytest.mark.parametrize(
    "path", ["/demo/.secret", "/demo/../outside.txt", "/demo/%2e%2e/outside.txt", "/demo/nope.js", "/"]
)
def test_nothing_outside_the_build_is_served(client, path):
    assert client.get(path, headers=GUEST).status_code in (403, 404)


def test_no_demo_routes_without_a_build(tmp_path):
    c = client_for(tmp_path, None)
    assert c.get("/demo/", headers=OWNER).status_code == 404
    assert c.get("/agent/build", headers=OWNER).status_code == 404


def test_the_owner_gets_no_ice_servers(client):
    assert client.get("/agent/ice", headers=OWNER).json() == {"iceServers": []}


def test_a_guest_without_the_turn_key_is_told_why(client):
    r = client.get("/agent/ice", headers=GUEST)
    assert r.status_code == 409 and "TURN key" in r.json()["detail"]


# The response headers (CONSOLE.md §5.1).


def test_the_demo_files_carry_the_pages_policy(client):
    r = client.get("/demo/", headers=GUEST)
    csp = r.headers["content-security-policy"]
    assert "script-src 'self'" in csp and "unsafe" not in csp
    assert "require-trusted-types-for 'script'" in csp and "trusted-types deets-html" in csp
    assert csp.endswith("report-uri /demo/agent/csp-report")
    assert r.headers["x-content-type-options"] == "nosniff"
    assert "strict-transport-security" in r.headers


@pytest.mark.parametrize("path", ["/agent/health", "/demo/agent/health"])
def test_every_other_reply_carries_the_apis_policy(client, path):
    want = "default-src 'none'; frame-ancestors 'none'"
    assert client.get(path, headers=GUEST).headers["content-security-policy"] == want
    assert client.get(path).headers["content-security-policy"] == want  # a refusal too


def test_a_report_logs_the_directive_and_a_host_only(client, capsys):
    report = {"csp-report": {
        "document-uri": "https://deetshealth.example.com/demo/?q=secret",
        "effective-directive": "img-src",
        "blocked-uri": "https://img.example.com/a/path?x=secret",
    }}
    r = client.post("/demo/agent/csp-report", headers=GUEST, json=report)
    assert r.status_code == 204
    err = capsys.readouterr().err
    assert "csp: blocked by img-src: https://img.example.com" in err and "secret" not in err
    sink = {"csp-report": {"effective-directive": "require-trusted-types-for", "blocked-uri": "trusted-types-sink"}}
    client.post("/agent/csp-report", headers=GUEST, json=sink)
    assert "require-trusted-types-for: trusted-types-sink" in capsys.readouterr().err


def test_a_report_that_is_not_one_is_refused(client):
    assert client.post("/agent/csp-report", headers=GUEST, content=b"not json").status_code == 400
    assert client.post("/agent/csp-report", headers=GUEST, content=b"x" * 9000).status_code == 413
    assert client.post("/agent/csp-report", json={"csp-report": {}}).status_code == 403


def test_nothing_with_data_is_stored_and_no_other_site_may_load_it(client):
    for path in ("/agent/health", "/agent/catalog", "/demo/agent/health"):
        r = client.get(path, headers=GUEST)
        assert r.headers["cache-control"] == "no-store"
        assert r.headers["cross-origin-resource-policy"] == "same-origin"
    page = client.get("/demo/", headers=GUEST)
    assert page.headers["cache-control"] == "no-store"
    assert page.headers["cross-origin-resource-policy"] == "same-origin"
    asset = client.get("/demo/assets/index-aaa.js", headers=GUEST)
    assert "immutable" in asset.headers["cache-control"]
