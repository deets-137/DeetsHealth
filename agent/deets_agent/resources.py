"""The Resources tab's numbers (CHAT.md §17): Cartesia's credits this cycle, counted from the
dev call records.

Cartesia has no API that reports usage or a balance (CARTESIA.md §4), so this counts what our
own voice calls used: each record's `cartesia` totals on `call_end` (schema 7), and the live
call's meter. It cannot see use outside these calls (Cartesia's playground, another machine), so
it is a floor, and Cartesia's dashboard is the full count. The page says so.

Nothing here is protected data: counts and dates only.
"""

from __future__ import annotations

import json
import re
from datetime import date, datetime
from pathlib import Path
from typing import Any

from deets_agent import config

# A record's name starts with its start time: 20261113-142233-test.jsonl (call_record.py).
_STAMP = re.compile(r"^(\d{8})-")


def cycle_start(today: date, day: int) -> date:
    """The first day of the credit cycle that holds `today`, when credits renew on `day`."""
    if today.day >= day:
        return today.replace(day=day)
    month, year = (today.month - 1, today.year) if today.month > 1 else (12, today.year - 1)
    return date(year, month, day)


def next_cycle(start: date) -> date:
    month, year = (start.month + 1, start.year) if start.month < 12 else (1, start.year + 1)
    return date(year, month, start.day)


def _cartesia(path: Path) -> dict[str, Any] | None:
    """The `cartesia` totals on a record's `call_end`, or None (a text call, or still open)."""
    try:
        with path.open(encoding="utf-8") as f:
            for raw in f:
                if '"call_end"' not in raw:
                    continue
                event = json.loads(raw)
                if event.get("kind") == "call_end" and isinstance(event.get("cartesia"), dict):
                    return event["cartesia"]
    except (OSError, ValueError):
        return None
    return None


def cartesia_cycle(folder: Path, today: date | None = None, live: dict[str, Any] | None = None) -> dict[str, Any]:
    """This cycle's credits from the records in `folder`, plus the live call's meter."""
    today = today or date.today()
    start = cycle_start(today, config.cycle_day())
    ink_s = 0.0
    sonic_chars = 0
    used = 0
    calls = 0
    for path in sorted(folder.glob("*.jsonl")):
        m = _STAMP.match(path.name)
        if not m:
            continue
        try:
            day = datetime.strptime(m.group(1), "%Y%m%d").date()
        except ValueError:
            continue
        if day < start:
            continue
        totals = _cartesia(path)
        if totals is None:
            continue
        calls += 1
        ink_s += float(totals.get("ink_s") or 0)
        sonic_chars += int(totals.get("sonic_chars") or 0)
        used += int(totals.get("credits") or 0)
    if live is not None:
        used += int(live.get("credits") or 0)
    plan = config.plan_credits()
    return {
        "plan": plan,
        "used": used,
        "left": max(0, plan - used),
        "calls": calls,
        "ink_s": round(ink_s, 1),
        "sonic_chars": sonic_chars,
        "cycle_start": start.isoformat(),
        "renews": next_cycle(start).isoformat(),
        "live": live,
        "rates": {"ink_per_s": config.INK_CREDITS_PER_S, "sonic_per_char": config.SONIC_CREDITS_PER_CHAR},
        "limits": {"idle_s": config.VOICE_IDLE_S, "max_s": config.VOICE_MAX_S},
    }
