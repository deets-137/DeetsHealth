"""The agent's dev server for the console's Chat page (CHAT.md §5; the owner, 2026-11-11: the
hybrid). Dev only, synthetic callers only, on 127.0.0.1.

    uv run python -m deets_agent.dev_server --port N --origin http://localhost:M

`npm run dev` (scripts/dev.mjs) starts it on a free port with a fresh token, then starts Vite,
which proxies `/agent/*` here and adds the token. The page never holds the token.

- `GET  /agent/health` — whether the Anthropic key is set, and the Cartesia key (`voice`). Test
  calls are always live: no replay cache (replay is Demo's alone). Without a key there is no
  test call; without the Cartesia key, no voice call.
- `GET  /agent/cartesia` — the Cartesia levers as set now, with their ranges, for the Config
  page (read only; never the key, only whether it is set).
- `GET  /agent/anthropic`, `POST /agent/anthropic` — Config › Anthropic (CONSOLE.md §8): the
  llm settings' versions, each model's rules and prices, this cycle's spend; a save makes a new
  active version for the next test call. Never the key.
- `GET  /agent/prompt`, `POST /agent/prompt` — Config › Agent (CONSOLE.md §9): the versions of
  the agent's words and each section's placeholders; a save makes a new active version for the
  next test call and writes the prompt file. `POST /agent/prompt/record` — Record and test.
- `GET  /agent/system`, `POST /agent/system` — Config › System (SYSTEM-PAGE.md): the System
  variables' versions, the code values and the tools; a save makes a new active version for the
  next test call and writes the variables file.
- `GET  /agent/resources` — Cartesia's credits this cycle, counted from the records and the live
  voice call (resources.py; CHAT.md §17).
- `POST /agent/voice?ehr=local|epic` — the voice seat: a WebRTC offer in, the answer out; the
  call runs over that connection (CHAT.md §17).
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
import tempfile
import time
from contextlib import asynccontextmanager
from pathlib import Path
from typing import Any

import uvicorn
from fastapi import FastAPI, HTTPException, Request, WebSocket, WebSocketDisconnect
from fastapi.responses import JSONResponse
from pipecat.services.cartesia.tts import _CARTESIA_API_VERSION as CARTESIA_API_VERSION
from pipecat.transports.base_transport import BaseTransport, TransportParams
from pipecat.transports.smallwebrtc.connection import SmallWebRTCConnection
from pipecat.transports.smallwebrtc.transport import SmallWebRTCTransport
from pipecat.transports.websocket.fastapi import FastAPIWebsocketParams, FastAPIWebsocketTransport

from deets_agent import config, prompts, redact
from deets_agent.call import CallFailed, TypedCall
from deets_agent.call_record import CALLS, CallRecord, replay_cost
from deets_agent.catalog import (
    DIMENSIONS,
    OUTCOMES,
    RUBRIC_VERSION,
    AnnotationError,
    Catalog,
    read_events,
)
from deets_agent.ehr_client import EhrClient, EhrError, ErrorKind, SettingVersion
from deets_agent.ehr_process import running_ehr
from deets_agent.fake_twilio import FakeTwilio
from deets_agent.flow import tool_scope, tool_steps
from deets_agent.llm import Mode, build_llm
from deets_agent.resources import anthropic_cycle, cartesia_cycle
from deets_agent.rtvi_json import RtviJsonSerializer
from deets_agent.voice import Voice

TOKEN_HEADER = "x-deets-token"
# The longest SDP offer the voice route takes: a browser's audio offer is a few kilobytes.
MAX_OFFER_BYTES = 64 * 1024
# Cartesia's List Voices takes only this API version (docs.cartesia.ai › Voices › List, read
# 2026-11-13); Pipecat's speech sockets use their own.
CARTESIA_VOICES_VERSION = "2026-08-14"


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
        self.calls = calls
        self.catalog = Catalog(calls).build()
        # Voice (CHAT.md §17): the Cartesia key's presence, and the live voice call's parts.
        self.has_voice = config.cartesia_key() is not None
        self.voice: Voice | None = None
        # The settings services whose Default version is known to exist (ensure_default).
        self.seeded: set[str] = set()
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
        # Voice calls run as tasks after their route returns the answer; kept so none is lost.
        self.tasks: set[asyncio.Task] = set()
        # The last Record and test (CONSOLE.md §9): its state, each script, the tests, the cost.
        self.recording: dict[str, Any] | None = None
        # The dry run's quote: the words it was made for, the scripts that need new replies, the
        # dollars (estimate_record).
        self.estimate: dict[str, Any] | None = None

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
            # A hand edit to the prompt file becomes a new version (PROMPT-EDITOR.md §6.2).
            await sync_prompt_file(state)
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
        return {
            "key": state.has_key,
            "live": state.live.call_id if state.live else None,
            "epic": state.epic,
            "voice": state.has_voice,
        }

    @app.get("/agent/cartesia")
    async def cartesia_settings(version: int = 0):
        """The voice settings (CONSOLE.md §7): the saved versions, one version's values (the
        active one when `version` is 0) and the active one's, each lever's range. Never the key:
        only whether it is set."""
        if version < 0:
            raise HTTPException(400, "version: not a version")
        try:
            return await cartesia_view(state, version)
        except LookupError:
            raise HTTPException(404, "no such version") from None

    @app.post("/agent/cartesia")
    async def save_cartesia(request: Request):
        """Save the whole set as a new version; it becomes the active one and the next call runs
        with it (CONSOLE.md §7.3). 400 names the setting that does not fit; 409 without a store."""
        try:
            body = await request.json()
        except ValueError:
            raise HTTPException(400, "not JSON") from None
        values = body.get("values") if isinstance(body, dict) else None
        if not isinstance(values, dict):
            raise HTTPException(400, "values: an object of every setting")
        if state.ehr is None:
            raise HTTPException(409, "ehr is not up yet")
        try:
            values = await resolve_voice(values)
        except VoiceNotFound as e:
            raise HTTPException(400, str(e)) from None
        await save_settings(state, "voice", values)
        return await cartesia_view(state, 0)

    @app.get("/agent/prompt")
    async def prompt_settings(version: int = 0):
        """The agent's words (CONSOLE.md §9): the versions and one version's sections, the
        placeholders each section must hold, whether the kept recordings match the prompt file,
        and the state of Record and test."""
        if version < 0:
            raise HTTPException(400, "version: not a version")
        try:
            return await prompt_view(state, version)
        except LookupError:
            raise HTTPException(404, "no such version") from None

    @app.post("/agent/prompt")
    async def save_prompt(request: Request):
        """Save every section as a new, active version: ehr checks each one (its placeholders,
        no stray brace, not empty); the next test call runs with it; the prompt file is
        written, so git and the tests see it. 400 names the section that does not fit."""
        try:
            body = await request.json()
        except ValueError:
            raise HTTPException(400, "not JSON") from None
        values = body.get("values") if isinstance(body, dict) else None
        if not isinstance(values, dict) or not all(isinstance(v, str) for v in values.values()):
            raise HTTPException(400, "values: every section's words")
        # A tool named in backticks must be one the section's steps offer (SYSTEM-PAGE.md §3.5).
        # The agent knows the steps, so this check is here, before ehr's.
        problems = prompts.mention_problems(values, tool_scope(), set(tool_steps()))
        if problems:
            raise HTTPException(400, problems[0])
        await save_settings(state, "prompt", values)
        try:
            prompts.write_file(values, names=set((await active_variables(state))[0]))
        except prompts.PromptFileError:
            # ehr checked the same rules, so this is a writer's fault, not the words'.
            raise HTTPException(500, "saved, but the prompt file could not be written") from None
        return await prompt_view(state, 0)

    @app.post("/agent/prompt/record")
    async def record_prompt():
        """Record and test (PROMPT-EDITOR.md §5): record the kept scripts against the prompt file
        (live requests to Claude for each new line), then run the agent's tests. It runs in the
        background; GET /agent/prompt shows its state. 409 while one runs or without a key."""
        if state.recording is not None and state.recording.get("state") == "running":
            raise HTTPException(409, "Record and test is already running")
        if state.estimate is not None and state.estimate.get("state") == "running":
            raise HTTPException(409, "the cost is still being worked out")
        if not state.has_key:
            raise HTTPException(409, "Record and test needs an Anthropic key in .env")
        task = asyncio.create_task(record_and_test(state))
        state.tasks.add(task)
        task.add_done_callback(state.tasks.discard)
        return {"recording": state.recording}

    @app.post("/agent/prompt/estimate")
    async def estimate_prompt():
        """Record and test's quote: a dry run with no Claude call (about 30 s, no tokens) finds
        the kept scripts that need new replies. In the background; GET /agent/prompt shows it.
        Kept until the words change, so a second press does not run it again."""
        busy = any(r and r.get("state") == "running" for r in (state.estimate, state.recording))
        if not busy and not (state.estimate and state.estimate.get("words") == _words_now()):
            task = asyncio.create_task(estimate_record(state))
            state.tasks.add(task)
            task.add_done_callback(state.tasks.discard)
        return {"estimate": state.estimate}

    @app.get("/agent/system")
    async def system_settings(version: int = 0):
        """Config › System (SYSTEM-PAGE.md §3.6): the versions and one version's System
        variables; the code values and the tools, with what each is (read only); which
        sections use each variable. Never a call's value."""
        if version < 0:
            raise HTTPException(400, "version: not a version")
        try:
            return await system_view(state, version)
        except LookupError:
            raise HTTPException(404, "no such version") from None

    @app.post("/agent/system")
    async def save_system(request: Request):
        """Save the System variables as a new, active version (SYSTEM-PAGE.md §3.3): ehr checks
        each variable and that the active words use none that is gone; the next test call runs
        with them; the variables file is written. 400 names the variable that does not fit."""
        try:
            body = await request.json()
        except ValueError:
            raise HTTPException(400, "not JSON") from None
        values = body.get("values") if isinstance(body, dict) else None
        if not isinstance(values, dict) or not isinstance(values.get("variables"), list):
            raise HTTPException(400, "values: {variables: [...]}")
        await save_settings(state, "system", {"variables": values["variables"]})
        try:
            prompts.write_variables(prompts.variables_from_store(values["variables"]))
        except prompts.PromptFileError:
            raise HTTPException(500, "saved, but the variables file could not be written") from None
        return await system_view(state, 0)

    @app.get("/agent/anthropic")
    async def anthropic_settings(version: int = 0):
        """The llm settings (CONSOLE.md §8): the saved versions, one version's values (the active
        one when `version` is 0) and the active one's, each model's rules and prices, this
        cycle's spend. Never the key: only whether it is set."""
        if version < 0:
            raise HTTPException(400, "version: not a version")
        try:
            return await anthropic_view(state, version)
        except LookupError:
            raise HTTPException(404, "no such version") from None

    @app.post("/agent/anthropic")
    async def save_anthropic(request: Request):
        """Save the whole set as a new version; it becomes the active one and the next test call
        runs with it (CONSOLE.md §8). 400 names the setting that does not fit (ehr checks the
        model, effort and thinking together); 409 without a store."""
        try:
            body = await request.json()
        except ValueError:
            raise HTTPException(400, "not JSON") from None
        values = body.get("values") if isinstance(body, dict) else None
        if not isinstance(values, dict):
            raise HTTPException(400, "values: an object of every setting")
        await save_settings(state, "llm", values)
        return await anthropic_view(state, 0)

    @app.get("/agent/resources")
    async def resources():
        """Cartesia's credits this cycle, from the records and the live voice call, less the
        active settings' offset (CHAT.md §17)."""
        voice = state.voice
        live = None
        if voice is not None:
            live = {**voice.meter.totals(), "seconds": round(voice.meter.elapsed())}
        values = await active_voice_settings(state)
        cycle = cartesia_cycle(
            state.calls,
            live=live,
            plan=int(values["plan_credits"]),
            offset=int(values["offset"]),
            day=int(values["cycle_day"]),
            limits=(int(values["idle_s"]), int(values["max_s"])),
        )
        return {"cartesia": cycle, "voice": state.has_voice}

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
        transport = FastAPIWebsocketTransport(
            ws,
            FastAPIWebsocketParams(
                serializer=RtviJsonSerializer(),
                audio_in_enabled=False,
                audio_out_enabled=False,
                session_timeout=30 * 60,
            ),
        )
        await _run_call(state, transport, ehr_source, None)

    @app.post("/agent/voice")
    async def voice(request: Request, ehr: str = "local"):
        """The voice seat (CHAT.md §17): the browser's WebRTC offer in, the answer out. The call
        then runs over that connection: the caller's microphone to Ink, Sonic to the speaker, and
        RTVI on its data channel (typed lines too). The browser sends its offer with every ICE
        candidate in it, so there is no trickle route. One call at a time, as on `/agent/call`."""
        refused = _refuse_call(state, ehr)
        if refused is None and not state.has_voice:
            refused = "a voice call needs a Cartesia key in .env"
        if refused is not None:
            raise HTTPException(409, refused)
        body = await request.body()
        if len(body) > MAX_OFFER_BYTES:
            raise HTTPException(413, "the offer is too long")
        try:
            offer = json.loads(body)
        except ValueError:
            raise HTTPException(400, "not JSON") from None
        if not (isinstance(offer, dict) and offer.get("type") == "offer" and isinstance(offer.get("sdp"), str)):
            raise HTTPException(400, "an SDP offer: {type: offer, sdp}")
        key = config.cartesia_key()
        if key is None:
            raise HTTPException(409, "a voice call needs a Cartesia key in .env")
        # The active voice settings (CONSOLE.md §7), read once: the whole call runs with them.
        values = await active_voice_settings(state)
        # On this PC both ends are local: host candidates only, no STUN or TURN server.
        connection = SmallWebRTCConnection()
        await connection.initialize(sdp=offer["sdp"], type="offer")
        answer = connection.get_answer()
        if answer is None:
            await connection.disconnect()
            raise HTTPException(500, "no answer")
        transport = SmallWebRTCTransport(
            connection, TransportParams(audio_in_enabled=True, audio_out_enabled=True)
        )
        llm = await active_settings(state, "llm")
        words = await active_words(state)
        # Claim the seat now, so a second offer is refused while this one connects.
        record = _open_record(state, ehr, "voice", llm, words)
        state.voice = Voice(key, values)
        task = asyncio.create_task(_run_call(state, transport, ehr, state.voice, record, llm, words))
        state.tasks.add(task)
        task.add_done_callback(state.tasks.discard)
        return {"sdp": answer["sdp"], "type": answer["type"]}

    return app


