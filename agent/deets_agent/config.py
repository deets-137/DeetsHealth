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


# ---- the language model's settings (Config › Anthropic, CONSOLE.md §8) ----
# The code's values above are the Default version; a saved version changes them for the next
# test call. The tests and the typed harness always run with the code's values, so their
# recorded replies stay valid.

# The models the page offers and what each accepts (ANTHROPIC.md §3; Anthropic's API reference,
# cached 2026-11-03). Effort's default differs by model. Thinking "off" is `disabled` on Haiku
# (effort high or below), `between_tools` on Sonnet (effort high or below); Opus 5.5 cannot turn
# it off. Prices are dollars per 1M tokens: input, output, cache read. ehr's settings.rs › LLM
# and `coherent` hold the same rules.
LLM_MODELS: dict[str, dict] = {
    "claude-haiku-5-5": {"name": "Haiku 5.5", "effort_default": "medium", "thinking_off": "disabled", "price": (0.10, 0.50, 0.01)},
    "claude-sonnet-5-5": {"name": "Sonnet 5.5", "effort_default": "high", "thinking_off": "between_tools", "price": (2.00, 10.00, 0.10)},
    "claude-opus-5-5": {"name": "Opus 5.5", "effort_default": "medium", "thinking_off": None, "price": (4.00, 20.00, 0.20)},
}
EFFORTS = ("low", "medium", "high", "xhigh", "max")
# Effort levels at which thinking may be off (Haiku and Sonnet).
THINKING_OFF_EFFORTS = ("low", "medium", "high")
# A cache write costs this many times the input price, by the cache's life (5 minutes, 1 hour).
CACHE_WRITE = {"5m": 1.25, "1h": 2.0}
# US-only inference (`inference_geo: "us"`, ANTHROPIC.md §2) costs 1.1× on every token.
INFERENCE_GEO = "us"
GEO_PRICE = 1.1
# The token caps' ranges on the page (settings.rs › LLM).
LLM_RANGES = {
    "max_tokens": {"min": 256, "max": 16_000},
    "retry_max_tokens": {"min": 256, "max": 32_000},
}
# The Anthropic budget a month in dollars (the owner: $100 of API credit a month).
# ANTHROPIC_BUDGET_USD and ANTHROPIC_CYCLE_DAY in .env set others.
BUDGET_USD = 100


def llm_defaults() -> dict:
    """Every llm setting at the code's value: what a test call runs with before any version is
    saved, and the Default version. The same names as ehr's settings.rs › LLM."""
    return {
        "model": MODEL,
        "effort": EFFORT,
        "thinking": "on",
        "max_tokens": MAX_TOKENS,
        "retry_max_tokens": RETRY_MAX_TOKENS,
        "cache_ttl": "5m",
        "budget_usd": _env_int("ANTHROPIC_BUDGET_USD", BUDGET_USD, 0, 1_000_000),
        "offset_usd": 0,
        "cycle_day": _env_int("ANTHROPIC_CYCLE_DAY", 1, 1, 28),
    }


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


# The voices Cartesia recommends for agents (its "Choosing a voice" guide, read 2026-11-12): the
# Config page's list. An id is known only where Cartesia's docs print it; the rest resolve by
# name from Cartesia's voice list when saving is built.
RECOMMENDED_VOICES: tuple[tuple[str, str | None], ...] = (
    ("Skylar", DEFAULT_VOICE_ID),
    ("Gemma", None),
    ("Jacqueline", None),
    ("Emma", None),
    ("Lauren", None),
    ("Daniel", "47c38ca4-5f35-497b-b1a3-415245fb35e1"),
    ("Archie", None),
    ("Parker", None),
    ("Jameson", None),
    ("Corey", None),
)

# Each lever's range and Cartesia's default (CARTESIA.md §9), for the Config page. None as a
# current value means "Cartesia's default": we send nothing for it.
INK_TURN_LEVERS = {
    "turn_start_threshold": {"min": 0.5, "max": 0.9, "step": 0.05, "default": 0.8},
    "turn_eager_end_threshold": {"min": 0.3, "max": 0.8, "step": 0.05, "default": 0.6},
    "turn_end_threshold": {"min": 0.05, "max": 0.5, "step": 0.05, "default": 0.3},
    "turn_end_timeout_ms": {"min": 640, "max": 11200, "step": 80, "default": 5600},
}
SONIC_LEVERS = {
    "speed": {"min": 0.6, "max": 1.5, "step": 0.05, "default": 1.0},
    "volume": {"min": 0.5, "max": 2.0, "step": 0.05, "default": 1.0},
}
SONIC_EMOTIONS = ("neutral", "calm", "content", "sad", "angry")
# Cartesia's starting sets of the four turn settings (docs.cartesia.ai › STT › Turns, read
# 2026-11-13). Not an API parameter: a profile is these four values. Balanced is the default.
TURN_PROFILES = {
    "balanced": {"turn_start_threshold": 0.8, "turn_eager_end_threshold": 0.6, "turn_end_threshold": 0.3, "turn_end_timeout_ms": 5600},
    "responsive": {"turn_start_threshold": 0.7, "turn_eager_end_threshold": 0.6, "turn_end_threshold": 0.4, "turn_end_timeout_ms": 4500},
    "patient": {"turn_start_threshold": 0.8, "turn_eager_end_threshold": 0.3, "turn_end_threshold": 0.1, "turn_end_timeout_ms": 8000},
}


def voice_defaults() -> dict:
    """Every voice setting at its default: what a call runs with before any version is saved,
    and the set the Config page starts from. The same names as ehr's settings.rs › VOICE.
    None is the provider's default."""
    vid = voice_id()
    name = next((n for n, i in RECOMMENDED_VOICES if i == vid), vid)
    return {
        "voice": {"name": name, "id": vid},
        "speed": None,
        "volume": None,
        "emotion": None,
        "normalization": "auto",
        "pronunciations": [],
        **{k: None for k in INK_TURN_LEVERS},
        "eager_end_of_turn": False,
        "barge_in": True,
        "keyterms": [],
        "idle_s": VOICE_IDLE_S,
        "max_s": VOICE_MAX_S,
        "plan_credits": plan_credits(),
        "offset": 0,
        "cycle_day": cycle_day(),
    }


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
