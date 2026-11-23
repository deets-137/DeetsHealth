"""The voice catalog as the agent reads it (VOICE-VENDORS.md §2.1; the owner's V-1 A: ehr serves
it). The catalog itself lives in the `deetshealth-vendors` crate (`vendors/catalog/voice/*.json`); the
agent never keeps a copy of a lever, a range or a rate.

- `load(ehr)` asks ehr (`GetVoiceCatalog`) once it is up.
- Before that, and in the tests, `current()` asks the ehr binary itself (`ehr voice-catalog`):
  the same compiled-in catalog, with no service running.

A voice version's keys (VOICE-VENDORS.md §2.3): `hearing.vendor`, `speaking.vendor`; the shared
levers bare (`barge_in`); each vendor's levers under its id (`cartesia.speed`). No value here is
protected data.
"""

from __future__ import annotations

import json
import os
import subprocess
from typing import Any

from dotenv import load_dotenv

from deets_agent import config

DIRECTIONS = ("hearing", "speaking")

_catalog: dict[str, Any] | None = None


async def load(ehr) -> dict[str, Any]:
    """Read the catalog from ehr and hold it. ehr's own copy wins over the binary's."""
    global _catalog
    _catalog = await ehr.voice_catalog()
    return _catalog


def current() -> dict[str, Any]:
    """The held catalog; the first time with none, the ehr binary's (`ehr voice-catalog`)."""
    global _catalog
    if _catalog is None:
        out = subprocess.run(
            [str(config.EHR_BINARY), "voice-catalog"], capture_output=True, text=True, timeout=30, check=False
        )
        if out.returncode != 0:
            raise RuntimeError(f"ehr voice-catalog: exit {out.returncode}")
        _catalog = json.loads(out.stdout)
    return _catalog


def vendor(vid: str, catalog: dict[str, Any] | None = None) -> dict[str, Any]:
    c = catalog or current()
    for v in c["vendors"]:
        if v["id"] == vid:
            return v
    raise KeyError(vid)


def offering(direction: str, catalog: dict[str, Any] | None = None) -> list[dict[str, Any]]:
    """The vendors that offer `direction`, in the catalog's order."""
    return [v for v in (catalog or current())["vendors"] if v.get(direction)]


def chosen(values: dict[str, Any], direction: str) -> str:
    """The vendor id a version chose for `direction`."""
    return str(values[f"{direction}.vendor"])


def own(values: dict[str, Any], vid: str) -> dict[str, Any]:
    """A vendor's own levers from a version, without the prefix: `cartesia.speed` → `speed`."""
    prefix = f"{vid}."
    return {k[len(prefix) :]: v for k, v in values.items() if k.startswith(prefix)}


def key(vid: str, catalog: dict[str, Any] | None = None) -> str | None:
    """A vendor's API key from the git-ignored .env, by the name its catalog file gives. Never
    printed, never logged."""
    load_dotenv(config.REPO / ".env", override=False)
    name = vendor(vid, catalog)["key_env"]
    got = os.environ.get(name, "").strip()
    return got or None


def keys_missing(values: dict[str, Any], catalog: dict[str, Any] | None = None) -> list[str]:
    """The `.env` names of the keys the chosen vendors need and lack (names only, never a key)."""
    out: list[str] = []
    for d in DIRECTIONS:
        vid = chosen(values, d)
        if key(vid, catalog) is None:
            name = vendor(vid, catalog)["key_env"]
            if name not in out:
                out.append(name)
    return out


def _env_int(name: str, low: int, high: int) -> int | None:
    load_dotenv(config.REPO / ".env", override=False)
    try:
        value = int(os.environ.get(name, "").strip())
    except ValueError:
        return None
    return value if low <= value <= high else None


def defaults(catalog: dict[str, Any] | None = None) -> dict[str, Any]:
    """Every voice setting at its first value (the catalog's `defaults`), with what `.env` sets
    for each vendor laid on it: `<ID>_VOICE_ID` (the voice), `<ID>_PLAN_CREDITS`,
    `<ID>_CYCLE_DAY`. This is the Default version and what a call runs with before any save."""
    c = catalog or current()
    out = dict(c["defaults"])
    load_dotenv(config.REPO / ".env", override=False)
    for v in c["vendors"]:
        env = v["id"].upper()
        vid = os.environ.get(f"{env}_VOICE_ID", "").strip()
        if vid and f"{v['id']}.voice" in out:
            known = {x.get("id"): x["name"] for x in (v.get("speaking") or {}).get("voices", [])}
            out[f"{v['id']}.voice"] = {"name": known.get(vid, vid), "id": vid}
        credits = _env_int(f"{env}_PLAN_CREDITS", 1, 1_000_000_000)
        if credits is not None and f"{v['id']}.plan_credits" in out:
            out[f"{v['id']}.plan_credits"] = credits
        day = _env_int(f"{env}_CYCLE_DAY", 1, 28)
        if day is not None and f"{v['id']}.cycle_day" in out:
            out[f"{v['id']}.cycle_day"] = day
    return out
