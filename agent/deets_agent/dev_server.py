"""The agent's dev server for the console's Chat page (CHAT.md §5; the owner, 2026-11-11: the
hybrid). Dev only, synthetic callers only, on 127.0.0.1.

    uv run python -m deets_agent.dev_server --port N --origin http://localhost:M

`npm run dev` (scripts/dev.mjs) starts it on a free port with a fresh token, then starts Vite,
which proxies `/agent/*` here and adds the token. The page never holds the token.

- `GET  /agent/health` — whether the Anthropic key is set. Test calls are always live: no
  replay cache (replay is Demo's alone). Without a key there is no test call.
- `GET  /agent/catalog` — every dev call record, its row and current ratings (catalog.py), and
  the rubric's dimensions.
- `GET  /agent/calls/{id}` — one record's events, for Demo.
- `GET  /agent/latency` — per-tool latency: a summary per tool and model (p50, p90, max of each
  span), and each tool call's row with `rows=true`. Filters: `source`, `model`, `since` (a day
  or a call id), `exact`, `with_replayed` (latency.py).
- `POST /agent/annotations` — a label, a note, a star or a rating; kept in the committed file.
- `WS   /agent/call` — the caller's seat (Test): RTVI as JSON (rtvi_json.py). One call at a time.
- `WS   /agent/watch` — the watch stream: the live call's events as the record writes them, the
  same lines Demo plays back.
- `GET  /agent/sandbox?ehr=local|epic` — the sandbox patients and their visits: `local` from
  ehr/fixtures/sandbox.json; `epic` from Epic's public sandbox through `ehr epic-sandbox`, kept
  for 5 minutes (CHAT.md §13). `more`: per patient, the visits past the ones listed.
- `WS   /agent/call?ehr=local|epic` — see below; `ehr` picks the EHR the test call reads.
- `POST /2010-04-01/Accounts/{sid}/Messages.json` — the fake Twilio (fake_twilio.py), for ehr
  only: its own Basic auth instead of the token. A text it takes joins the live call's record.

**The wall.** A WebSocket is not covered by CORS: any web page can open one to 127.0.0.1. So
every request must carry the launch token (only Vite's proxy has it) and come from the
console's own origin. The events carry caller lines and tool arguments; that is safe only
because every caller here is synthetic (call_record.py).
"""

from __future__ import annotations

import argparse
import asyncio
import hmac
import json
import os
import sys
import time
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any

import uvicorn
from fastapi import FastAPI, HTTPException, Request, WebSocket, WebSocketDisconnect
from fastapi.responses import JSONResponse
from pipecat.transports.websocket.fastapi import FastAPIWebsocketParams, FastAPIWebsocketTransport

from deets_agent import config, redact
from deets_agent.call import CallFailed, TypedCall
from deets_agent.call_record import CALLS, CallRecord
from deets_agent.catalog import (
    DIMENSIONS,
    OUTCOMES,
    RUBRIC_VERSION,
    AnnotationError,
    Catalog,
    read_events,
)
from deets_agent.ehr_client import EhrClient, EhrError
from deets_agent.ehr_process import running_ehr
from deets_agent.fake_twilio import FakeTwilio
from deets_agent.llm import Mode, build_llm
from deets_agent.rtvi_json import RtviJsonSerializer

TOKEN_HEADER = "x-deets-token"


SANDBOX = config.REPO / "ehr" / "fixtures" / "sandbox.json"
# Epic's sandbox (EPIC.md §5.2): the client ID and the non-production key, both git-ignored.
EPIC_CLIENT = config.REPO / "epic-client.json"
EPIC_KEY = config.CERTS / "epic-nonprod-es384.key"
# The Epic list is read again after this long: Epic's sandbox moves its visits each day, and
# every flip of the page's Epic / Local button must not cost Epic about 15 reads.
EPIC_LIST_TTL_S = 5 * 60
EPIC_LIST_LIMIT_S = 60


