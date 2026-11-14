---
status: built
desk_test: open
sources: [agent/deets_agent/dev_server.py, agent/deets_agent/catalog.py, agent/deets_agent/rtvi_json.py, agent/deets_agent/fake_twilio.py, agent/deets_agent/resources.py, console/src/agent/api.ts, console/src/agent/transport.ts, console/vite.config.ts, scripts/docs-check.mjs, agent/tests/test_dev_server.py]
updated: 2026-11-14
---
# DeetsHealth — The dev API (the console and the agent's dev server)

> The one reference for every route between the console and the agent's dev server, and a map
> to the other APIs. It is a map into the code, like [CALL-FLOW.md](CALL-FLOW.md): it names the
> handler and the client for each route, and points to the symbol that holds each limit. It
> never copies a limit (DOCS-ORG.md §1, rule 8). `npm run docs:check` (checks 12–14, §7) fails
> when this doc and the code disagree.

**Terms**
- **The console:** the web page in `console/`. It reaches the agent only through Vite's proxy.
- **The dev server:** `agent/deets_agent/dev_server.py`, started by `npm run dev`. Dev only,
  synthetic callers only, on 127.0.0.1, on a free port.
- **The launch token:** a fresh random value that `scripts/dev.mjs` makes at each launch. Only
  Vite's proxy holds it; the page never does.
- **The client:** the function in `console/src/agent/api.ts` (or the class in `transport.ts`)
  that calls a route.

## 1. The surfaces at a glance
| From → to | Wire | Reference |
|---|---|---|
| Console → dev server | HTTP and WebSocket, through Vite's proxy | this doc, §2–6 |
| Agent → `ehr` | gRPC over mTLS | EHR-SERVICE.md §3 (the RPC ledger) |
| Agent → the model | Pipecat Flows functions (the tools) | CALL-FLOW.md, AGENT-PROMPT.md |
| `ehr` → Twilio Messages | HTTPS (dev: the fake in the dev server, §2) | TEXTS.md, TWILIO.md |
| Epic → the JWKS worker | HTTPS `GET` of a key set | EPIC.md, `ehr/jwks-worker/src/index.ts` |
| Agent → Anthropic API | HTTPS | ANTHROPIC.md, AGENT.md |
| Agent → Cartesia (Ink, Sonic) | WebSocket over TLS | CARTESIA.md, `voice.py` |
| Console ↔ dev server, voice | WebRTC (offer by HTTP, §2) | §6, CHAT.md §17 |

No production API exists yet: no staff API (ideas/CONSOLE-MCP.md), no Twilio voice webhook
(Phase 3), no settings API (FUTURE-SETTINGS.md).

## 2. The routes (the ledger)
One row per route the dev server serves. Check 12 compares this table with the route
decorators in `dev_server.py`, both ways. Check 14 asks that every client named here exists.

