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

# The tools whose fixed line ends the call (flow.py). Words from the model before one of these
# contradict it ("Goodbye." then "I'm connecting you…"); the tests allow none (AGENT.md §3).
CLOSING_TOOLS = frozenset({"end_call", "hand_off"})
# Tools that speak a fixed line the model must not speak before (the role prompt says so). The
# tests allow no words before any of them. `emergency` does not end the call.
SILENT_TOOLS = CLOSING_TOOLS | {"emergency", "safety_reminder", "medical_question", "offer_staff"}


# ---- the language model (VOICE-VENDORS.md §10) ----
# The models, their prices, the efforts, the thinking rules, the cache and inference factors, the
# `llm` settings' ranges and the code's defaults (Haiku 5.5, effort low, 1,200 and 2,400 tokens,
# a $100 budget) are in the model catalog (the vendors crate, vendors/catalog/model/*.json),
# which ehr serves: models.py reads it. Nothing of a vendor's is copied here.


def anthropic_key() -> str | None:
    """The key from the git-ignored .env at the repo root. Never printed, never logged."""
    load_dotenv(REPO / ".env", override=False)
    key = os.environ.get("ANTHROPIC_API_KEY", "").strip()
    return key or None


# ---- voice (VOICE-VENDORS.md) ----
# Each vendor's models, levers, ranges, rates and recommended voices are in the voice catalog
# (the `deetshealth-vendors` crate, vendors/catalog/voice/*.json), which ehr serves: voice/vendors.py
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