# Each settings service (ehr's settings.rs › service) and its code defaults: the Default version.
# The prompt's Default is the prompt file as it is when the store first sees it; the System
# variables' Default is the variables file (SYSTEM-PAGE.md §3.4).
DEFAULTS = {
    "voice": config.voice_defaults,
    "llm": config.llm_defaults,
    "prompt": prompts.load,
    "system": lambda: {"variables": prompts.variables_to_store(prompts.load_variables())},
}

# The kept scripts (AGENT.md §1) and the hash of the words they were last recorded and tested with.
SCRIPTS = config.AGENT / "tests" / "scripts"
RECORDED_STAMP = config.AGENT / "tests" / "RECORDED-PROMPT.sha256"
# What recording each kept script cost the last time, in dollars: the dry run's quote.
RECORDED_COSTS = config.AGENT / "tests" / "RECORDED-COSTS.json"
# Record and test: the longest one script or the tests may run.
RECORD_LIMIT_S = 180
# The estimate Record and test shows before a run: about a cent a script (AGENT-PROMPT.md's own
# note; not measured). After a run the page shows what it spent.
RECORD_CENTS_PER_SCRIPT = 1


async def sync_prompt_file(state: State) -> None:
    """The prompt file and the active version agree (PROMPT-EDITOR.md §6.2). On a store with no
    prompt version yet, the file becomes the Default. When the file differs from the active
    version (an edit by hand, another branch), the file becomes a new active version, by "file".
    Never the other way: the file is what git and the tests read. The variables file goes first,
    the same way: ehr checks the words against the active System variables (SYSTEM-PAGE.md
    §3.3)."""
    if state.ehr is None:
        return
    try:
        await ensure_default(state, "system")
        got = await state.ehr.get_settings("system")
        if got is not None and got[0] is not None:
            file = prompts.load_variables()
            if prompts.variables_from_store(got[1].get("variables")) != file:
                await state.ehr.save_settings("system", {"variables": prompts.variables_to_store(file)}, by="file")
                print("system: the variables file differs from the active version; saved it as a new version", flush=True)
    except (EhrError, prompts.PromptFileError) as e:
        print(f"system: the variables file was not synced ({type(e).__name__})", file=sys.stderr, flush=True)
    try:
        await ensure_default(state, "prompt")
        got = await state.ehr.get_settings("prompt")
        if got is None or got[0] is None:
            return
        file = prompts.load()
        if got[1] != file:
            await state.ehr.save_settings("prompt", file, by="file")
            print("prompt: the file differs from the active version; saved it as a new version", flush=True)
    except (EhrError, prompts.PromptFileError) as e:
        print(f"prompt: the file was not synced ({type(e).__name__})", file=sys.stderr, flush=True)


