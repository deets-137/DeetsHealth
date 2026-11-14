"""TURN credentials for a guest's voice call (turn.py; docs/ops/DEMO.md §3.7). No network: the
POST to Cloudflare is replaced here."""

from __future__ import annotations

import pytest

from deets_agent.turn import TTL, TurnError, TurnSettings, for_aiortc, ice_servers

SETTINGS = TurnSettings(key_id="abc123", api_token="secret-token")

# The shape Cloudflare's docs show for generate-ice-servers.
ANSWER = {
    "iceServers": [
        {"urls": ["stun:stun.cloudflare.com:3478", "stun:stun.cloudflare.com:53"]},
        {
            "urls": [
                "turn:turn.cloudflare.com:3478?transport=udp",
                "turn:turn.cloudflare.com:53?transport=udp",
                "turn:turn.cloudflare.com:3478?transport=tcp",
                "turns:turn.cloudflare.com:5349?transport=tcp",
                "turns:turn.cloudflare.com:443?transport=tcp",
            ],
            "username": "short-user",
            "credential": "short-pass",
        },
    ]
}


def fake(status=201, data=ANSWER, seen=None):
    async def post(url, headers, body):
        if seen is not None:
            seen.append((url, headers, body))
        return status, data

    return post


async def test_the_request_names_the_key_and_the_ttl():
    seen: list = []
    await ice_servers(SETTINGS, post=fake(seen=seen))
    url, headers, body = seen[0]
    assert url.endswith("/turn/keys/abc123/credentials/generate-ice-servers")
    assert headers == {"Authorization": "Bearer secret-token"}
    assert body == {"ttl": TTL}


async def test_port_53_is_dropped_and_the_relay_kept():
    servers = await ice_servers(SETTINGS, post=fake())
    urls = [u for s in servers for u in s["urls"]]
    assert not any(":53?" in u or u.endswith(":53") for u in urls)
    assert "turns:turn.cloudflare.com:443?transport=tcp" in urls
    relay = next(s for s in servers if "username" in s)
    assert relay["username"] == "short-user" and relay["credential"] == "short-pass"


async def test_a_single_server_object_is_taken_too():
    one = {"iceServers": ANSWER["iceServers"][1]}
    servers = await ice_servers(SETTINGS, post=fake(data=one))
    assert len(servers) == 1


@pytest.mark.parametrize(
    "status,data",
    [
        (401, None),
        (500, None),
        (201, {}),
        (201, {"iceServers": "nope"}),
        (201, {"iceServers": [ANSWER["iceServers"][0]]}),  # STUN only, no relay
    ],
)
async def test_no_relay_is_an_error_that_names_no_secret(status, data):
    with pytest.raises(TurnError) as raised:
        await ice_servers(SETTINGS, post=fake(status, data))
    assert "secret-token" not in str(raised.value)


async def test_the_agent_end_gets_the_same_servers():
    servers = await ice_servers(SETTINGS, post=fake())
    mine = for_aiortc(servers)
    assert [m.urls for m in mine] == [s["urls"] for s in servers]
    assert mine[-1].username == "short-user"


def test_settings_come_from_env(monkeypatch):
    monkeypatch.delenv("DEETS_TURN_KEY_ID", raising=False)
    monkeypatch.delenv("DEETS_TURN_API_TOKEN", raising=False)
    assert TurnSettings.from_env() is None
    monkeypatch.setenv("DEETS_TURN_KEY_ID", "abc123")
    monkeypatch.setenv("DEETS_TURN_API_TOKEN", "secret-token")
    assert TurnSettings.from_env() == SETTINGS
    monkeypatch.setenv("DEETS_TURN_KEY_ID", "abc/../x")
    with pytest.raises(ValueError):
        TurnSettings.from_env()
