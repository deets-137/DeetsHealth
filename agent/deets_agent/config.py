"""Paths and settings for the agent. No value here is protected data."""

from __future__ import annotations

import os
from pathlib import Path

from dotenv import load_dotenv

AGENT = Path(__file__).resolve().parents[1]
REPO = AGENT.parent
CERTS = REPO / "dev-certs"
REPLAYS = AGENT / "tests" / "replays"
EHR_BINARY = REPO / "target" / "debug" / ("deetshealth-ehr.exe" if os.name == "nt" else "deetshealth-ehr")

# The cheapest model first; move up only when a named test call shows it falls short
# (VOICE-AGENT.md §8.3; the owner, 2026-10-20).
MODEL = "claude-haiku-5-5"
# Haiku 5.5 thinks by default and cannot turn it off at every effort; low effort is the
# fastest first token and the fewest tokens.
EFFORT = "low"
# A spoken reply is a sentence or two. The cap holds cost inside the $100-a-month budget.
MAX_TOKENS = 600


def anthropic_key() -> str | None:
    """The key from the git-ignored .env at the repo root. Never printed, never logged."""
    load_dotenv(REPO / ".env", override=False)
    key = os.environ.get("ANTHROPIC_API_KEY", "").strip()
    return key or None
