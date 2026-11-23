"""Paths and settings for the agent. No value here is protected data."""

from __future__ import annotations

import json
import os
from pathlib import Path

from dotenv import load_dotenv

AGENT = Path(__file__).resolve().parents[1]
REPO = AGENT.parent
CERTS = REPO / "dev-certs"
REPLAYS = AGENT / "tests" / "replays"
# The dev hospital's own setup: the Default version of Config › Hospital, which ehr builds in
# (ehr/src/hospital.rs; HOSPITAL-CONFIG.md).
HOSPITAL = REPO / "ehr" / "fixtures" / "hospital.json"
# Every list and number of the hospital, in the page's order: ehr's hospital.rs › KEYS and
# settings.rs › HOSPITAL are the same list (test_dev_server.py checks the file holds each).
HOSPITAL_KEYS = (
    "time_zone",
    "visit_type_list",
    "visit_types",
    "visit_groups",
    "diagnosis_list",
    "reasons",
    "times_of_day",
    "hours",
    "closed_days",
    "facilities",
    "locations",
    "specialty_list",
    "specialties",
    "slots_offered",
    "narrow_over",
    "groups_named",
    "past_days",
    "established_years",
    "sooner_with_others",
    "ahead_windows",
    "booking_note_label",
    # The Booking rules (BOOKING-RULES.md §3.5).
    "booking_rules",
)
# The limits ehr's hospital.rs checks, for the page's own checks before a save. The same numbers
# as CELL_MAX, ROWS_MAX, PLACES_MAX (facilities and departments), WORDS_MAX, TIMES_MAX and
# WINDOWS_MAX there.
HOSPITAL_LIMITS = {"cell": 120, "rows": 200, "places": 2000, "words": 30, "times": 12, "windows": 5}


def hospital_defaults() -> dict:
    """The hospital's Default version: ehr/fixtures/hospital.json, each list and number."""
    data = json.loads(HOSPITAL.read_text(encoding="utf-8"))
    return {k: data[k] for k in HOSPITAL_KEYS}
# DEETS_EHR_BIN points at another copy of ehr: npm run dev sets it to target/run's copy, so a
# running dev server never locks target/debug's (Windows locks a running .exe).
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


# ---- voice (VOICE-VENDORS.md) ----
# Each vendor's models, levers, ranges, rates and recommended voices are in the voice catalog
# (the `deetshealth-voice` crate, voice/catalog/*.json), which ehr serves: voice/vendors.py
# reads it. Nothing of a vendor's is copied here.

# The limits of a voice test call (the owner, 2026-11-13: 60 s and 6 min). A dev guard for the
# Free plan's credits, not a caller's experience: the call ends with no spoken line. What a
# real call does is a hospital setting (FUTURE-SETTINGS.md §12).
VOICE_IDLE_S = 60
VOICE_MAX_S = 6 * 60


def _env_int(name: str, default: int, low: int, high: int) -> int:
    load_dotenv(REPO / ".env", override=False)
    try:
        value = int(os.environ.get(name, "").strip())
    except ValueError:
        return default
    return value if low <= value <= high else default

