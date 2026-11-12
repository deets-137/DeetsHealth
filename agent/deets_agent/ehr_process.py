"""Starts the ehr service for a typed call or a test, on a port the OS picks (never a fixed
port: several Deets apps run on this PC at once). ehr prints its address as its first line.
"""

from __future__ import annotations

import asyncio
import os
import re
from contextlib import asynccontextmanager
from pathlib import Path

from deets_agent import config

_LISTENING = re.compile(r"^ehr listening on (\S+)$")


_STORE_KEYS = ("DEETS_DATABASE_URL", "DEETS_SEAL_KEY_FILE")


def store_env() -> dict[str, str]:
    """The call store's settings for ehr (DATABASE.md §4a). `npm run dev` sets them; a typed call
    run by hand reads the ones scripts/postgres.mjs left in the git-ignored `.pg/ehr.env`."""
    if all(os.environ.get(k) for k in _STORE_KEYS):
        return {k: os.environ[k] for k in _STORE_KEYS}
    path = config.REPO / ".pg" / "ehr.env"
    if not path.exists():
        return {}
    found = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        key, _, value = line.partition("=")
        if key in _STORE_KEYS and value:
            found[key] = value
    return found if len(found) == len(_STORE_KEYS) else {}


@asynccontextmanager
async def running_ehr(
    binary: Path = config.EHR_BINARY,
    certs: Path = config.CERTS,
    env: dict[str, str] | None = None,
    store: bool = False,
):
    """Yields ehr's address (host:port). Stops ehr on exit. `env` adds settings to ehr's
    environment (the dev server's fake Twilio, ehr/src/texts.rs). `store` gives ehr the call
    store, so each ended call is filed (FileCall); the tests run without one."""
    env = {**(store_env() if store else {}), **(env or {})}
    base = {k: v for k, v in os.environ.items() if store or k not in _STORE_KEYS}
    if not binary.exists():
        raise FileNotFoundError("ehr is not built. Run: cargo build -p deetshealth-ehr")
    if not (certs / "agent.pem").exists():
        raise FileNotFoundError(
            "No dev certificates. Run: cargo run -p deetshealth-ehr --example dev_certs"
        )
    proc = await asyncio.create_subprocess_exec(
        str(binary),
        "--port",
        "0",
        "--certs",
        str(certs),
        env={**base, **env},
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    try:
        line = await asyncio.wait_for(proc.stdout.readline(), timeout=10)
        match = _LISTENING.match(line.decode().strip())
        if not match:
            err = (await proc.stderr.read()).decode().strip()
            raise RuntimeError(f"ehr did not start: {err or 'no output'}")
        yield match.group(1)
    finally:
        if proc.returncode is None:
            proc.kill()
            await proc.wait()
