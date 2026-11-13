"""Paths and settings for the agent. No value here is protected data."""

from __future__ import annotations

import os
from pathlib import Path

from dotenv import load_dotenv

AGENT = Path(__file__).resolve().parents[1]
REPO = AGENT.parent
CERTS = REPO / "dev-certs"
REPLAYS = AGENT / "tests" / "replays"
# DEETS_EHR_BIN points at another build, for when a running dev server holds this one (Windows
# locks a running .exe; scripts/postgres.mjs reads the same setting).
EHR_BINARY = Path(os.environ["DEETS_EHR_BIN"]) if os.environ.get("DEETS_EHR_BIN") else (
    REPO / "target" / "debug" / ("deetshealth-ehr.exe" if os.name == "nt" else "deetshealth-ehr")
)

# The cheapest model first; move up only when a named test call shows it falls short
# (VOICE-AGENT.md §8.3; the owner, 2026-10-20).
MODEL = "claude-haiku-5-5"
# Haiku 5.5 thinks by default and cannot turn it off at every effort; low effort is the
# fastest first token and the fewest tokens.
EFFORT = "low"
# A spoken reply is a sentence or two, but Haiku thinks first, and at 600 one reply spent the
# whole cap thinking and said nothing (2026-11-12). The owner raised it: of 663 requests the
# 95th percentile used 261. At $0.50 a million output tokens, the room costs nothing unless used.
MAX_TOKENS = 1200
# A reply cut off with nothing to say is sent once more with this cap, after the holding line
# (llm.py `_CutOffGuard`).
RETRY_MAX_TOKENS = 2400

# The tools whose fixed line ends the call (flow.py). Words from the model before one of these
# contradict it ("Goodbye." then "I'm connecting you…"); the tests allow none (AGENT.md §3).
CLOSING_TOOLS = frozenset({"end_call", "hand_off"})
# Tools that speak a fixed line the model must not speak before (the role prompt says so). The
# tests allow no words before any of them. `emergency` does not end the call.
SILENT_TOOLS = CLOSING_TOOLS | {"emergency", "safety_reminder", "medical_question", "offer_staff"}


def anthropic_key() -> str | None:
    """The key from the git-ignored .env at the repo root. Never printed, never logged."""
    load_dotenv(REPO / ".env", override=False)
    key = os.environ.get("ANTHROPIC_API_KEY", "").strip()
    return key or None


# ---- voice: Cartesia Ink in, Sonic out (integrations/CARTESIA.md §9 lists every lever) ----

# Pinned by name: Cartesia lists no dated snapshots (CARTESIA.md §3). Never `sonic-latest`.
STT_MODEL = "ink-2"
TTS_MODEL = "sonic-3.6"
# Skylar, from Cartesia's list of stable voices for agents (CARTESIA.md §3): a draft until the
# owner picks one by ear. CARTESIA_VOICE_ID in .env overrides it.
DEFAULT_VOICE_ID = "db6b0ed5-d5d3-463d-ae85-518a07d3c2b4"

# What Cartesia charges (docs.cartesia.ai/pricing, read 2026-11-12): Ink-2, 3 credits per second
# of audio streamed, silence included; Sonic, about 1 credit per character.
INK_CREDITS_PER_S = 3
SONIC_CREDITS_PER_CHAR = 1

# The limits of a voice test call (the owner, 2026-11-13: 60 s and 6 min). A dev guard for the
# Free plan's credits, not a caller's experience: the call ends with no spoken line. What a
# real call does is a hospital setting (FUTURE-SETTINGS.md §12).
VOICE_IDLE_S = 60
VOICE_MAX_S = 6 * 60

# The Free plan's credits per month (cartesia.ai/pricing). CARTESIA_PLAN_CREDITS in .env sets
# another plan's; CARTESIA_CYCLE_DAY the day of the month the credits renew (the day the
# account was opened).
FREE_PLAN_CREDITS = 20_000


def cartesia_key() -> str | None:
    """The Cartesia key from the git-ignored .env. Never printed, never logged."""
    load_dotenv(REPO / ".env", override=False)
    key = os.environ.get("CARTESIA_API_KEY", "").strip()
    return key or None


def voice_id() -> str:
    load_dotenv(REPO / ".env", override=False)
    return os.environ.get("CARTESIA_VOICE_ID", "").strip() or DEFAULT_VOICE_ID


def _env_int(name: str, default: int, low: int, high: int) -> int:
    load_dotenv(REPO / ".env", override=False)
    try:
        value = int(os.environ.get(name, "").strip())
    except ValueError:
        return default
    return value if low <= value <= high else default


def plan_credits() -> int:
    return _env_int("CARTESIA_PLAN_CREDITS", FREE_PLAN_CREDITS, 1, 1_000_000_000)


def cycle_day() -> int:
    """The day of the month Cartesia's credits renew. 28 at most, so every month has it."""
    return _env_int("CARTESIA_CYCLE_DAY", 1, 1, 28)
