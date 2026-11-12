"""The check behind the owner's fork C (AGENT.md §3): our LLM client notes the model's words that
come before a tool call in the same reply. On a voice call those words are spoken before the
tool's fixed line. The check reads the API's own events (a text block, then a tool_use block),
not Pipecat's frames: the tool-call frame is a system frame and can overtake the text frames.
"""

from anthropic.types import Message
from pydantic import TypeAdapter

from deets_agent.llm import _EVENT, WordsBeforeTool


def _events(*blocks: tuple[str, str]) -> list:
    """A streamed reply: each block is ("text", words) or ("tool", name)."""
    raw: list[dict] = []
    for i, (kind, value) in enumerate(blocks):
        if kind == "text":
            raw.append({"type": "content_block_start", "index": i, "content_block": {"type": "text", "text": ""}})
            raw.append({"type": "content_block_delta", "index": i, "delta": {"type": "text_delta", "text": value}})
        else:
            block = {"type": "tool_use", "id": f"t{i}", "name": value, "input": {}}
            raw.append({"type": "content_block_start", "index": i, "content_block": block})
        raw.append({"type": "content_block_stop", "index": i})
    return [_EVENT.validate_python(e) for e in raw]


def _seen(*blocks: tuple[str, str]) -> list[tuple[str, str]]:
    found: list[tuple[str, str]] = []
    w = WordsBeforeTool(found)
    for e in _events(*blocks):
        w.see(e)
    return found


def test_words_then_a_tool_are_noted():
    assert _seen(("text", "Goodbye."), ("tool", "hand_off")) == [("hand_off", "Goodbye.")]


def test_a_tool_alone_is_not_noted():
    assert _seen(("tool", "end_call")) == []


def test_words_alone_are_not_noted():
    assert _seen(("text", "What can I help you with?")) == []


def test_a_whole_message_is_checked_too():
    message = TypeAdapter(Message).validate_python(
        {
            "id": "m1",
            "type": "message",
            "role": "assistant",
            "model": "claude-haiku-5-5",
            "content": [
                {"type": "text", "text": "One moment."},
                {"type": "tool_use", "id": "t1", "name": "find_slots", "input": {}},
            ],
            "stop_reason": "tool_use",
            "stop_sequence": None,
            "usage": {"input_tokens": 1, "output_tokens": 1},
        }
    )
    found: list[tuple[str, str]] = []
    WordsBeforeTool(found).see_message(message)
    assert found == [("find_slots", "One moment.")]
