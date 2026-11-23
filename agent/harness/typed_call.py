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
- Each call writes a dev call record to agent/calls/ (call_record.py): both sides, each tool
  call and result, each Claude request with its time and tokens, each ehr call, and library
  errors in full. Synthetic callers only. `--no-record` turns it off. Read one with
  `uv run python harness/show_call.py` (the newest) or `... show_call.py FILE`.

Fake callers (ehr/src/verify.rs): Jordan Ellis, born 1984-03-12, has a visit on November 10.
Priya Raman, born 1971-11-30, has none. The text code is always 246810.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from datetime import date
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from deets_agent import config, llm, models, redact
from deets_agent.call import CallFailed, TypedCall
from deets_agent.call_record import CALLS, CallRecord
from deets_agent.ehr_client import EhrClient, EhrError
from deets_agent.ehr_process import running_ehr
from deets_agent.flow import FAKE_TODAY
from deets_agent.llm import Mode, build_llm

# The test data, shown when the call starts. All of it is synthetic (ehr/src/verify.rs); keep it
# in step with that file. Nothing is texted: the code below is the only one ehr accepts.
TEST_DATA = """(test data: Jordan Ellis, born March 12, 1984, has a visit on November 10
 - Priya Raman, born November 30, 1971, has none
 - Casey Morgan and Casey Morgen share June 5, 1990, so neither verifies
 - the text code is always 246810; no text is sent - type /quit to hang up)"""


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
    parser.add_argument("--no-record", action="store_true", help="write no dev call record")
    parser.add_argument(
        "--origin",
        choices=["typed", "kept"],
        default="typed",
        help="who starts the call (DATABASE.md §7.1): `kept` for a kept-script draw in a Test run",
    )
    parser.add_argument(
        "--calls",
        type=Path,
        default=None,
        help="the folder for the record (default agent/calls/; a Test run passes a temporary one)",
    )
    parser.add_argument(
        "--keys-out", type=Path, help="write the key of each Claude request, as JSON (Record and test's dry run)"
    )
    parser.add_argument(
        "--today",
        default=FAKE_TODAY.isoformat(),
        help="the agent's today, YYYY-MM-DD or 'real' (default: the day ehr's fake slots follow)",
    )
    args = parser.parse_args()
    if args.keys_out is not None:
        llm.SEEN = []
    today = date.today() if args.today == "real" else date.fromisoformat(args.today)
    record = (
        None
        if args.no_record
        else CallRecord(
            args.mode,
            models.defaults()["model"],
            folder=args.calls or CALLS,
            source=args.origin,
            scenario=args.script.stem if args.script else None,
        )
    )
    # A kept draw is filed by the Test run, which decides whether the store needs it
    # (DATABASE.md §8.2): its ehr opens no store, and the harness files nothing.
    files = record is not None and args.origin != "kept"
    if record is not None:
        redact.set_dev_tap(record.log)
    redact.install(args.log)

    reason = "hung up"
    code = 0
    async with running_ehr(store=files) as target:
        ehr = await EhrClient.connect(target, config.CERTS)
        call = TypedCall(ehr, build_llm(Mode(args.mode)), record, today=today)
        try:
            print(TEST_DATA)
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
                    reason = "the agent ended it"
                    break
        except CallFailed as e:
            reason, code = f"Claude's request failed ({e})", 1
            print(f"(the call failed: Claude's request failed, {e})")
        except (EOFError, KeyboardInterrupt):
            reason = "the harness was stopped"
        except BaseException as e:
            reason, code = f"the harness failed ({type(e).__name__})", 1
            if record is not None:
                record.event("error", type=type(e).__name__, message=str(e))
            raise
        finally:
            try:
                await call.hang_up()
            except Exception as e:  # noqa: BLE001 - a failed call may not stop cleanly; the record must still close
                if record is not None:
                    record.event("error", type=type(e).__name__, message="hang-up did not finish cleanly")
            if record is not None:
                record.close(reason)
                redact.set_dev_tap(None)
                print(f"(record: {record.path})")
                print(f"({record.summary()})")
                # File the ended call into the call store (DATABASE.md §4a). Never fails the run.
                if files:
                    try:
                        filed = await ehr.file_call(record.call_id, record.path)
                        print({True: "(filed)", False: "(already filed)", None: "(not filed: no store)"}[filed])
                    except EhrError as e:
                        print(f"(not filed: {e.kind.value}; the next ingest files it)")
            await ehr.close()
            if args.keys_out is not None:
                args.keys_out.write_text(json.dumps(llm.SEEN or []), encoding="utf-8")
    return code


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
