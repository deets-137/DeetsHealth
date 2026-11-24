"""Test: does Pipecat load and run in a Cloudflare Python Worker? No key, no network.

GET /         imports the typed call's Pipecat modules, one by one.
GET /run      runs a two-step pipeline on three text frames and returns what came out.
GET /call     runs one kept script through the real Flow and Pipecat's Anthropic service:
              Claude's answers from the replay files, ehr's answers from a tape.
"""

import asyncio
import importlib
import json
import sys
import time
import traceback
import types
from pathlib import Path

from workers import Response, WorkerEntrypoint

# The Anthropic and OpenAI SDKs are imported from one zip file (zip_vendor.py): as loose files
# they are 5,297 of the bundle's 7,496, and the upload API refused that many.
_VENDOR = Path(__file__).parent / "vendor.zip"
if _VENDOR.exists() and str(_VENDOR) not in sys.path:
    sys.path.insert(0, str(_VENDOR))

# Pipecat is NOT imported at the top level. Tried 2026-11-24: the deploy runs the top level to
# take its memory snapshot, and refused it with "Python startup exceeded memory limits".

MODULES = [
    "pipecat",
    "pipecat.frames.frames",
    "pipecat.pipeline.pipeline",
    "pipecat.pipeline.worker",
    "pipecat.workers.runner",
    "pipecat.processors.frame_processor",
    "pipecat.processors.aggregators.llm_context",
    "pipecat.processors.aggregators.llm_response_universal",
    "pipecat.processors.frameworks.rtvi",
    "pipecat.processors.frameworks.rtvi.observer",
    "pipecat.transports.base_transport",
    "pipecat.services.anthropic.llm",
    "pipecat.flows",
]


def imports() -> dict:
    out = {}
    for name in MODULES:
        t = time.time()
        try:
            importlib.import_module(name)
            out[name] = f"ok ({(time.time() - t) * 1000:.0f} ms)"
        except BaseException as e:  # noqa: BLE001 - the test reports every failure
            out[name] = f"{type(e).__name__}: {e}"[:300]
            out[name + "#trace"] = traceback.format_exc().splitlines()[-8:]
    return out


async def run_pipeline() -> dict:
    from pipecat.frames.frames import EndFrame, TextFrame
    from pipecat.pipeline.pipeline import Pipeline
    from pipecat.pipeline.worker import PipelineParams, PipelineWorker
    from pipecat.processors.frame_processor import FrameDirection, FrameProcessor
    from pipecat.workers.runner import WorkerRunner

    seen: list[str] = []

    class Upper(FrameProcessor):
        async def process_frame(self, frame, direction: FrameDirection):
            await super().process_frame(frame, direction)
            if isinstance(frame, TextFrame):
                frame = TextFrame(frame.text.upper())
            await self.push_frame(frame, direction)

    class Collect(FrameProcessor):
        async def process_frame(self, frame, direction: FrameDirection):
            await super().process_frame(frame, direction)
            if isinstance(frame, TextFrame):
                seen.append(frame.text)
            await self.push_frame(frame, direction)

    worker = PipelineWorker(
        Pipeline([Upper(), Collect()]), params=PipelineParams(), cancel_on_idle_timeout=False, enable_rtvi=False
    )
    runner = WorkerRunner(handle_sigint=False)
    started = asyncio.Event()

    @worker.event_handler("on_pipeline_started")
    async def _on_started(worker, frame):
        started.set()

    t = time.time()
    await runner.add_workers(worker)
    run = asyncio.create_task(runner.run())
    await asyncio.wait_for(started.wait(), timeout=10)
    for word in ("one", "two", "three"):
        await worker.queue_frame(TextFrame(word))
    await worker.queue_frame(EndFrame())
    await asyncio.wait_for(run, timeout=10)
    return {"out": seen, "ms": round((time.time() - t) * 1000)}