async def active_words(state: State) -> prompts.Words:
    """The words a test call runs with: the active prompt version and the active System
    variables, each with its id and label; the files when there is no store. A pair that fails
    the agent's own check (it never should: ehr checked both) gives way to the files, and the
    log says so, so no call ever meets a `{name}` it cannot fill."""
    files = prompts.Words(prompts.load(), variables=prompts.load_variables())
    if state.ehr is None:
        return files
    try:
        await ensure_default(state, "system")
        await ensure_default(state, "prompt")
        got = await state.ehr.get_settings("prompt")
        system = await state.ehr.get_settings("system")
    except EhrError:
        return files
    if got is None or got[0] is None:
        return files
    version, values = got
    try:
        if system is not None and system[0] is not None:
            variables = prompts.variables_from_store(system[1].get("variables"))
            prompts.check_variables(variables)
            sys_id, sys_label = system[0].id, system[0].label
        else:
            variables, sys_id, sys_label = files.variables, 0, "file"
        prompts.check(values, set(variables))
    except prompts.PromptFileError:
        print("prompt: the active versions failed the agent's check; the files are used", file=sys.stderr, flush=True)
        return files
    return prompts.Words(values, version.id, version.label, variables, sys_id, sys_label)


def recordings_match() -> bool:
    """Whether the kept recordings were made, and tested, with the prompt file's words."""
    try:
        return RECORDED_STAMP.read_text(encoding="utf-8").strip() == prompts.file_digest()
    except (OSError, prompts.PromptFileError):
        return False


