"""The repeat guard (call.py › TurnCollector; the owner, 2026-11-15): the first model reply after
a fixed line is never the same line again. In `emergency_reminder` the model said the 911
reminder a second time, word for word, so the caller would have heard it twice.
"""

from pipecat.frames.frames import (
    FunctionCallsStartedFrame,
    LLMFullResponseEndFrame,
    LLMFullResponseStartFrame,
    LLMMessagesAppendFrame,
    LLMTextFrame,
    TTSSpeakFrame,
)
from pipecat.processors.frame_processor import FrameDirection

from deets_agent.call import TurnCollector, _after, _plain

LINE = "Understood. Please hang up and call 911 if it gets worse, or if this may be an emergency."


def collector():
    c = TurnCollector()
    c.out = []
    c.asked = []

    async def push(frame, direction=FrameDirection.DOWNSTREAM):
        c.out.append(frame)

    c.push_frame = push
    c.requeue = c.asked.append
    return c


async def feed(c, *frames):
    for f in frames:
        await c.process_frame(f, FrameDirection.DOWNSTREAM)


def texts(c):
    return "".join(f.text for f in c.out if isinstance(f, LLMTextFrame))


def test_plain_and_after():
    assert _plain("Understood.  Please, hang UP!") == "understood please hang up"
    assert _after("Understood. Please hang up. And then", len(_plain("Understood. Please hang up"))) == "And then"


async def test_a_reply_that_repeats_the_line_is_dropped_and_the_model_asked_once():
    c = collector()
    chunks = ["Understood. Please hang up and call ", "911 if it gets worse, or if this ", "may be an emergency."]
    await feed(c, TTSSpeakFrame(text=LINE), LLMFullResponseStartFrame())
    await feed(c, *(LLMTextFrame(text=t) for t in chunks), LLMFullResponseEndFrame())
    assert texts(c) == "", "the caller never hears it twice"
    assert c.repeats_dropped == 1
    assert len(c.asked) == 1 and isinstance(c.asked[0], LLMMessagesAppendFrame)
    assert c.asked[0].messages[0]["role"] == "developer" and c.asked[0].run_llm
    # A second repeat of the same line is dropped too, but asks no more: no loop.
    await feed(c, TTSSpeakFrame(text=LINE), LLMFullResponseStartFrame(), LLMTextFrame(text=LINE), LLMFullResponseEndFrame())
    assert c.repeats_dropped == 2 and len(c.asked) == 1


async def test_a_different_reply_goes_on_at_the_first_word_that_differs():
    c = collector()
    await feed(c, TTSSpeakFrame(text=LINE), LLMFullResponseStartFrame(), LLMTextFrame(text="Understood. "))
    assert texts(c) == "", "held while it reads as the line"
    await feed(c, LLMTextFrame(text="Let me look that up."), LLMFullResponseEndFrame())
    assert texts(c) == "Understood. Let me look that up."
    assert c.repeats_dropped == 0 and not c.asked


async def test_the_line_then_more_keeps_the_more_and_a_tool_call_asks_nothing():
    c = collector()
    await feed(c, TTSSpeakFrame(text=LINE), LLMFullResponseStartFrame(), LLMTextFrame(text=LINE + " Shall we go on?"))
    await feed(c, FunctionCallsStartedFrame(function_calls=[]), LLMFullResponseEndFrame())
    assert texts(c).strip() == "Shall we go on?"
    assert c.repeats_dropped == 1 and not c.asked, "it acted, or said something new: no retry"


async def test_only_the_first_reply_after_a_line_is_guarded():
    c = collector()
    await feed(c, TTSSpeakFrame(text=LINE), LLMFullResponseStartFrame(), LLMTextFrame(text="Okay."), LLMFullResponseEndFrame())
    await feed(c, LLMFullResponseStartFrame(), LLMTextFrame(text=LINE), LLMFullResponseEndFrame())
    assert LINE in texts(c), "a later reply is the model's own choice; the guard does not hold it"
