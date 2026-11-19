---
status: built
desk_test: open
sources: [agent/deets_agent/dev_server.py, agent/deets_agent/voice_catalog.py, agent/deets_agent/catalog.py, agent/deets_agent/rtvi_json.py, agent/deets_agent/fake_twilio.py, agent/deets_agent/resources.py, agent/deets_agent/access.py, agent/deets_agent/turn.py, agent/deets_agent/cloudflare.py, console/src/reload.ts, console/src/agent/api.ts, console/src/agent/transport.ts, console/vite.config.ts, scripts/docs-check.mjs, agent/tests/test_dev_server.py]
updated: 2026-11-19
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
| `GET /agent/health` | `health` | `health()` | `key` (an Anthropic key is set), `live` (the newest live call's id, or null), `calls` (how many are live), `epic` (`ehr` was given Epic's sandbox) and `voice` (a Cartesia key is set) |
| `GET /agent/cartesia` | `cartesia_settings` | `cartesiaSettings()` | Config › Cartesia (CONSOLE.md §7): the saved versions (newest first, each with its "MM-DD \| n" label and whether it is active), one version's values (`?version=`; the active one by default; the code's defaults before any save), the active values, and each lever's range. `store`: false when `ehr` has no Postgres. 404 for a version that does not exist. Never the key, only `key` (set or not) |
| `GET /agent/cartesia/voices` | `cartesia_voices` | `cartesiaVoices()` | Config › Cartesia's Voice menu and its table of every voice (CONSOLE.md §7.8): Cartesia's English voices from its List Voices (`voice_catalog.py`; no credits), each with its id, name, tagline, description, gender and accents; voices our organization owns and pro clones left out. Held for `voice_catalog.TTL_S`; `?fresh=true` (Refresh) reads again when the held list is older than `FRESH_GAP_S`. `checked_at` (the last good read, or null), `fault` (Cartesia's status when the last read failed, or null; the held list stays). Never the key, only `key` |
| `GET /agent/prompt` | `prompt_settings` | `promptSettings()` | Config › Agent (CONSOLE.md §9): the versions and values as for Cartesia (the `prompt` service: each section's words), the code values each section must hold, what the `{` and " ." lists offer (the active System variables, the code values, the tools, and the tools each section may name: SYSTEM-PAGE.md §3.6), whether the kept recordings match the prompt file (`recordings`), and the last Record and test (`recording`: its state, each kept script's name and result, the tests, the dollars spent, `failures` (each failed test's name and the first line of its error, for the Test hint; DATABASE.md §8.5), and `filed` (what the call store took: the run's id, the records sent, the records pruned)). Never the key, only `key` |
| `POST /agent/prompt` | `save_prompt` | `savePrompt()` | `{values}`, every section's words: the agent first checks each tool named in backticks against the section's steps (SYSTEM-PAGE.md §3.5); then saved through `ehr`'s `SaveSettings` as a new, active version (ehr checks each section's code values, and that every other `{name}` is a System variable of the active System version), then written into `agent/prompts/AGENT-PROMPT.md`. The next test call runs with it. 400 names the section that does not fit; 409 when `ehr` is not up or has no store |
| `GET /agent/system` | `system_settings` | `systemSettings()` | Config › System (SYSTEM-PAGE.md §3.6): the versions and values as for Cartesia (the `system` service: `variables`, a list of `{name, meaning, value}`), the code values (what each is, its source, whether it is protected data, its sections: never a value), the tools (what each does from `agent/prompts/TOOLS.md`; its steps, fields and lines from the code), which sections of the active words use each variable (`used`), the limits and the built-in variables. 404 for a version that does not exist |
| `POST /agent/system` | `save_system` | `saveSystem()` | `{values: {variables}}`: saved through `ehr`'s `SaveSettings` as a new, active version (ehr checks each variable, and that the active words use none that is gone), then written into `agent/prompts/VARIABLES.md`. The next test call runs with it. 400 names the variable that does not fit; 409 when `ehr` is not up or has no store |
| `GET /agent/hospital` | `hospital_settings` | `hospitalSettings()` | Config › Hospital ([HOSPITAL-CONFIG.md](HOSPITAL-CONFIG.md)): the versions and values as for Cartesia (the `hospital` service: one setting per list or number, `config.py › HOSPITAL_KEYS`), the keys in the page's order (`keys`) and the limits `ehr` checks (`limits`, `config.py › HOSPITAL_LIMITS`). Facts about the hospital, never a call's value. 404 for a version that does not exist |
| `POST /agent/hospital` | `save_hospital` | `saveHospital()` | `{values}`, every list and number once: saved through `ehr`'s `SaveSettings` as a new, active version (`hospital.rs › check_key` and `Hospital::from_pairs`). Each call takes the active version when it opens (`OpenCall`). 400 names the list and the row that do not fit, never a value; 409 when `ehr` is not up or has no store |
| `POST /agent/hospital/import-places` | `import_places` | `importPlaces()` | `{addresses}`, 1 to 5, each a state (`WI`) or a city and its state (`Verona, WI`): Places › Import from Epic ([HOSPITAL-CONFIG.md](HOSPITAL-CONFIG.md) §8), through `ehr`'s `ImportPlaces`. `{values: {facilities, locations, specialty_list, specialties}, others, other_specialty_lists, reads}`; saves nothing (the page merges the rows into its draft). One `Location` search per address, its pages followed. 400 for an address that is neither form; 409 when `ehr` is not up or does not serve Epic's sandbox; 503 when Epic does not answer. Facts about the hospital only |
| `POST /agent/prompt/estimate` | `estimate_prompt` | `estimatePrompt()` | Record and test's quote: a dry run that replays each kept script with no Claude call (about 30 s, no tokens), in the background; `GET /agent/prompt` shows it as `estimate` (the scripts that need new replies, by name, and the dollars recording them cost last time). Kept until the words change; never runs beside Record and test |
| `POST /agent/prompt/record` | `record_prompt` | `recordPrompt()` | Record and test: records each kept script with the typed harness, then runs the agent's tests, in the background; `GET /agent/prompt` shows its state. Spends Claude tokens for each new line. 409 while one runs or without the Anthropic key |
| `GET /agent/anthropic` | `anthropic_settings` | `anthropicSettings()` | Config › Anthropic (CONSOLE.md §8): the versions and values as for Cartesia (the `llm` service), each model's rules (its default effort, whether thinking can be off) and prices, the efforts that allow thinking off, the token caps' ranges, the cache write prices, the region, and this cycle's spend in dollars from the call records (`usage`). Never the key, only `key` (set or not) |
| `POST /agent/anthropic` | `save_anthropic` | `saveAnthropic()` | `{values}`, every llm setting: saved through `ehr`'s `SaveSettings` as a new, active version; the next test call runs with it. 400 names the setting that does not fit (`ehr` checks the model, effort and thinking together: `settings.rs › coherent`); 409 when `ehr` is not up or has no store |
| `POST /agent/cartesia` | `save_cartesia` | `saveCartesia()` | `{values}`, every voice setting: saved through `ehr`'s `SaveSettings` as a new version, the active one; the reply is the view above. The voice is its id: one the held list lacks is asked of Cartesia's Get Voice (`resolve_voice`). 400 names the setting that does not fit (or the voice Cartesia cannot find or no longer has); 409 when `ehr` is not up or has no store |
| `GET /agent/resources` | `resources` | `resources()` | Cartesia's credits this cycle, counted from the records' `cartesia` totals and the live voice call's meter; Claude's dollars this cycle against the active budget (`anthropic`); with `?call=<id>`, that call's use so far (`call`: seat, dollars, credits; a live call's running totals, else its record's) (`resources.py`; CHAT.md §17). Counts and dates only |
| `POST /agent/voice` | `voice` | `voiceOffer()` | the voice seat: a WebRTC offer `{type: "offer", sdp}` in, the answer `{type, sdp}` out; `?ehr=`, `?label=` and `?tag=` as on `/agent/call`. The call then runs over that connection (§6, CHAT.md §17). 409 with the reason when a call cannot start, 413 for an offer over 64 KB |
| `GET /agent/sandbox` | `sandbox` | `sandbox()` | the sandbox patients and visits, by `?ehr=`: `local` from `ehr/fixtures/sandbox.json` (CHAT.md §8.1); `epic` from Epic's sandbox through `ehr epic-sandbox`, kept 5 minutes (CHAT.md §13). `more`: per patient, the visits past those listed. 400 for another word, 503 with no Epic files, 502 when Epic did not answer |
| `GET /agent/catalog` | `catalog` | `catalog()` | every call row with its current ratings, and the rubric (CHAT.md §7) |
| `GET /agent/calls/{call_id}` | `call_events` | `callEvents()` | one record's events, for Demo; 404 when there is no such call |
| `GET /agent/latency` | `tool_latency` | none yet | latency per tool; the query and the reply are CHAT.md §10.3 |
| `POST /agent/annotations` | `annotate` | `annotate()` | `{ok: true}`, or 400 with the gate's reason (§4), or 503 when the call store is not running (DATABASE.md §9) |
| `WS /agent/watch` | `watch` | `watch()` | the watch stream (§5) |
| `WS /agent/call` | `call` | `TextSeat` | the caller's seat (§6) |
| `POST /2010-04-01/Accounts/{account}/Messages.json` | `twilio_messages` | none: `ehr` calls it | the fake Twilio (`fake_twilio.py`; TEXTS.md) |
| `GET /agent/ice` | `ice` | `iceServers()` | `{iceServers}` for a voice call's browser end, asked for before its offer: `[]` for the owner on this PC; for a guest in the demo, Cloudflare STUN and TURN with one-hour credentials (`turn.py`; ops/DEMO.md §3.7). 409 when a guest asks and `.env` has no TURN key, 502 when Cloudflare does not answer |
| `GET /agent/build` | `build_stream` | `startLiveReload()` (reload.ts) | the demo's live reload (ops/DEMO.md §3.2), as server-sent events: the built page's entry script now and again each time a build changes it, with a keep-alive every 15 s. 404 outside the demo |
| `GET /agent/people` | `people` | `people()` | Teams › People and Security › Auth (ops/DEMO.md §3.5): `people` (each `{id, email, name, first_name, last_name, phone, phone_verified, source, photo_at, owner, added_by, added_at, last_seen_at, role_ids}`, the owner first), `store` (false without Postgres), `linked` (Cloudflare's group is kept in step), `invite` (the demo's address), `you` (`owner`, or the asker's email) and `me` (the asker's user id, null without a roles store) |
| `POST /agent/people` | `add_person` | `addPerson()` | `{email, name}`: on the list through `ehr`'s `AddUser`, then the whole list written to Cloudflare's group. Either may be empty, not both: a card with no email is on the list but not in the group, so it cannot sign in (users.rs, the stand-in key). The new person has no role (deny by default). The view again, with `added` (the new id) and `warning` when Cloudflare failed (the list is saved). 400 names the field; 409 when the email is on the list or there is no store |
| `DELETE /agent/people/{person_id}` | `remove_person` | `removePerson()` | off the list (`RemoveUser`; the row stays, their roles revoked), off the group, the person's Access sessions ended, and the second door's cache for them dropped. The view again, with `warning`. 403 for someone above you and for the last person who can manage the console; 404 for no such person |
| `PATCH /agent/people/{person_id}` | `update_person` | `updatePerson()` | Security › Auth's Save (ops/DEMO.md §3.5): `{first_name, last_name, phone, email}`, the whole card, through `UpdateUser`. The person must be at or below you (`access.rs`; your own card too). Any field may be empty, but a saved email cannot be cleared (400). When the email changed: the group written again, the old email's Access sessions ended and its cache entry dropped (none for a card's first email). The view again, with `warning`. 400 names the field; 404 for no such person; 409 when the email is someone else's |
| `GET /agent/people/{person_id}/photo` | `person_photo` | `personPhoto()` | the person's picture, `image/webp`, `cache-control: no-store` (a face is PII); 404 when there is none |
| `PUT /agent/people/{person_id}/photo` | `set_person_photo` | `setPersonPhoto()` | the body is the 256 px WebP the console made (`content-type: image/webp`, else 415; at most 256 KB, else 400); sealed in the store through `SetUserPhoto`. The view again |
| `DELETE /agent/people/{person_id}/photo` | `clear_person_photo` | `setPersonPhoto()` | with `null`: deletes the picture (`SetUserPhoto` with none). The view again |
| `GET /agent/me` | `me` | `me()` | who you are (docs/architecture/ACCESS-ROLES.md §5): `{user_id, door, role_ids, roles_version, levels, store}`; `levels` names every lever (`none`, `view`, `edit`). `store` false without a roles store: then the launch token has Edit everywhere. Never the email |
| `POST /agent/log` | `log_look` | `logLook()` | `{lever, level, action, object_kind?, object_id?}`: a look that changes nothing (a page opened), recorded in the audit trail through `ehr`'s `LogAccess`, allowed or refused. `{allowed}`. 400 for words `ehr` does not take |
| `GET /agent/roles` | `roles` | `roles()` | Security › Roles: the `roles` versions, one version's roles (`?version=`), the active roles, the levers and levels in `access.rs`'s order. 404 for a version that does not exist; 409 without a store |
| `POST /agent/roles` | `save_roles` | `saveRoles()` | `{roles}`, the whole list, as a new active version through `SaveSettings` (`access.rs` checks it). 400 names the role's place; 409 names the rule it breaks (a role in use removed, a conflict, no one left to manage the console). Everyone's levels are read again |
| `POST /agent/people/{person_id}/roles/{role_id}` | `grant` | `grantRole()` | a grant through `GrantRole`; the People view again. 403 with the rule (your own roles, more than your own access, a conflict); 404 for no such person or role |
| `DELETE /agent/people/{person_id}/roles/{role_id}` | `revoke` | `revokeRole()` | a revoke through `RevokeRole`; the People view again. 403 with the rule (your own roles, someone above you, the last who can manage the console); 404 when they do not hold it |
| `GET /agent/audit` | `audit` | `audit()` | the audit trail through `ReadAudit`, newest first: `rows` of ids and field names only (ACCESS-ROLES.md §5.1). `?before=` a `seq`, `?limit=` 1 to 500, `?changes=true` the changes and refusals only. 409 without a store |
| `GET /agent/audit/verify` | `audit_verify` | `auditVerify()` | `{ok, rows, first_bad}` through `VerifyAudit`: the whole chain walked |
| `POST /agent/csp-report` | `csp_report` | none: the browser, from the policy's `report-uri` | a report of what the page's Content-Security-Policy blocked (CONSOLE.md §5.1). Prints one line to stderr: the directive and what was blocked as a kind (`inline`, `eval`, `trusted-types-sink`) or a scheme and host, the first time a pair is seen and every hundredth after. Never the page's path, a query or a sample. 204; 400 when it is not a report; 413 over 8 KB |
| `GET /demo` | `demo_root` | none: the address a guest types | 308 to `/demo/` |
| `GET /demo/{path:path}` | `demo_page` | none: the browser | the demo build (`--static`, ops/DEMO.md §3.1): a file under its folder, `index.html` for `/demo/`. Hashed assets cached a year, the rest `no-store` (CONSOLE.md §5.2). 404 for a dotfile, a path outside the folder, a missing file, or outside the demo |

## 2a. What each route needs
> **Part:** built · 2026-11-18 · docs/architecture/ACCESS-ROLES.md §7 (the owner's to edit)

The wall checks this before a route runs (`dev_server.py › ROUTE_NEEDS`), and the server does
not start with a route missing from it. A refusal is 403 `{error: "forbidden", lever, level}`.
`agent/tests/test_roles.py` compares this table with `ROUTE_NEEDS`, both ways. The WebSockets
are checked when they open; a watch closes (1008) within 15 s of its viewer losing PHI View.

| Route | Needs |
|---|---|
| `GET /agent/health` | any person |
| `GET /agent/me` | any person |
| `POST /agent/log` | any person |
| `POST /agent/csp-report` | any person |
| `GET /agent/build` | any person |
| `GET /demo` | any person |
| `GET /demo/{path:path}` | any person |
| `POST /2010-04-01/Accounts/{account}/Messages.json` | ehr only (its own check) |
| `GET /agent/cartesia` | Vendors View |
| `GET /agent/cartesia/voices` | Vendors View |
| `POST /agent/cartesia` | Vendors Edit |
| `GET /agent/anthropic` | Vendors View |
| `POST /agent/anthropic` | Vendors Edit |
| `GET /agent/prompt` | Agent View |
| `POST /agent/prompt` | Agent Edit |
| `POST /agent/prompt/estimate` | Agent Edit |
| `POST /agent/prompt/record` | Agent Edit |
| `GET /agent/system` | Agent View |
| `POST /agent/system` | Agent Edit |
| `GET /agent/hospital` | Rules View |
| `POST /agent/hospital` | Rules Edit |
| `POST /agent/hospital/import-places` | Rules Edit |
| `GET /agent/resources` | Analytics View |
| `GET /agent/latency` | Analytics View |
| `GET /agent/sandbox` | Testing Edit |
| `GET /agent/ice` | Testing Edit |
| `POST /agent/voice` | Testing Edit |
| `WS /agent/call` | Testing Edit |
| `GET /agent/catalog` | Testing View |
| `GET /agent/calls/{call_id}` | Testing View |
| `POST /agent/annotations` | Testing Edit |
| `WS /agent/watch` | PHI View |
| `GET /agent/people` | People View |
| `GET /agent/people/{person_id}/photo` | People View |
| `POST /agent/people` | People Edit |
| `PATCH /agent/people/{person_id}` | People Edit |
| `DELETE /agent/people/{person_id}` | People Edit |
| `PUT /agent/people/{person_id}/photo` | People Edit |
| `DELETE /agent/people/{person_id}/photo` | People Edit |
| `GET /agent/roles` | Roles View |
| `POST /agent/roles` | Roles Edit |
| `POST /agent/people/{person_id}/roles/{role_id}` | Roles Edit |
| `DELETE /agent/people/{person_id}/roles/{role_id}` | Roles Edit |
| `GET /agent/audit` | Audit logs View |
| `GET /agent/audit/verify` | Audit logs View |

**The replies when a route refuses:**

| Reply | When |
|---|---|
| HTTP 403 `{"error": "forbidden"}` | no launch token, a wrong one, or another origin (§3) |
| WebSocket close 1008 | the same, on `/agent/watch` or `/agent/call` |
| WebSocket close 1013 | `/agent/call` only: `MAX_LIVE_CALLS` (8) calls are running, there is no Anthropic key, Epic mode has no Epic files, or `ehr` is not up yet. The close reason says which. |
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
  the launch's fake account, and never a request with an `Origin`. The tunnel's requests also
  come from 127.0.0.1, so one with Cloudflare's `cf-connecting-ip` header is refused there too.
- **The second door** (ops/DEMO.md §3.3), only with the demo on in `npm run dev` (`--static`) and Access's
  settings in `.env`. `State.admit` tries the launch token first (`OWNER`); then
  `AccessGate.email` (`access.py`): the `Cf-Access-Jwt-Assertion` token's RS256 signature
  against the team's keys, its issuer, audience and expiry, an email in it, and an `Origin`,
  when present, equal to the demo's address. The WebSocket routes ask `State.admit` too.
  Without `--static` there is no second door, whatever `.env` holds.
- **The base path.** `DemoPrefix` takes `/demo/agent/…` as `/agent/…`, for requests and
  WebSockets, so the demo build calls the same routes under its base (`at()` in `api.ts`).

## 4. Annotations
`POST /agent/annotations` takes one annotation. The gate checks it and stamps it, then the call
store keeps it through `ehr` (`AddAnnotation`, DATABASE.md §9), then the catalog applies it
(CHAT.md §7, §12). Without a store: 503, and nothing is kept. Since 2026-11-15 nothing writes
`agent/calls/annotations.jsonl`; a file left from before is still read.

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
| `hello` | `live`, `calls` | at once, on connect: the newest live call's id, and every live call's `LiveInfo` (`call`, `started`, `seat`, `ehr`, `by`, `label`, `tag`), oldest first. Then each live call's events so far. |
| `started` | `call`, `info` | a test call starts; `info` is its `LiveInfo`. A page knows its own call by the `tag` it sent. |
| `event` | `call`, `event` | one line of a live call's record (`CallEvent`: `t_ms`, `kind`, and the kind's fields; the kinds are CHAT.md §11.2 and `call_record.py`). Every live call's lines come on the one stream. |
| `ended` | `call` | the call ended and joined the catalog |

`watch()` opens it again after a close while the page is open. A watcher that falls behind
loses lines; it never holds up the call (`State.broadcast`).

## 6. The caller's seat (`WS /agent/call`)
- **Several test calls at once,** up to `MAX_LIVE_CALLS` (8), across both seats (ops/DEMO.md
  §3.6). Each is always live: every line goes to Claude (CHAT.md §6).
- **`?ehr=local|epic`** picks the EHR the call reads (CHAT.md §13). Another word closes 1008;
  `epic` with no Epic files closes 1013, with the reason.
- **`?label=`** the words typed beside Start: one line, at most 200 characters, on the record's
  `call_start` and in `LiveInfo`; the call's catalog label when it ends. **`?tag=`** the page's
  own mark (letters, digits, dashes, up to 40), sent back in `started`.
- **Who started it:** `owner` or `guest` on the record; the guest's email only in `LiveInfo`.
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
  offer; the dev server answers with Pipecat's `SmallWebRTCConnection`. No STUN or TURN on
  this PC; a guest's call gets them from `GET /agent/ice` (ops/DEMO.md §3.7). No trickle route.
- **The wire after it:** the microphone's audio to the agent, Sonic's audio back, and on the
  data channel RTVI as JSON (typed lines, as on `/agent/call`) plus a ping each second, which
  tells the server the page is still there. The client is `VoiceSeat` in `transport.ts`.
- **The same rules:** the limit of live calls across both seats; the wall (§3) on the request;
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
3. Restart `npm run dev` and play the same call: the star, the label and the verdict are still
   there (the call store keeps them; DATABASE.md §9). `agent/calls/annotations.jsonl` does not
   change.
4. Stop the dev server (Ctrl+C) and set another verdict: the status says "Not saved: the
   agent's dev server is not running".