async def active_variables(state: State) -> tuple[dict[str, prompts.Variable], str]:
    """The active System variables and their version's label; the variables file's when there is
    no store or no version."""
    if state.ehr is not None:
        try:
            await ensure_default(state, "system")
            got = await state.ehr.get_settings("system")
            if got is not None and got[0] is not None:
                return prompts.variables_from_store(got[1].get("variables")), got[0].label
        except (EhrError, prompts.PromptFileError):
            pass
    return prompts.load_variables(), "file"


def _variables_list(variables: dict[str, prompts.Variable]) -> list[dict[str, Any]]:
    return [
        {"name": k, "meaning": v.meaning, "value": v.value, "built_in": k in prompts.BUILT_IN}
        for k, v in variables.items()
    ]


def _code_values() -> list[dict[str, Any]]:
    """The code values for the page: what each is, where it comes from, its sections. Never a
    value: there is none outside a call."""
    out = []
    for name, (meaning, source, protected) in prompts.CODE_VALUE_INFO.items():
        sections = [k for k, need in prompts.PLACEHOLDERS.items() if name in need]
        out.append(
            {
                "name": name,
                "meaning": meaning,
                "source": source,
                "protected": protected,
                # `today` is the code's own; the rest come from the caller or the record.
                "from_call": name in prompts.CALL_VALUES,
                "sections": sections,
            }
        )
    return out


