"""Per-tool latency, worked out from a dev call record (CHAT.md §10; the owner, 2026-11-12: "an
API for now"; the analytics tab builds on it later).

One row per tool call, with names and numbers only: the tool, its Flow step, its spans, and
how far each span can be trusted. Never an argument, a result or a word of the call, so the
rows carry no protected data and fit the production call log's "latency per step"
(DEBUGGING.md §1) as they are.

The spans, all in milliseconds:
- `trigger_ms` — the caller's line → the model calls the tool. What the caller waits through
  first. Schema 3 records it exactly; older records give whole-ms event times.
- `model_ms` — the model alone: its request → the tool call in its reply (`first_tool_ms` on
  the `llm` event).
- `confirm_ms` — the tool's own run: its call → its result (`ms` on `tool_result`).
- `ehr_ms` — the part of that run spent in `ehr` calls (the `ehr` events inside it).
- `answer_ms` — the tool call → the first frame of the agent's next line: when a voice caller
  would start to hear the answer. Schema 4 only (`first_ms` on `agent`); older records stamp
  the line after the harness's turn grace, so they leave it empty rather than guess.
- `total_ms` — the caller's line → that first frame: the caller's whole wait. Schema 4 only.

Rows from a replayed request (`replayed`) measure the replay cache, not the model: the
summary leaves them out unless asked.
"""

from __future__ import annotations

import math
from typing import Any

SPANS = ("trigger_ms", "model_ms", "confirm_ms", "ehr_ms", "answer_ms", "total_ms")


def tool_rows(call_id: str, events: list[dict[str, Any]]) -> list[dict[str, Any]]:
    """Every tool call of one record, in order, with its spans."""
    schema = 1
    model = source = None
    node = None
    caller_t: float | None = None
    llm: dict[str, Any] | None = None
    rows: list[dict[str, Any]] = []
    running: list[dict[str, Any]] = []  # called, no result yet
    unanswered: list[dict[str, Any]] = []  # called, no agent line yet
    for e in events:
        kind = e.get("kind")
        t = float(e.get("t_ms") or 0)
        if kind == "call_start":
            schema = int(e.get("schema_version") or 1)
            model, source = e.get("model"), e.get("source") or "typed"
        elif kind == "node":
            node = e.get("name")
        elif kind == "caller":
            caller_t = t
            llm = None
        elif kind == "llm":
            llm = e
        elif kind == "tool_call":
            trigger = e.get("trigger_ms")
            if trigger is None and caller_t is not None:
                trigger = round(t - caller_t, 1)
            row = {
                "call_id": call_id,
                "seq": len(rows) + 1,
                "t_ms": t,
                "tool": e.get("name"),
                "node": node,
                "model": model,
                "source": source,
                "schema_version": schema,
                "trigger_ms": trigger,
                "model_ms": (llm or {}).get("first_tool_ms"),
                "confirm_ms": None,
                "ehr_ms": None,
                "ehr_calls": [],
                "answer_ms": None,
                "total_ms": None,
                "replayed": bool((llm or {}).get("hit")),
                "exact": schema >= 3,
                "failed": False,
                "_caller_t": caller_t,
            }
            rows.append(row)
            running.append(row)
            unanswered.append(row)
        elif kind == "ehr" and running:
            # An ehr call while a tool runs belongs to the newest running tool.
            row = running[-1]
            row["ehr_ms"] = round((row["ehr_ms"] or 0) + float(e.get("ms") or 0), 1)
            row["ehr_calls"].append(e.get("call"))
        elif kind == "tool_result":
            match = next((r for r in running if r["tool"] == e.get("name")), None)
            if match is not None:
                running.remove(match)
                match["confirm_ms"] = e.get("ms")
                match["failed"] = bool(e.get("error"))
        elif kind == "agent":
            first = e.get("first_ms")
            for row in unanswered:
                if first is not None:
                    row["answer_ms"] = round(float(first) - row["t_ms"], 1)
                    if row["_caller_t"] is not None:
                        row["total_ms"] = round(float(first) - row["_caller_t"], 1)
            unanswered = []
    for row in rows:
        del row["_caller_t"]
    return rows


def percentile(values: list[float], p: float) -> float:
    """Nearest rank: the smallest value with at least p % of the values at or below it."""
    ordered = sorted(values)
    return ordered[max(0, math.ceil(p / 100 * len(ordered)) - 1)]


def summarize(rows: list[dict[str, Any]], with_replayed: bool = False) -> list[dict[str, Any]]:
    """One entry per tool and model: for each span, how many calls have it, and their p50, p90
    and max. Most-called tools first."""
    groups: dict[tuple[str, str], list[dict[str, Any]]] = {}
    for r in rows:
        if r["replayed"] and not with_replayed:
            continue
        groups.setdefault((r["tool"] or "", r["model"] or ""), []).append(r)
    out = []
    for (tool, model), rs in groups.items():
        spans = {}
        for span in SPANS:
            vals = [float(r[span]) for r in rs if r[span] is not None]
            spans[span] = (
                {"n": len(vals), "p50": percentile(vals, 50), "p90": percentile(vals, 90), "max": max(vals)}
                if vals
                else {"n": 0, "p50": None, "p90": None, "max": None}
            )
        out.append({"tool": tool, "model": model, "calls": len(rs), "failed": sum(r["failed"] for r in rs), "spans": spans})
    return sorted(out, key=lambda g: (-g["calls"], g["tool"]))
