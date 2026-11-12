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


@asynccontextmanager
async def running_ehr(
    binary: Path = config.EHR_BINARY,
    certs: Path = config.CERTS,
    env: dict[str, str] | None = None,
):
    """Yields ehr's address (host:port). Stops ehr on exit. `env` adds settings to ehr's
    environment (the dev server's fake Twilio, ehr/src/texts.rs)."""
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
        env={**os.environ, **env} if env else None,
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
