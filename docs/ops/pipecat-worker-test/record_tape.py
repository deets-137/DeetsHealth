"""Run one kept script natively (real ehr, Claude from the replays) and keep ehr's answers on a tape.

The tape lets the same call run where ehr cannot (a Python Worker): each gRPC reply, in order, per
call name. Synthetic patients only. Usage (from agent/): uv run --no-sync python record_tape.py OUT.json
"""

import asyncio
import base64
import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path.cwd()))

import grpc  # noqa: E402

from deets_agent import config, llm, models  # noqa: E402
from deets_agent.call import TypedCall  # noqa: E402
from deets_agent.call_record import CallRecord  # noqa: E402
from deets_agent.catalog import read_events  # noqa: E402
from deets_agent.ehr_client import EhrClient  # noqa: E402
from deets_agent.ehr_process import running_ehr  # noqa: E402
from deets_agent.llm import Mode, build_llm  # noqa: E402

import os  # noqa: E402

SCRIPT = "book_a_visit.txt"
# A copy of the agent outside the repo has no dev certificates: point at the repo's own.
if os.environ.get("DH_CERTS"):
    config.CERTS = Path(os.environ["DH_CERTS"])


class TapeStub:
    def __init__(self, real, tape: list[dict]) -> None:
        self._real = real
        self._tape = tape

    def __getattr__(self, name: str):
        fn = getattr(self._real, name)

        async def call(request, timeout=None):
            try:
                reply = await fn(request, timeout=timeout)
            except grpc.aio.AioRpcError as e:
                self._tape.append({"name": name, "error": e.code().name})
                raise
            self._tape.append({"name": name, "reply": base64.b64encode(reply.SerializeToString()).decode()})
            return reply

        return call


async def main(out: Path) -> None:
    lines = [
        x.strip()
        for x in (Path("tests/scripts") / SCRIPT).read_text(encoding="utf-8").splitlines()
        if x.strip() and not x.startswith("#")
    ]
    tape: list[dict] = []
    async with running_ehr(certs=config.CERTS) as target:
        ehr = await EhrClient.connect(target, config.CERTS)
        ehr._stub = TapeStub(ehr._stub, tape)
        folder = Path(tempfile.mkdtemp(prefix="deets-tape-"))
        record = CallRecord("replay", models.defaults()["model"], folder=folder, source="kept", scenario=Path(SCRIPT).stem)
        llm.SEEN = []
        call = TypedCall(ehr, build_llm(Mode.REPLAY), record)
        replies = []
        try:
            greeting = await call.start()
            for line in lines:
                replies.append(await call.say(line))
        finally:
            await call.hang_up()
            record.close("the agent ended it" if call.ended else "hung up")
            ehr.record = None
            await ehr.close()
        events = read_events(record.path)
    out.write_text(
        json.dumps(
            {
                "script": SCRIPT,
                "lines": lines,
                "greeting": greeting,
                "replies": replies,
                "ended": call.ended,
                "tools": [e.get("name") for e in events if e.get("kind") == "tool_call"],
                "keys": sorted(set(llm.SEEN or [])),
                "model_catalog": models.current(),
                "tape": tape,
            },
            indent=1,
        ),
        encoding="utf-8",
    )
    print(f"lines {len(lines)}, replies {len(replies)}, ehr calls {len(tape)}, replay keys {len(set(llm.SEEN or []))}, ended {call.ended}")


asyncio.run(main(Path(sys.argv[1])))
