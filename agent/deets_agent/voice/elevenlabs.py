"""ElevenLabs' adapter (VOICE-VENDORS.md §2.2, §11; integrations/ELEVENLABS.md): Scribe hears the
caller, Flash speaks the agent.

**Scribe** is `ElevenLabsRealtimeSTTService` (`scribe_v2_realtime`, pinned) in its manual commit
mode: Scribe has no turns of its own, so Pipecat decides them (VOICE-VENDORS.md §4.3). Pipecat's
Silero voice detector, in the user aggregator, says when the caller starts and stops making
sound; each stop sends Scribe a commit, so it finishes the words so far. The Smart Turn model
(v3, local, on the CPU) then reads the audio and says whether the turn is over or the caller only
paused ("my date of birth is …"). `user_params()` gives call.py the detector and the two
strategies. Barge-in: the start strategies interrupt the agent only when `barge_in` is on; the
911 line is marked uninterruptible on its frame (flow.py), so it plays to its end either way.

**Flash** is `ElevenLabsTTSService` (`eleven_flash_v2_5`, pinned) over its WebSocket. Fixed
lines arrive as `TTSSpeakFrame` and Flash speaks them word for word.

**Zero retention** (VOICE-VENDORS.md §4, item 5, the owner's B): `enable_logging=False` on both
services while the account lever `elevenlabs.zero_retention` is on (its default). **Below
Enterprise the flag is accepted and ignored** (tested 2026-11-23: a Flash request sent with it
showed in the account's speech history). So `check_retention` refuses every call that is not
synthetic, with the switch on or off. Every call today is synthetic (call_record.py).

**Pronunciations** are not sent: Flash v2.5 reads no phoneme tags (ElevenLabs' docs: phoneme
tags work on Flash v2, Turbo v2 and English v1 only). The page says so.

Each lever's range and default, the pinned models and the rates come from the catalog
(`vendors/catalog/voice/elevenlabs.json`, served by ehr); this file maps them only.
"""

from __future__ import annotations

from typing import Any

from pipecat.audio.turn.smart_turn.base_smart_turn import SmartTurnParams
from pipecat.audio.turn.smart_turn.local_smart_turn_v3 import LocalSmartTurnAnalyzerV3
from pipecat.audio.vad.silero import SileroVADAnalyzer
from pipecat.audio.vad.vad_analyzer import VADParams
from pipecat.processors.aggregators.llm_response_universal import LLMUserAggregatorParams
from pipecat.services.elevenlabs.stt import CommitStrategy, ElevenLabsRealtimeSTTService
from pipecat.services.elevenlabs.tts import ElevenLabsTTSService
from pipecat.turns.user_start import TranscriptionUserTurnStartStrategy, VADUserTurnStartStrategy
from pipecat.turns.user_stop import TurnAnalyzerUserTurnStopStrategy
from pipecat.turns.user_turn_strategies import UserTurnStrategies

from deets_agent.voice import vendors

ID = "elevenlabs"
# The turn levers: Pipecat's voice detector and Smart Turn, not ElevenLabs' (Scribe has none).
TURN_LEVERS = ("vad_confidence", "vad_start_secs", "turn_stop_secs")
# Flash's voice levers and the names Pipecat gives them.
VOICE_LEVERS = {"speed": "speed", "stability": "stability", "similarity": "similarity_boost", "style": "style"}
# Every lever this file maps (a test checks the catalog lists no other).
MAPPED = {*TURN_LEVERS, *VOICE_LEVERS, "voice", "speaker_boost", "normalization"}
# Scribe has no turns of its own: Voice takes the turn events from the user aggregator.
OWN_TURNS = False


class RetentionRefused(Exception):
    """ElevenLabs with zero retention off on a call that is not synthetic (§4, item 5). The message
    names the lever, never a value from a call."""


def zero_retention(values: dict[str, Any]) -> bool:
    """Whether the version keeps zero retention on (a missing value is on: security first)."""
    return vendors.own(values, ID).get("zero_retention") is not False


def check_retention(values: dict[str, Any], data_class: str) -> None:
    """Refuse ElevenLabs on any call that is not synthetic, whatever `zero_retention` says. A plan
    below Enterprise accepts `enable_logging=false` and keeps the request anyway (tested
    2026-11-23: a Flash request with it showed in the account's history), and no BAA is signed
    (SECURITY.md §5). `values` is kept for the day a BAA and an Enterprise plan make the switch
    real."""
    del values
    if data_class != "synthetic":
        raise RetentionRefused("ElevenLabs takes synthetic calls only: no BAA, and this plan keeps requests")


def build_stt(key: str, values: dict[str, Any], catalog: dict[str, Any] | None = None) -> ElevenLabsRealtimeSTTService:
    """Scribe realtime in manual commit mode, with the shared keyterms."""
    model = vendors.vendor(ID, catalog)["hearing"]["model"]
    keyterms = [row[0] for row in values.get("keyterms") or []]
    return ElevenLabsRealtimeSTTService(
        api_key=key,
        commit_strategy=CommitStrategy.MANUAL,
        enable_logging=not zero_retention(values),
        settings=ElevenLabsRealtimeSTTService.Settings(model=model, keyterms=keyterms or None),
    )


def build_tts(key: str, values: dict[str, Any], catalog: dict[str, Any] | None = None) -> ElevenLabsTTSService:
    """Flash, pinned by name, with the active settings: the voice, its four voice levers, speaker
    boost and number reading. None sends nothing (ElevenLabs' default)."""
    mine = vendors.own(values, ID)
    speaking = vendors.vendor(ID, catalog)["speaking"]
    voice = (mine.get("voice") or {}).get("id") or speaking["voices"][0]["id"]
    given = {name: mine[k] for k, name in VOICE_LEVERS.items() if mine.get(k) is not None}
    if mine.get("speaker_boost") is not None:
        given["use_speaker_boost"] = bool(mine["speaker_boost"])
    return ElevenLabsTTSService(
        api_key=key,
        enable_logging=not zero_retention(values),
        settings=ElevenLabsTTSService.Settings(
            model=speaking["model"],
            voice=voice,
            apply_text_normalization=mine.get("normalization") or "auto",
            **given,
        ),
    )


def user_params(values: dict[str, Any], catalog: dict[str, Any] | None = None) -> LLMUserAggregatorParams:
    """The user aggregator's voice detector and turn strategies, for a call that Scribe hears:
    Silero says sound started or stopped; a start (or a first word) starts the caller's turn;
    Smart Turn ends it. The stop delay stays Pipecat's 0.2 s, which Smart Turn expects."""
    mine = vendors.own(values, ID)
    vad = {"confidence": mine.get("vad_confidence"), "start_secs": mine.get("vad_start_secs")}
    vad_params = VADParams(**{k: float(x) for k, x in vad.items() if x is not None})
    turn = SmartTurnParams(stop_secs=float(mine["turn_stop_secs"])) if mine.get("turn_stop_secs") is not None else None
    barge_in = bool(values.get("barge_in", True))
    return LLMUserAggregatorParams(
        vad_analyzer=SileroVADAnalyzer(params=vad_params),
        user_turn_strategies=UserTurnStrategies(
            start=[
                VADUserTurnStartStrategy(enable_interruptions=barge_in),
                TranscriptionUserTurnStartStrategy(enable_interruptions=barge_in),
            ],
            stop=[TurnAnalyzerUserTurnStopStrategy(turn_analyzer=LocalSmartTurnAnalyzerV3(params=turn))],
        ),
    )