def _tools() -> list[dict[str, Any]]:
    """The tools for the page and the " ." list: what each does (TOOLS.md), its steps and
    arguments (the code)."""
    ref = prompts.load_tools()
    steps = tool_steps()
    return [
        {
            "name": name,
            "does": ref.get(name, {}).get("does", ""),
            "record": ref.get(name, {}).get("record", "none"),
            "lines": ref.get(name, {}).get("lines", []),
            "steps": list(where),
            "args": sorted(k.split(".")[2] for k in prompts.PLACEHOLDERS if k.startswith(f"arg.{name}.")),
        }
        for name, where in steps.items()
    ]


async def system_view(state: State, version: int) -> dict[str, Any]:
    """The Config page's System view (SYSTEM-PAGE.md §3.6)."""
    view = await settings_view(state, "system", version)
    words = await active_words(state)
    used: dict[str, list[str]] = {}
    for key, text in words.sections.items():
        for name in prompts.placeholders_in(text) - prompts.PLACEHOLDERS.get(key, set()):
            used.setdefault(name, []).append(key)
    view.update(
        code_values=_code_values(),
        tools=_tools(),
        used=used,
        limits={
            "name": prompts.NAME_MAX,
            "meaning": prompts.MEANING_MAX,
            "value": prompts.VALUE_MAX,
            "count": prompts.VARIABLES_MAX,
        },
        built_in=list(prompts.BUILT_IN),
    )
    return view


async def prompt_view(state: State, version: int) -> dict[str, Any]:
    """The Config page's Agent view (CONSOLE.md §9), with what the `{` and " ." lists offer
    (SYSTEM-PAGE.md §3.6): the active System variables, the code values, the tools, and the
    tools each section may name."""
    view = await settings_view(state, "prompt", version)
    scripts = sorted(SCRIPTS.glob("*.txt"))
    variables, _ = await active_variables(state)
    view.update(
        variables=_variables_list(variables),
        code_values=_code_values(),
        tools=[{"name": t["name"], "does": t["does"], "steps": t["steps"]} for t in _tools()],
        tool_scope={k: sorted(v) for k, v in tool_scope().items()},
        placeholders={k: sorted(v) for k, v in prompts.PLACEHOLDERS.items()},
        recordings={"match": recordings_match(), "scripts": len(scripts)},
        # The dry run's quote for these words, or None while none is known (the page asks for one).
        estimate=state.estimate if state.estimate and state.estimate.get("words") == _words_now() else None,
        recording=state.recording,
        key=state.has_key,
    )
    return view


def _words_now() -> str | None:
    try:
        return prompts.file_digest()
    except prompts.PromptFileError:
        return None


