"""Cartesia's adapter (VOICE-VENDORS.md §2.2; integrations/CARTESIA.md §9 lists every lever):
Ink hears the caller, Sonic speaks the agent.

**Ink** is `CartesiaTurnsSTTService`: Ink-2's own turn detection, so no local voice detector
runs. Its `turn.start` interrupts the agent (barge-in); its `turn.end` gives the caller's line.
Eager end of turn stays off: it starts Claude on a guess that Ink may withdraw.

**Sonic** is `CartesiaTTSService`. Fixed lines arrive as `TTSSpeakFrame` (flow.py, screen.py) and
Sonic speaks them word for word; the 911 line cannot be interrupted (screen.py, flow.py).

Each lever's range, default and the pinned models come from the catalog (`vendors/catalog/voice/
cartesia.json`, served by ehr); this file maps them to Pipecat's parameters only.
"""

from __future__ import annotations

from typing import Any

from pipecat.services.cartesia.tts import CartesiaTTSService, GenerationConfig
from pipecat.services.cartesia.turns.stt import CartesiaTurnsSTTService

from deets_agent.voice import vendors

ID = "cartesia"
# Ink's four turn levers: the catalog lists them; None sends nothing (Cartesia's default).
TURN_LEVERS = ("turn_start_threshold", "turn_eager_end_threshold", "turn_end_threshold", "turn_end_timeout_ms")
# Every lever this file maps (a test checks the catalog lists no other).
MAPPED = {*TURN_LEVERS, "eager_end_of_turn", "voice", "speed", "volume", "emotion", "normalization"}
# Ink has its own turns: the three events Voice needs come from it.
OWN_TURNS = True


def build_stt(key: str, values: dict[str, Any], catalog: dict[str, Any] | None = None) -> CartesiaTurnsSTTService:
    """Ink on the turns endpoint, with the active settings: the four turn values, barge-in,
    early end of turn, keyterms (shared)."""
    mine = vendors.own(values, ID)
    model = vendors.vendor(ID, catalog)["hearing"]["model"]
    turn = {k: mine.get(k) for k in TURN_LEVERS if mine.get(k) is not None}
    if "turn_end_timeout_ms" in turn:
        turn["turn_end_timeout_ms"] = int(turn["turn_end_timeout_ms"])
    keyterms = [row[0] for row in values.get("keyterms") or []]
    return CartesiaTurnsSTTService(
        api_key=key,
        should_interrupt=bool(values.get("barge_in", True)),
        enable_eager_end_of_turn=bool(mine.get("eager_end_of_turn", False)),
        settings=CartesiaTurnsSTTService.Settings(model=model, keyterm=keyterms or None, **turn),
    )


def build_tts(key: str, values: dict[str, Any], catalog: dict[str, Any] | None = None) -> CartesiaTTSService:
    """Sonic, pinned by name, with the active settings: the voice, speed, volume, emotion and
    number reading. The output rate is the pipeline's. Pronunciations are stored but not sent
    yet (CONSOLE.md §7.6)."""
    mine = vendors.own(values, ID)
    speaking = vendors.vendor(ID, catalog)["speaking"]
    voice = (mine.get("voice") or {}).get("id") or speaking["voices"][0]["id"]
    gen = {k: mine[k] for k in ("speed", "volume", "emotion") if mine.get(k) is not None}
    return CartesiaTTSService(
        api_key=key,
        settings=CartesiaTTSService.Settings(
            model=speaking["model"],
            voice=voice,
            generation_config=GenerationConfig(**gen) if gen else None,
            normalization=mine.get("normalization") or "auto",
        ),
    )
