"""The typed call (test layer 3; BUILD-ORDER.md Phase 1's desk test): type as the caller, read
the agent's replies.

    uv run python harness/typed_call.py [--mode record|replay|live] [--script FILE]

- It starts ehr on a free port with the fake patients, connects over mTLS, and runs the real
  Flow with Claude Haiku 5.5.
- `record` (the default): a reply already on disk is replayed for free; a new one calls Claude
  once and is kept in tests/replays/. `replay`: no network, a new line is an error. `live`:
  no cache.
- `--script FILE`: one caller line per line of the file, instead of typing. A script run twice
  costs tokens once.
- Type `/quit` to hang up.

Fake callers (ehr/src/verify.rs): Jordan Ellis, born 1984-03-12, has a visit on November 10.
Priya Raman, born 1971-11-30, has none. The text code is always 246810.
"""

from __future__ import annotations

import argparse
import asyncio
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from deets_agent import config, redact
from deets_agent.call import TypedCall
from deets_agent.ehr_client import EhrClient
from deets_agent.ehr_process import running_ehr
from deets_agent.llm import Mode, build_llm


async def _lines(script: Path | None):
    if script is not None:
        for line in script.read_text(encoding="utf-8").splitlines():
            if line.strip() and not line.startswith("#"):
                print(f"you > {line.strip()}")
                yield line.strip()
        return
    loop = asyncio.get_running_loop()
    while True:
        line = await loop.run_in_executor(None, lambda: input("you > "))
        yield line


async def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--mode", choices=[m.value for m in Mode], default=Mode.RECORD.value)
    parser.add_argument("--script", type=Path)
    parser.add_argument("--log", default="WARNING", help="log level (the logs carry no PHI)")
    args = parser.parse_args()
    redact.install(args.log)

    async with running_ehr() as target:
        ehr = await EhrClient.connect(target, config.CERTS)
        call = TypedCall(ehr, build_llm(Mode(args.mode)))
        try:
            greeting = await call.start()
            print(f"agent > {greeting}")
            async for line in _lines(args.script):
                if line.strip() == "/quit":
                    break
                reply = await call.say(line)
                if reply is not None:
                    print(f"agent > {reply}")
                if call.ended:
                    print("(the call ended)")
                    break
        finally:
            await call.hang_up()
            await ehr.close()
    return 0


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
