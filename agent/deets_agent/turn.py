"""STUN and TURN for a guest's voice call (docs/ops/DEMO.md §3.7).

On this PC the voice seat needs no relay: both ends are local. A guest's browser is on another
network and cannot reach this PC's UDP ports, so for each guest voice call the agent asks
Cloudflare's TURN service for short-lived credentials. The browser and the agent get the same
ICE servers. ICE tries a direct link first (STUN, free) and uses the relay only when that fails.

The TURN key id and its API token are long-term secrets: they stay in `.env` and in this
process, never in a browser. A browser gets only the short-lived username and credential.
WebRTC encrypts the audio end to end (DTLS), so the relay carries only encrypted packets.

TURN serves only the browser seat. Phone calls come in through Twilio and never use it
(DEMO.md §3.8).
"""

from __future__ import annotations

import os
import re
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from typing import Any

import aiohttp
from aiortc import RTCIceServer

API = "https://rtc.live.cloudflare.com/v1/turn/keys/{key}/credentials/generate-ice-servers"
# One hour: longer than any test call. Cloudflare stops relaying when credentials expire.
TTL = 3600
# Browsers block port 53, and the voice seat sends its offer with every candidate in it (no
# trickle), so a port-53 URL would only add a timeout (Cloudflare's TURN docs).
_PORT_53 = re.compile(r":53(\?|$)")


@dataclass(frozen=True)
class TurnSettings:
    key_id: str
    api_token: str

    @classmethod
    def from_env(cls) -> TurnSettings | None:
        """The TURN key from `.env`, or None: then a guest's voice call is refused with a
        plain reason, and local calls are not affected."""
        key_id = os.environ.get("DEETS_TURN_KEY_ID", "").strip()
        api_token = os.environ.get("DEETS_TURN_API_TOKEN", "").strip()
        if not (key_id and api_token):
            return None
        if not re.fullmatch(r"[A-Za-z0-9]+", key_id):
            raise ValueError("DEETS_TURN_KEY_ID is the TURN key's id (letters and digits)")
        return cls(key_id=key_id, api_token=api_token)


Post = Callable[[str, dict[str, str], dict[str, Any]], Awaitable[tuple[int, Any]]]


async def _post(url: str, headers: dict[str, str], body: dict[str, Any]) -> tuple[int, Any]:
    timeout = aiohttp.ClientTimeout(total=10)
    async with (
        aiohttp.ClientSession(timeout=timeout) as session,
        session.post(url, headers=headers, json=body) as response,
    ):
        data = await response.json(content_type=None) if response.status == 201 else None
        return response.status, data


async def ice_servers(settings: TurnSettings, ttl: int = TTL, post: Post = _post) -> list[dict]:
    """The ICE servers for one call, as the browser's `RTCPeerConnection` takes them. Raises
    `TurnError` when Cloudflare does not give them; the caller refuses the guest's voice call."""
    status, data = await post(
        API.format(key=settings.key_id),
        {"Authorization": f"Bearer {settings.api_token}"},
        {"ttl": ttl},
    )
    if status != 201 or not isinstance(data, dict):
        raise TurnError(f"Cloudflare's TURN service answered {status}")
    servers = data.get("iceServers")
    if isinstance(servers, dict):  # one server object instead of a list
        servers = [servers]
    if not isinstance(servers, list):
        raise TurnError("Cloudflare's TURN answer had no iceServers")
    out: list[dict] = []
    for server in servers:
        if not isinstance(server, dict):
            continue
        urls = server.get("urls")
        urls = [urls] if isinstance(urls, str) else urls
        if not isinstance(urls, list):
            continue
        kept = [u for u in urls if isinstance(u, str) and not _PORT_53.search(u)]
        if not kept:
            continue
        entry: dict[str, Any] = {"urls": kept}
        for field in ("username", "credential"):
            if isinstance(server.get(field), str):
                entry[field] = server[field]
        out.append(entry)
    if not any("username" in s for s in out):
        raise TurnError("Cloudflare's TURN answer had no relay")
    return out


def for_aiortc(servers: list[dict]) -> list[RTCIceServer]:
    """The same servers for the agent's end (`SmallWebRTCConnection(ice_servers=...)`)."""
    return [
        RTCIceServer(urls=s["urls"], username=s.get("username"), credential=s.get("credential"))
        for s in servers
    ]


class TurnError(RuntimeError):
    """No ICE servers for this call. The message names no secret."""
