"""The caller screen: code that reads every caller line as it arrives, before the model sees it
(the owner's option 2A, 2026-11-11).

- **The emergency floor.** A short list of phrases ("chest pain", "can't breathe", "am I
  dying") makes the code speak the fixed 911 line itself, whether or not the model would have
  called `emergency`. The model stays the ceiling: it still has the `emergency` tool for the
  rest. The list is a hospital setting later (FUTURE-SETTINGS.md §3); this is the default.
- **The name guard's input.** Every caller line goes to `CallFlow.heard`, so `identify` can
  check that each name the model sends was said.

On a hit the screen adds the caller's line and the 911 line to the context, speaks the line,
and does not run the model: the line ends in a question, so the caller answers next. It fires
once per call; after that a match is only recorded, and the model's tool covers the rest.

Typed calls arrive as an `LLMMessagesAppendFrame` with one user message. Phase 3 adds the voice
path's `TranscriptionFrame` here. Logs carry the kind of event only, never the words.
"""

from __future__ import annotations

import re

from loguru import logger
from pipecat.frames.frames import Frame, LLMMessagesAppendFrame, TTSSpeakFrame
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor

from deets_agent.flow import EMERGENCY_LINE, CallFlow

# The default phrase list. Each entry is a regular expression matched against the caller's line
# in lower case, with curly apostrophes made straight. Over-calling is the safe side: a false
# alarm costs the caller one sentence, because the call goes on after the line (option A).
EMERGENCY_PHRASES: tuple[str, ...] = (
    r"\bchest pains?\b",
    r"\bheart attack\b",
    r"\b(can'?t|cannot|can not|couldn'?t) breathe?\b",
    r"\b(trouble|difficulty|hard time) breathing\b",
    r"\bnot breathing\b",
    r"\bam i dying\b",
    r"\bi'?m dying\b(?! to\b)",
    r"\bgoing to die\b",
    r"\boverdos(e|ed|ing)\b",
    r"\bkill(ing)? myself\b",
    r"\bend my life\b",
    r"\bwant to die\b",
    r"\bsuicid(e|al)\b",
    r"\b(having a |had a )?stroke\b",
    r"\bseizures?\b",
    r"\bunconscious\b",
    r"\bpassed out\b",
    r"\b(bleeding a lot|heavy bleeding|won'?t stop bleeding)\b",
    r"\b911\b",
)

_PATTERNS = [re.compile(p) for p in EMERGENCY_PHRASES]


def emergency_match(text: str) -> bool:
    """True when the caller's line holds a phrase from the emergency list."""
    line = text.lower().replace("’", "'")
    return any(p.search(line) for p in _PATTERNS)


class CallerScreen(FrameProcessor):
    """Reads each caller line before the model does. Sits first in the pipeline."""

    def __init__(self, flow: CallFlow) -> None:
        super().__init__()
        self.flow = flow
        self.fired = False
        # Set by TypedCall when a dev call record is open.
        self.record = None

    @staticmethod
    def _caller_line(frame: Frame) -> str | None:
        if isinstance(frame, LLMMessagesAppendFrame) and len(frame.messages) == 1:
            message = frame.messages[0]
            if message.get("role") == "user" and isinstance(message.get("content"), str):
                return message["content"]
        return None

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        line = self._caller_line(frame) if direction is FrameDirection.DOWNSTREAM else None
        if line is None:
            await self.push_frame(frame, direction)
            return
        self.flow.heard(line)
        if not emergency_match(line):
            await self.push_frame(frame, direction)
            return
        if self.fired:
            logger.info("emergency phrase heard again; left to the model")
            if self.record is not None:
                self.record.event("screen", action="emergency phrase, already warned")
            await self.push_frame(frame, direction)
            return
        self.fired = True
        logger.info("emergency phrase heard; 911 line spoken by the screen")
        if self.record is not None:
            self.record.event("screen", action="emergency phrase, 911 line spoken")
        # The caller's line and the 911 line go into the context, and the model does not run:
        # the line asks a question, so the caller speaks next.
        await self.push_frame(
            LLMMessagesAppendFrame(
                messages=[
                    {"role": "user", "content": line},
                    {"role": "assistant", "content": EMERGENCY_LINE},
                ],
                run_llm=False,
            ),
            direction,
        )
        await self.push_frame(TTSSpeakFrame(text=EMERGENCY_LINE, append_to_context=False), direction)