| Route | Handler | Client | What it answers |
|---|---|---|---|
| `GET /agent/health` | `health` | `health()` | `key` (an Anthropic key is set), `live` (the live call's id, or null), `epic` (`ehr` was given Epic's sandbox) and `voice` (a Cartesia key is set) |
| `GET /agent/cartesia` | `cartesia_settings` | `cartesiaSettings()` | Config › Cartesia (CONSOLE.md §7): the saved versions (newest first, each with its "MM-DD \| n" label and whether it is active), one version's values (`?version=`; the active one by default; the code's defaults before any save), the active values, and each lever's range. `store`: false when `ehr` has no Postgres. 404 for a version that does not exist. Never the key, only `key` (set or not) |
| `GET /agent/prompt` | `prompt_settings` | `promptSettings()` | Config › Agent (CONSOLE.md §9): the versions and values as for Cartesia (the `prompt` service: each section's words), the code values each section must hold, what the `{` and " ." lists offer (the active System variables, the code values, the tools, and the tools each section may name: SYSTEM-PAGE.md §3.6), whether the kept recordings match the prompt file (`recordings`), and the last Record and test (`recording`: its state, each kept script's name and result, the tests, the dollars spent). Never the key, only `key` |
| `POST /agent/prompt` | `save_prompt` | `savePrompt()` | `{values}`, every section's words: the agent first checks each tool named in backticks against the section's steps (SYSTEM-PAGE.md §3.5); then saved through `ehr`'s `SaveSettings` as a new, active version (ehr checks each section's code values, and that every other `{name}` is a System variable of the active System version), then written into `agent/prompts/AGENT-PROMPT.md`. The next test call runs with it. 400 names the section that does not fit; 409 when `ehr` is not up or has no store |
| `GET /agent/system` | `system_settings` | `systemSettings()` | Config › System (SYSTEM-PAGE.md §3.6): the versions and values as for Cartesia (the `system` service: `variables`, a list of `{name, meaning, value}`), the code values (what each is, its source, whether it is protected data, its sections: never a value), the tools (what each does from `agent/prompts/TOOLS.md`; its steps, fields and lines from the code), which sections of the active words use each variable (`used`), the limits and the built-in variables. 404 for a version that does not exist |
| `POST /agent/system` | `save_system` | `saveSystem()` | `{values: {variables}}`: saved through `ehr`'s `SaveSettings` as a new, active version (ehr checks each variable, and that the active words use none that is gone), then written into `agent/prompts/VARIABLES.md`. The next test call runs with it. 400 names the variable that does not fit; 409 when `ehr` is not up or has no store |
| `POST /agent/prompt/estimate` | `estimate_prompt` | `estimatePrompt()` | Record and test's quote: a dry run that replays each kept script with no Claude call (about 30 s, no tokens), in the background; `GET /agent/prompt` shows it as `estimate` (the scripts that need new replies, by name, and the dollars recording them cost last time). Kept until the words change; never runs beside Record and test |
| `POST /agent/prompt/record` | `record_prompt` | `recordPrompt()` | Record and test: records each kept script with the typed harness, then runs the agent's tests, in the background; `GET /agent/prompt` shows its state. Spends Claude tokens for each new line. 409 while one runs or without the Anthropic key |
| `GET /agent/anthropic` | `anthropic_settings` | `anthropicSettings()` | Config › Anthropic (CONSOLE.md §8): the versions and values as for Cartesia (the `llm` service), each model's rules (its default effort, whether thinking can be off) and prices, the efforts that allow thinking off, the token caps' ranges, the cache write prices, the region, and this cycle's spend in dollars from the call records (`usage`). Never the key, only `key` (set or not) |
| `POST /agent/anthropic` | `save_anthropic` | `saveAnthropic()` | `{values}`, every llm setting: saved through `ehr`'s `SaveSettings` as a new, active version; the next test call runs with it. 400 names the setting that does not fit (`ehr` checks the model, effort and thinking together: `settings.rs › coherent`); 409 when `ehr` is not up or has no store |
| `POST /agent/cartesia` | `save_cartesia` | `saveCartesia()` | `{values}`, every voice setting: saved through `ehr`'s `SaveSettings` as a new version, the active one; the reply is the view above. 400 names the setting that does not fit (or the voice Cartesia cannot find); 409 when `ehr` is not up or has no store |
| `GET /agent/resources` | `resources` | `resources()` | Cartesia's credits this cycle, counted from the records' `cartesia` totals and the live voice call's meter (`resources.py`; CHAT.md §17). Counts and dates only |
| `POST /agent/voice` | `voice` | `voiceOffer()` | the voice seat: a WebRTC offer `{type: "offer", sdp}` in, the answer `{type, sdp}` out; `?ehr=` as on `/agent/call`. The call then runs over that connection (§6, CHAT.md §17). 409 with the reason when a call cannot start, 413 for an offer over 64 KB |
| `GET /agent/sandbox` | `sandbox` | `sandbox()` | the sandbox patients and visits, by `?ehr=`: `local` from `ehr/fixtures/sandbox.json` (CHAT.md §8.1); `epic` from Epic's sandbox through `ehr epic-sandbox`, kept 5 minutes (CHAT.md §13). `more`: per patient, the visits past those listed. 400 for another word, 503 with no Epic files, 502 when Epic did not answer |
| `GET /agent/catalog` | `catalog` | `catalog()` | every call row with its current ratings, and the rubric (CHAT.md §7) |
| `GET /agent/calls/{call_id}` | `call_events` | `callEvents()` | one record's events, for Demo; 404 when there is no such call |
| `GET /agent/latency` | `tool_latency` | none yet | latency per tool; the query and the reply are CHAT.md §10.3 |
| `POST /agent/annotations` | `annotate` | `annotate()` | `{ok: true}`, or 400 with the gate's reason (§4) |
| `WS /agent/watch` | `watch` | `watch()` | the watch stream (§5) |
| `WS /agent/call` | `call` | `TextSeat` | the caller's seat (§6) |
| `POST /2010-04-01/Accounts/{account}/Messages.json` | `twilio_messages` | none: `ehr` calls it | the fake Twilio (`fake_twilio.py`; TEXTS.md) |

