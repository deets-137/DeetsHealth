"""Claude for the call: Pipecat's Anthropic service behind our own client (BUILD-ORDER.md
Phase 1; VOICE-AGENT.md §8.2, §8.3).

The client does two jobs, at the one place every request passes:

1. **GA only.** Pipecat calls `client.beta.messages.create(..., betas=["interleaved-thinking-
   2025-05-14"])`. Beta features are reported as outside the BAA (VOICE-AGENT.md §8.2), so the
   PHI path stays on GA: the `betas` list is dropped and the request goes to the GA
   `messages.create`. Current models interleave thinking with tools without that flag.
2. **Record and replay.** Each request is keyed by a hash of its exact parameters. A reply on
   disk is replayed: no network, no tokens. The $100-a-month budget (BUILD-ORDER.md Phase 1)
   depends on it.
   - `replay`: a miss is an error. Tests run this way, so a test never spends a token.
   - `record`: a miss calls Claude once and keeps the reply. The typed harness runs this way.
   - `live`: no cache.
   Recordings come from typed or simulated calls against ehr's fake patients only; they live in
   tests/replays/ and are committed, so a fresh clone replays for free.

3. **Thinking blocks that no longer fit are dropped.** A thinking block is bound to the tool
   list it was made with, and each Flow node has its own tools. Sent back after a node change,
   the block is refused (400, "the `tools` list differs"), and the call dies. The client keeps
   each block's signature with the tool list of the request that made it, and drops a block
   whose tool list differs from the request's. Anthropic's fix for this is a beta header,
   which the GA rule above rules out.

Every live request logs its token counts (never its content), so the spend is visible.
"""

from __future__ import annotations

import enum
import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
from typing import Any

from anthropic import AsyncAnthropic, NotGiven, Omit
from anthropic.types import Message, RawMessageStreamEvent
from loguru import logger
from pipecat.services.anthropic.llm import AnthropicLLMService
from pydantic import TypeAdapter

from deets_agent import config

_EVENT = TypeAdapter(RawMessageStreamEvent)


class Mode(enum.Enum):
    REPLAY = "replay"
    RECORD = "record"
    LIVE = "live"


class ReplayMiss(Exception):
    """A request with no recording, in replay mode. The message names the key only."""


def request_key(params: dict[str, Any]) -> str:
    """The hash of the exact request. Any change to the prompt, the tools, the model or the
    conversation is a new key."""
    canon = json.dumps(params, sort_keys=True, separators=(",", ":"), default=str)
    return hashlib.sha256(canon.encode()).hexdigest()[:32]


def _clean(params: dict[str, Any]) -> dict[str, Any]:
    params = dict(params)
    params.pop("betas", None)
    return {k: v for k, v in params.items() if not isinstance(v, (NotGiven, Omit))}


def tools_key(params: dict[str, Any]) -> str:
    """The hash of a request's tool list: what a thinking block is bound to."""
    canon = json.dumps(params.get("tools") or [], sort_keys=True, separators=(",", ":"), default=str)
    return hashlib.sha256(canon.encode()).hexdigest()[:16]


def drop_stale_thinking(params: dict[str, Any], bound: dict[str, str]) -> tuple[dict[str, Any], int]:
    """The request without the thinking blocks made under another tool list. `bound` maps a
    block's signature to the tool list it was made with. A block with an unknown signature is
    kept. Returns the request and the number of blocks dropped."""
    here = tools_key(params)
    dropped = 0
    messages = []
    for m in params.get("messages", []):
        content = m.get("content")
        if m.get("role") != "assistant" or not isinstance(content, list):
            messages.append(m)
            continue
        kept = []
        for block in content:
            sig = block.get("signature") if isinstance(block, dict) else None
            if isinstance(block, dict) and block.get("type") == "thinking" and bound.get(sig, here) != here:
                dropped += 1
                continue
            kept.append(block)
        if kept:
            messages.append({**m, "content": kept})
        # A message that held only stale thinking goes with it.
    if not dropped:
        return params, 0
    return {**params, "messages": messages}, dropped


