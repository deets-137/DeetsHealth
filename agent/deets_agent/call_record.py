"""The dev call record: one JSON Lines file per typed call, for tracing what went wrong
(DEBUGGING.md §2, "the transcript": synthetic calls only, in dev).

**Dev only.** Only the typed harness opens a record, and only against ehr's fake patients. The
record holds what the PHI wall keeps out of the logs: both sides of the conversation, each tool
call with its arguments and its result, and the full text of library errors. That is safe only
because every caller here is synthetic. No real call may ever open one: the deploy has no path
to `CallRecord`. The files in `agent/calls/` are committed to the private repo (the owner,
2026-11-11), so they must stay synthetic: never type a real person's details into a typed call.

Each line is one event: `t_ms` (from the call's start), `kind`, and its fields.
- `call_start`, `call_end` — the schema version, the call's id (the file's name), the source
  (`typed`, `test`), the scenario (a script's name, or none), the mode, the model; at the end,
  the totals and the estimated cost.
- `caller`, `agent` — what each side said.
- `node` — the Flow moved to a new node.
- `tool_call`, `tool_result` — a function the model called, its arguments, its result, its time.
- `llm` — one request to Claude: replayed or live, the request key, the tools offered, the
  thinking blocks dropped, time to first event, total time, tokens, or the error in full.
- `ehr` — one gRPC call to ehr: its name, time and gRPC outcome, and for `CheckCode` and
  `Book` the domain result (`pass` · `fail` · `locked`; `booked` · `slot_taken`). Never its
  values; those are in `tool_call` and `tool_result`.
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

# 1: the records of 2026-11-11 (no id, source or scenario in `call_start`; no `result` on an
# `ehr` event). 2: since 2026-11-12. The catalog reads both (catalog.py).
SCHEMA_VERSION = 2


class _LlmTrace:
    def __init__(self, record: CallRecord, fields: dict[str, Any]) -> None:
        self._record = record
        self._fields = fields
        self._start = time.perf_counter()
        self._first: float | None = None
        # The first spoken word and the first tool call. With thinking on, the first event can
        # be a thinking block, so `first_event_ms` is not what a caller waits for; on a voice call
        # `first_text_ms` is (Phase 3 measures it).
        self._first_text: float | None = None
        self._first_tool: float | None = None
        self._closed = False

    def _ms(self, at: float) -> int:
        return round((at - self._start) * 1000)

    def first_event(self) -> None:
        if self._first is None:
            self._first = time.perf_counter()

    def see(self, event) -> None:
        """Note the time of the first text and the first tool call in a live reply."""
        kind = getattr(event, "type", None)
        delta = getattr(getattr(event, "delta", None), "type", None)
        block = getattr(getattr(event, "content_block", None), "type", None)
        if self._first_text is None and kind == "content_block_delta" and delta == "text_delta":
            self._first_text = time.perf_counter()
        if self._first_tool is None and kind == "content_block_start" and block == "tool_use":
            self._first_tool = time.perf_counter()

    def done(self, usage: dict[str, int], replayed: bool) -> None:
        if self._closed:
            return
        self._closed = True
        now = time.perf_counter()
        fields = dict(self._fields, usage=usage, total_ms=self._ms(now))
        if self._first is not None:
            fields["first_event_ms"] = self._ms(self._first)
        if self._first_text is not None:
            fields["first_text_ms"] = self._ms(self._first_text)
        if self._first_tool is not None:
            fields["first_tool_ms"] = self._ms(self._first_tool)
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
    def __init__(
        self,
        mode: str,
        model: str,
        folder: Path = CALLS,
        source: str = "typed",
        scenario: str | None = None,
    ) -> None:
        folder.mkdir(parents=True, exist_ok=True)
        stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
        # The file's name is the call's id in the catalog (catalog.py). Two calls in one second
        # get a suffix.
        self.path = folder / f"{stamp}-{source}.jsonl"
        n = 2
        while self.path.exists():
            self.path = folder / f"{stamp}-{source}-{n}.jsonl"
            n += 1
        self.call_id = self.path.stem
        self._file = self.path.open("x", encoding="utf-8")
        # Each event also goes to these, as it is written: the dev server's watch stream.
        self.listeners: list = []
        self._start = time.perf_counter()
        self._model = model
        self._tool_start: dict[str, float] = {}
        self._node: str | None = None
        self.turns = 0
        self.requests = {"live": 0, "replayed": 0, "failed": 0}
        self.tokens = {"input_tokens": 0, "output_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0}
        self.event(
            "call_start",
            schema_version=SCHEMA_VERSION,
            call_id=self.call_id,
            source=source,
            scenario=scenario,
            mode=mode,
            model=model,
            started=datetime.now().isoformat(timespec="seconds"),
        )

    def event(self, kind: str, **fields: Any) -> None:
        line = {"t_ms": round((time.perf_counter() - self._start) * 1000), "kind": kind, **fields}
        if kind == "llm":
            self.requests["failed" if "error" in fields else "replayed" if fields.get("hit") else "live"] += 1
        text = json.dumps(line, default=str, ensure_ascii=False)
        self._file.write(text + "\n")
        self._file.flush()
        for listener in self.listeners:
            listener(text)

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

    def ehr(self, name: str, ms: int, outcome: str, result: str | None = None) -> None:
        fields: dict[str, Any] = {"call": name, "ms": ms, "outcome": outcome}
        if result is not None:
            fields["result"] = result
        self.event("ehr", **fields)

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