**The replies when a route refuses:**

| Reply | When |
|---|---|
| HTTP 403 `{"error": "forbidden"}` | no launch token, a wrong one, or another origin (§3) |
| WebSocket close 1008 | the same, on `/agent/watch` or `/agent/call` |
| WebSocket close 1013 | `/agent/call` only: a call is already running, there is no Anthropic key, or `ehr` is not up yet. The close reason says which. |
| HTTP 400 `{"detail": …}` | `/agent/annotations`: the body is not a JSON object, or the gate refused it (§4) |
| HTTP 404 | `/agent/calls/{call_id}`: no such call |
| HTTP 409 `{"detail": …}` | `/agent/voice`: as close 1013 on `/agent/call`, or there is no Cartesia key. The detail says which |
| HTTP 401 / 404 (Twilio's shape) | the fake Twilio: wrong Basic auth, a request with an `Origin`, or another account |

## 3. The wall
- **The token.** Every request through the proxy carries the header named by `TOKEN_HEADER` in
  `dev_server.py`. The proxy adds it (`console/vite.config.ts`); `State.allowed` compares it in
  constant time.
- **The origin.** `State.allowed` also asks that the `Origin` header, when present, is one of
  the console origins that `scripts/dev.mjs` passes with `--origin`. A WebSocket is not covered
  by CORS, so this check is what keeps another page in the same browser out.
- **Dev only.** The events on these routes carry caller lines and tool arguments. That is
  safe only because every caller is synthetic (`call_record.py`). A real call has no path to
  this server.
- The fake Twilio route has its own check instead (`FakeTwilio.authorized`): Basic auth with
  the launch's fake account, and never a request with an `Origin`.

## 4. Annotations
`POST /agent/annotations` takes one annotation. Each line goes to the committed file
`agent/calls/annotations.jsonl`, then into the catalog (CHAT.md §7, §12).

**The body.** One shape per `kind`. The fields of each kind are `FIELDS` in `catalog.py`; the
console's type is `Annotation` in `api.ts`. Check 14 asks that the two list the same kinds and
the same fields.

| `kind` | Fields besides `kind` and `call` |
|---|---|
| `label`, `note` | `text` |
| `keep` (the star) | `value` |
| `rating` | `grader_kind`, `grader`, `dimension`, `verdict`, and optional `score` and `note` |

**The gate** (`check_annotation` in `catalog.py`; the limits are `MAX_TEXT`, `MAX_GRADER`,
`DIMENSIONS`, `GRADER_KINDS`, `VERDICTS` there):
- A field that the kind does not take is refused. So nothing the gate has not checked reaches
  the committed file, which keeps every line forever.
- **The stamps are the catalog's.** `Catalog.annotate` sets `at` (the time) and, on a rating,
  `rubric_version`. A sender that sets either is refused. The file's own lines carry the
  stamps, so a rebuild reads them with `stamped=True`.
- **A refusal names the field, never a value** (`AnnotationError`). The Review panel shows
  the reason in its status line: "Not saved: the dev server refused it (…)"
  (`AnnotationRefused` in `api.ts`).

## 5. The watch stream (`WS /agent/watch`)
The server sends; the page sends nothing. Each frame is one JSON message. The types are
`WatchMessage` in `api.ts`, and check 14 asks that they match the `"type"` values that
`dev_server.py` sends.

| `type` | Fields | When |
|---|---|---|
| `hello` | `live` | at once, on connect. Then, if a call is live, each of its events so far. |
| `started` | `call` | a test call starts |
| `event` | `call`, `event` | one line of the live call's record (`CallEvent`: `t_ms`, `kind`, and the kind's fields; the kinds are CHAT.md §11.2 and `call_record.py`) |
| `ended` | `call` | the call ended and joined the catalog |