class _Signatures:
    """Collects the signature of each thinking block in a reply, as its events pass."""

    def __init__(self, bound: dict[str, str], tools: str) -> None:
        self._bound = bound
        self._tools = tools
        self._open: dict[int, str] = {}

    def see(self, event) -> None:
        kind = getattr(event, "type", None)
        if kind == "content_block_start" and getattr(event.content_block, "type", None) == "thinking":
            self._open[event.index] = getattr(event.content_block, "signature", "") or ""
        elif kind == "content_block_delta" and getattr(event.delta, "type", None) == "signature_delta":
            self._open[event.index] = self._open.get(event.index, "") + event.delta.signature
        elif kind == "content_block_stop" and event.index in self._open:
            sig = self._open.pop(event.index)
            if sig:
                self._bound[sig] = self._tools

    def see_message(self, message: Message) -> None:
        for block in message.content:
            if block.type == "thinking" and block.signature:
                self._bound[block.signature] = self._tools


class WordsBeforeTool:
    """Notes the model's words that come before a tool call in the same reply (the owner's fork
    C, AGENT.md §3). On a voice call those words are spoken before the tool's fixed line.

    It reads the reply as the API sends it: a text block, then a tool_use block. The pipeline's
    frames cannot tell this: Pipecat's tool-call frame is a system frame and can overtake the
    text frames before it.
    """

    def __init__(self, found: list[tuple[str, str]], record=None) -> None:
        self._found = found
        self._record = record
        self._text: list[str] = []

    def _tool(self, name: str) -> None:
        spoken = "".join(self._text).strip()
        if spoken:
            self._found.append((name, spoken))
            if self._record is not None:
                closing = name in config.CLOSING_TOOLS
                self._record.event("words_before_tool", name=name, closing=closing, text=spoken)

    def see(self, event) -> None:
        kind = getattr(event, "type", None)
        if kind == "content_block_start" and getattr(event.content_block, "type", None) == "tool_use":
            self._tool(event.content_block.name)
        elif kind == "content_block_delta" and getattr(event.delta, "type", None) == "text_delta":
            self._text.append(event.delta.text)

    def see_message(self, message: Message) -> None:
        for block in message.content:
            if block.type == "text":
                self._text.append(block.text)
            elif block.type == "tool_use":
                self._tool(block.name)


class _Watch:
    """Passes each event of a reply to every watcher."""

    def __init__(self, *watchers) -> None:
        self._watchers = watchers

    def see(self, event) -> None:
        for w in self._watchers:
            w.see(event)

    def see_message(self, message: Message) -> None:
        for w in self._watchers:
            w.see_message(message)


class _Replay:
    """A recorded stream, played back as the SDK's own event objects."""

    def __init__(self, events: list[dict], sigs: _Watch | None = None, trace=None) -> None:
        self._events = [_EVENT.validate_python(e) for e in events]
        self._sigs = sigs
        self._trace = trace

    def __aiter__(self):
        return self._gen()

    async def _gen(self):
        for e in self._events:
            if self._sigs is not None:
                self._sigs.see(e)
            yield e
        if self._trace is not None:
            self._trace.done(_usage_of(self._events), replayed=True)


def _usage_of(events) -> dict[str, int]:
    usage: dict[str, int] = {}
    for event in events:
        u = getattr(getattr(event, "message", None), "usage", None) or getattr(event, "usage", None)
        if u is not None:
            for name in ("input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"):
                if getattr(u, name, None) is not None:
                    usage[name] = getattr(u, name)
    return usage


class _Recording:
    """A live stream, passed through. Kept on disk only when it ran to its end."""

    def __init__(
        self, stream, path: Path | None, key: str, sigs: _Watch | None = None, trace=None, on_failure=None
    ) -> None:
        self._stream = stream
        self._path = path
        self._key = key
        self._sigs = sigs
        self._trace = trace
        self._on_failure = on_failure

    def __aiter__(self):
        return self._gen()

    async def _gen(self):
        events: list[dict] = []
        seen = []
        try:
            async for event in self._stream:
                if not seen and self._trace is not None:
                    self._trace.first_event()
                events.append(event.model_dump(mode="json"))
                seen.append(event)
                if self._sigs is not None:
                    self._sigs.see(event)
                yield event
        except Exception as e:
            if self._trace is not None:
                self._trace.failed(e)
            if self._on_failure is not None:
                self._on_failure(e)
            raise
        usage = _usage_of(seen)
        logger.info(
            "claude live request {} tokens: in={} out={} cache_read={} cache_write={}",
            self._key[:8],
            usage.get("input_tokens", 0),
            usage.get("output_tokens", 0),
            usage.get("cache_read_input_tokens", 0),
            usage.get("cache_creation_input_tokens", 0),
        )
        if self._trace is not None:
            self._trace.done(usage, replayed=False)
        if self._path is not None:
            _write(self._path, {"stream": True, "events": events})


