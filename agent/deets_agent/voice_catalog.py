"""Cartesia's voice list, held by the dev server for Config › Cartesia (CARTESIA.md §3; the
owner, 2026-11-16: fork A, the shortlist checked live and every English voice in a table).

`GET /voices` (docs.cartesia.ai › Voices › List) costs no credits. The list is read at most once
every `TTL_S`, one read at a time; the page never waits on Cartesia for its settings, only for
this list, through its own route. A failed read keeps the list held before it, with its age.

What comes back is the catalog's public facts only: id, name, tagline, description, gender, the
accents. Never the key. Voices our organization owns, and pro clones, are left out: no voice
cloning (CARTESIA.md §1, outside ZDR).
"""

from __future__ import annotations

import asyncio
import time
from datetime import UTC, datetime
from typing import Any

import aiohttp

# Cartesia's List Voices takes only this API version (docs.cartesia.ai › Voices › List, read
# 2026-11-16); Pipecat's speech sockets use their own.
VERSION = "2026-08-14"
URL = "https://api.cartesia.ai/voices"
# How long a read is kept before the next page open reads again.
TTL_S = 10 * 60
# Refresh reads again only when the held list is older than this: a stuck button cannot flood
# Cartesia.
FRESH_GAP_S = 15
# 100 a page is Cartesia's most. The cap stops a paging loop: 30 pages is 3,000 voices.
PAGE = 100
MAX_PAGES = 30
# The longest description kept: the table shows it in the row's help.
MAX_TEXT = 400


class VoiceCatalog:
    def __init__(self, key: str | None) -> None:
        self._key = key
        self._voices: list[dict[str, Any]] | None = None
        self._at: float | None = None  # time.time() of the last good read
        self._tried: float = 0.0  # time.monotonic() of the last read, good or not
        self._fault: str | None = None
        self._lock = asyncio.Lock()

    @property
    def held(self) -> list[dict[str, Any]] | None:
        return self._voices

    def find(self, voice_id: str) -> dict[str, Any] | None:
        return next((v for v in self._voices or [] if v["id"] == voice_id), None)

    async def view(self, fresh: bool = False) -> dict[str, Any]:
        """The held list, read again first when it is older than TTL_S (or FRESH_GAP_S, for
        `fresh`). `fault`: why the last read failed, as Cartesia's status or "no answer"."""
        if self._key is not None:
            async with self._lock:
                age = time.monotonic() - self._tried
                # No list yet (the first open, or every read so far failed): the short gap.
                wait = FRESH_GAP_S if fresh or self._voices is None else TTL_S
                if self._tried == 0.0 or age > wait:
                    await self._read()
        return {
            "key": self._key is not None,
            "checked_at": datetime.fromtimestamp(self._at, UTC).isoformat().replace("+00:00", "Z") if self._at else None,
            "fault": self._fault,
            "ttl_s": TTL_S,
            "voices": self._voices or [],
        }

    async def _read(self) -> None:
        self._tried = time.monotonic()
        headers = {"Authorization": f"Bearer {self._key}", "Cartesia-Version": VERSION}
        out: list[dict[str, Any]] = []
        after: str | None = None
        try:
            async with aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=30)) as s:
                for _ in range(MAX_PAGES):
                    params = {"limit": str(PAGE), "language": "en"}
                    if after:
                        params["starting_after"] = after
                    async with s.get(URL, params=params, headers=headers) as r:
                        if r.status != 200:
                            self._fault = f"Cartesia answered {r.status}"
                            return
                        body = await r.json()
                    page = body.get("data") or []
                    out.extend(v for v in (_shape(x) for x in page) if v is not None)
                    after = body.get("next_page") or (page[-1].get("id") if page else None)
                    if not body.get("has_more") or not page or not after:
                        break
        except (aiohttp.ClientError, TimeoutError):
            self._fault = "Cartesia did not answer"
            return
        except ValueError:
            self._fault = "Cartesia's answer was not JSON"
            return
        # No voice is never a good read (a filter or a shape we got wrong): keep what was held.
        if not out:
            self._fault = "Cartesia listed no English voices"
            return
        # One row per id, in name order (a page may repeat one at its edge).
        seen: dict[str, dict[str, Any]] = {}
        for v in out:
            seen.setdefault(v["id"], v)
        self._voices = sorted(seen.values(), key=lambda v: (v["name"].lower(), v["id"]))
        self._at = time.time()
        self._fault = None


def _text(x: Any, limit: int = MAX_TEXT) -> str:
    return str(x).strip()[:limit] if isinstance(x, str) else ""


def _shape(v: Any) -> dict[str, Any] | None:
    """One voice as the page shows it, or None: not a voice, archived, ours, or a clone."""
    if not isinstance(v, dict) or not isinstance(v.get("id"), str) or not isinstance(v.get("name"), str):
        return None
    if v.get("status", "active") != "active" or v.get("is_owner") or v.get("is_pro"):
        return None
    accents = [a for a in v.get("accents") or [] if isinstance(a, dict) and isinstance(a.get("locale"), str)]
    native = next((a["locale"] for a in accents if a.get("is_native")), None)
    locales = sorted({a["locale"] for a in accents})
    if not locales and isinstance(v.get("language"), str):
        locales = [v["language"]]
    gender = v.get("gender")
    return {
        "id": v["id"],
        "name": _text(v["name"], 80),
        "tagline": _text(v.get("tagline"), 64),
        "description": _text(v.get("description")),
        "gender": gender if gender in ("masculine", "feminine", "gender_neutral") else None,
        "accent": native or (locales[0] if locales else None),
        "locales": locales,
    }