`watch()` opens it again after a close while the page is open. A watcher that falls behind
loses lines; it never holds up the call (`State.broadcast`).

## 6. The caller's seat (`WS /agent/call`)
- **One test call at a time.** It is always live: every line goes to Claude (CHAT.md §6).
- **`?ehr=local|epic`** picks the EHR the call reads (CHAT.md §13). Another word closes 1008;
  `epic` with no Epic files closes 1013, with the reason.
- **The wire:** RTVI messages as JSON text, one message per frame, no audio. The client is
  `TextSeat` in `console/src/agent/transport.ts`, driven by `PipecatClient.sendText` in
  `chat.ts`. The server's half is `RtviJsonSerializer` in `rtvi_json.py`: it drops a binary
  frame, a frame longer than `MAX_MESSAGE`, and a message whose label is not RTVI's.
- **The seat gets no tool data:** RTVI's tool reports are off in the seat (CHAT.md §6, the
  wall). The rail's panels get tool calls from the watch stream.
- The call ends when the socket closes, when the agent ends it, or after the session timeout
  in `dev_server.py › call`.

**The voice seat (`POST /agent/voice`, CHAT.md §17).** The same call with voice ends:
- **The handshake:** one request. The browser gathers its ICE candidates first, then sends the
  offer; the dev server answers with Pipecat's `SmallWebRTCConnection`. No STUN or TURN: both
  ends are on this PC. No trickle route.
- **The wire after it:** the microphone's audio to the agent, Sonic's audio back, and on the
  data channel RTVI as JSON (typed lines, as on `/agent/call`) plus a ping each second, which
  tells the server the page is still there. The client is `VoiceSeat` in `transport.ts`.
- **The same rules:** one test call at a time across both seats; the wall (§3) on the request;
  no tool data on RTVI.
- **It ends** when the page hangs up or leaves, when the agent ends it, or at a limit: no
  speech from the caller for `config.VOICE_IDLE_S`, or the call's length reaches
  `config.VOICE_MAX_S` (`call.py › TypedCall._watch_limits`). The record's `call_end` says
  which, with the Cartesia totals.

## 7. How this doc stays true (`npm run docs:check`)
- **Check 12, the route ledger:** every route decorator in `dev_server.py` is a row in §2, and
  every row in §2 is a route. A new route without a row fails.
- **Check 13, the RPC ledger:** every `rpc` in `ehr.proto` is a row in EHR-SERVICE.md §3, has
  its handler in `ehr/src/service.rs` and its method in `agent/deets_agent/ehr_client.py`; and
  every row is an `rpc`.
- **Check 14, the console and the server agree:** the clients named in §2 exist in `api.ts`
  or `transport.ts`; the annotation kinds and fields in `api.ts › Annotation` equal
  `catalog.py › FIELDS`; the watch types in `api.ts › WatchMessage` equal the `"type"` values
  `dev_server.py` sends.
- **Check 6, stale docs (a warning, never a failure):** a `built` doc whose `sources` changed
  after its `updated` date is listed, so a reader can check it.
- **The tests:** `agent/tests/test_dev_server.py` runs every route over HTTP: the wall on each,
  the close codes, the annotation gate's answers and the stamps.

## 8. Desk test
1. `npm run dev`; open the address Vite prints. Open Chat, then Demo, and play any call to its
   end. The Review panel slides in.
2. Star the call, type a label, and set a verdict on one dimension. Each time the status says
   "Saved".
3. Run `git diff agent/calls/annotations.jsonl`: each new line has only its kind's fields,
   plus `at`, and on the rating `rubric_version`.
4. Stop the dev server (Ctrl+C) and set another verdict: the status says "Not saved: the
   agent's dev server is not running".