def _write(path: Path, body: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(body, indent=1, sort_keys=True) + "\n", encoding="utf-8")


class _Messages:
    def __init__(self, real: AsyncAnthropic | None, mode: Mode, replays: Path) -> None:
        self._real = real
        self._mode = mode
        self._replays = replays
        # Signature of each thinking block seen on this call → the tool list it was made with.
        self._bound: dict[str, str] = {}
        # The dev call record (call_record.py), set by the typed harness only; None otherwise.
        self.record = None
        # Called with the exception when a request fails, so the call ends at once (call.py).
        self.on_failure = None
        # Each tool call that came after words in the same reply: (tool name, the words).
        # Kept on every call, so the replay tests can check it with no record open.
        self.words_before_tool: list[tuple[str, str]] = []

    def _failed(self, e: BaseException, trace) -> None:
        if trace is not None:
            trace.failed(e)
        if self.on_failure is not None:
            self.on_failure(e)

    async def create(self, **params: Any):
        params, dropped = drop_stale_thinking(_clean(params), self._bound)
        if dropped:
            logger.info("dropped {} thinking blocks made under another tool list", dropped)
        key = request_key(params)
        path = self._replays / f"{key}.json"
        stream = bool(params.get("stream"))
        sigs = _Watch(
            _Signatures(self._bound, tools_key(params)), WordsBeforeTool(self.words_before_tool, self.record)
        )
        hit = self._mode is not Mode.LIVE and path.exists()
        trace = self.record.llm_request(key, params, hit=hit, dropped=dropped) if self.record else None

        if hit:
            body = json.loads(path.read_text(encoding="utf-8"))
            if stream:
                return _Replay(body["events"], sigs, trace)
            message = Message.model_validate(body["message"])
            sigs.see_message(message)
            if trace is not None:
                trace.done(_usage_of([SimpleNamespace(usage=message.usage)]), replayed=True)
            return message
        if self._mode is Mode.REPLAY or self._real is None:
            err = ReplayMiss(f"no recording for request {key}; run the typed harness to record it")
            self._failed(err, trace)
            raise err

        try:
            reply = await self._real.messages.create(**params)
        except Exception as e:
            self._failed(e, trace)
            raise
        if stream:
            path = path if self._mode is Mode.RECORD else None
            return _Recording(reply, path, key, sigs, trace, lambda e: self._failed(e, None))
        sigs.see_message(reply)
        if trace is not None:
            trace.done(_usage_of([SimpleNamespace(usage=reply.usage)]), replayed=False)
        if self._mode is Mode.RECORD:
            _write(path, {"stream": False, "message": reply.model_dump(mode="json")})
        return reply


class GaClient:
    """Stands in for `AsyncAnthropic` inside Pipecat. Both `client.messages` and
    `client.beta.messages` lead to the GA endpoint."""

    def __init__(self, mode: Mode, replays: Path = config.REPLAYS, api_key: str | None = None) -> None:
        real = AsyncAnthropic(api_key=api_key) if api_key else None
        self.messages = _Messages(real, mode, replays)
        self.beta = SimpleNamespace(messages=self.messages)


def build_llm(mode: Mode, replays: Path = config.REPLAYS) -> AnthropicLLMService:
    key = config.anthropic_key()
    if mode is not Mode.REPLAY and key is None:
        raise RuntimeError("ANTHROPIC_API_KEY is not set in the repo's .env")
    return AnthropicLLMService(
        api_key=key or "replay-only",
        client=GaClient(mode, replays, key),
        settings=AnthropicLLMService.Settings(
            model=config.MODEL,
            max_tokens=config.MAX_TOKENS,
            effort=config.EFFORT,
            enable_prompt_caching=True,
        ),
    )
