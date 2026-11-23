"""The voice ends of a call (VOICE-VENDORS.md): a vendor hears the caller, a vendor speaks the
agent. Each direction's vendor is a setting (`hearing.vendor`, `speaking.vendor`); each vendor
has an adapter here: `cartesia.py` (Ink hears, Sonic speaks; the default of both) and
`elevenlabs.py` (Scribe hears, Flash speaks).

    seat in ─► hearing ─► screen ─► user ─► Claude ─► turns ─► speaking ─► meter ─► seat out ─► assistant

**Turns.** A hearing vendor with its own turns (Ink) says when the caller starts and stops; its
start interrupts the agent (barge-in). A vendor without (Scribe) hears only: the user aggregator
runs Pipecat's voice detector and Smart Turn (the adapter's `user_params`), and `HeardWords` sits
after the service. The idle limit and the Timeline's `heard` stage come from the same three
events (start, words changed, end), whoever gives them.

**The meter** counts what the vendors bill, as raw amounts only: Pipecat's usage metrics give
the seconds of audio sent to hearing (silence included) and the characters sent to speaking, the
same for every vendor. It never prices them: ehr does, with the vendors crate's cost.rs
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
    InterimTranscriptionFrame,
    MetricsFrame,
    TranscriptionFrame,
    TTSAudioRawFrame,
    TTSStartedFrame,
)
from pipecat.metrics.metrics import STTUsageMetricsData, TTSUsageMetricsData
from pipecat.processors.frame_processor import FrameDirection, FrameProcessor

from deets_agent import config
from deets_agent.voice import cartesia, elevenlabs, vendors

# Each vendor's adapter, by its catalog id (VOICE-VENDORS.md §2.2).
ADAPTERS = {cartesia.ID: cartesia, elevenlabs.ID: elevenlabs}


class HeardWords(FrameProcessor):
    """For a hearing vendor without its own turns (Scribe): sits right after it and says when its
    words for the current turn change (an interim or a final line). Never reads the words."""

    def __init__(self, changed) -> None:
        super().__init__()
        self._changed = changed

    async def process_frame(self, frame: Frame, direction: FrameDirection) -> None:
        await super().process_frame(frame, direction)
        if direction == FrameDirection.DOWNSTREAM and isinstance(frame, (InterimTranscriptionFrame, TranscriptionFrame)):
            self._changed()
        await self.push_frame(frame, direction)


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
        """The call's raw use so far, as the record's `voice_use` holds it (the vendors crate's
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

    def __init__(
        self, values: dict[str, Any] | None = None, catalog: dict[str, Any] | None = None, *, data_class: str
    ) -> None:
        """`values`: the active voice settings (VOICE-VENDORS.md §2.3); the defaults when none.
        `data_class`: the call's class (`synthetic` for every call today; call_record.py), which
        a vendor's retention rule reads (ElevenLabs, §4, item 5). No default, so no call skips it."""
        c = catalog or vendors.current()
        v = values or vendors.defaults(c)
        hear_id, speak_id = vendors.chosen(v, "hearing"), vendors.chosen(v, "speaking")
        missing = vendors.keys_missing(v, c)
        if missing:
            raise VoiceKeyMissing(f"add {' and '.join(missing)} to .env")
        if elevenlabs.ID in (hear_id, speak_id):
            elevenlabs.check_retention(v, data_class)
        hear, speak = ADAPTERS[hear_id], ADAPTERS[speak_id]
        self.vendors = {"hearing": hear_id, "speaking": speak_id}
        # Each direction's product and vendor, for a failure's reason ("Scribe", "ElevenLabs").
        self.names = {
            d: (vendors.vendor(vid, c)[d]["product"], vendors.vendor(vid, c)["name"]) for d, vid in self.vendors.items()
        }
        self.stt = hear.build_stt(vendors.key(hear_id, c), v, c)
        self.tts = speak.build_tts(vendors.key(speak_id, c), v, c)
        # A hearing vendor with its own turns (Ink) gives the three turn events itself. Without
        # (Scribe), the user aggregator's voice detector and Smart Turn decide the turns:
        # `user_params` holds them for call.py, `attach` takes the start and end from the
        # aggregator, and `HeardWords` after the service says when the words change.
        self.own_turns = bool(hear.OWN_TURNS)
        self.user_params = None if self.own_turns else hear.user_params(v, c)
        self.hearing: list[FrameProcessor] = [self.stt]
        if not self.own_turns:
            self.hearing.append(HeardWords(self._words_changed))
        self.meter = VoiceMeter(
            int(v.get("idle_s") or config.VOICE_IDLE_S),
            int(v.get("max_s") or config.VOICE_MAX_S),
            chosen=dict(self.vendors),
        )
        self._record = None
        # When hearing's words for the current turn last changed (perf_counter): about when the
        # caller stopped speaking, since a turn's end waits for its own silence after it.
        self._last_words: float | None = None

        if self.own_turns:
            self.stt.add_event_handler("on_turn_start", self._turn_started)
            self.stt.add_event_handler("on_turn_update", self._turn_updated)
            self.stt.add_event_handler("on_turn_end", self._turn_ended)

    def attach(self, user_aggregator) -> None:
        """For a hearing vendor without its own turns: take the caller's turn start and end from
        the user aggregator (call.py calls this once it has built the aggregators)."""
        if self.own_turns:
            return
        user_aggregator.add_event_handler("on_user_turn_started", self._turn_started)
        user_aggregator.add_event_handler("on_user_turn_stopped", self._turn_ended)

    def _words_changed(self) -> None:
        self._last_words = time.perf_counter()

    # The three turn events (start, words changed, end), whoever gives them.
    async def _turn_started(self, *_args) -> None:
        self.meter.heard()
        self._last_words = time.perf_counter()

    async def _turn_updated(self, *_args) -> None:
        self._words_changed()

    async def _turn_ended(self, *_args) -> None:
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
