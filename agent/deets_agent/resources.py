"""The Resources tab's numbers (CHAT.md §17): the voice's and the model's raw use from the dev call
records (`voice_records`, `model_records`, `call_use`), for ehr to price with the vendors crate
(`PriceVoiceUse`, `PriceModelUse`; VOICE-VENDORS.md §9, §10). Nothing is priced here.

The voice is never priced here: each record's `call_end` holds its raw use (`voice_use`, or the
older `cartesia`), and ehr turns it into credits and each vendor's cycle with the vendors crate
(`PriceVoiceUse`; VOICE-VENDORS.md §9). No vendor has an API that reports usage (CARTESIA.md
§4), so the count is a floor; the vendor's dashboard is the full count. The page says so.

Nothing here is protected data: counts and dates only.
"""

from __future__ import annotations

import json
import re
from datetime import date, datetime
from pathlib import Path
from typing import Any

from deets_agent import models

# A record's name holds its start time: test_20261113-142233.jsonl since schema 14, and
# 20261113-142233-test.jsonl before it (call_record.py).
_STAMP = re.compile(r"^(?:[a-z]+_)?(\d{8})-")


def voice_part(end: dict[str, Any]) -> dict[str, Any] | None:
    """The voice's raw use on a `call_end`, as ehr reads it: `voice_use` (schema 16) or the older
    `cartesia` (schema 7 to 15). None for a text call."""
    part = {k: end[k] for k in ("voice_use", "cartesia") if isinstance(end.get(k), dict)}
    return part or None


def _voice(path: Path) -> dict[str, Any] | None:
    """The voice's raw use on a record's `call_end`, or None (a text call, or still open)."""
    try:
        with path.open(encoding="utf-8") as f:
            for raw in f:
                if '"call_end"' not in raw:
                    continue
                event = json.loads(raw)
                if event.get("kind") == "call_end":
                    return voice_part(event)
    except (OSError, ValueError):
        return None
    return None


def voice_records(folder: Path, since: date | None = None) -> list[dict[str, Any]]:
    """Every voice call's raw use in `folder` (from `since`, when given), for `PriceVoiceUse`:
    `{id, day, use}`. The id is the record's name; counts and dates only."""
    out: list[dict[str, Any]] = []
    for path in sorted(folder.glob("*.jsonl")):
        m = _STAMP.match(path.name)
        if not m:
            continue
        try:
            when = datetime.strptime(m.group(1), "%Y%m%d").date()
        except ValueError:
            continue
        if since is not None and when < since:
            continue
        use = _voice(path)
        if use is not None:
            out.append({"id": path.stem, "day": when.isoformat(), "use": use})
    return out


def model_part(start: dict[str, Any], end: dict[str, Any]) -> dict[str, Any]:
    """The model's use on a record, for `PriceModelUse`: `{usd}` when the record holds its dollars
    (priced by ehr when it closed, or by the code before 2026-11-23), else `{use}`, its tokens
    with the model, the cache's life and the place, for ehr to price now."""
    if isinstance(end.get("cost_usd"), (int, float)):
        return {"usd": float(end["cost_usd"])}
    llm = start.get("llm") if isinstance(start.get("llm"), dict) else {}
    return {
        "use": models.use(
            str(start.get("model") or llm.get("model") or ""),
            str(llm.get("cache_ttl") or "5m"),
            str(start.get("inference_geo") or ""),
            end.get("tokens") if isinstance(end.get("tokens"), dict) else {},
        )
    }


def _model(path: Path) -> dict[str, Any] | None:
    """The model's use on a record (`model_part`), or None (still open, or no end)."""
    start: dict[str, Any] = {}
    try:
        with path.open(encoding="utf-8") as f:
            for raw in f:
                if '"call_start"' in raw and not start:
                    event = json.loads(raw)
                    if event.get("kind") == "call_start":
                        start = event
                elif '"call_end"' in raw:
                    event = json.loads(raw)
                    if event.get("kind") == "call_end":
                        return model_part(start, event)
    except (OSError, ValueError):
        return None
    return None


def model_records(folder: Path) -> list[dict[str, Any]]:
    """Every ended call's model use in `folder`, for `PriceModelUse`: `{id, day, usd | use}`.
    ehr sums this cycle's (the budget's cycle, model/cost.rs). Like the voice's count, a floor: it
    cannot see requests outside these records; the Claude Console's usage page is the full
    count. The id is the record's name; counts, dollars and dates only."""
    out: list[dict[str, Any]] = []
    for path in sorted(folder.glob("*.jsonl")):
        m = _STAMP.match(path.name)
        if not m:
            continue
        try:
            when = datetime.strptime(m.group(1), "%Y%m%d").date()
        except ValueError:
            continue
        part = _model(path)
        if part is not None:
            out.append({"id": path.stem, "day": when.isoformat(), **part})
    return out


def call_use(events: list[dict[str, Any]]) -> dict[str, Any]:
    """One ended call's use, from its record's events (the Resources tab's "This call", CHAT.md
    §17): its seat, Claude's dollars (`call_end` › `cost_usd`) and the voice's raw use (`use`;
    None for a text call), which the dev server has ehr price. Counts only."""
    seat = next((e.get("seat") for e in events if e.get("kind") == "call_start"), None) or "text"
    end = next((e for e in reversed(events) if e.get("kind") == "call_end"), {})
    cost = end.get("cost_usd")
    return {
        "seat": seat,
        "usd": float(cost) if isinstance(cost, (int, float)) else 0.0,
        "use": voice_part(end),
        "live": False,
    }
