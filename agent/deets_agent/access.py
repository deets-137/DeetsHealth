"""The Access token: the demo's second door in the wall (docs/ops/DEMO.md §3.3).

A guest reaches the agent through Cloudflare Access and the tunnel. Access signs every request
it lets through with a JWT in the `Cf-Access-Jwt-Assertion` header. This module checks that
token itself, so a request that slips past Access (a route left out of the Access app, a
mistake in the tunnel's config) still gets no answer:

- the signature, RS256 only, against the team's public keys (`/cdn-cgi/access/certs`, cached);
- the issuer (the team's own domain), the audience (the Access app's `aud` tag) and the expiry;
- an email in the token (a service token has none and is refused);
- the request's Origin, when the browser sends one: the demo's own address only;
- the guest list, when one is set (DEMO.md §3.5): a removed guest is out within `GUEST_TTL`,
  even with a live Access session.

The email is a guest's identity, not protected data, but it never goes in a log line all the
same (CLAUDE.md › Working style). Nothing here logs the token or the email.
"""

from __future__ import annotations

import asyncio
import os
import time
from collections.abc import Awaitable, Callable, Mapping
from dataclasses import dataclass
from typing import Any, Protocol

import jwt

TOKEN_HEADER = "cf-access-jwt-assertion"
# A real Access token is about 1 KB; anything far longer is refused before it is parsed.
MAX_TOKEN_BYTES = 8192
# How long a guest-list answer is kept (DEMO.md §3.3: "out within 30 s").
GUEST_TTL = 30.0
# Clock skew allowed between Cloudflare and this PC on `exp`, `iat` and `nbf`.
LEEWAY = 30
# How long the team's public keys are kept before they are fetched again.
KEYS_TTL = 300


@dataclass(frozen=True)
class AccessSettings:
    """Where the tokens come from and what they must name."""

    team: str  # the team's domain, e.g. "deetssolutions.cloudflareaccess.com"
    aud: str  # the Access app's audience tag
    origin: str  # the demo's address, e.g. "https://deetshealth.deets.solutions"

    @property
    def issuer(self) -> str:
        return f"https://{self.team}"

    @property
    def certs_url(self) -> str:
        return f"https://{self.team}/cdn-cgi/access/certs"

    @classmethod
    def from_env(cls) -> AccessSettings | None:
        """The demo's settings from `.env` (DEMO.md §3.4), or None when the demo is off: then
        the second door does not exist and only the launch token opens the wall."""
        team = os.environ.get("DEETS_ACCESS_TEAM", "").strip()
        aud = os.environ.get("DEETS_ACCESS_AUD", "").strip()
        origin = os.environ.get("DEETS_DEMO_ORIGIN", "").strip().rstrip("/")
        if not (team and aud and origin):
            return None
        if "/" in team or not origin.startswith("https://"):
            raise ValueError("DEETS_ACCESS_TEAM is a bare domain; DEETS_DEMO_ORIGIN is https://")
        return cls(team=team, aud=aud, origin=origin)


class KeySource(Protocol):
    """The public key that signed a token, or an exception when there is none."""

    async def key_for(self, token: str) -> Any: ...


class TeamKeys:
    """The team's public keys from Cloudflare, cached for `KEYS_TTL`. PyJWT's client fetches
    them with urllib, so the fetch runs off the event loop. A token whose `kid` is not in the
    cache makes one new fetch (Cloudflare rotates its keys)."""

    def __init__(self, settings: AccessSettings) -> None:
        self._client = jwt.PyJWKClient(
            settings.certs_url, cache_jwk_set=True, lifespan=KEYS_TTL, timeout=10
        )

    async def key_for(self, token: str) -> Any:
        found = await asyncio.to_thread(self._client.get_signing_key_from_jwt, token)
        return found.key


GuestCheck = Callable[[str], Awaitable[bool]]


class AccessGate:
    """The second door. `email(headers)` is the guest's email when the request passes, else
    None. One gate per dev server."""

    def __init__(
        self,
        settings: AccessSettings,
        keys: KeySource | None = None,
        guests: GuestCheck | None = None,
        clock: Callable[[], float] = time.monotonic,
    ) -> None:
        self.settings = settings
        self._keys = keys or TeamKeys(settings)
        self._guests = guests
        self._clock = clock
        self._guest_cache: dict[str, tuple[float, bool]] = {}

    async def email(self, headers: Mapping[str, str]) -> str | None:
        token = headers.get(TOKEN_HEADER, "")
        if not token or len(token) > MAX_TOKEN_BYTES:
            return None
        origin = headers.get("origin")
        if origin is not None and origin != self.settings.origin:
            return None
        email = await self._verify(token)
        if email is None:
            return None
        if self._guests is not None and not await self._is_guest(email):
            return None
        return email

    async def _verify(self, token: str) -> str | None:
        try:
            # The header's algorithm is checked before any key is fetched: only RS256, so a
            # token signed with a shared secret ("HS256" with the public key) never passes.
            if jwt.get_unverified_header(token).get("alg") != "RS256":
                return None
            key = await self._keys.key_for(token)
            claims = jwt.decode(
                token,
                key,
                algorithms=["RS256"],
                audience=self.settings.aud,
                issuer=self.settings.issuer,
                leeway=LEEWAY,
                options={"require": ["exp", "iat", "iss", "aud"]},
            )
        except Exception:  # noqa: BLE001 - a bad token, an unknown key, no answer: no entry
            return None
        email = claims.get("email")
        if not isinstance(email, str) or "@" not in email:
            return None
        return email.strip().lower()

    async def _is_guest(self, email: str) -> bool:
        now = self._clock()
        cached = self._guest_cache.get(email)
        if cached is not None and now - cached[0] < GUEST_TTL:
            return cached[1]
        try:
            allowed = bool(await self._guests(email))  # type: ignore[misc]
        except Exception:  # noqa: BLE001 - the list cannot be read: no entry, ask again next time
            return False
        self._guest_cache[email] = (now, allowed)
        return allowed

    def set_guests(self, guests: GuestCheck | None) -> None:
        """Ask this list from now on (the dev server sets it once ehr is up, DEMO.md §3.5)."""
        self._guests = guests
        self._guest_cache.clear()

    def forget(self, email: str) -> None:
        """Drop one guest's cached answer, so a Remove takes effect on the next request."""
        self._guest_cache.pop(email.strip().lower(), None)
