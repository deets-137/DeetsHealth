"""The voice ends with no network: the credit count, the limits, the credit cycle, the screen on
a spoken line, and the 911 line that no barge-in cuts (voice.py, resources.py, screen.py)."""

from __future__ import annotations

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
from deets_agent.screen import CallerScreen
from deets_agent.voice import VoiceMeter


def test_the_meter_keeps_raw_use_only():
    """Credits are the vendors crate's (cost.rs; its tests): the meter counts amounts by vendor."""
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


# ElevenLabs (VOICE-VENDORS.md §11): built with a placeholder key, never connected, no credits.
FAKE_KEY = "not-a-key"


def _elevenlabs_values(**over):
    from deets_agent.voice import vendors

    v = vendors.defaults()
    v.update({"hearing.vendor": "elevenlabs", "speaking.vendor": "elevenlabs", **over})
    return v


def test_elevenlabs_keeps_zero_retention_on_by_default():
    from deets_agent.voice import elevenlabs

    v = _elevenlabs_values()
    assert elevenlabs.zero_retention(v)
    stt, tts = elevenlabs.build_stt(FAKE_KEY, v), elevenlabs.build_tts(FAKE_KEY, v)
    assert stt._enable_logging is False, "Scribe: zero retention"
    assert tts._settings.model == "eleven_flash_v2_5"
    assert tts._enable_logging is False, "Flash: zero retention"


def test_elevenlabs_takes_synthetic_calls_only_with_zero_retention_on_or_off():
    """The owner's B (VOICE-VENDORS.md §4, item 5), made stricter: below Enterprise the flag is
    accepted and ignored (§11.7), so a call that is not synthetic is refused either way."""
    import pytest

    from deets_agent.voice import elevenlabs

    for on in (True, False):
        v = _elevenlabs_values(**{"elevenlabs.zero_retention": on})
        elevenlabs.check_retention(v, "synthetic")
        with pytest.raises(elevenlabs.RetentionRefused):
            elevenlabs.check_retention(v, "real")
    off = _elevenlabs_values(**{"elevenlabs.zero_retention": False})
    assert elevenlabs.build_tts(FAKE_KEY, off)._enable_logging is True


async def test_a_whole_elevenlabs_voice_is_wired_for_pipecats_turns(monkeypatch):
    """The call's path when Scribe hears (call.py): the aggregator gets the detector, the turn
    events reach the meter, and the screen passes Scribe's commit (upstream) and the caller's
    audio (downstream) through. No network, no key used."""
    from pipecat.frames.frames import InputAudioRawFrame, VADUserStoppedSpeakingFrame
    from pipecat.processors.aggregators.llm_context import LLMContext
    from pipecat.processors.aggregators.llm_response_universal import LLMContextAggregatorPair

    from deets_agent.voice import HeardWords, Voice

    monkeypatch.setenv("ELEVENLABS_API_KEY", FAKE_KEY)
    voice = Voice(_elevenlabs_values(), data_class="synthetic")
    assert not voice.own_turns and voice.user_params is not None
    assert isinstance(voice.hearing[1], HeardWords)
    assert voice.names == {"hearing": ("Scribe", "ElevenLabs"), "speaking": ("Flash", "ElevenLabs")}
    pair = LLMContextAggregatorPair(LLMContext(), user_params=voice.user_params)
    voice.attach(pair.user())
    import asyncio

    voice.meter.last_speech = 0.0
    await pair.user()._call_event_handler("on_user_turn_started", None)
    await asyncio.sleep(0.05)  # a handler may run as its own task
    assert voice.meter.last_speech > 0.0, "a turn start from the aggregator resets the idle limit"

    s = _Screen()
    commit = VADUserStoppedSpeakingFrame()
    await s.process_frame(commit, FrameDirection.UPSTREAM)
    audio = InputAudioRawFrame(audio=b"\0\0" * 160, sample_rate=16000, num_channels=1)
    await s.process_frame(audio, FrameDirection.DOWNSTREAM)
    assert commit in s.out and audio in s.out


def test_a_refused_voice_ends_the_call_with_the_vendors_words():
    from deets_agent.call import voice_failure

    failed = voice_failure("Scribe", "ElevenLabs", "Error: quota_exceeded: the plan is used up", unusable=False)
    assert failed is not None and failed.why.startswith("Scribe (ElevenLabs) failed: Error: quota_exceeded")
    assert voice_failure("Scribe", "ElevenLabs", "Error: commit_throttled", unusable=False) is None
    assert voice_failure("Sonic", "Cartesia", "", unusable=True).why == "Sonic (Cartesia) failed: no message"


def test_scribe_hears_with_pipecats_voice_detector_and_smart_turn():
    """Scribe has no turns of its own: the aggregator's detector and Smart Turn decide them, and
    barge-in off means no start strategy interrupts the agent."""
    from pipecat.turns.user_stop import TurnAnalyzerUserTurnStopStrategy

    from deets_agent.voice import elevenlabs

    p = elevenlabs.user_params(_elevenlabs_values(**{"elevenlabs.turn_stop_secs": 4.0, "barge_in": False}))
    assert p.vad_analyzer is not None
    assert all(not s._enable_interruptions for s in p.user_turn_strategies.start)
    stop = p.user_turn_strategies.stop[0]
    assert isinstance(stop, TurnAnalyzerUserTurnStopStrategy)
    assert stop._turn_analyzer.params.stop_secs == 4.0
