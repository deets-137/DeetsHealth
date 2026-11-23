"""The model catalog as the agent reads it (VOICE-VENDORS.md §10). The catalog lives in the
`deetshealth-vendors` crate (`vendors/catalog/model/*.json`), served by ehr; the agent keeps no
copy of a model, a price, an effort or a range, and prices nothing itself (`PriceModelUse`).

- `load(ehr)` asks ehr (`GetModelCatalog`) once it is up.
- Before that, and in the tests, `current()` asks the ehr binary (`ehr model-catalog`): the same
  compiled-in catalog, with no service running.

The `llm` settings keep their keys (`model`, `effort`, `thinking`, …): one model vendor today.
No value here is protected data.
"""

from __future__ import annotations

import json
import os
import subprocess
from typing import Any

from dotenv import load_dotenv

from deets_agent import config

_catalog: dict[str, Any] | None = None


async def load(ehr) -> dict[str, Any]:
    """Read the catalog from ehr and hold it."""
    global _catalog
    _catalog = await ehr.model_catalog()
    return _catalog


def current() -> dict[str, Any]:
    """The held catalog; the first time with none, the ehr binary's (`ehr model-catalog`)."""
    global _catalog
    if _catalog is None:
        out = subprocess.run(
            [str(config.EHR_BINARY), "model-catalog"], capture_output=True, text=True, timeout=30, check=False
        )
        if out.returncode != 0:
            raise RuntimeError(f"ehr model-catalog: exit {out.returncode}")
        _catalog = json.loads(out.stdout)
    return _catalog


def vendor(catalog: dict[str, Any] | None = None) -> dict[str, Any]:
    """The model vendor the agent runs with (the catalog's first)."""
    return (catalog or current())["vendors"][0]


def model(mid: str, catalog: dict[str, Any] | None = None) -> dict[str, Any] | None:
    return next((m for m in vendor(catalog)["models"] if m["id"] == mid), None)


def thinking_off(mid: str, catalog: dict[str, Any] | None = None) -> str | None:
    """What the API calls thinking turned off on this model; None where it cannot be."""
    m = model(mid, catalog)
    return m.get("thinking_off") if m else None


def inference_geo(catalog: dict[str, Any] | None = None) -> str:
    """Where every live request runs (`inference_geo`; ANTHROPIC.md §2)."""
    return str(vendor(catalog)["inference_geo"])


def _env_int(name: str, low: int, high: int) -> int | None:
    load_dotenv(config.REPO / ".env", override=False)
    try:
        value = int(os.environ.get(name, "").strip())
    except ValueError:
        return None
    return value if low <= value <= high else None


def defaults(catalog: dict[str, Any] | None = None) -> dict[str, Any]:
    """Every `llm` setting at its first value (the catalog's `defaults`), with `.env` laid on it:
    `ANTHROPIC_BUDGET_USD`, `ANTHROPIC_CYCLE_DAY` (the vendor's id, upper case). The Default
    version, and what the tests and the typed harness run with, so their recorded replies stay
    valid."""
    c = catalog or current()
    out = dict(c["defaults"])
    env = vendor(c)["id"].upper()
    budget = _env_int(f"{env}_BUDGET_USD", 0, 1_000_000)
    if budget is not None:
        out["budget_usd"] = budget
    day = _env_int(f"{env}_CYCLE_DAY", 1, 28)
    if day is not None:
        out["cycle_day"] = day
    return out


def use(mid: str, cache_ttl: str, geo: str, usage: dict[str, int]) -> dict[str, Any]:
    """One use as `PriceModelUse` takes it: the model, the cache's life, the place, the tokens in
    Anthropic's names."""
    return {"model": mid, "cache_ttl": cache_ttl, "geo": geo, "usage": dict(usage)}
