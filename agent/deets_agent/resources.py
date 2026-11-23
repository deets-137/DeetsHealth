"""The Resources tab's numbers (CHAT.md §17): the voice's raw use from the dev call records, for
ehr to price (`voice_records`, `call_use`); and Claude's dollars this cycle for Config ›
Anthropic (`anthropic_cycle`).

The voice is never priced here: each record's `call_end` holds its raw use (`voice_use`, or the
older `cartesia`), and ehr turns it into credits and each vendor's cycle with the voice crate
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

from deets_agent import config

# A record's name holds its start time: test_20261113-142233.jsonl since schema 14, and
# 20261113-142233-test.jsonl before it (call_record.py).
_STAMP = re.compile(r"^(?:[a-z]+_)?(\d{8})-")


def cycle_start(today: date, day: int) -> date:
    """The first day of the credit cycle that holds `today`, when credits renew on `day`."""
    if today.day >= day:
        return today.replace(day=day)
    month, year = (today.month - 1, today.year) if today.month > 1 else (12, today.year - 1)
    return date(year, month, day)


def next_cycle(start: date) -> date:
    month, year = (start.month + 1, start.year) if start.month < 12 else (1, start.year + 1)
    return date(year, month, start.day)


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


def _cost(path: Path) -> float | None:
    """The `cost_usd` on a record's `call_end`: what its live Claude requests cost, or None
    (still open, or no end)."""
    try:
        with path.open(encoding="utf-8") as f:
            for raw in f:
                if '"call_end"' not in raw:
                    continue
                event = json.loads(raw)
                if event.get("kind") == "call_end" and isinstance(event.get("cost_usd"), (int, float)):
                    return float(event["cost_usd"])
    except (OSError, ValueError):
        return None
    return None


def anthropic_cycle(
    folder: Path,
    today: date | None = None,
    live: float | None = None,
    budget: float = config.BUDGET_USD,
    offset: float = 0.0,
    day: int = 1,
) -> dict[str, Any]:
    """This cycle's Claude spend in dollars (Config › Anthropic, CONSOLE.md §8): the `cost_usd` of
    every record in `folder` since the cycle began, plus the live call's so far. Like Cartesia's
    count, a floor: it cannot see requests outside these records (the Console's workbench,
    another machine). The Claude Console's usage page is the full count."""
    today = today or date.today()
    start = cycle_start(today, day)
    spent = 0.0
    calls = 0
    for path in sorted(folder.glob("*.jsonl")):
        m = _STAMP.match(path.name)
        if not m:
            continue
        try:
            when = datetime.strptime(m.group(1), "%Y%m%d").date()
        except ValueError:
            continue
        if when < start:
            continue
        cost = _cost(path)
        if cost is None:
            continue
        calls += 1
        spent += cost
    if live is not None:
        spent += live
    return {
        "budget": budget,
        "offset": offset,
        "spent": round(spent, 4),
        "left": round(budget - offset - spent, 4),
        "calls": calls,
        "cycle_start": start.isoformat(),
        "renews": next_cycle(start).isoformat(),
        "live": live,
    }


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