def place_files(notes: dict) -> None:
    """Write the agent's data files where its code looks for them: beside the code."""
    import bundle_data

    import shutil

    # The Worker's module folder (/session/metadata) is read-only, and the agent finds its files
    # by its own location (<repo>/agent/deets_agent). So the agent's code is copied to the
    # in-memory /tmp in the repo's shape, and loaded from there.
    root = Path("/tmp/dh")
    if not (root / "agent" / "deets_agent").exists():
        spec = importlib.util.find_spec("deets_agent")
        shutil.copytree(Path(spec.origin).parent, root / "agent" / "deets_agent")
        for rel, text in bundle_data.FILES.items():
            path = root / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding="utf-8")
    if str(root / "agent") not in sys.path:
        sys.path.insert(0, str(root / "agent"))
        importlib.invalidate_caches()
    notes["agent_dir"] = str(root / "agent")
    notes["files_written"] = len(bundle_data.FILES)


async def run_call() -> dict:
    import bundle_data

    notes: dict = {}
    t0 = time.time()
    place_files(notes)

    try:
        importlib.import_module("deets_agent.voice")
        notes["voice"] = "the real package loaded"
    except BaseException as e:  # noqa: BLE001
        stub = types.ModuleType("deets_agent.voice")
        stub.Voice = type("Voice", (), {})
        sys.modules["deets_agent.voice"] = stub
        notes["voice"] = f"stand-in ({type(e).__name__}: {str(e)[:120]})"

    import grpc.aio
    from deets_agent import llm, models
    from deets_agent.call import TypedCall
    from deets_agent.call_record import CallRecord
    from deets_agent.ehr_client import EhrClient
    from deets_agent.llm import Mode, build_llm

    notes["import_ms"] = round((time.time() - t0) * 1000)
    tape = bundle_data.TAPE
    models._catalog = tape["model_catalog"]  # natively this comes from the ehr binary (a subprocess)

    channel = grpc.aio.TapeChannel(tape["tape"])
    ehr = EhrClient(channel)
    folder = Path("/tmp/dh-calls")
    folder.mkdir(parents=True, exist_ok=True)
    record = CallRecord("replay", models.defaults()["model"], folder=folder, source="kept", scenario="book_a_visit")
    llm.SEEN = []
    call = TypedCall(ehr, build_llm(Mode.REPLAY), record)
    replies: list = []
    error = None
    t1 = time.time()
    greeting = None
    try:
        greeting = await call.start()
        for line in tape["lines"]:
            replies.append(await call.say(line))
    except BaseException as e:  # noqa: BLE001
        error = {"error": f"{type(e).__name__}: {e}"[:300], "trace": traceback.format_exc().splitlines()[-14:]}
    finally:
        try:
            await call.hang_up()
            record.close("the agent ended it" if call.ended else "hung up")
        except BaseException as e:  # noqa: BLE001
            notes["hang_up"] = f"{type(e).__name__}: {e}"[:200]
    same = [a == b for a, b in zip(replies, tape["replies"], strict=False)]
    return {
        "notes": notes,
        "error": error,
        "call_ms": round((time.time() - t1) * 1000),
        "greeting_same_as_native": greeting == tape["greeting"],
        "replies": len(replies),
        "replies_expected": len(tape["replies"]),
        "replies_same_as_native": sum(same),
        "ended": call.ended,
        "ehr_calls_played": channel.played,
        "ehr_calls_native": [x["name"] for x in tape["tape"]],
        "replay_keys_used": len(set(llm.SEEN or [])),
        "replay_keys_native": len(tape["keys"]),
    }


class Default(WorkerEntrypoint):
    async def fetch(self, request):
        try:
            url = request.url.rstrip("/")
            if url.endswith("/run"):
                body = await run_pipeline()
            elif url.endswith("/call"):
                body = await run_call()
            else:
                body = imports()
        except BaseException as e:  # noqa: BLE001 - the test reports every failure
            body = {"error": f"{type(e).__name__}: {e}"[:300], "trace": traceback.format_exc().splitlines()[-14:]}
        return Response(json.dumps(body, indent=1), headers={"content-type": "application/json"})
