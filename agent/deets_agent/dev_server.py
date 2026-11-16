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
- `GET  /agent/hospital`, `POST /agent/hospital` — Config › Hospital (HOSPITAL-CONFIG.md): the
  hospital's lists and numbers, their versions and limits; a save makes a new active version
  that each call takes when it opens.
- `POST /agent/hospital/import-places` — Places › Import from Epic (HOSPITAL-CONFIG.md §8): the
  hospital's facilities, departments and specialties from Epic's directory; saves nothing.
- `GET  /agent/resources?call=` — Cartesia's credits and Claude's dollars this cycle, counted
  from the records and the live calls, and one call's use (resources.py; CHAT.md §17).
- `POST /agent/voice?ehr=local|epic` — the voice seat: a WebRTC offer in, the answer out; the
  call runs over that connection (CHAT.md §17).
- `GET  /agent/catalog` — every dev call record, its row and current ratings (catalog.py), and
  the rubric's dimensions.
- `GET  /agent/calls/{id}` — one record's events, for Demo.
- `GET  /agent/latency` — per-tool latency: a summary per tool and model (p50, p90, max of each
  span), and each tool call's row with `rows=true`. Filters: `source`, `model`, `since` (a day
  or a call id), `exact`, `with_replayed` (latency.py).
- `POST /agent/annotations` — a label, a note, a star or a rating; kept in the call store
  through ehr (DATABASE.md §9); 503 without a store.
- `WS   /agent/call` — the caller's seat (Test): RTVI as JSON (rtvi_json.py). Several calls run
  at once, up to `MAX_LIVE_CALLS` (docs/ops/DEMO.md §3.6). `?label=` the words typed beside
  Start; `?tag=` the page's own mark, so it knows which started call is its own.
- `WS   /agent/watch` — the watch stream: every live call's events as the records write them,
  the same lines Demo plays back. `hello` lists the live calls; `started` names each new one.
- `GET  /agent/sandbox?ehr=local|epic` — the sandbox patients and their visits: `local` from
  ehr/fixtures/sandbox.json; `epic` from Epic's public sandbox through `ehr epic-sandbox`, kept
  for 5 minutes (CHAT.md §13). `more`: per patient, the visits past the ones listed.
- `WS   /agent/call?ehr=local|epic` — see below; `ehr` picks the EHR the test call reads.
- `POST /2010-04-01/Accounts/{sid}/Messages.json` — the fake Twilio (fake_twilio.py), for ehr
  only: its own Basic auth instead of the token. A text it takes joins the live call's record.
- `GET  /agent/ice` — the ICE servers a voice call's browser end uses: none on this PC; for a
  guest, Cloudflare STUN and TURN (turn.py; docs/ops/DEMO.md §3.7).
- `GET  /demo`, `GET /demo/{path}` — the demo (`--static`, docs/ops/DEMO.md): the built console.
  `GET /agent/build` — its live reload: the page's entry script, again whenever it changes.
- `POST /agent/csp-report` — a browser's report of what the page's policy blocked
  (security_headers.py; CONSOLE.md §5.1). The demo's files carry the page's headers; every
  other reply carries the API's.

