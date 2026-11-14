"""The Access token check (access.py; docs/ops/DEMO.md §3.3). No network: the team's key is
made here and handed to the gate."""

from __future__ import annotations

import time

import jwt
import pytest
from cryptography.hazmat.primitives.asymmetric import rsa

from deets_agent.access import (
    GUEST_TTL,
    TOKEN_HEADER,
    AccessGate,
    AccessSettings,
)

SETTINGS = AccessSettings(
    team="team.cloudflareaccess.com", aud="aud-tag", origin="https://demo.example.com"
)
GUEST = "guest@example.com"


def _key():
    return rsa.generate_private_key(public_exponent=65537, key_size=2048)


TEAM_KEY = _key()
OTHER_KEY = _key()


class Keys:
    def __init__(self, key) -> None:
        self.key = key

    async def key_for(self, token: str):
        return self.key.public_key()


def token(key=TEAM_KEY, **claims) -> str:
    now = int(time.time())
    body = {
        "aud": [SETTINGS.aud],
        "iss": SETTINGS.issuer,
        "email": GUEST,
        "iat": now,
        "exp": now + 600,
        "sub": "user-id",
    }
    body.update(claims)
    body = {k: v for k, v in body.items() if v is not None}
    return jwt.encode(body, key, algorithm="RS256", headers={"kid": "k1"})


def headers(tok: str | None = None, origin: str | None = SETTINGS.origin) -> dict[str, str]:
    h = {TOKEN_HEADER: token() if tok is None else tok}
    if origin is not None:
        h["origin"] = origin
    return h


@pytest.fixture
def gate() -> AccessGate:
    return AccessGate(SETTINGS, keys=Keys(TEAM_KEY))


async def test_a_good_token_names_the_guest(gate):
    assert await gate.email(headers()) == GUEST


async def test_a_same_origin_get_without_origin_passes(gate):
    assert await gate.email(headers(origin=None)) == GUEST


async def test_the_email_is_lower_case(gate):
    assert await gate.email(headers(token(email="Guest@Example.COM"))) == GUEST


@pytest.mark.parametrize(
    "tok",
    [
        "",
        "not-a-jwt",
        "x" * 9000,
        token(key=OTHER_KEY),  # signed by someone else
        token(aud=["another-app"]),
        token(iss="https://evil.cloudflareaccess.com"),
        token(exp=int(time.time()) - 3600),  # expired
        token(exp=None),  # no expiry
        token(email=None),  # a service token
        token(email="not-an-email"),
    ],
)
async def test_a_bad_token_gets_no_entry(gate, tok):
    assert await gate.email(headers(tok)) is None


async def test_no_header_gets_no_entry(gate):
    assert await gate.email({"origin": SETTINGS.origin}) is None


async def test_another_origin_gets_no_entry(gate):
    assert await gate.email(headers(origin="https://evil.example.com")) is None
    assert await gate.email(headers(origin="http://localhost:1420")) is None


async def test_a_shared_secret_token_never_passes(gate):
    """Algorithm confusion: HS256 signed with a guessable secret is refused before any key."""
    forged = jwt.encode(
        {"aud": SETTINGS.aud, "iss": SETTINGS.issuer, "email": GUEST, "iat": 1, "exp": 2**40},
        "secret-of-32-bytes-or-more-padding!",
        algorithm="HS256",
    )
    assert await gate.email(headers(forged)) is None


async def test_a_key_fetch_that_fails_gets_no_entry():
    class Broken:
        async def key_for(self, token: str):
            raise OSError("Cloudflare did not answer")

    assert await AccessGate(SETTINGS, keys=Broken()).email(headers()) is None


async def test_the_guest_list_is_asked_and_kept_for_its_ttl():
    asked: list[str] = []
    allowed = {GUEST}
    now = [0.0]

    async def guests(email: str) -> bool:
        asked.append(email)
        return email in allowed

    gate = AccessGate(SETTINGS, keys=Keys(TEAM_KEY), guests=guests, clock=lambda: now[0])
    assert await gate.email(headers()) == GUEST
    allowed.clear()  # removed from the list
    now[0] = GUEST_TTL - 1
    assert await gate.email(headers()) == GUEST  # still the cached answer
    now[0] = GUEST_TTL + 1
    assert await gate.email(headers()) is None  # out within the TTL
    assert asked == [GUEST, GUEST]


async def test_forget_ends_a_removed_guest_at_once():
    allowed = {GUEST}

    async def guests(email: str) -> bool:
        return email in allowed

    gate = AccessGate(SETTINGS, keys=Keys(TEAM_KEY), guests=guests)
    assert await gate.email(headers()) == GUEST
    allowed.clear()
    gate.forget(GUEST)
    assert await gate.email(headers()) is None


async def test_a_guest_list_that_fails_gets_no_entry():
    async def guests(email: str) -> bool:
        raise OSError("Cloudflare did not answer")

    gate = AccessGate(SETTINGS, keys=Keys(TEAM_KEY), guests=guests)
    assert await gate.email(headers()) is None


def test_settings_come_from_env(monkeypatch):
    for name in ("DEETS_ACCESS_TEAM", "DEETS_ACCESS_AUD", "DEETS_DEMO_ORIGIN"):
        monkeypatch.delenv(name, raising=False)
    assert AccessSettings.from_env() is None  # the demo is off
    monkeypatch.setenv("DEETS_ACCESS_TEAM", "team.cloudflareaccess.com")
    monkeypatch.setenv("DEETS_ACCESS_AUD", "aud-tag")
    monkeypatch.setenv("DEETS_DEMO_ORIGIN", "https://demo.example.com/")
    s = AccessSettings.from_env()
    assert s == SETTINGS
    monkeypatch.setenv("DEETS_DEMO_ORIGIN", "http://demo.example.com")
    with pytest.raises(ValueError):
        AccessSettings.from_env()