def _script_costs() -> dict[str, float]:
    """What recording each kept script cost the last time, in dollars (RECORDED_COSTS)."""
    try:
        costs = json.loads(RECORDED_COSTS.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return {k: float(v) for k, v in costs.items() if isinstance(v, (int, float))}


async def _dry(script: Path) -> tuple[bool, list[str]]:
    """Replay one kept script with no Claude call (the typed harness in replay mode): whether
    every reply it needs is already recorded, and the keys of the requests it made."""
    with tempfile.TemporaryDirectory() as tmp:
        keys_file = Path(tmp) / "keys.json"
        ok = await _run(
            [sys.executable, "harness/typed_call.py", "--mode", "replay", "--script", str(script), "--no-record",
             "--keys-out", str(keys_file)]
        )
        try:
            keys = json.loads(keys_file.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            keys = []
    return ok, keys


async def estimate_record(state: State) -> None:
    """Record and test's quote (the owner, 2026-11-13: a free dry run first): replay each kept
    script with no Claude call; a script that misses a reply needs recording, and its quote is
    what recording it cost last time (RECORDED_COSTS; the mean of the others for a script with
    none). About 30 s, no tokens. An over-estimate: the lines before the first changed one are
    replayed for free."""
    words = _words_now()
    run: dict[str, Any] = {"words": words, "state": "running", "missing": [], "usd": None}
    state.estimate = run
    costs = _script_costs()
    mean = sum(costs.values()) / len(costs) if costs else RECORD_CENTS_PER_SCRIPT / 100
    for script in sorted(SCRIPTS.glob("*.txt")):
        ok, _ = await _dry(script)
        if not ok:
            run["missing"].append(script.stem)
    run["usd"] = round(sum(costs.get(s, mean) for s in run["missing"]), 2)
    run["state"] = "done"


async def _measure_costs() -> None:
    """After a passing Record and test: each kept script's cost, from the replies it reads (a
    dry run that logs its request keys), for the next quote."""
    costs: dict[str, float] = {}
    for script in sorted(SCRIPTS.glob("*.txt")):
        ok, keys = await _dry(script)
        if ok:
            costs[script.stem] = round(sum(replay_cost(config.REPLAYS / f"{k}.json") for k in set(keys)), 4)
    RECORDED_COSTS.write_text(json.dumps(costs, indent=1, sort_keys=True) + "\n", encoding="utf-8")


async def _run(args: list[str]) -> bool:
    """Run one step of Record and test in the agent's folder; True when it passed."""
    proc = await asyncio.create_subprocess_exec(
        *args, cwd=config.AGENT, stdout=asyncio.subprocess.DEVNULL, stderr=asyncio.subprocess.DEVNULL
    )
    try:
        return await asyncio.wait_for(proc.wait(), timeout=RECORD_LIMIT_S) == 0
    except TimeoutError:
        proc.kill()
        return False


async def record_and_test(state: State) -> None:
    """Record each kept script with the prompt file's words (the typed harness in record mode:
    a line already recorded costs nothing; a new one calls Claude once), then run the agent's
    tests. When every script ran and the tests pass, the recordings are stamped as made with
    these words. Synthetic callers only: the scripts' fake patients. Counts only on the page."""
    started = time.time()
    words = prompts.file_digest()
    run: dict[str, Any] = {"state": "running", "scripts": [], "tests": None, "spent_usd": None}
    state.recording = run
    for script in sorted(SCRIPTS.glob("*.txt")):
        ok = await _run([sys.executable, "harness/typed_call.py", "--script", str(script), "--no-record"])
        run["scripts"].append({"name": script.stem, "ok": ok})
    # Every test but the stamp's own: the stamp is written only after the others pass.
    stamp_test = "tests/test_dev_server.py::test_the_kept_recordings_match_the_prompt_file"
    run["tests"] = await _run([sys.executable, "-m", "pytest", "-q", "--deselect", stamp_test])
    run["spent_usd"] = round(
        sum(replay_cost(p) for p in config.REPLAYS.glob("*.json") if p.stat().st_mtime >= started), 4
    )
    passed = run["tests"] and all(s["ok"] for s in run["scripts"])
    if passed and prompts.file_digest() == words:
        RECORDED_STAMP.write_text(words + "\n", encoding="utf-8")
        await _measure_costs()
    state.estimate = None
    run["state"] = "passed" if passed else "failed"


async def active_settings(state: State, service: str) -> dict[str, Any]:
    """The active version's settings of a service, over the defaults; the defaults alone when
    there is no store, no version yet, or ehr is not up (CONSOLE.md §7.3)."""
    values = DEFAULTS[service]()
    if state.ehr is None:
        return values
    try:
        got = await state.ehr.get_settings(service)
    except EhrError:
        return values
    if got is not None:
        values.update(got[1])
    return values


async def active_voice_settings(state: State) -> dict[str, Any]:
    return await active_settings(state, "voice")


async def ensure_default(state: State, service: str = "voice") -> None:
    """Every implementation's first version is Default (the owner, 2026-11-13): the code's
    defaults, number 0, active when nothing else is. Made once per service, the first time the
    page asks, when the store is up and has none. A store that had versions first gets its
    Default too, listed last and not active."""
    if state.ehr is None or service in state.seeded:
        return
    try:
        versions = await state.ehr.list_setting_versions(service)
        if versions is None:
            return  # no store: nothing to seed
        if not any(v.seq == 0 for v in versions):
            await state.ehr.save_settings(service, DEFAULTS[service](), by="default", seed=True)
        state.seeded.add(service)
    except EhrError:
        # Another request seeded it first (ALREADY_EXISTS), or ehr is busy: the next view tries
        # again until the list shows a Default.
        pass


async def settings_view(state: State, service: str, version: int) -> dict[str, Any]:
    """A Config page's common part (CONSOLE.md §7, §8): the saved versions, newest first; one
    version's values (the active one when `version` is 0); the active version's values, so the
    page can mark what an older version changed. Raises LookupError for a version that does not
    exist."""
    await ensure_default(state, service)
    active = await active_settings(state, service)
    versions: list[SettingVersion] | None = None
    shown = active
    shown_version: SettingVersion | None = None
    if state.ehr is not None:
        try:
            versions = await state.ehr.list_setting_versions(service)
            if versions is not None:
                got = await state.ehr.get_settings(service, version)
                if got is not None and got[0] is not None:
                    shown_version = got[0]
                    shown = {**DEFAULTS[service](), **got[1]}
                elif version:
                    raise LookupError(version)
        except EhrError as e:
            if e.kind is ErrorKind.OTHER and version:
                raise LookupError(version) from None
            versions = None
    return {
        "store": versions is not None,
        "versions": [_version_row(v) for v in versions or []],
        "version": _version_row(shown_version) if shown_version else None,
        "values": shown,
        "active": active,
    }


async def save_settings(state: State, service: str, values: dict[str, Any]) -> None:
    """Save a whole set as the new active version, or raise the HTTPException the page shows."""
    if state.ehr is None:
        raise HTTPException(409, "ehr is not up yet")
    try:
        await state.ehr.save_settings(service, values, by="dev")
    except EhrError as e:
        if e.kind is ErrorKind.BAD_INPUT:
            raise HTTPException(400, e.detail or "a setting does not fit") from None
        if e.kind is ErrorKind.NOT_ALLOWED_NOW:
            raise HTTPException(409, "no settings store: start Postgres (npm run db:start)") from None
        raise HTTPException(502, "ehr could not save the settings") from None
        # The sections' names and tabs, for the links to Config › Agent.
        guide=prompts.load_guide(),


async def anthropic_view(state: State, version: int) -> dict[str, Any]:
    """The Config page's Anthropic view (CONSOLE.md §8): the versions and values, each model's
    rules and prices, this cycle's spend, and the fixed facts. Never the key: only whether it is
    set."""
    view = await settings_view(state, "llm", version)
    active = view["active"]
    live = state.live.cost() if state.live is not None else None
    view.update(
        key=state.has_key,
        models=[
            {
                "id": mid,
                "name": m["name"],
                "effort_default": m["effort_default"],
        # Where each section shows, its name, and when it plays (GUIDE.md; AGENT-PAGE.md).
        guide=prompts.load_guide(),
                "thinking_off": m["thinking_off"] is not None,
                "price": dict(zip(("input", "output", "cache_read"), m["price"])),
            }
            for mid, m in config.LLM_MODELS.items()
        ],
        efforts=list(config.EFFORTS),
        thinking_off_efforts=list(config.THINKING_OFF_EFFORTS),
        ranges=config.LLM_RANGES,
        cache_write=config.CACHE_WRITE,
        inference_geo=config.INFERENCE_GEO,
        geo_price=config.GEO_PRICE,
        usage=anthropic_cycle(
            state.calls,
            live=live,
            budget=float(active["budget_usd"]),
            offset=float(active["offset_usd"]),
            day=int(active["cycle_day"]),
        ),
    )
    return view


def _version_row(v: SettingVersion) -> dict[str, Any]:
    return {"id": v.id, "label": v.label, "active": v.active, "created_at": v.created_at, "created_by": v.created_by}


async def cartesia_view(state: State, version: int) -> dict[str, Any]:
    """The Config page's Cartesia view (CONSOLE.md §7): the saved versions, newest first; one
    version's values (the active one when `version` is 0); the active version's values, so the
    page can mark what an older version changed; and each lever's range. Raises LookupError for
    a version that does not exist."""
    view = await settings_view(state, "voice", version)
    return {
        "key": state.has_voice,
        **view,
        "api_version": CARTESIA_API_VERSION,
        "tts": {
            "model": config.TTS_MODEL,
            "voices": [{"name": n, "id": i} for n, i in config.RECOMMENDED_VOICES],
            "emotions": list(config.SONIC_EMOTIONS),
            "ranges": config.SONIC_LEVERS,
        },
        "stt": {
            "model": config.STT_MODEL,
            "profiles": config.TURN_PROFILES,
            "ranges": config.INK_TURN_LEVERS,
        },
        "rates": {"ink_per_s": config.INK_CREDITS_PER_S, "sonic_per_char": config.SONIC_CREDITS_PER_CHAR},
    }


class VoiceNotFound(Exception):
    """Cartesia has no voice of that name, or its id cannot be found (the message is safe)."""


async def resolve_voice(values: dict[str, Any]) -> dict[str, Any]:
    """Give the voice its Cartesia id before a save. A voice from Cartesia's docs has one
    (config.RECOMMENDED_VOICES); any other is found by name in Cartesia's voice list, which needs
    the key and costs no credits. The name never leaves this machine except to Cartesia."""
    voice = values.get("voice")
    if not isinstance(voice, dict) or not isinstance(voice.get("name"), str):
        raise VoiceNotFound("voice: a name")
    name = voice["name"].strip()
    known = dict(config.RECOMMENDED_VOICES)
    vid = voice.get("id") or known.get(name)
    if not vid:
        key = config.cartesia_key()
        if key is None:
            raise VoiceNotFound("voice: finding its id needs the Cartesia key in .env")
        vid = await _find_voice_id(key, name)
    return {**values, "voice": {"name": name, "id": vid}}


async def _find_voice_id(key: str, name: str) -> str:
    """Cartesia's List Voices (GET /voices?q=), exact name match. Its own API version."""
    import aiohttp

    headers = {"Authorization": f"Bearer {key}", "Cartesia-Version": CARTESIA_VOICES_VERSION}
    url = "https://api.cartesia.ai/voices"
    try:
        async with (
            aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=10)) as s,
            s.get(url, params={"q": name, "limit": "20"}, headers=headers) as r,
        ):
            if r.status != 200:
                raise VoiceNotFound(f"voice: Cartesia did not list voices ({r.status})")
            body = await r.json()
    except aiohttp.ClientError:
        raise VoiceNotFound("voice: Cartesia did not answer") from None
    for v in body.get("data") or []:
        if isinstance(v, dict) and str(v.get("name", "")).strip().lower() == name.lower() and v.get("id"):
            return str(v["id"])
    raise VoiceNotFound("voice: Cartesia has no voice of that name")