**The wall.** A WebSocket is not covered by CORS: any web page can open one to 127.0.0.1. So
every request must carry the launch token (only Vite's proxy has it) and come from the
console's own origin. The events carry caller lines and tool arguments; that is safe only
because every caller here is synthetic (call_record.py).

**The second door** (docs/ops/DEMO.md §3.3), only when `.env` names an Access app: a guest
comes through Cloudflare Access and the tunnel with no launch token. The request passes with a
valid Access token and the demo's own origin (access.py). Paths under `/demo/agent/` are the
same routes as `/agent/` (`DemoPrefix`), so the built console works under its base path.
"""

from __future__ import annotations

import argparse
import asyncio
import hmac
import itertools
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET
from contextlib import asynccontextmanager
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any

import aiohttp
import uvicorn
from dotenv import load_dotenv
from fastapi import FastAPI, HTTPException, Request, WebSocket, WebSocketDisconnect
from fastapi.responses import (
    FileResponse,
    JSONResponse,
    RedirectResponse,
    Response,
    StreamingResponse,
)
from pipecat.services.cartesia.tts import _CARTESIA_API_VERSION as CARTESIA_API_VERSION
from pipecat.transports.base_transport import BaseTransport, TransportParams
from pipecat.transports.smallwebrtc.connection import SmallWebRTCConnection
from pipecat.transports.smallwebrtc.transport import SmallWebRTCTransport
from pipecat.transports.websocket.fastapi import FastAPIWebsocketParams, FastAPIWebsocketTransport

from deets_agent import config, prompts, redact
from deets_agent.access import AccessGate, AccessSettings
from deets_agent.call import CallFailed, TypedCall
from deets_agent.call_record import CALLS, CallRecord, replay_cost, shape_digest
from deets_agent.catalog import (
    DIMENSIONS,
    OUTCOMES,
    RUBRIC_VERSION,
    AnnotationError,
    Catalog,
    read_events,
)
from deets_agent.cloudflare import AccessGroup, CloudflareError, CloudflareSettings
from deets_agent.ehr_client import EhrClient, EhrError, ErrorKind, SettingVersion
from deets_agent.ehr_process import running_ehr
from deets_agent.fake_twilio import FakeTwilio
from deets_agent.flow import tool_scope, tool_steps
from deets_agent.llm import Mode, build_llm
from deets_agent.resources import anthropic_cycle, call_use, cartesia_cycle
from deets_agent.rtvi_json import RtviJsonSerializer
from deets_agent.security_headers import MAX_REPORT_BYTES, SecurityHeaders
from deets_agent.turn import TurnError, TurnSettings, for_aiortc, ice_servers
from deets_agent.voice import Voice

TOKEN_HEADER = "x-deets-token"
# Who a request is (State.admit): the owner, through the launch token; else a guest's email.
OWNER = "owner"
# How often the owner's requests on this PC stamp their row's last seen, in seconds.
OWNER_SEEN_EVERY = 60.0
# The largest picture the People routes take (ehr's users.rs › MAX_PHOTO).
MAX_PHOTO = 256 * 1024
# The demo's base path (docs/ops/DEMO.md §3.1): the built console and its routes under it.
DEMO_BASE = "/demo"
# The most calls that run at once (docs/ops/DEMO.md §3.6): room for the owner and a few
# analysts, and a stop for a page stuck in a loop of Starts.
MAX_LIVE_CALLS = 8
# The longest label typed beside Start (the catalog's own label limit is longer).
MAX_LABEL = 200
# How often the live reload looks at the demo build, and how often it sends a keep-alive.
BUILD_POLL_S = 1.0
BUILD_KEEPALIVE_S = 15.0
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


@dataclass
class LiveCall:
    """One call in progress (docs/ops/DEMO.md §3.6). `by` is who started it: `OWNER`, or a
    guest's email from the Access token. The email lives here, in memory, for the Caller rail's
    list; the record holds only `owner` or `guest`."""

    record: CallRecord
    seat: str
    ehr: str
    by: str
    label: str | None
    tag: str | None
    started: str = field(default_factory=lambda: datetime.now().astimezone().isoformat(timespec="seconds"))
    voice: Voice | None = None
    # When this call last called a tool. ehr sends a code by text during the tool's RPC, before
    # the `ehr` event is written, so the fake Twilio's text joins the call whose tool call is
    # the newest. Two calls verifying in the same instant could swap texts (dev only).
    texted_at: int = 0

    def info(self) -> dict[str, Any]:
        return {
            "call": self.record.call_id,
            "started": self.started,
            "seat": self.seat,
            "ehr": self.ehr,
            "by": self.by,
            "label": self.label,
            "tag": self.tag,
        }


class State:
    def __init__(
        self,
        token: str,
        origins: set[str],
        port: int,
        calls: Path = CALLS,
        access: AccessGate | None = None,
        static: Path | None = None,
        turn: TurnSettings | None = None,
        cloudflare: AccessGroup | None = None,
    ) -> None:
        self.token = token
        self.origins = origins
        # The demo (docs/ops/DEMO.md): the second door, the built console, the TURN key.
        self.access = access
        self.static = static.resolve() if static is not None else None
        # The response headers (CONSOLE.md §5.1), from the one file the deploy reads too.
        self.headers = SecurityHeaders.load()
        self.turn = turn
        # The People list's link to Cloudflare's Access group (cloudflare.py; DEMO.md §3.5), and
        # one change at a time to the list and the group.
        self.cloudflare = cloudflare
        self.people_lock = asyncio.Lock()
        # When the owner's row was last stamped as seen (the launch token's door), so this PC's
        # requests stamp it at most once a minute.
        self.owner_seen_at = 0.0
        self.twilio = FakeTwilio(port)
        self.calls = calls
        self.catalog = Catalog(calls).build()
        # Voice (CHAT.md §17): the Cartesia key's presence. Each live voice call's parts are on
        # its own `LiveCall`.
        self.has_voice = config.cartesia_key() is not None
        # The settings services whose Default version is known to exist (ensure_default).
        self.seeded: set[str] = set()
        self.ehr: EhrClient | None = None
        # Test is always live (the owner, 2026-11-12: "limit replaying conversations only to the
        # Demo view"): every line goes to Claude, no replay cache. Without a key, no test call.
        self.has_key = config.anthropic_key() is not None
        self.mode = Mode.LIVE
        # The live calls by id, oldest first (docs/ops/DEMO.md §3.6: several at once).
        self.lives: dict[str, LiveCall] = {}
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

    async def admit(self, headers) -> str | None:
        """Who the request is: `OWNER` through the launch token (the first door), a guest's
        email through a valid Access token (the second door, only in the demo), else None."""
        if self.allowed(headers):
            return OWNER
        if self.access is not None:
            return await self.access.email(headers)
        return None

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
            # Labels, stars and ratings live in the store (DATABASE.md §9).
            await load_annotations(state)
            # The People list and Cloudflare's group agree before the first guest arrives.
            await sync_people(state)
            try:
                yield
            finally:
                await state.ehr.close()

    app = FastAPI(lifespan=lifespan, docs_url=None, redoc_url=None, openapi_url=None)

    @app.middleware("http")
    async def wall(request: Request, call_next):
        if request.url.path == state.twilio.path():
            # ehr, not the console: the fake Twilio's own check (fake_twilio.py). The tunnel's
            # requests also come from 127.0.0.1, so one that carries Cloudflare's header is
            # never ehr.
            host = request.client.host if request.client else None
            through_tunnel = "cf-connecting-ip" in request.headers
            if through_tunnel or not state.twilio.authorized(request.headers, host):
                return JSONResponse({"code": 20003, "message": "Authenticate", "status": 401}, status_code=401)
            return await call_next(request)
        who = await state.admit(request.headers)
        if who is None:
            return JSONResponse({"error": "forbidden"}, status_code=403)
        request.state.who = who
        if who == OWNER and state.ehr is not None and time.monotonic() - state.owner_seen_at > OWNER_SEEN_EVERY:
            state.owner_seen_at = time.monotonic()
            try:
                await state.ehr.seen_owner()
            except EhrError:
                pass  # no store, or no owner yet: nothing to stamp
        return await call_next(request)

    # Added after the wall, so it runs outside it: a refusal carries the headers too.
    # `DemoPrefix` has already taken `/demo/agent/…` to `/agent/…`, so a path under the demo's
    # base here is one of the console's files.
    @app.middleware("http")
    async def response_headers(request: Request, call_next):
        response = await call_next(request)
        if state.static is not None and request.url.path.startswith(f"{DEMO_BASE}/"):
            response.headers.update(state.headers.page(f"{DEMO_BASE}/", demo=True))
        else:
            response.headers.update(state.headers.api())
        return response

    @app.post("/agent/csp-report")
    async def csp_report(request: Request):
        """A browser's report of a blocked load or sink (CONSOLE.md §5.1). The log line names
        the directive and what was blocked as a kind or a scheme and host; nothing else."""
        body = b""
        async for chunk in request.stream():
            body += chunk
            if len(body) > MAX_REPORT_BYTES:
                raise HTTPException(413, "too large")
        if state.headers.report(body) is None:
            raise HTTPException(400, "not a report")
        return Response(status_code=204)

    @app.post("/2010-04-01/Accounts/{account}/Messages.json")
    async def twilio_messages(account: str, request: Request):
        if request.url.path != state.twilio.path():
            return JSONResponse({"code": 20404, "message": "Not found", "status": 404}, status_code=404)
        status, reply = state.twilio.take(await request.body())
        text = reply.pop("_text", None)
        # The call that asked ehr for a code most recently (several calls at once, DEMO.md
        # §3.6); ehr's request does not name the call.
        asked = max(state.lives.values(), key=lambda c: c.texted_at, default=None)
        if text is not None and asked is not None:
            # The phone that got it, on the Chat page: the code button (CHAT.md §8).
            asked.record.event("text", to=text["to"], body=text["body"])
        return JSONResponse(reply, status_code=status)

    @app.get("/agent/health")
    async def health():
        newest = next(reversed(state.lives), None)
        return {
            "key": state.has_key,
            "live": newest,
            "calls": len(state.lives),
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

    @app.get("/agent/hospital")
    async def hospital_settings(version: int = 0):
        """Config › Hospital (HOSPITAL-CONFIG.md): the versions and one version's lists and
        numbers, the active one's, and the limits ehr checks. Facts about the hospital only,
        never a call's value."""
        if version < 0:
            raise HTTPException(400, "version: not a version")
        try:
            return await hospital_view(state, version)
        except LookupError:
            raise HTTPException(404, "no such version") from None

    @app.post("/agent/hospital")
    async def save_hospital(request: Request):
        """Save the hospital as a new, active version: ehr checks every list and number (the
        error names the list and the row, never a value). The next call opens with it; a call
        in progress keeps its own."""
        try:
            body = await request.json()
        except ValueError:
            raise HTTPException(400, "not JSON") from None
        values = body.get("values") if isinstance(body, dict) else None
        if not isinstance(values, dict) or set(values) != set(config.HOSPITAL_KEYS):
            raise HTTPException(400, "values: every list and number of the hospital, once")
        await save_settings(state, "hospital", {k: values[k] for k in config.HOSPITAL_KEYS})
        return await hospital_view(state, 0)

    @app.post("/agent/hospital/import-places")
    async def import_places(request: Request):
        """Config › Hospital › Places, Import from Epic (HOSPITAL-CONFIG.md §8): the hospital's
        facilities, departments and specialties from Epic's directory, searched by each address
        (a state or a city). Saves nothing: the page merges the rows into its draft, and Save
        makes the version. One to a few reads to Epic per import. Facts about the hospital
        only."""
        try:
            body = await request.json()
        except ValueError:
            raise HTTPException(400, "not JSON") from None
        addresses = body.get("addresses") if isinstance(body, dict) else None
        if not isinstance(addresses, list) or not all(isinstance(a, str) for a in addresses):
            raise HTTPException(400, "addresses: a list of states or cities")
        if state.ehr is None:
            raise HTTPException(409, "ehr is not up yet")
        try:
            return await state.ehr.import_places([a.strip() for a in addresses])
        except EhrError as e:
            if e.kind is ErrorKind.BAD_INPUT:
                raise HTTPException(400, e.detail or "the addresses do not fit") from None
            if e.kind is ErrorKind.NOT_ALLOWED_NOW:
                raise HTTPException(409, "ehr does not serve Epic's sandbox: epic-client.json and the Epic key are not here") from None
            if e.kind is ErrorKind.UNAVAILABLE:
                raise HTTPException(503, "Epic's directory did not answer") from None
            raise HTTPException(502, e.detail or "the import failed") from None

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
    async def resources(call: str | None = None):
        """Cartesia's credits this cycle, from the records and the live voice calls (summed when
        several run at once), less the active settings' offset; Claude's dollars this cycle
        against the active budget; and, with `call`, that one call's use so far: Claude's
        dollars and Cartesia's credits (CHAT.md §17). Counts only."""
        live: dict[str, Any] | None = None
        for c in state.lives.values():
            if c.voice is None:
                continue
            now = {**c.voice.meter.totals(), "seconds": round(c.voice.meter.elapsed())}
            live = now if live is None else {k: live.get(k, 0) + v for k, v in now.items()}
        values = await active_voice_settings(state)
        cycle = cartesia_cycle(
            state.calls,
            live=live,
            plan=int(values["plan_credits"]),
            offset=int(values["offset"]),
            day=int(values["cycle_day"]),
            limits=(int(values["idle_s"]), int(values["max_s"])),
        )
        llm = await active_settings(state, "llm")
        claude = anthropic_cycle(
            state.calls,
            live=sum(c.record.cost() for c in state.lives.values()) if state.lives else None,
            budget=float(llm["budget_usd"]),
            offset=float(llm["offset_usd"]),
            day=int(llm["cycle_day"]),
        )
        return {"cartesia": cycle, "anthropic": claude, "call": call_now(state, call), "voice": state.has_voice}

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
        await file_annotation(state, body)
        return {"ok": True}

    @app.websocket("/agent/watch")
    async def watch(ws: WebSocket):
        if await state.admit(ws.headers) is None:
            await ws.close(code=1008)
            return
        await ws.accept()
        q: asyncio.Queue = asyncio.Queue()
        state.watchers.add(q)
        try:
            lives = list(state.lives.values())
            hello = {
                "type": "hello",
                "live": lives[-1].record.call_id if lives else None,
                "calls": [c.info() for c in lives],
            }
            await ws.send_text(json.dumps(hello))
            # Each live call so far, oldest first, then every call's events as they come.
            for c in lives:
                for event in read_events(c.record.path):
                    await ws.send_text(
                        json.dumps({"type": "event", "call": c.record.call_id, "event": event})
                    )
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
        who = await state.admit(ws.headers)
        if who is None:
            await ws.close(code=1008)
            return
        # The EHR this call reads: the page's Epic / Local button (CHAT.md §13).
        ehr_source = ws.query_params.get("ehr", "local")
        if ehr_source not in ("local", "epic"):
            await ws.accept()
            await ws.close(code=1008, reason="ehr: local or epic")
            return
        refused = _refuse_call(state, ehr_source)
        if refused is not None:
            await ws.accept()
            await ws.close(code=1013, reason=refused)
            return
        label, tag = _label(ws.query_params.get("label")), _tag(ws.query_params.get("tag"))
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
        llm = await active_settings(state, "llm")
        words = await active_words(state)
        live = _open_record(state, ehr_source, "text", llm, words, who, label, tag)
        await _run_call(state, transport, live, llm, words)

    @app.post("/agent/voice")
    async def voice(request: Request, ehr: str = "local", label: str = "", tag: str = ""):
        """The voice seat (CHAT.md §17): the browser's WebRTC offer in, the answer out. The call
        then runs over that connection: the caller's microphone to Ink, Sonic to the speaker, and
        RTVI on its data channel (typed lines too). The browser sends its offer with every ICE
        candidate in it, so there is no trickle route. `label` and `tag` as on `/agent/call`."""
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
        # A voice Cartesia no longer has makes every line silent (the call of 2026-11-15, 22:38:
        # "Emma" was gone, and the caller heard nothing for a minute). Ask before the call
        # starts; no credits.
        missing = await _voice_missing(key, (values.get("voice") or {}).get("id") or config.voice_id())
        if missing is not None:
            raise HTTPException(409, missing)
        # On this PC both ends are local: host candidates only, no STUN or TURN server. A guest
        # is on another network: STUN and TURN, with this end's own credentials (DEMO.md §3.7).
        servers = await _ice_for(state, request.state.who)
        connection = SmallWebRTCConnection(ice_servers=for_aiortc(servers) if servers else None)
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
        # Count the call now, so the limit holds while this one connects.
        live = _open_record(
            state, ehr, "voice", llm, words, request.state.who, _label(label), _tag(tag)
        )
        live.voice = Voice(key, values)
        task = asyncio.create_task(_run_call(state, transport, live, llm, words))
        state.tasks.add(task)
        task.add_done_callback(state.tasks.discard)
        return {"sdp": answer["sdp"], "type": answer["type"]}

    @app.get("/agent/ice")
    async def ice(request: Request):
        """The ICE servers for a voice call's browser end, fetched before its offer: none on
        this PC; Cloudflare STUN and TURN for a guest (DEMO.md §3.7)."""
        return {"iceServers": await _ice_for(state, request.state.who)}

    @app.get("/agent/build")
    async def build_stream(request: Request):
        """The demo's live reload (DEMO.md §3.2), as server-sent events: the built page's entry
        script now, then again each time a build changes it. The page reloads when it differs
        from its own, once no call or unsaved edit holds it."""
        if state.static is None:
            raise HTTPException(404, "not the demo")

        async def events():
            index = state.static / "index.html"
            sent, seen, quiet = None, None, 0.0
            while not await request.is_disconnected():
                try:
                    stamp = index.stat().st_mtime_ns
                except OSError:
                    stamp = None
                if stamp is not None and stamp != seen:
                    seen = stamp
                    entry = _entry_script(index)
                    if entry is not None and entry != sent:
                        sent, quiet = entry, 0.0
                        yield f"data: {entry}\n\n"
                if quiet >= BUILD_KEEPALIVE_S:
                    quiet = 0.0
                    yield ": keep-alive\n\n"
                await asyncio.sleep(BUILD_POLL_S)
                quiet += BUILD_POLL_S

        return StreamingResponse(
            events(), media_type="text/event-stream", headers={"cache-control": "no-store"}
        )

    @app.get("/demo")
    async def demo_root():
        return RedirectResponse(f"{DEMO_BASE}/", status_code=308)

    @app.get("/demo/{path:path}")
    async def demo_page(path: str):
        """The built console (`--static`): a file under its folder, or index.html for the base
        path itself. Never a file outside the folder, and never a dotfile."""
        if state.static is None:
            raise HTTPException(404, "not the demo")
        file = _static_file(state.static, path)
        if file is None:
            raise HTTPException(404, "not found")
        # The hashed assets never change under their name; the page and the rest are never
        # stored (CONSOLE.md §5.1).
        return FileResponse(file, headers={"cache-control": state.headers.cache(path)})

    @app.get("/agent/people")
    async def people(request: Request):
        """Teams › People (DEMO.md §3.5): everyone who may sign in, the owner first; whether the
        list reaches Cloudflare's group; the invite link; and who is asking (`you`)."""
        return await _people_view(state, request.state.who)

    @app.post("/agent/people")
    async def add_person(request: Request):
        """`{email, name}`: put a person on the list, then on Cloudflare's group. Either may be
        empty, not both (a card to finish later; with no email, no sign-in). Anyone signed in
        may (the owner's rule until roles, 2026-11-13). The view, with `added` (the new id).
        400 names the field that does not fit; 409 when the email is on the list or the store
        is missing."""
        try:
            body = await request.json()
        except ValueError:
            raise HTTPException(400, "not JSON") from None
        if not isinstance(body, dict) or not all(isinstance(body.get(k), str) for k in ("email", "name")):
            raise HTTPException(400, "an object with email and name")
        assert state.ehr is not None
        by = "" if request.state.who == OWNER else request.state.who
        async with state.people_lock:
            try:
                added = await state.ehr.add_user(body["email"], body["name"], by)
            except EhrError as e:
                raise HTTPException(_people_status(e), e.detail or "not added") from None
            # No email: nothing new for the group.
            warning = await _push_people(state) if body["email"].strip() else None
        return {**await _people_view(state, request.state.who), "warning": warning, "added": added}

    @app.delete("/agent/people/{person_id}")
    async def remove_person(person_id: str, request: Request):
        """Take a person off the list (the row stays, for audits), then off Cloudflare's group,
        and end their Access sessions. 403 for the owner; 404 for no such person."""
        assert state.ehr is not None
        async with state.people_lock:
            try:
                email = await state.ehr.remove_user(person_id)
            except EhrError as e:
                raise HTTPException(_people_status(e), e.detail or "not removed") from None
            # A card with no email never reached the group: nothing to forget or sign out.
            if not email:
                return {**await _people_view(state, request.state.who), "warning": None}
            if state.access is not None:
                state.access.forget(email)
            warning = await _push_people(state)
            if state.cloudflare is not None and warning is None:
                try:
                    await state.cloudflare.end_sessions(email)
                except CloudflareError as e:
                    warning = str(e)
        return {**await _people_view(state, request.state.who), "warning": warning}

    @app.patch("/agent/people/{person_id}")
    async def update_person(person_id: str, request: Request):
        """Security › Auth's Save (DEMO.md §3.5): `{first_name, last_name, phone, email}`, the
        whole card. Anyone signed in may edit any card, the owner's too (the owner's rule until
        roles). When the email changed, Cloudflare's group follows, and the old email's Access
        sessions end. 400 names the field; 404 for no such person; 409 when the email is taken."""
        try:
            body = await request.json()
        except ValueError:
            raise HTTPException(400, "not JSON") from None
        keys = ("first_name", "last_name", "phone", "email")
        if not isinstance(body, dict) or not all(isinstance(body.get(k), str) for k in keys):
            raise HTTPException(400, "an object with first_name, last_name, phone and email")
        assert state.ehr is not None
        async with state.people_lock:
            try:
                old = await state.ehr.update_user(person_id, *(body[k] for k in keys))
            except EhrError as e:
                raise HTTPException(_people_status(e), e.detail or "not saved") from None
            warning = None
            if old is not None:
                # "" when the card had no email yet: nothing to forget or sign out.
                if state.access is not None and old:
                    state.access.forget(old)
                warning = await _push_people(state)
                if state.cloudflare is not None and warning is None and old:
                    try:
                        await state.cloudflare.end_sessions(old)
                    except CloudflareError as e:
                        warning = str(e)
        return {**await _people_view(state, request.state.who), "warning": warning}

    @app.get("/agent/people/{person_id}/photo")
    async def person_photo(person_id: str):
        """A person's picture, a WebP; 404 when they have none. Never cached: it is PII."""
        assert state.ehr is not None
        try:
            photo = await state.ehr.user_photo(person_id)
        except EhrError as e:
            raise HTTPException(_people_status(e), e.detail or "no picture") from None
        if photo is None:
            raise HTTPException(404, "no picture")
        return Response(photo, media_type="image/webp", headers={"cache-control": "no-store"})

    @app.put("/agent/people/{person_id}/photo")
    async def set_person_photo(person_id: str, request: Request):
        """Saves a person's picture: the body is the WebP the console made (256 px square)."""
        if request.headers.get("content-type", "").split(";")[0].strip() != "image/webp":
            raise HTTPException(415, "photo: image/webp")
        body = await request.body()
        if not body or len(body) > MAX_PHOTO:
            raise HTTPException(400, "photo: at most 256 KB")
        return await _photo_change(state, request, person_id, body)

    @app.delete("/agent/people/{person_id}/photo")
    async def clear_person_photo(person_id: str, request: Request):
        """Deletes a person's picture."""
        return await _photo_change(state, request, person_id, None)

    return app


async def _photo_change(state: State, request: Request, person_id: str, photo: bytes | None) -> dict[str, Any]:
    assert state.ehr is not None
    try:
        await state.ehr.set_user_photo(person_id, photo)
    except EhrError as e:
        raise HTTPException(_people_status(e), e.detail or "not saved") from None
    return {**await _people_view(state, request.state.who), "warning": None}


def _people_status(e: EhrError) -> int:
    return {
        ErrorKind.BAD_INPUT: 400,
        ErrorKind.NOT_VERIFIED: 403,
        ErrorKind.NOT_ALLOWED_NOW: 409,
    }.get(e.kind, 404 if "no such" in e.detail else 409 if "already" in e.detail else 502)


async def _people_view(state: State, who: str) -> dict[str, Any]:
    """The People view's data. `invite`: the demo's address, for the Copy invite button."""
    assert state.ehr is not None
    try:
        people = await state.ehr.list_users()
        store = True
    except EhrError as e:
        if e.kind is not ErrorKind.NOT_ALLOWED_NOW:
            raise HTTPException(502, "ehr could not read the list") from None
        people, store = [], False
    origin = state.access.settings.origin if state.access is not None else os.environ.get("DEETS_DEMO_ORIGIN")
    return {
        "people": people,
        "store": store,
        "linked": state.cloudflare is not None,
        "invite": f"{origin.rstrip('/')}{DEMO_BASE}/" if origin else None,
        "you": who,
    }


async def _push_people(state: State) -> str | None:
    """Make Cloudflare's group the list's emails. None when done or not linked; else the
    reason, for the page to show (the list itself is saved either way)."""
    if state.cloudflare is None or state.ehr is None:
        return None
    try:
        # A card with no email yet cannot sign in: it is not in the group.
        emails = [p["email"] for p in await state.ehr.list_users() if p["email"]]
        await state.cloudflare.set_emails(emails)
    except (EhrError, CloudflareError) as e:
        return str(e) if isinstance(e, CloudflareError) else "ehr could not read the list"
    return None


async def sync_people(state: State) -> None:
    """At start (DEMO.md §3.5): bring the group's emails into the list (people added in
    Cloudflare's dashboard), then make the group the list, and from then on let a guest in only
    while they are on the list. Without a link to Cloudflare, Access's own policy is the list,
    as before. Prints outcomes only, never an email."""
    if state.cloudflare is None or state.ehr is None:
        return
    async with state.people_lock:
        try:
            added = await state.ehr.seed_users(await state.cloudflare.emails())
        except CloudflareError as e:
            print(f"people: {e}; guests are checked by Access alone", file=sys.stderr, flush=True)
            return
        except EhrError:
            print("people: no store, so no list; guests are checked by Access alone", file=sys.stderr, flush=True)
            return
        warning = await _push_people(state)
    if warning:
        print(f"people: {warning}", file=sys.stderr, flush=True)
    if added:
        print(f"people: {added} brought in from Cloudflare's group", flush=True)
    if state.access is not None:
        ehr = state.ehr
        state.access.set_guests(lambda email: ehr.admit_user(email))


async def _ice_for(state: State, who: str) -> list[dict]:
    """The ICE servers for a voice call: none for the owner on this PC; for a guest, Cloudflare
    STUN and TURN (turn.py), or 409 with the reason when they cannot be had."""
    if who == OWNER:
        return []
    if state.turn is None:
        raise HTTPException(409, "a guest voice call needs the TURN key in .env")
    try:
        return await ice_servers(state.turn)
    except (TurnError, aiohttp.ClientError, TimeoutError):
        raise HTTPException(502, "Cloudflare's TURN service did not answer") from None


def _static_file(root: Path, path: str) -> Path | None:
    """The file `path` names under `root`, or None: outside the folder, a dotfile, a folder."""
    if not path or path.endswith("/"):
        path += "index.html"
    if any(part.startswith(".") for part in path.split("/")) or "\\" in path:
        return None
    file = (root / path).resolve()
    if not file.is_relative_to(root) or not file.is_file():
        return None
    return file


def _entry_script(index: Path) -> str | None:
    """The built page's entry script (`assets/index-<hash>.js`), which changes with each build
    that changes the code."""
    try:
        html = index.read_text(encoding="utf-8")
    except OSError:
        return None
    m = re.search(r'<script[^>]+type="module"[^>]+src="([^"]+)"', html) or re.search(
        r'<script[^>]+src="([^"]+)"[^>]+type="module"', html
    )
    return m.group(1) if m else None


class DemoPrefix:
    """`/demo/agent/…` is `/agent/…` (DEMO.md §3.1): the built console calls its routes under
    its base path, for requests and WebSockets alike. Everything else passes as it is."""

    def __init__(self, app) -> None:
        self.app = app

    async def __call__(self, scope, receive, send):
        if scope["type"] in ("http", "websocket"):
            path = scope.get("path", "")
            if path.startswith(f"{DEMO_BASE}/agent/"):
                path = path[len(DEMO_BASE) :]
                scope = {**scope, "path": path, "raw_path": path.encode()}
        await self.app(scope, receive, send)


# Each settings service (ehr's settings.rs › service) and its code defaults: the Default version.
# The prompt's Default is the prompt file as it is when the store first sees it; the System
# variables' Default is the variables file (SYSTEM-PAGE.md §3.4).
DEFAULTS = {
    "voice": config.voice_defaults,
    "llm": config.llm_defaults,
    "prompt": prompts.load,
    "system": lambda: {"variables": prompts.variables_to_store(prompts.load_variables())},
    "hospital": config.hospital_defaults,
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
    version, saved = got
    # A version saved before a release lacks the sections the release added: those take the
    # shipped words, and the version's own words stay (PROMPT-EDITOR.md §9). A section the code
    # no longer knows is dropped. Without this, one new section sent every call back to the
    # files, and a hospital's own words were lost without a word (2026-11-14).
    values = {**files.sections, **{k: v for k, v in saved.items() if k in prompts.PLACEHOLDERS}}
    filled = sorted(set(prompts.PLACEHOLDERS) - set(saved))
    if filled:
        print(f"prompt: {len(filled)} section(s) new since the active version take the shipped words", file=sys.stderr, flush=True)
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
        # The sections' names and tabs, for the links to Config › Agent.
        guide=prompts.load_guide(),
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
        # Where each section shows, its name, and when it plays (GUIDE.md; AGENT-PAGE.md).
        guide=prompts.load_guide(),
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
    # Any error ends the quote as "failed", never "running": the page stops showing "Working out
    # the cost" and Record and test still records (2026-11-14: a crash left it waiting forever).
    try:
        costs = _script_costs()
        mean = sum(costs.values()) / len(costs) if costs else RECORD_CENTS_PER_SCRIPT / 100
        for script in sorted(SCRIPTS.glob("*.txt")):
            ok, _ = await _dry(script)
            if not ok:
                run["missing"].append(script.stem)
        run["usd"] = round(sum(costs.get(s, mean) for s in run["missing"]), 2)
        run["state"] = "done"
    except Exception as e:  # noqa: BLE001 - the quote is a convenience; the record must still run
        print(f"prompt: the cost quote failed ({type(e).__name__})", file=sys.stderr, flush=True)
        run["state"] = "failed"


async def _measure_costs() -> None:
    """After a passing Record and test: each kept script's cost, from the replies it reads (a
    dry run that logs its request keys), for the next quote."""
    costs: dict[str, float] = {}
    for script in sorted(SCRIPTS.glob("*.txt")):
        ok, keys = await _dry(script)
        if ok:
            costs[script.stem] = round(sum(replay_cost(config.REPLAYS / f"{k}.json") for k in set(keys)), 4)
    RECORDED_COSTS.write_text(json.dumps(costs, indent=1, sort_keys=True) + "\n", encoding="utf-8")


async def _run(args: list[str], env: dict[str, str] | None = None) -> bool:
    """Run one step of Record and test in the agent's folder; True when it passed. Its output is
    not kept: what a run needs (failures, records) comes back in files."""
    proc = await asyncio.create_subprocess_exec(
        *args,
        cwd=config.AGENT,
        stdout=asyncio.subprocess.DEVNULL,
        stderr=asyncio.subprocess.DEVNULL,
        env={**os.environ, **env} if env else None,
    )
    try:
        return await asyncio.wait_for(proc.wait(), timeout=RECORD_LIMIT_S) == 0
    except TimeoutError:
        proc.kill()
        return False


# The longest failure line the Test hint shows and the store keeps.
FAILURE_CHARS = 300
# The most failures the Test hint lists.
FAILURES_SHOWN = 20


def junit_failures(path: Path) -> list[dict[str, str]]:
    """Each failed test in a pytest `--junitxml` file: its name and the first line of its error
    (DATABASE.md §8.5). Synthetic callers only, so a line may quote the agent's words."""
    try:
        root = ET.parse(path).getroot()
    except (OSError, ET.ParseError):
        return []
    out: list[dict[str, str]] = []
    for case in root.iter("testcase"):
        for kind in ("failure", "error"):
            found = case.find(kind)
            if found is None:
                continue
            message = (found.get("message") or found.text or kind).strip()
            first = next((line.strip() for line in message.splitlines() if line.strip()), kind)
            out.append({"test": case.get("name") or "a test", "line": first[:FAILURE_CHARS]})
            break
    return out


def script_tags(path: Path) -> dict[str, str]:
    """A kept script's tags, from its `# key: value` header (DATABASE.md §8.4)."""
    tags: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        m = re.fullmatch(r"#\s*([a-z_]+):\s*(\S.*?)\s*", line)
        if not line.startswith("#"):
            break
        if m and m.group(1) in ("safety", "area", "ehr", "channel", "caller"):
            tags[m.group(1)] = m.group(2)
    return tags


def draw_numbers(path: Path) -> dict[str, Any]:
    """What a draw's record says about it: its shape, time, tool times, live requests and cost."""
    events = read_events(path)
    end = next((e for e in reversed(events) if e.get("kind") == "call_end"), {})
    tools: dict[str, float] = {}
    for e in events:
        if e.get("kind") == "tool_result" and isinstance(e.get("ms"), (int, float)):
            tools[e.get("name") or "?"] = round(tools.get(e.get("name") or "?", 0.0) + float(e["ms"]), 1)
    return {
        "shape_sha256": shape_digest(events),
        "duration_ms": float(max((e.get("t_ms") or 0 for e in events), default=0)),
        "tool_ms": json.dumps(tools),
        "live_requests": int(((end.get("requests") or {}).get("live")) or 0),
        "cost_usd": float(end.get("cost_usd") or 0.0),
        "hospital": next((e.get("version") for e in events if e.get("kind") == "hospital"), None),
    }


def _git_commit() -> str:
    try:
        done = subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"], check=False, cwd=config.AGENT, capture_output=True, text=True, timeout=5
        )
        return done.stdout.strip() if done.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


async def file_test_run(state: State, run: dict[str, Any], folder: Path, request: dict[str, Any]) -> None:
    """Send a finished run to the call store (DATABASE.md §8.3), then the records it asks for.
    Never fails the run: without a store, nothing is filed and the hint still shows."""
    if state.ehr is None:
        return
    try:
        filed = await state.ehr.file_test_run(request)
        if filed is None:
            run["filed"] = {"store": False}
            return
        sent = 0
        for call_id in filed["needed"]:
            path = next(folder.rglob(f"{call_id}.jsonl"), None)
            if path is not None and await state.ehr.file_call(call_id, path) is not None:
                sent += 1
        run["filed"] = {"store": True, "run_id": filed["run_id"], "records": sent, "pruned": filed["pruned"]}
    except (EhrError, OSError, ValueError, TypeError) as e:
        print(f"prompt: the Test run was not filed ({type(e).__name__})", file=sys.stderr, flush=True)
        run["filed"] = {"store": False}


async def record_and_test(state: State) -> None:
    """Record each kept script with the prompt file's words (the typed harness in record mode:
    a line already recorded costs nothing; a new one calls Claude once), then run the agent's
    tests. When every script ran and the tests pass, the recordings are stamped as made with
    these words. Synthetic callers only: the scripts' fake patients.

    Each draw's record goes to a temporary folder, never to agent/calls/; the run, its draws and
    the records the store needs go to the call store (DATABASE.md §8). The page gets the counts
    and each failed test's name and first error line (`failures`, the Test hint)."""
    started = time.time()
    started_at = datetime.now().astimezone().isoformat(timespec="seconds")
    words = prompts.file_digest()
    run: dict[str, Any] = {"state": "running", "scripts": [], "tests": None, "spent_usd": None, "failures": [], "filed": None}
    state.recording = run
    results: list[dict[str, Any]] = []
    hospital: str | None = None
    with tempfile.TemporaryDirectory(prefix="deets-test-run-") as tmp:
        folder = Path(tmp)
        for script in sorted(SCRIPTS.glob("*.txt")):
            keys_file = folder / f"{script.stem}.keys.json"
            draws = folder / "record" / script.stem
            ok = await _run(
                [sys.executable, "harness/typed_call.py", "--script", str(script), "--origin", "kept",
                 "--calls", str(draws), "--keys-out", str(keys_file)]
            )
            run["scripts"].append({"name": script.stem, "ok": ok})
            if not ok:
                run["failures"].append({"test": script.stem, "line": "the script did not finish recording"})
            record = next(draws.glob("kept_*.jsonl"), None)
            if record is None:
                continue
            numbers = draw_numbers(record)
            hospital = hospital or numbers.pop("hospital")
            numbers.pop("hospital", None)
            # A record draw that replayed every line is not a fresh draw: its replay draw follows.
            if numbers["live_requests"] == 0 and ok:
                continue
            try:
                keys = json.loads(keys_file.read_text(encoding="utf-8"))
            except (OSError, ValueError):
                keys = []
            results.append({
                "script": script.stem, "draw": 1, "mode": "record", "passed": ok, "test": "",
                "failure": "" if ok else "the script did not finish recording", "call_id": record.stem,
                "tags": json.dumps(script_tags(script)), "reply_keys": sorted(set(keys)), **numbers,
            })
        # Every test but the stamp's own: the stamp is written only after the others pass.
        stamp_test = "tests/test_dev_server.py::test_the_kept_recordings_match_the_prompt_file"
        replays = folder / "replay"
        replays.mkdir()
        junit = folder / "junit.xml"
        run["tests"] = await _run(
            [sys.executable, "-m", "pytest", "-q", "--deselect", stamp_test, f"--junitxml={junit}"],
            env={"DEETS_KEPT_DIR": str(replays)},
        )
        failed = junit_failures(junit)
        run["failures"] = (run["failures"] + failed)[:FAILURES_SHOWN]
        if not run["tests"] and not failed:
            run["failures"].append({"test": "pytest", "line": "the tests did not finish (a time limit, or a crash before any test)"})
        lines = folder / "replay" / "kept.jsonl"
        by_test = {f["test"]: f["line"] for f in failed}
        for raw in lines.read_text(encoding="utf-8").splitlines() if lines.exists() else []:
            try:
                kept = json.loads(raw)
                record = replays / kept["record"]
                numbers = draw_numbers(record)
            except (OSError, ValueError, KeyError):
                continue
            numbers.pop("hospital", None)
            script = SCRIPTS / f"{kept['script']}.txt"
            line = by_test.get(kept["test"])
            results.append({
                "script": kept["script"], "draw": 1, "mode": "replay", "passed": line is None, "test": kept["test"],
                "failure": line or "", "call_id": record.stem,
                "tags": json.dumps(script_tags(script) if script.exists() else {}),
                "reply_keys": [k for k in kept.get("keys", []) if isinstance(k, str)], **numbers,
            })
        run["spent_usd"] = round(
            sum(replay_cost(p) for p in config.REPLAYS.glob("*.json") if p.stat().st_mtime >= started), 4
        )
        passed = run["tests"] and all(s["ok"] for s in run["scripts"])
        await file_test_run(state, run, folder, {
            "started": started_at,
            "finished": datetime.now().astimezone().isoformat(timespec="seconds"),
            "run_by": "dev",
            "words_sha256": words,
            "prompt_version": "file",
            "system_version": "file",
            "hospital_version": hospital or "",
            "llm_version": "",
            "model": config.MODEL,
            "git_commit": _git_commit(),
            "tests_passed": bool(run["tests"]),
            "cost_usd": run["spent_usd"],
            "results": results,
        })
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


def call_now(state: State, call: str | None) -> dict[str, Any] | None:
    """One call's use for the Resources tab: a live call's running totals, else its record's
    (the catalog checks the id). None when there is no such call."""
    if not call:
        return None
    lc = state.lives.get(call)
    if lc is not None:
        return {
            "seat": lc.seat,
            "usd": round(lc.record.cost(), 6),
            "credits": int(lc.voice.meter.totals().get("credits") or 0) if lc.voice is not None else None,
            "live": True,
        }
    events = state.catalog.events(call)
    return call_use(events) if events is not None else None


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


async def hospital_view(state: State, version: int) -> dict[str, Any]:
    """The Config page's Hospital view (HOSPITAL-CONFIG.md): the common part and the limits ehr's
    hospital.rs checks, so the page can say what does not fit before a save."""
    view = await settings_view(state, "hospital", version)
    view.update(keys=list(config.HOSPITAL_KEYS), limits=config.HOSPITAL_LIMITS)
    return view


async def anthropic_view(state: State, version: int) -> dict[str, Any]:
    """The Config page's Anthropic view (CONSOLE.md §8): the versions and values, each model's
    rules and prices, this cycle's spend, and the fixed facts. Never the key: only whether it is
    set."""
    view = await settings_view(state, "llm", version)
    active = view["active"]
    live = sum(c.record.cost() for c in state.lives.values()) if state.lives else None
    view.update(
        key=state.has_key,
        models=[
            {
                "id": mid,
                "name": m["name"],
                "effort_default": m["effort_default"],
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


async def _voice_missing(key: str, voice_id: str) -> str | None:
    """Why the voice cannot speak, or None: Cartesia's Get Voice answers 404 for a voice it no
    longer has. Any other answer, or none, lets the call start: the call itself then shows a
    fault. No credits; the message names no value."""
    import aiohttp

    headers = {"Authorization": f"Bearer {key}", "Cartesia-Version": CARTESIA_VOICES_VERSION}
    try:
        async with (
            aiohttp.ClientSession(timeout=aiohttp.ClientTimeout(total=5)) as s,
            s.get(f"https://api.cartesia.ai/voices/{voice_id}", headers=headers) as r,
        ):
            if r.status == 404:
                return "Cartesia no longer has the active voice: choose another on Config › Cartesia"
    except (aiohttp.ClientError, TimeoutError):
        return None
    return None


def _refuse_call(state: State, ehr_source: str) -> str | None:
    """Why a new test call cannot start now, or None."""
    if len(state.lives) >= MAX_LIVE_CALLS:
        return f"{MAX_LIVE_CALLS} calls are running; wait for one to end"
    if state.ehr is None:
        return "ehr is not up yet"
    if not state.has_key:
        return "a test call needs an Anthropic key in .env"
    if ehr_source not in ("local", "epic"):
        return "ehr: local or epic"
    if ehr_source == "epic" and not state.epic:
        return "Epic mode needs epic-client.json and the non-production key"
    return None


# Every tool call of every live call, counted in order (LiveCall.texted_at).
_TOOL_CALLS = itertools.count(1)


def _label(raw: str | None) -> str | None:
    """The words typed beside Start (DEMO.md §3.6), trimmed to one line of `MAX_LABEL`; None
    when empty."""
    if not raw:
        return None
    text = " ".join("".join(ch if ch.isprintable() else " " for ch in raw).split())[:MAX_LABEL]
    return text or None


def _tag(raw: str | None) -> str | None:
    """The page's own mark for its call: letters, digits and dashes, or None."""
    return raw if raw and re.fullmatch(r"[A-Za-z0-9-]{1,40}", raw) else None


def _open_record(
    state: State,
    ehr_source: str,
    seat: str,
    llm: dict[str, Any],
    words: prompts.Words,
    who: str,
    label: str | None = None,
    tag: str | None = None,
) -> LiveCall:
    """Open the test call's record and add it to the live calls: the watch stream carries it.
    `llm` and `words`: the llm settings and the agent's words the call runs with (CONSOLE.md §8,
    §9). `who`: `OWNER` or a guest's email; the record gets only `owner` or `guest`."""
    record = CallRecord(
        state.mode.value,
        llm["model"],
        folder=state.calls,
        source="test",
        ehr=ehr_source,
        seat=seat,
        llm=llm,
        prompt_version=words.label,
        system_version=words.system_label,
        label=label,
        started_by="owner" if who == OWNER else "guest",
    )
    live = LiveCall(record=record, seat=seat, ehr=ehr_source, by=who, label=label, tag=tag)
    state.lives[record.call_id] = live

    def heard(text: str) -> None:
        event = json.loads(text)
        if event.get("kind") == "tool_call":
            # A count, not a clock: Windows' clock can give two calls the same time.
            live.texted_at = next(_TOOL_CALLS)
        state.broadcast({"type": "event", "call": record.call_id, "event": event})

    record.listeners.append(heard)
    state.broadcast({"type": "started", "call": record.call_id, "info": live.info()})
    return live


async def _run_call(
    state: State,
    transport: BaseTransport,
    live: LiveCall,
    llm: dict[str, Any],
    words: prompts.Words,
) -> None:
    """Run one test call on its seat until it ends, then close and file its record. The whole
    call runs with the llm settings and the agent's words active when it starts (Config ›
    Anthropic and Agent, CONSOLE.md §8, §9): the words are this task's own (prompts.use), so a
    save during the call changes nothing in it. Several run at once, each with its own record,
    voice meter and log tap (DEMO.md §3.6)."""
    assert state.ehr is not None
    record, voice = live.record, live.voice
    token = prompts.use(words)
    # This call's warnings go to this call's record: the tasks the call starts inherit it.
    redact.set_dev_tap(record.log)

    reason = "the caller hung up"
    try:
        typed = TypedCall(
            state.ehr,
            build_llm(state.mode, values=llm),
            record,
            transport=transport,
            ehr_source=live.ehr,
            voice=voice,
        )

        @transport.event_handler("on_client_disconnected")
        async def _gone(transport, client):
            await typed.cancel()

        reason = await typed.serve()
    except CallFailed as e:
        reason = f"Claude's request failed ({e})"
    except EhrError as e:
        # An EHR that does not answer (Epic's sandbox down, 2026-11-14) is not our crash: say so,
        # in words the canvas can show. ehr's terminal line says why and when it is back.
        if e.kind is ErrorKind.UNAVAILABLE:
            reason = f"the EHR did not answer ({e})"
        else:
            reason = f"the harness failed ({type(e).__name__})"
        record.event("error", type=type(e).__name__, message=str(e))
    except Exception as e:  # noqa: BLE001 - the record must close, and the server must live on
        reason = f"the harness failed ({type(e).__name__})"
        record.event("error", type=type(e).__name__, message=str(e))
    finally:
        prompts.reset(token)
        record.close(reason, cartesia=voice.meter.totals() if voice is not None else None)
        redact.set_dev_tap(None)
        await _file(state.ehr, record)
        state.lives.pop(record.call_id, None)
        state.catalog.refresh(record.call_id)
        # The label typed beside Start becomes the call's label in the catalog (DEMO.md §3.6),
        # so nobody types it twice. A later label from the Review panel replaces it.
        if live.label:
            try:
                await file_annotation(state, {"call": record.call_id, "kind": "label", "text": live.label})
            except HTTPException:
                pass  # no store, or the call was not filed: the label is not kept
        state.broadcast({"type": "ended", "call": record.call_id})


async def file_annotation(state: State, body: dict[str, Any]) -> None:
    """One annotation (DATABASE.md §9): the catalog's gate and stamps, then the store through
    ehr, then the catalog. The store is the only place it is kept: without one, 503 and nothing
    is written. 400 names the field that does not fit."""
    try:
        a = state.catalog.stamp(body)
    except AnnotationError as e:
        raise HTTPException(400, str(e)) from None
    if state.ehr is None:
        raise HTTPException(503, "the store is not running")
    try:
        filed = await state.ehr.add_annotation(a)
    except EhrError as e:
        if e.kind is ErrorKind.BAD_INPUT:
            raise HTTPException(400, "call: not in the store yet") from None
        raise HTTPException(503, "the store did not answer") from None
    if filed is None:
        raise HTTPException(503, "the store is not running")
    state.catalog.apply(a)


async def load_annotations(state: State) -> None:
    """Build the catalog again with the store's annotations (DATABASE.md §9). Without a store,
    the catalog has the records only, and says so once."""
    if state.ehr is None:
        return
    try:
        found = await state.ehr.list_annotations()
    except EhrError:
        found = None
    if found is None:
        print("catalog: no store, so no labels, stars or ratings", file=sys.stderr, flush=True)
        return
    state.catalog.close()
    state.catalog = Catalog(state.calls).build(found)


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
    parser.add_argument(
        "--static",
        type=Path,
        default=None,
        help="the demo: serve this built console under /demo/ and open the second door (docs/ops/DEMO.md)",
    )
    args = parser.parse_args()
    token = os.environ.get("DEETS_DEV_TOKEN", "")
    if len(token) < 32:
        print("DEETS_DEV_TOKEN is missing or short. Start the dev server with `npm run dev`.", file=sys.stderr)
        return 2
    redact.install(args.log)
    # The demo's and the People list's settings are in .env, which config otherwise loads only
    # when a key is first asked for.
    load_dotenv(config.REPO / ".env", override=False)
    try:
        linked = CloudflareSettings.from_env()
    except ValueError as e:
        print(f"the People list's settings in .env: {e}", file=sys.stderr)
        return 2
    cloudflare = AccessGroup(linked) if linked is not None else None
    access, turn = None, None
    if args.static is not None:
        # The demo: no second door without Access's settings, and never a door left open.
        try:
            settings = AccessSettings.from_env()
            turn = TurnSettings.from_env()
        except ValueError as e:
            print(f"the demo's settings in .env: {e}", file=sys.stderr)
            return 2
        if settings is None:
            print(
                "the demo needs DEETS_ACCESS_TEAM, DEETS_ACCESS_AUD and DEETS_DEMO_ORIGIN in .env",
                file=sys.stderr,
            )
            return 2
        access = AccessGate(settings)
    state = State(
        token,
        set(args.origin),
        args.port,
        access=access,
        static=args.static,
        turn=turn,
        cloudflare=cloudflare,
    )
    keyed = "test calls live" if state.has_key else "no Anthropic key: Demo only"
    print(f"agent dev server on 127.0.0.1:{args.port} ({keyed})", flush=True)
    if access is not None:
        relay = "guest voice through TURN" if turn is not None else "no TURN key: guests type only"
        print(f"demo at {access.settings.origin}{DEMO_BASE}/ ({relay})", flush=True)
    uvicorn.run(DemoPrefix(build_app(state)), host="127.0.0.1", port=args.port, log_level="warning", access_log=False)
    return 0


if __name__ == "__main__":
    sys.exit(main())
