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
from pipecat.services.cartesia.tts import CartesiaTTSService
from pipecat.services.cartesia.turns.stt import CartesiaTurnsSTTService

from deets_agent import config


def build_stt(key: str) -> CartesiaTurnsSTTService:
    """Ink-2 on the turns endpoint, every turn threshold at Cartesia's default."""
    return CartesiaTurnsSTTService(
        api_key=key,
        settings=CartesiaTurnsSTTService.Settings(model=config.STT_MODEL),
    )


def build_tts(key: str) -> CartesiaTTSService:
    """Sonic, pinned by name, in the chosen voice. The output rate is the pipeline's."""
    return CartesiaTTSService(
        api_key=key,
        settings=CartesiaTTSService.Settings(model=config.TTS_MODEL, voice=config.voice_id()),
    )


def credits(ink_s: float, sonic_chars: int) -> int:
    """Cartesia's credits for this much use (CARTESIA.md §4), rounded up."""
    whole = ink_s * config.INK_CREDITS_PER_S + sonic_chars * config.SONIC_CREDITS_PER_CHAR
    return int(-(-whole // 1))


class VoiceMeter(FrameProcessor):
    """Counts Ink's seconds and Sonic's characters from Pipecat's usage metrics, and the time
    of the caller's last speech. Sits after Sonic, where both services' metrics have passed."""

    def __init__(self) -> None:
        super().__init__()
        self.ink_s = 0.0
        self.sonic_chars = 0
        self.started = time.monotonic()
        self.last_speech = self.started

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
        if now - self.started >= config.VOICE_MAX_S:
            return f"the {config.VOICE_MAX_S // 60}-minute limit for a voice test call"
        if now - self.last_speech >= config.VOICE_IDLE_S:
            return f"{config.VOICE_IDLE_S} s with no speech from the caller"
        return None


class Voice:
    """The voice parts of one call: Ink, Sonic and the meter."""

    def __init__(self, key: str) -> None:
        self.stt = build_stt(key)
        self.tts = build_tts(key)
        self.meter = VoiceMeter()

        @self.stt.event_handler("on_turn_start")
        async def _start(service, *args):
            self.meter.heard()

        @self.stt.event_handler("on_turn_end")
        async def _end(service, *args):
            self.meter.heard()
