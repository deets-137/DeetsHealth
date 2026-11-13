"""The voice ends of a call: Cartesia Ink hears the caller, Sonic speaks the agent
(integrations/CARTESIA.md; §9 lists every lever and how it is set here).

    seat in ─► Ink ─► screen ─► user ─► Claude ─► turns ─► Sonic ─► meter ─► seat out ─► assistant

**Ink** is `CartesiaTurnsSTTService`: Ink-2's own turn detection, so no local voice detector
runs. Its `turn.start` interrupts the agent (barge-in); its `turn.end` gives the caller's line.
Eager end of turn stays off: it starts Claude on a guess that Ink may withdraw.

**Sonic** is `CartesiaTTSService`. Fixed lines arrive as `TTSSpeakFrame` (flow.py, screen.py) and
Sonic speaks them word for word; the 911 line cannot be interrupted (screen.py, flow.py).

**The meter** counts what Cartesia bills: Pipecat's usage metrics give the seconds of audio sent
to Ink (silence included, as Cartesia bills it) and the characters sent to Sonic. It also keeps
the time of the caller's last speech, for the idle limit. It is an estimate: Cartesia has no
API that reports usage, and its dashboard is the full count (CARTESIA.md §4).

**The limits** (`config.VOICE_IDLE_S`, `config.VOICE_MAX_S`) end a voice test call with no
spoken line: a dev guard for the Free plan's credits, not a caller's experience.

Logs carry the kind of event only, never the words (SECURITY.md §4).
"""

from __future__ import annotations

import time

from pipecat.frames.frames import Frame, MetricsFrame
from pipecat.metrics.metrics import STTUsageMetricsData, TTSUsageMetricsData
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor
from pipecat.services.cartesia.tts import CartesiaTTSService, GenerationConfig
from pipecat.services.cartesia.turns.stt import CartesiaTurnsSTTService

from deets_agent import config


def build_stt(key: str, values: dict | None = None) -> CartesiaTurnsSTTService:
    """Ink-2 on the turns endpoint, with the active settings (CONSOLE.md §7): the four turn
    values (None sends nothing: Cartesia's default), barge-in, early end of turn, keyterms."""
    v = values or config.voice_defaults()
    turn = {k: v.get(k) for k in config.INK_TURN_LEVERS if v.get(k) is not None}
    if "turn_end_timeout_ms" in turn:
        turn["turn_end_timeout_ms"] = int(turn["turn_end_timeout_ms"])
    keyterms = [row[0] for row in v.get("keyterms") or []]
    return CartesiaTurnsSTTService(
        api_key=key,
        should_interrupt=bool(v.get("barge_in", True)),
        enable_eager_end_of_turn=bool(v.get("eager_end_of_turn", False)),
        settings=CartesiaTurnsSTTService.Settings(model=config.STT_MODEL, keyterm=keyterms or None, **turn),
    )


def build_tts(key: str, values: dict | None = None) -> CartesiaTTSService:
    """Sonic, pinned by name, with the active settings: the voice, speed, volume, emotion and
    number reading. The output rate is the pipeline's. Pronunciations are stored but not sent
    yet: they need a Cartesia pronunciation dictionary (CONSOLE.md §7.6)."""
    v = values or config.voice_defaults()
    voice = (v.get("voice") or {}).get("id") or config.voice_id()
    gen = {k: v[k] for k in ("speed", "volume", "emotion") if v.get(k) is not None}
    return CartesiaTTSService(
        api_key=key,
        settings=CartesiaTTSService.Settings(
            model=config.TTS_MODEL,
            voice=voice,
            generation_config=GenerationConfig(**gen) if gen else None,
            normalization=v.get("normalization") or "auto",
        ),
    )


def credits(ink_s: float, sonic_chars: int) -> int:
    """Cartesia's credits for this much use (CARTESIA.md §4), rounded up."""
    whole = ink_s * config.INK_CREDITS_PER_S + sonic_chars * config.SONIC_CREDITS_PER_CHAR
    return int(-(-whole // 1))


class VoiceMeter(FrameProcessor):
    """Counts Ink's seconds and Sonic's characters from Pipecat's usage metrics, and the time
    of the caller's last speech. Sits after Sonic, where both services' metrics have passed."""

    def __init__(self, idle_s: int = config.VOICE_IDLE_S, max_s: int = config.VOICE_MAX_S) -> None:
        super().__init__()
        self.ink_s = 0.0
        self.sonic_chars = 0
        self.started = time.monotonic()
        self.last_speech = self.started
        # The test-call guard (CONSOLE.md §7, the active settings).
        self.idle_s = idle_s
        self.max_s = max_s

    @property
    def credits(self) -> int:
        return credits(self.ink_s, self.sonic_chars)

    def totals(self) -> dict[str, float | int]:
        return {"ink_s": round(self.ink_s, 1), "sonic_chars": self.sonic_chars, "credits": self.credits}

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        if isinstance(frame, MetricsFrame):
            for data in frame.data:
                if isinstance(data, STTUsageMetricsData):
                    self.ink_s += data.value.audio_seconds
                elif isinstance(data, TTSUsageMetricsData):
                    self.sonic_chars += data.value
        await self.push_frame(frame, direction)

    def heard(self) -> None:
        """The caller spoke: Ink's `turn.start` or `turn.end` (Voice wires both)."""
        self.last_speech = time.monotonic()

    def elapsed(self) -> float:
        return time.monotonic() - self.started

    def over_limit(self, now: float | None = None) -> str | None:
        """Why the call must end now, or None: the idle limit, then the call's own limit."""
        now = time.monotonic() if now is None else now
        if now - self.started >= self.max_s:
            return f"the {self.max_s} s limit for a voice test call"
        if now - self.last_speech >= self.idle_s:
            return f"{self.idle_s} s with no speech from the caller"
        return None


class Voice:
    """The voice parts of one call: Ink, Sonic and the meter."""

    def __init__(self, key: str, values: dict | None = None) -> None:
        """`values`: the active voice settings (CONSOLE.md §7); the defaults when none."""
        v = values or config.voice_defaults()
        self.stt = build_stt(key, v)
        self.tts = build_tts(key, v)
        self.meter = VoiceMeter(int(v.get("idle_s") or config.VOICE_IDLE_S), int(v.get("max_s") or config.VOICE_MAX_S))

        @self.stt.event_handler("on_turn_start")
        async def _start(service, *args):
            self.meter.heard()

        @self.stt.event_handler("on_turn_end")
        async def _end(service, *args):
            self.meter.heard()
