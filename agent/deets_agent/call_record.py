"""The dev call record: one JSON Lines file per typed call, for tracing what went wrong
(DEBUGGING.md §2, "the transcript": synthetic calls only, in dev).

**Dev only.** Only the typed harness opens a record, and only against ehr's fake patients. The
record holds what the PHI wall keeps out of the logs: both sides of the conversation, each tool
call with its arguments and its result, and the full text of library errors. That is safe only
because every caller here is synthetic. No real call may ever open one: the deploy has no path
to `CallRecord`. The files in `agent/calls/` are committed to the private repo (the owner,
2026-11-11), so they must stay synthetic: never type a real person's details into a typed call.

Each line is one event: `t_ms` (from the call's start), `kind`, and its fields.
- `call_start`, `call_end` — the mode, the model, the totals and the estimated cost.
- `caller`, `agent` — what each side said.
- `node` — the Flow moved to a new node.
- `tool_call`, `tool_result` — a function the model called, its arguments, its result, its time.
- `llm` — one request to Claude: replayed or live, the request key, the tools offered, the
  thinking blocks dropped, time to first event, total time, tokens, or the error in full.
- `ehr` — one gRPC call to ehr: its name, time and outcome (never its values; those are in
  `tool_call` and `tool_result`).
- `log` — a library warning or error, with its text (the console shows it withheld).
- `error` — the harness itself failed.
"""

from __future__ import annotations

import json
import time
from datetime import datetime
from pathlib import Path
from typing import Any

from deets_agent import config

CALLS = config.AGENT / "calls"

# Haiku 5.5, per 1M tokens (integrations/ANTHROPIC.md §3). Cache writes are billed at 1.25× the
# input price and cache reads at 0.1×. An estimate for the record, not a bill.
PRICES = {"claude-haiku-5-5": (0.10, 0.50)}


class _LlmTrace:
    def __init__(self, record: CallRecord, fields: dict[str, Any]) -> None:
        self._record = record
        self._fields = fields
        self._start = time.perf_counter()
        self._first: float | None = None
        self._closed = False

    def _ms(self, at: float) -> int:
        return round((at - self._start) * 1000)

    def first_event(self) -> None:
        if self._first is None:
            self._first = time.perf_counter()

    def done(self, usage: dict[str, int], replayed: bool) -> None:
        if self._closed:
            return
        self._closed = True
        now = time.perf_counter()
        fields = dict(self._fields, usage=usage, total_ms=self._ms(now))
        if self._first is not None:
            fields["first_event_ms"] = self._ms(self._first)
        if not replayed:
            self._record.add_spend(usage)
        self._record.event("llm", **fields)

    def failed(self, e: BaseException) -> None:
        if self._closed:
            return
        self._closed = True
        error = {"type": type(e).__name__, "message": str(e)}
        for attr in ("status_code", "request_id"):
            if getattr(e, attr, None) is not None:
                error[attr] = getattr(e, attr)
        self._record.event("llm", **dict(self._fields, total_ms=self._ms(time.perf_counter()), error=error))


class CallRecord:
    def __init__(self, mode: str, model: str, folder: Path = CALLS) -> None:
        folder.mkdir(parents=True, exist_ok=True)
        stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
        self.path = folder / f"{stamp}-typed.jsonl"
        self._file = self.path.open("a", encoding="utf-8")
        self._start = time.perf_counter()
        self._model = model
        self._tool_start: dict[str, float] = {}
        self._node: str | None = None
        self.turns = 0
        self.requests = {"live": 0, "replayed": 0, "failed": 0}
        self.tokens = {"input_tokens": 0, "output_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0}
        self.event("call_start", mode=mode, model=model, started=datetime.now().isoformat(timespec="seconds"))

    def event(self, kind: str, **fields: Any) -> None:
        line = {"t_ms": round((time.perf_counter() - self._start) * 1000), "kind": kind, **fields}
        if kind == "llm":
            self.requests["failed" if "error" in fields else "replayed" if fields.get("hit") else "live"] += 1
        self._file.write(json.dumps(line, default=str, ensure_ascii=False) + "\n")
        self._file.flush()

    # ---- the hooks ----

    def caller(self, text: str) -> None:
        self.turns += 1
        self.event("caller", text=text)

    def agent(self, text: str | None) -> None:
        self.event("agent", text=text)

    def node(self, name: str | None) -> None:
        if name and name != self._node:
            self._node = name
            self.event("node", name=name)

    def tool_call(self, call_id: str, name: str, arguments: Any) -> None:
        self._tool_start[call_id] = time.perf_counter()
        self.event("tool_call", name=name, arguments=arguments)

    def tool_result(self, call_id: str, name: str, result: Any, error: Any = None) -> None:
        start = self._tool_start.pop(call_id, None)
        fields: dict[str, Any] = {"name": name, "result": result}
        if start is not None:
            fields["ms"] = round((time.perf_counter() - start) * 1000)
        if error:
            fields["error"] = str(error)
        self.event("tool_result", **fields)

    def llm_request(self, key: str, params: dict[str, Any], hit: bool, dropped: int) -> _LlmTrace:
        tools = [t.get("name") for t in params.get("tools") or [] if isinstance(t, dict)]
        fields = {
            "key": key[:8],
            "hit": hit,
            "messages": len(params.get("messages", [])),
            "tools": tools,
            "thinking_dropped": dropped,
        }
        return _LlmTrace(self, fields)

    def ehr(self, name: str, ms: int, outcome: str) -> None:
        self.event("ehr", call=name, ms=ms, outcome=outcome)

    def log(self, level: str, where: str, text: str, exc: str | None) -> None:
        fields = {"level": level, "where": where, "text": text}
        if exc:
            fields["exception"] = exc
        self.event("log", **fields)

    def add_spend(self, usage: dict[str, int]) -> None:
        for name in self.tokens:
            self.tokens[name] += usage.get(name, 0) or 0

    # ---- the end ----

    def cost(self) -> float:
        price_in, price_out = PRICES.get(self._model, (0.0, 0.0))
        t = self.tokens
        dollars = (
            t["input_tokens"] * price_in
            + t["cache_creation_input_tokens"] * price_in * 1.25
            + t["cache_read_input_tokens"] * price_in * 0.1
            + t["output_tokens"] * price_out
        )
        return dollars / 1_000_000

    def summary(self) -> str:
        r = self.requests
        return (
            f"{self.turns} caller turns - Claude requests: {r['live']} live, {r['replayed']} replayed, "
            f"{r['failed']} failed - {self.tokens['output_tokens']} tokens out - about ${self.cost():.4f}"
        )

    def close(self, reason: str) -> None:
        if self._file.closed:
            return
        self.event("call_end", reason=reason, turns=self.turns, requests=self.requests,
                   tokens=self.tokens, cost_usd=round(self.cost(), 6))
        self._file.close()
