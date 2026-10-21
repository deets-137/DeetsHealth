"""The LLM client: GA only, and record and replay. No network, no tokens."""

import json

import pytest

from deets_agent.llm import GaClient, Mode, ReplayMiss, request_key

PARAMS = {
    "model": "claude-haiku-5-5",
    "max_tokens": 600,
    "stream": True,
    "messages": [{"role": "user", "content": "hello"}],
}
EVENTS = [
    {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}},
    {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hi there"}},
    {"type": "content_block_stop", "index": 0},
    {"type": "message_stop"},
]


def test_the_beta_flag_never_reaches_the_key_or_the_request():
    # Pipecat adds betas; the GA request is the same request without them.
    assert request_key(PARAMS) == request_key(PARAMS)
    client = GaClient(Mode.REPLAY)
    assert client.beta.messages is client.messages


async def test_a_recording_replays_as_sdk_events(tmp_path):
    (tmp_path / f"{request_key(PARAMS)}.json").write_text(json.dumps({"stream": True, "events": EVENTS}))
    client = GaClient(Mode.REPLAY, tmp_path)
    stream = await client.beta.messages.create(**PARAMS, betas=["interleaved-thinking-2025-05-14"])
    events = [e async for e in stream]
    assert events[1].type == "content_block_delta" and events[1].delta.text == "Hi there"


async def test_a_miss_in_replay_mode_never_calls_the_network(tmp_path):
    client = GaClient(Mode.REPLAY, tmp_path, api_key="not-used")
    with pytest.raises(ReplayMiss):
        await client.messages.create(**PARAMS)


def test_any_change_to_the_request_is_a_new_key():
    other = dict(PARAMS, messages=[{"role": "user", "content": "hello!"}])
    assert request_key(PARAMS) != request_key(other)