def _refuse_call(state: State, ehr_source: str) -> str | None:
    """Why a new test call cannot start now, or None."""
    if state.live is not None:
        return "a test call is already running"
    if state.ehr is None:
        return "ehr is not up yet"
    if not state.has_key:
        return "a test call needs an Anthropic key in .env"
    if ehr_source not in ("local", "epic"):
        return "ehr: local or epic"
    if ehr_source == "epic" and not state.epic:
        return "Epic mode needs epic-client.json and the non-production key"
    return None


def _open_record(
    state: State, ehr_source: str, seat: str, llm: dict[str, Any], words: prompts.Words
) -> CallRecord:
    """Open the test call's record and make it the live call: the watch stream follows it.
    `llm` and `words`: the llm settings and the agent's words the call runs with (CONSOLE.md §8,
    §9)."""
    record = CallRecord(
        state.mode.value,
        llm["model"],
        source="test",
        ehr=ehr_source,
        seat=seat,
        llm=llm,
        prompt_version=words.label,
        system_version=words.system_label,
    )
    state.live = record
    redact.dev_tap = record.log
    record.listeners.append(
        lambda text: state.broadcast({"type": "event", "call": record.call_id, "event": json.loads(text)})
    )
    state.broadcast({"type": "started", "call": record.call_id})
    return record


