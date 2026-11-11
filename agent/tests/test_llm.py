"""The LLM client: GA only, and record and replay. No network, no tokens."""

import json

import pytest

from deets_agent.llm import GaClient, Mode, ReplayMiss, drop_stale_thinking, request_key

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


GREET_TOOLS = [{"name": "identify", "input_schema": {"type": "object"}}]
VERIFY_TOOLS = [{"name": "check_code", "input_schema": {"type": "object"}}]
THINK_THEN_IDENTIFY = [
    {"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": "", "signature": ""}},
    {"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "hm"}},
    {"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "SIG-A"}},
    {"type": "content_block_stop", "index": 0},
    {"type": "content_block_start", "index": 1,
     "content_block": {"type": "tool_use", "id": "t1", "name": "identify", "input": {}}},
    {"type": "content_block_stop", "index": 1},
    {"type": "message_stop"},
]


def _after_identify(tools):
    return {
        "model": "claude-haiku-5-5",
        "max_tokens": 600,
        "stream": True,
        "tools": tools,
        "messages": [
            {"role": "user", "content": "hello"},
            {"role": "assistant", "content": [
                {"type": "thinking", "thinking": "hm", "signature": "SIG-A"},
                {"type": "tool_use", "id": "t1", "name": "identify", "input": {}},
            ]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t1", "content": "{}"}]},
        ],
    }


async def test_a_thinking_block_is_dropped_when_the_node_changes_the_tools(tmp_path):
    # The 2026-11-11 crash: Claude thought, then called identify under the greet node's tools;
    # the next request (the verify node's tools) sent that block back, and the API refused it.
    first = dict(PARAMS, tools=GREET_TOOLS)
    (tmp_path / f"{request_key(first)}.json").write_text(json.dumps({"stream": True, "events": THINK_THEN_IDENTIFY}))
    seen = []
    client = GaClient(Mode.REPLAY, tmp_path)
    async for _ in await client.messages.create(**first):
        pass

    for tools, kept in ((VERIFY_TOOLS, False), (GREET_TOOLS, True)):
        sent = drop_stale_thinking(_after_identify(tools), client.messages._bound)[0]
        blocks = [b["type"] for b in sent["messages"][1]["content"]]
        seen.append(blocks)
        assert ("thinking" in blocks) is kept
        assert "tool_use" in blocks
    assert seen == [["tool_use"], ["thinking", "tool_use"]]


def test_a_thinking_block_of_unknown_origin_is_kept():
    params = _after_identify(VERIFY_TOOLS)
    assert drop_stale_thinking(params, {}) == (params, 0)


def test_any_change_to_the_request_is_a_new_key():
    other = dict(PARAMS, messages=[{"role": "user", "content": "hello!"}])
    assert request_key(PARAMS) != request_key(other)
