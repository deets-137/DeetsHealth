"""The voice ends of a call (VOICE-VENDORS.md): a vendor hears the caller, a vendor speaks the
agent. Each direction's vendor is a setting (`hearing.vendor`, `speaking.vendor`); each vendor
has an adapter here (`cartesia.py`). Today the catalog has one vendor, Cartesia: Ink hears,
Sonic speaks.

    seat in ─► hearing ─► screen ─► user ─► Claude ─► turns ─► speaking ─► meter ─► seat out ─► assistant

**Turns.** A hearing vendor with its own turns (Ink) says when the caller starts and stops; its
start interrupts the agent (barge-in). The idle limit and the Timeline's `heard` stage come from
the same three events (start, words changed, end).

**The meter** counts what the vendors bill, as raw amounts only: Pipecat's usage metrics give
the seconds of audio sent to hearing (silence included) and the characters sent to speaking, the
same for every vendor. It never prices them: ehr does, with the voice crate's cost.rs
(`PriceVoiceUse`; VOICE-VENDORS.md §9), and the record keeps the raw use (`voice_use`). It also
keeps the time of the caller's last speech, for the idle limit. The vendor's dashboard is the
full count (CARTESIA.md §4).

**The limits** (`config.VOICE_IDLE_S`, `config.VOICE_MAX_S`) end a voice test call with no
spoken line: a dev guard for the plan's credits, not a caller's experience.

**The stages** (the Chat page's Timeline, CHAT.md §18): with a dev call record open, each turn's
voice stages go to it as `voice` events (call_record.py): hearing's final line with the time its
words last changed, speaking's first audio for each line, and the seat beginning to play it.
Times only, never words or audio.

Logs carry the kind of event only, never the words (SECURITY.md §4).
"""

from __future__ import annotations

import time
from typing import Any

from pipecat.frames.frames import (
    BotStartedSpeakingFrame,
    Frame,
    MetricsFrame,
    TTSAudioRawFrame,
    TTSStartedFrame,
)
from pipecat.metrics.metrics import STTUsageMetricsData, TTSUsageMetricsData
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor

from deets_agent import config
from deets_agent.voice import cartesia, vendors

# Each vendor's adapter, by its catalog id (VOICE-VENDORS.md §2.2).
ADAPTERS = {cartesia.ID: cartesia}


class VoiceMeter(FrameProcessor):
    """Counts hearing's seconds and speaking's characters from Pipecat's usage metrics, and the
    time of the caller's last speech. Sits after speaking, where both services' metrics have
    passed."""

    def __init__(
        self,
        idle_s: int = config.VOICE_IDLE_S,
        max_s: int = config.VOICE_MAX_S,
        chosen: dict[str, str] | None = None,
    ) -> None:
        super().__init__()
        # The vendor of each direction (`hearing`, `speaking`), for the use's record; the
        # catalog's defaults when none is given.
        first = vendors.defaults() if chosen is None else {}
        self.vendors = chosen or {d: vendors.chosen(first, d) for d in vendors.DIRECTIONS}
        # The raw amounts: seconds of audio heard, characters spoken.
        self.heard_s = 0.0
        self.spoken_chars = 0
        self.started = time.monotonic()
        self.last_speech = self.started
        # The test-call guard (the active settings).
        self.idle_s = idle_s
        self.max_s = max_s
        # The dev call record (Voice sets it): each line's first audio and its start at the seat.
        self.record = None
        self._line_started = False

    def use(self) -> dict[str, dict[str, Any]]:
        """The call's raw use so far, as the record's `voice_use` holds it (the voice crate's
        `cost::CallUse`): each direction's vendor and amount."""
        return {
            "hearing": {"vendor": self.vendors["hearing"], "amount": round(self.heard_s, 1)},
            "speaking": {"vendor": self.vendors["speaking"], "amount": self.spoken_chars},
        }

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        if isinstance(frame, MetricsFrame):
            for data in frame.data:
                if isinstance(data, STTUsageMetricsData):
                    self.heard_s += data.value.audio_seconds
                elif isinstance(data, TTSUsageMetricsData):
                    self.spoken_chars += data.value
        if self.record is not None:
            # Speaking's frames come down from it; the seat's "speaking" comes back up to here.
            if isinstance(frame, TTSStartedFrame):
                self._line_started = True
            elif isinstance(frame, TTSAudioRawFrame) and self._line_started:
                self._line_started = False
                self.record.event("voice", stage="sonic_audio")
            elif isinstance(frame, BotStartedSpeakingFrame) and direction == FrameDirection.UPSTREAM:
                self.record.event("voice", stage="speaking")
        await self.push_frame(frame, direction)

    def heard(self) -> None:
        """The caller spoke: hearing's turn start or turn end (Voice wires both)."""
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


class VoiceKeyMissing(Exception):
    """A chosen vendor's key is not in .env. The message names the key's name, never a key."""


class Voice:
    """The voice parts of one call: the hearing service, the speaking service and the meter,
    each built by the chosen vendor's adapter."""

    def __init__(self, values: dict[str, Any] | None = None, catalog: dict[str, Any] | None = None) -> None:
        """`values`: the active voice settings (VOICE-VENDORS.md §2.3); the defaults when none."""
        c = catalog or vendors.current()
        v = values or vendors.defaults(c)
        hear_id, speak_id = vendors.chosen(v, "hearing"), vendors.chosen(v, "speaking")
        missing = vendors.keys_missing(v, c)
        if missing:
            raise VoiceKeyMissing(f"add {' and '.join(missing)} to .env")
        hear, speak = ADAPTERS[hear_id], ADAPTERS[speak_id]
        self.vendors = {"hearing": hear_id, "speaking": speak_id}
        self.stt = hear.build_stt(vendors.key(hear_id, c), v, c)
        self.tts = speak.build_tts(vendors.key(speak_id, c), v, c)
        self.meter = VoiceMeter(
            int(v.get("idle_s") or config.VOICE_IDLE_S),
            int(v.get("max_s") or config.VOICE_MAX_S),
            chosen=dict(self.vendors),
        )
        self._record = None
        # When hearing's words for the current turn last changed (perf_counter): about when the
        # caller stopped speaking, since a turn's end waits for its own silence after it.
        self._last_words: float | None = None

        @self.stt.event_handler("on_turn_start")
        async def _start(service, *args):
            self.meter.heard()
            self._last_words = time.perf_counter()

        @self.stt.event_handler("on_turn_update")
        async def _update(service, *args):
            self._last_words = time.perf_counter()

        @self.stt.event_handler("on_turn_end")
        async def _end(service, *args):
            self.meter.heard()
            if self._record is not None:
                fields = {"last_words_ms": self._record.clock(self._last_words)} if self._last_words else {}
                self._record.event("voice", stage="heard", **fields)
            self._last_words = None

    @property
    def record(self):
        return self._record

    @record.setter
    def record(self, record) -> None:
        """The dev call record: the stages go to it (call.py sets it when one is open)."""
        self._record = record
        self.meter.record = record
