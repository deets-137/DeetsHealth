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
- `GET  /agent/sandbox` — the sandbox patients and their visits (ehr/fixtures/sandbox.json).
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
from deets_agent.ehr_client import EhrClient
from deets_agent.ehr_process import running_ehr
from deets_agent.fake_twilio import FakeTwilio
from deets_agent.llm import Mode, build_llm
from deets_agent.rtvi_json import RtviJsonSerializer

TOKEN_HEADER = "x-deets-token"


SANDBOX = config.REPO / "ehr" / "fixtures" / "sandbox.json"


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
        async with running_ehr(env=state.twilio.ehr_env()) as target:
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
        return {"key": state.has_key, "live": state.live.call_id if state.live else None}

    @app.get("/agent/sandbox")
    async def sandbox():
        """The sandbox patients and their visits, the same file ehr's fakes read."""
        data = json.loads(SANDBOX.read_text(encoding="utf-8"))
        return {"place": data["place"], "patients": data["patients"], "visits": data["visits"]}

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
        await ws.accept()
        record = CallRecord(state.mode.value, config.MODEL, source="test")
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
        typed = TypedCall(state.ehr, build_llm(state.mode), record, transport=transport)

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
            state.live = None
            state.catalog.refresh(record.call_id)
            state.broadcast({"type": "ended", "call": record.call_id})

    return app


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