async def _run_call(
    state: State,
    transport: BaseTransport,
    ehr_source: str,
    voice: Voice | None,
    record: CallRecord | None = None,
    llm: dict[str, Any] | None = None,
    words: prompts.Words | None = None,
) -> None:
    """Run one test call on its seat until it ends, then close and file its record. The whole
    call runs with the llm settings and the agent's words active when it starts (Config ›
    Anthropic and Agent, CONSOLE.md §8, §9): the words are this task's own (prompts.use), so a
    save during the call changes nothing in it."""
    if llm is None:
        llm = await active_settings(state, "llm")
    if words is None:
        words = await active_words(state)
    if record is None:
        record = _open_record(state, ehr_source, "text", llm, words)
    assert state.ehr is not None
    token = prompts.use(words)

    reason = "the caller hung up"
    try:
        typed = TypedCall(
            state.ehr, build_llm(state.mode, values=llm), record, transport=transport, ehr_source=ehr_source, voice=voice
        )

        @transport.event_handler("on_client_disconnected")
        async def _gone(transport, client):
            await typed.cancel()

        reason = await typed.serve()
    except CallFailed as e:
        reason = f"Claude's request failed ({e})"
    except Exception as e:  # noqa: BLE001 - the record must close, and the server must live on
        reason = f"the harness failed ({type(e).__name__})"
        record.event("error", type=type(e).__name__, message=str(e))
    finally:
        prompts.reset(token)
        record.close(reason, cartesia=voice.meter.totals() if voice is not None else None)
        redact.dev_tap = None
        state.voice = None
        await _file(state.ehr, record)
        state.live = None
        state.catalog.refresh(record.call_id)
        state.broadcast({"type": "ended", "call": record.call_id})


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
