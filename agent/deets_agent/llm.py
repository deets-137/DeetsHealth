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


class _Replay:
    """A recorded stream, played back as the SDK's own event objects."""

    def __init__(self, events: list[dict]) -> None:
        self._events = [_EVENT.validate_python(e) for e in events]

    def __aiter__(self):
        return self._gen()

    async def _gen(self):
        for e in self._events:
            yield e


class _Recording:
    """A live stream, passed through. Kept on disk only when it ran to its end."""

    def __init__(self, stream, path: Path | None, key: str) -> None:
        self._stream = stream
        self._path = path
        self._key = key

    def __aiter__(self):
        return self._gen()

    async def _gen(self):
        events: list[dict] = []
        usage: dict[str, int] = {}
        async for event in self._stream:
            events.append(event.model_dump(mode="json"))
            u = getattr(getattr(event, "message", None), "usage", None) or getattr(event, "usage", None)
            if u is not None:
                for name in ("input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"):
                    if getattr(u, name, None) is not None:
                        usage[name] = getattr(u, name)
            yield event
        logger.info(
            "claude live request {} tokens: in={} out={} cache_read={} cache_write={}",
            self._key[:8],
            usage.get("input_tokens", 0),
            usage.get("output_tokens", 0),
            usage.get("cache_read_input_tokens", 0),
            usage.get("cache_creation_input_tokens", 0),
        )
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

    async def create(self, **params: Any):
        params = _clean(params)
        key = request_key(params)
        path = self._replays / f"{key}.json"
        stream = bool(params.get("stream"))

        if self._mode is not Mode.LIVE and path.exists():
            body = json.loads(path.read_text(encoding="utf-8"))
            return _Replay(body["events"]) if stream else Message.model_validate(body["message"])
        if self._mode is Mode.REPLAY or self._real is None:
            raise ReplayMiss(f"no recording for request {key}; run the typed harness to record it")

        reply = await self._real.messages.create(**params)
        if stream:
            return _Recording(reply, path if self._mode is Mode.RECORD else None, key)
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
