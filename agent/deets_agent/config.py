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
