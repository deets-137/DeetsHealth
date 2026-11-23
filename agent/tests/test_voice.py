"""The voice ends with no network: the credit count, the limits, the credit cycle, the screen on
a spoken line, and the 911 line that no barge-in cuts (voice.py, resources.py, screen.py)."""

from __future__ import annotations

from datetime import date

import pytest
from pipecat.frames.frames import (
    BotStartedSpeakingFrame,
    LLMMessagesAppendFrame,
    TranscriptionFrame,
    TTSAudioRawFrame,
    TTSSpeakFrame,
    TTSStartedFrame,
)
from pipecat.processors.frame_processor import FrameDirection

from deets_agent import config
from deets_agent.flow import CallFlow, emergency_frame, emergency_line
from deets_agent.resources import cycle_start, next_cycle
from deets_agent.screen import CallerScreen
from deets_agent.voice import VoiceMeter


def test_the_meter_keeps_raw_use_only():
    """Credits are the voice crate's (cost.rs; its tests): the meter counts amounts by vendor."""
    m = VoiceMeter(chosen={"hearing": "cartesia", "speaking": "cartesia"})
    m.heard_s, m.spoken_chars = 12.34, 350
    assert m.use() == {"hearing": {"vendor": "cartesia", "amount": 12.3}, "speaking": {"vendor": "cartesia", "amount": 350}}
    assert not hasattr(m, "credits")


def test_the_limits():
    m = VoiceMeter()
    assert m.over_limit(m.started + 10) is None
    assert "no speech" in m.over_limit(m.started + config.VOICE_IDLE_S)
    m.last_speech = m.started + config.VOICE_MAX_S - 1
    assert "limit" in m.over_limit(m.started + config.VOICE_MAX_S)


@pytest.mark.parametrize(
    ("today", "day", "start", "renews"),
    [
        (date(2026, 11, 13), 1, date(2026, 11, 1), date(2026, 12, 1)),
        (date(2026, 11, 13), 20, date(2026, 10, 20), date(2026, 11, 20)),
        (date(2026, 1, 5), 20, date(2025, 12, 20), date(2026, 1, 20)),
        (date(2026, 12, 20), 20, date(2026, 12, 20), date(2027, 1, 20)),
    ],
)
def test_the_credit_cycle(today, day, start, renews):
    assert cycle_start(today, day) == start
    assert next_cycle(start) == renews


def test_the_911_line_cannot_be_interrupted():
    frame = emergency_frame()
    assert frame.text == emergency_line() and frame.interruptible is False
    assert TTSSpeakFrame(text="x").interruptible is True


class _Meter(VoiceMeter):
    """The meter with its output caught, not sent down a pipeline."""

    async def push_frame(self, frame, direction=FrameDirection.DOWNSTREAM):
        pass


class _Record:
    def __init__(self) -> None:
        self.events: list[tuple[str, dict]] = []

    def event(self, kind: str, **fields) -> None:
        self.events.append((kind, fields))


async def test_the_meter_records_each_lines_first_audio_and_the_seat_speaking():
    m, r = _Meter(), _Record()
    m.record = r
    audio = lambda: TTSAudioRawFrame(audio=b"\0\0", sample_rate=16000, num_channels=1)
    for frame, way in [
        (TTSStartedFrame(), FrameDirection.DOWNSTREAM),
        (audio(), FrameDirection.DOWNSTREAM),
        (audio(), FrameDirection.DOWNSTREAM),  # not the first: no event
        (BotStartedSpeakingFrame(), FrameDirection.DOWNSTREAM),  # the seat's own copy: no event
        (BotStartedSpeakingFrame(), FrameDirection.UPSTREAM),
    ]:
        await m.process_frame(frame, way)
    assert r.events == [("voice", {"stage": "sonic_audio"}), ("voice", {"stage": "speaking"})]
    assert all("text" not in f for _, f in r.events), "times only, never words"


class _Screen(CallerScreen):
    """The screen with its output caught, not sent down a pipeline."""

    def __init__(self) -> None:
        super().__init__(CallFlow(ehr=None))
        self.out: list = []

    async def push_frame(self, frame, direction=FrameDirection.DOWNSTREAM):
        self.out.append(frame)


def _spoken(text: str) -> TranscriptionFrame:
    return TranscriptionFrame(text=text, user_id="caller", timestamp="")


async def test_the_screen_hears_a_spoken_line():
    s = _Screen()
    heard: list[str] = []
    s.on_line = heard.append
    await s.process_frame(_spoken("I need a check-up"), FrameDirection.DOWNSTREAM)
    assert heard == ["I need a check-up"]
    assert isinstance(s.out[-1], TranscriptionFrame), "an ordinary line goes on to the model"


async def test_a_spoken_emergency_speaks_the_line_and_keeps_the_model_out():
    s = _Screen()
    await s.process_frame(_spoken("I have chest pain"), FrameDirection.DOWNSTREAM)
    assert s.fired
    assert not any(isinstance(f, TranscriptionFrame) for f in s.out), "the model does not run on it"
    context = next(f for f in s.out if isinstance(f, LLMMessagesAppendFrame))
    assert context.run_llm is False
    line = next(f for f in s.out if isinstance(f, TTSSpeakFrame))
    assert line.text == emergency_line() and line.interruptible is False
