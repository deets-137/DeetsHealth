"""The dev call record: one JSON Lines file per typed call, for tracing what went wrong
(DEBUGGING.md §2, "the transcript": synthetic calls only, in dev).

**Dev only.** Only the typed harness opens a record, and only against ehr's fake patients or
Epic's public sandbox test patients (fhir.epic.com › Test Data: public fakes). The
record holds what the PHI wall keeps out of the logs: both sides of the conversation, each tool
call with its arguments and its result, and the full text of library errors. That is safe only
because every caller here is synthetic. No real call may ever open one: the deploy has no path
to `CallRecord`. The files in `agent/calls/` are committed to the private repo (the owner,
2026-11-11), so they must stay synthetic: never type a real person's details into a typed call.

Each line is one event: `t_ms` (from the call's start; to a tenth of a millisecond since schema
4, whole before), `kind`, and its fields.
- `call_start`, `call_end` — the schema version, the call's id (the file's name), the source
  (`typed`, `test`), the scenario (a script's name, or none), the EHR it read (`ehr`: `local`
  for ehr's fakes, `epic` for Epic's public sandbox; schema 6), the seat (`text` or `voice`;
  schema 7), the mode, the model, its `llm` settings and `inference_geo` (schema 8), its
  `prompt_version` (schema 9), its `system_version` (schema 10), the `label` typed beside
  Start and `started_by` (`owner` or `guest`; schema 11); at the end, the totals and the estimated cost, and for a
  voice call `cartesia`: Ink's seconds, Sonic's characters and their credits (schema 7).
- `caller`, `agent` — what each side said. `first_ms` on `agent` (schema 4): when the line's
  first frame passed, on the call's clock; the event's own `t_ms` comes after the harness's
  turn grace (`TURN_GRACE_S` in call.py).
- `node` — the model's first reply in a new node (late by the model's time; never written for
  a closing node). Kept for the catalog's `reached`.
- `step` (schema 5) — the Flow moved to a node, stamped at the move: its `name`, its own
  `tools`, and the `global_tools` the model has in every node (flow.py `CallFlow._step`);
  `steps` and `closing_steps`, the Flow's whole list (flow.py `STEPS`, `CLOSING_STEPS`).
- `tool_call`, `tool_result` — a function the model called, its arguments, its result, its time.
  `trigger_ms` on the call: from the caller's last line to the call (schema 3). `ms` on the
  result: the tool's own run, from its call to its result. Both are measured with the
  high-resolution clock and kept to a tenth of a millisecond; `t_ms` is whole ms only.
- `llm` — one request to Claude: replayed or live, the request key, the tools offered, the
  thinking blocks dropped, time to first event, total time, tokens, or the error in full.
- `hospital` (schema 13) — the label of the hospital version the call opened with (Config ›
  Hospital, HOSPITAL-CONFIG.md), as ehr's OpenCall gave it. A label, never a value.
- `ehr` — one gRPC call to ehr: its name, time and gRPC outcome, and for `CheckCode` and
  `Book` the domain result (`pass` · `fail` · `locked`; `booked` · `slot_taken`). Never its
  values; those are in `tool_call` and `tool_result`. Since schema 5, `left` on `OpenCall`
  (every limit in full), `Identify`, `ResendCode` and `CheckCode`: the verification limits
  left after the call, as counts (`identify`, `resend`, `code`).
- `text` — a text message the fake Twilio took from ehr during the call (fake_twilio.py): the
  number it went to and its words, the code among them. Test calls only. A real call never
  has one: Twilio itself sends a real text, and nothing copies it here.
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


# 1: the records of 2026-11-11 (no id, source or scenario in `call_start`; no `result` on an
# `ehr` event). 2: since 2026-11-12. 3: since 2026-11-12, afternoon: `tool_call` has
# `trigger_ms`, and a tool's `ms` keeps a tenth of a millisecond (it was whole). 4: since
# 2026-11-12, evening: every `t_ms` keeps a tenth of a millisecond, and `agent` has
# `first_ms`. 5: since 2026-11-12, night: `step` events, stamped when the Flow moves, and
# `left` on the ehr events that spend a verification limit. 6: since 2026-11-12, late night:
# `ehr` on `call_start`, the EHR the call read (`local`, `epic`). 7: since 2026-11-13: `seat` on
# `call_start` (`text`, `voice`), and `cartesia` on a voice call's `call_end`. 8: since
# 2026-11-13: `llm` on `call_start` (effort, thinking, the token caps, the cache's life: Config ›
# Anthropic) and `inference_geo`; `cost_usd` counts the model's own prices and US-only
# inference (1.1×). 9: since 2026-11-13, evening: `prompt_version` on `call_start`, the label of
# the words the call ran with ("MM-DD | n", "Default", or "file"). 10: since 2026-11-13, night:
# `system_version` on `call_start`, the label of the System variables the call ran with
# (SYSTEM-PAGE.md §3.4). 11: since 2026-11-13, late night: `label` on `call_start` when the
# person typed one beside Start, and `started_by` (`owner` or `guest`) on a dev server call
# (docs/ops/DEMO.md §3.6). 12: since 2026-11-15, each `llm_request` holds what was sent to
# Claude, as a delta (the system prompt when it changed, the messages added since the last
# request), and `epic` events hold ehr's own requests to Epic, from CloseCall (AGENT.md §1).
# 13: since 2026-11-15, night: a `hospital` event after OpenCall, its `version` the label of the
# Config › Hospital version the call opened with ("MM-DD | n", "Default", or "file").
# The catalog reads all thirteen.
SCHEMA_VERSION = 13


def cost_usd(tokens: dict[str, int], model: str, cache_ttl: str = "5m", geo: bool = True) -> float:
    """Dollars for these tokens on `model` (config.LLM_MODELS: per 1M tokens, ANTHROPIC.md §3).
    A cache write costs 1.25× the input price for a 5-minute cache, 2× for 1 hour; a cache read
    is the model's own read price; US-only inference is 1.1× on all of it. An estimate for the
    record, not a bill."""
    price_in, price_out, price_read = config.LLM_MODELS.get(model, {}).get("price", (0.0, 0.0, 0.0))
    dollars = (
        tokens.get("input_tokens", 0) * price_in
        + tokens.get("cache_creation_input_tokens", 0) * price_in * config.CACHE_WRITE.get(cache_ttl, 1.25)
        + tokens.get("cache_read_input_tokens", 0) * price_read
        + tokens.get("output_tokens", 0) * price_out
    )
    return dollars / 1_000_000 * (config.GEO_PRICE if geo else 1.0)


def replay_cost(path: Path, model: str = config.MODEL) -> float:
    """What one recorded reply in tests/replays/ cost, from its usage events (cost_usd).
    0 when the file is missing or unreadable."""
    try:
        body = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return 0.0
    usage: dict[str, int] = {}
    events = body.get("events") or [{"message": body.get("message") or {}}]
    for e in events:
        u = (e.get("message") or {}).get("usage") or e.get("usage") or {}
        for name in ("input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"):
            if isinstance(u.get(name), int):
                usage[name] = u[name]
    return cost_usd(usage, model)


def _ms(seconds: float) -> float:
    """A span in milliseconds, to a tenth: a fake tool takes 1 to 4 ms, so whole ms hide it."""
    return round(seconds * 1000, 1)


def _bare(message: Any) -> Any:
    """A message without its cache markers and thinking blocks: what the turn said."""
    if not isinstance(message, dict) or not isinstance(message.get("content"), list):
        return message
    blocks = [
        {k: v for k, v in b.items() if k != "cache_control"} if isinstance(b, dict) else b
        for b in message["content"]
        if not (isinstance(b, dict) and b.get("type") in ("thinking", "redacted_thinking"))
    ]
    return {**message, "content": blocks}


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
        ehr: str = "local",
        seat: str = "text",
        llm: dict[str, Any] | None = None,
        prompt_version: str = "file",
        system_version: str = "file",
        label: str | None = None,
        started_by: str | None = None,
    ) -> None:
        """`llm`: the call's llm settings (Config › Anthropic, CONSOLE.md §8); the code's when
        none. Written on `call_start` (schema 8) and used for the cost. `label`: what the person
        typed beside Start; `started_by`: `owner` or `guest` (schema 11; docs/ops/DEMO.md
        §3.6). Never a guest's email: these files are committed."""
        llm = {**config.llm_defaults(), **(llm or {}), "model": model}
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
        self.cache_ttl = str(llm["cache_ttl"])
        # Every live request is sent US-only (llm.py), billed at 1.1×.
        self.geo = True
        self._tool_start: dict[str, float] = {}
        # What the last request to Claude held, so the next one records only what is new.
        self._sent_system: Any = None
        self._sent_messages: list[Any] = []
        self._last_caller: float | None = None
        self._answer_start: float | None = None
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
            ehr=ehr,
            seat=seat,
            mode=mode,
            model=model,
            llm={k: llm[k] for k in ("effort", "thinking", "max_tokens", "retry_max_tokens", "cache_ttl")},
            inference_geo=config.INFERENCE_GEO,
            prompt_version=prompt_version,
            system_version=system_version,
            started=datetime.now().isoformat(timespec="seconds"),
            **({"label": label} if label else {}),
            **({"started_by": started_by} if started_by else {}),
        )

    def event(self, kind: str, **fields: Any) -> None:
        line = {"t_ms": _ms(time.perf_counter() - self._start), "kind": kind, **fields}
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
        self._last_caller = time.perf_counter()
        self.event("caller", text=text)

    def answer_started(self) -> None:
        """The first frame of the agent's next line passed (call.py's TurnCollector)."""
        self._answer_start = time.perf_counter()

    def agent(self, text: str | None) -> None:
        fields: dict[str, Any] = {"text": text}
        if self._answer_start is not None:
            # When the line began, on the call's clock: the event itself is stamped after the
            # text harness's turn grace, which a voice caller never waits through.
            fields["first_ms"] = _ms(self._answer_start - self._start)
            self._answer_start = None
        self.event("agent", **fields)

    def node(self, name: str | None) -> None:
        if name and name != self._node:
            self._node = name
            self.event("node", name=name)

    def tool_call(self, call_id: str, name: str, arguments: Any) -> None:
        now = time.perf_counter()
        self._tool_start[call_id] = now
        fields: dict[str, Any] = {"name": name, "arguments": arguments}
        if self._last_caller is not None:
            # From the caller's line to the model's call: what the caller waits through first.
            fields["trigger_ms"] = _ms(now - self._last_caller)
        self.event("tool_call", **fields)

    def tool_result(self, call_id: str, name: str, result: Any, error: Any = None) -> None:
        start = self._tool_start.pop(call_id, None)
        fields: dict[str, Any] = {"name": name, "result": result}
        if start is not None:
            # The tool's own run, from its call to its result (the EHR, the text service).
            fields["ms"] = _ms(time.perf_counter() - start)
        if error:
            fields["error"] = str(error)
        self.event("tool_result", **fields)

    def llm_request(self, key: str, params: dict[str, Any], hit: bool, dropped: int) -> _LlmTrace:
        tools = [t.get("name") for t in params.get("tools") or [] if isinstance(t, dict)]
        messages = list(params.get("messages", []))
        fields = {
            "key": key[:8],
            "hit": hit,
            "messages": len(messages),
            "tools": tools,
            "thinking_dropped": dropped,
        }
        # What was sent, as a delta (schema 12): a test call's record keeps the full text, like
        # the rest of it (REGULATIONS.md §8); synthetic callers only. A request that does not
        # extend the last one (a step change rewrote the context) is kept whole.
        system = params.get("system")
        if system != self._sent_system:
            fields["system"] = system
            self._sent_system = system
        # Compared without the cache markers and thinking blocks, which move from turn to turn.
        messages = [_bare(m) for m in messages]
        n = len(self._sent_messages)
        if messages[:n] == self._sent_messages:
            fields["new_messages"] = messages[n:]
        else:
            fields["all_messages"] = messages
        self._sent_messages = messages
        return _LlmTrace(self, fields)

    def epic(self, entry: dict[str, Any]) -> None:
        """One of ehr's requests to Epic on this call (CloseCall's ledger): names and counts only."""
        self.event("epic", **entry)

    def ehr(
        self, name: str, ms: float, outcome: str, result: str | None = None, left: dict[str, int] | None = None
    ) -> None:
        fields: dict[str, Any] = {"call": name, "ms": ms, "outcome": outcome}
        if result is not None:
            fields["result"] = result
        if left is not None:
            fields["left"] = left
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
        return cost_usd(self.tokens, self._model, self.cache_ttl, self.geo)

    def summary(self) -> str:
        r = self.requests
        return (
            f"{self.turns} caller turns - Claude requests: {r['live']} live, {r['replayed']} replayed, "
            f"{r['failed']} failed - {self.tokens['output_tokens']} tokens out - about ${self.cost():.4f}"
        )

    def close(self, reason: str, cartesia: dict[str, Any] | None = None) -> None:
        """`cartesia`: a voice call's use of Ink and Sonic (voice.py `VoiceMeter.totals`)."""
        if self._file.closed:
            return
        fields: dict[str, Any] = {}
        if cartesia is not None:
            fields["cartesia"] = cartesia
        self.event("call_end", reason=reason, turns=self.turns, requests=self.requests,
                   tokens=self.tokens, cost_usd=round(self.cost(), 6), **fields)
        self._file.close()