def epic_env() -> dict[str, str]:
    """ehr's settings to serve Epic's sandbox, when both dev files are here; else none."""
    if not (EPIC_CLIENT.exists() and EPIC_KEY.exists()):
        return {}
    return {
        "DEETS_EPIC_SANDBOX": "1",
        "DEETS_EPIC_CLIENT_FILE": str(EPIC_CLIENT),
        "DEETS_EPIC_KEY_FILE": str(EPIC_KEY),
    }


async def epic_list() -> dict[str, Any]:
    """The Chat page's Epic list: `ehr epic-sandbox` (epic_ehr.rs), Epic's public test patients
    and their visits ahead, in the shape of sandbox.json. Raises RuntimeError with ehr's own
    fixed message when it fails."""
    proc = await asyncio.create_subprocess_exec(
        str(config.EHR_BINARY),
        "epic-sandbox",
        cwd=str(config.REPO),
        env={**os.environ, **epic_env()},
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    try:
        out, err = await asyncio.wait_for(proc.communicate(), timeout=EPIC_LIST_LIMIT_S)
    except TimeoutError:
        proc.kill()
        raise RuntimeError("Epic did not answer in time") from None
    if proc.returncode != 0:
        raise RuntimeError(err.decode().strip().removeprefix("ehr: ") or "ehr epic-sandbox failed")
    return json.loads(out)


class State:
    def __init__(self, token: str, origins: set[str], port: int, calls: Path = CALLS) -> None:
        self.token = token
        self.origins = origins
        self.twilio = FakeTwilio(port)
        self.catalog = Catalog(calls).build()
        self.ehr: EhrClient | None = None
        # Test is always live (the owner, 2026-11-12: "limit replaying conversations only to the
        # Demo view"): every line goes to Claude, no replay cache. Without a key, no test call.
        self.has_key = config.anthropic_key() is not None
        self.mode = Mode.LIVE
        self.live: CallRecord | None = None
        self.watchers: set[asyncio.Queue] = set()
        # Epic mode (CHAT.md §13): whether ehr was given Epic's sandbox, and the last Epic list
        # with the time it was read.
        self.epic = bool(epic_env())
        self.epic_list: tuple[float, dict[str, Any]] | None = None
        self.epic_lock = asyncio.Lock()

    def allowed(self, headers) -> bool:
        """The launch token, compared in constant time, and the console's own origin."""
        token = headers.get(TOKEN_HEADER, "")
        origin = headers.get("origin")
        if not hmac.compare_digest(token.encode(), self.token.encode()):
            return False
        return origin is None or origin in self.origins

    def broadcast(self, message: dict[str, Any]) -> None:
        text = json.dumps(message, ensure_ascii=False)
        for q in list(self.watchers):
            if q.qsize() < 1000:  # a stuck watcher loses lines; it never holds up the call
                q.put_nowait(text)


def build_app(state: State) -> FastAPI:
    @asynccontextmanager
    async def lifespan(app: FastAPI):
        # ehr sends its texts to the fake Twilio below, with this launch's fake account.
        # With the call store, so each test call is filed as it ends (DATABASE.md §4a).
        # With Epic's sandbox as a second source when its dev files are here (CHAT.md §13).
        async with running_ehr(env={**state.twilio.ehr_env(), **epic_env()}, store=True) as target:
            state.ehr = await EhrClient.connect(target, config.CERTS)
            try:
                yield
            finally:
                await state.ehr.close()

    app = FastAPI(lifespan=lifespan, docs_url=None, redoc_url=None, openapi_url=None)

    @app.middleware("http")
    async def wall(request: Request, call_next):
        if request.url.path == state.twilio.path():
            # ehr, not the console: the fake Twilio's own check (fake_twilio.py).
            host = request.client.host if request.client else None
            if not state.twilio.authorized(request.headers, host):
                return JSONResponse({"code": 20003, "message": "Authenticate", "status": 401}, status_code=401)
            return await call_next(request)
        if not state.allowed(request.headers):
            return JSONResponse({"error": "forbidden"}, status_code=403)
        return await call_next(request)

    @app.post("/2010-04-01/Accounts/{account}/Messages.json")
    async def twilio_messages(account: str, request: Request):
        if request.url.path != state.twilio.path():
            return JSONResponse({"code": 20404, "message": "Not found", "status": 404}, status_code=404)
        status, reply = state.twilio.take(await request.body())
        text = reply.pop("_text", None)
        live = state.live
        if text is not None and live is not None:
            # The phone that got it, on the Chat page: the code button (CHAT.md §8).
            live.event("text", to=text["to"], body=text["body"])
        return JSONResponse(reply, status_code=status)

    @app.get("/agent/health")
    async def health():
        return {"key": state.has_key, "live": state.live.call_id if state.live else None, "epic": state.epic}

    @app.get("/agent/sandbox")
    async def sandbox(ehr: str = "local"):
        """The sandbox patients and their visits. `local`: the same file ehr's fakes read.
        `epic`: Epic's public test patients, read live through ehr and kept for 5 minutes."""
        if ehr == "local":
            data = json.loads(SANDBOX.read_text(encoding="utf-8"))
            return {"place": data["place"], "patients": data["patients"], "visits": data["visits"], "more": {}}
        if ehr != "epic":
            raise HTTPException(400, "ehr: local or epic")
        if not state.epic:
            raise HTTPException(503, "no Epic files: epic-client.json and the non-production key")
        async with state.epic_lock:
            cached = state.epic_list
            if cached is None or time.monotonic() - cached[0] > EPIC_LIST_TTL_S:
                try:
                    state.epic_list = (time.monotonic(), await epic_list())
                except RuntimeError as e:
                    raise HTTPException(502, str(e)) from None
            return state.epic_list[1]

    @app.get("/agent/catalog")
    async def catalog():
        return {
            "calls": state.catalog.list_calls(),
            "dimensions": DIMENSIONS,
            "outcomes": OUTCOMES,
            "rubric_version": RUBRIC_VERSION,
        }

    @app.get("/agent/latency")
    async def tool_latency(
        source: str | None = None,
        model: str | None = None,
        since: str | None = None,
        exact: bool = False,
        with_replayed: bool = False,
        rows: bool = False,
    ):
        """Per-tool latency from the catalog (latency.py; CHAT.md §10)."""
        return state.catalog.latency(source, model, since, exact, with_replayed, rows)

    @app.get("/agent/calls/{call_id}")
    async def call_events(call_id: str):
        events = state.catalog.events(call_id)
        if events is None:
            raise HTTPException(404, "no such call")
        return {"id": call_id, "events": events}

    @app.post("/agent/annotations")
    async def annotate(request: Request):
        try:
            body = await request.json()
        except ValueError:
            raise HTTPException(400, "not JSON") from None
        if not isinstance(body, dict):
            raise HTTPException(400, "not an object")
        try:
            state.catalog.annotate(body)
        except AnnotationError as e:
            raise HTTPException(400, str(e)) from None
        return {"ok": True}

    @app.websocket("/agent/watch")
    async def watch(ws: WebSocket):
        if not state.allowed(ws.headers):
            await ws.close(code=1008)
            return
        await ws.accept()
        q: asyncio.Queue = asyncio.Queue()
        state.watchers.add(q)
        try:
            live = state.live
            await ws.send_text(json.dumps({"type": "hello", "live": live.call_id if live else None}))
            if live is not None:
                # The live call so far, then its events as they come.
                for event in read_events(live.path):
                    await ws.send_text(json.dumps({"type": "event", "call": live.call_id, "event": event}))
            sender = asyncio.create_task(_drain(ws, q))
            try:
                while True:
                    await ws.receive_text()  # the watcher sends nothing; this notices a close
            except WebSocketDisconnect:
                pass
            finally:
                sender.cancel()
        finally:
            state.watchers.discard(q)

    @app.websocket("/agent/call")
    async def call(ws: WebSocket):
        if not state.allowed(ws.headers):
            await ws.close(code=1008)
            return
        if state.live is not None:
            await ws.accept()
            await ws.close(code=1013, reason="a test call is already running")
            return
        if state.ehr is None:
            await ws.accept()
            await ws.close(code=1013, reason="ehr is not up yet")
            return
        if not state.has_key:
            await ws.accept()
            await ws.close(code=1013, reason="a test call needs an Anthropic key in .env")
            return
        # The EHR this call reads: the page's Epic / Local button (CHAT.md §13).
        ehr_source = ws.query_params.get("ehr", "local")
        if ehr_source not in ("local", "epic"):
            await ws.accept()
            await ws.close(code=1008, reason="ehr: local or epic")
            return
        if ehr_source == "epic" and not state.epic:
            await ws.accept()
            await ws.close(code=1013, reason="Epic mode needs epic-client.json and the non-production key")
            return
        await ws.accept()
        record = CallRecord(state.mode.value, config.MODEL, source="test", ehr=ehr_source)
        state.live = record
        redact.dev_tap = record.log
        record.listeners.append(
            lambda text: state.broadcast({"type": "event", "call": record.call_id, "event": json.loads(text)})
        )
        state.broadcast({"type": "started", "call": record.call_id})
        transport = FastAPIWebsocketTransport(
            ws,
            FastAPIWebsocketParams(
                serializer=RtviJsonSerializer(),
                audio_in_enabled=False,
                audio_out_enabled=False,
                session_timeout=30 * 60,
            ),
        )
        typed = TypedCall(state.ehr, build_llm(state.mode), record, transport=transport, ehr_source=ehr_source)

        @transport.event_handler("on_client_disconnected")
        async def _gone(transport, client):
            await typed.cancel()

        reason = "the caller hung up"
        try:
            reason = await typed.serve()
        except CallFailed as e:
            reason = f"Claude's request failed ({e})"
        except Exception as e:  # noqa: BLE001 - the record must close, and the server must live on
            reason = f"the harness failed ({type(e).__name__})"
            record.event("error", type=type(e).__name__, message=str(e))
        finally:
            record.close(reason)
            redact.dev_tap = None
            await _file(state.ehr, record)
            state.live = None
            state.catalog.refresh(record.call_id)
            state.broadcast({"type": "ended", "call": record.call_id})

    return app


async def _file(ehr: EhrClient, record: CallRecord) -> None:
    """File the ended call into the call store through ehr (DATABASE.md §4a). A failure never
    stops the server: the next `ehr ingest` files what was missed. Prints the outcome only."""
    try:
        filed = await ehr.file_call(record.call_id, record.path)
    except EhrError as e:
        print(f"call not filed ({e.kind.value}); the next ingest files it", file=sys.stderr, flush=True)
        return
    print(
        {True: "call filed", False: "call already filed", None: "call not filed: ehr has no store"}[filed],
        flush=True,
    )


async def _drain(ws: WebSocket, q: asyncio.Queue) -> None:
    while True:
        await ws.send_text(await q.get())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--origin", action="append", default=[], help="the console's origin (repeatable)")
    parser.add_argument("--log", default="WARNING", help="log level (the logs carry no PHI)")
    args = parser.parse_args()
    token = os.environ.get("DEETS_DEV_TOKEN", "")
    if len(token) < 32:
        print("DEETS_DEV_TOKEN is missing or short. Start the dev server with `npm run dev`.", file=sys.stderr)
        return 2
    redact.install(args.log)
    state = State(token, set(args.origin), args.port)
    keyed = "test calls live" if state.has_key else "no Anthropic key: Demo only"
    print(f"agent dev server on 127.0.0.1:{args.port} ({keyed})", flush=True)
    uvicorn.run(build_app(state), host="127.0.0.1", port=args.port, log_level="warning", access_log=False)
    return 0


if __name__ == "__main__":
    sys.exit(main())
