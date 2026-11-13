---
status: sop
desk_test: none
sources: []
updated: 2026-11-12
---
# DeetsHealth — Work log

One dated `##` per sitting, newest first. Never correct an old entry; correct HANDOFF.md.

## 2026-11-13 (Windows) — Console pages with their own nav; Chat's edges and corners
- New pages: Teams (Org Chart, People) and Config (Agent, System) under Analytics; Security
  (Auth, Roles) and Audit Logs (Config, Calls, Security) above Settings. Each view is empty.
  Analytics lost its "Nothing here yet." line.
- Analytics' own nav became `pages/subnav.ts` and `styles/subnav.css`, so all five pages share
  one nav (CONSOLE.md §3, ANALYTICS.md §2).
- Chat: the title sits where every other page's title does; the canvas and the bar sit
  `--pad-panel-x` in from the sides and the bottom, so the canvas lines up with the title and
  the toggle (CHAT.md §3). `--pad-panel` split into `--pad-panel-y` and `--pad-panel-x`.
- A trial the owner asked for: one 16 px radius for panels, controls, menus, the nav pill and
  bubbles. `--radius-control` (6 px, the short rows) stays.
- The mode toggle left the controls hidden till a reload: the running dev server was older than
  the page (its sandbox reply had no `more`), and the throw stopped the swap halfway. The swap
  now always finishes, and the rail reads a missing `more` as none.
- Committed with the Epic work's `console/src/agent/api.ts` and `transport.ts`, because
  `chat.ts` holds both and needs them to compile. The rest of the Epic work stays uncommitted.

## 2026-11-12 (Windows, late night) — Epic: the sign-in (E-2) and Epic mode on the Chat page
- Branch `epic-time`, from `main` after the database work merged. The owner chose fork 1A (start
  Epic now) and 2A (`ehr` runs one adapter at a time; the rail shows that adapter's patients),
  with a button near Test that flips between Epic and Local.
- The patient app's Epic wire (`core/src/auth.rs`) was a standalone patient launch with PKCE. The
  phone agent needs Backend Services, so only the base URL and the `smart-configuration` read
  carried over.
- Built `ehr/src/epic.rs` and `ehr epic-check` (EPIC.md §5.1). First run: the key matches the
  published set, the token came back, `metadata` 200, the `Patient` search for Camila Lopez found
  1 match. About 5 requests, at no cost. Two new tests.
- `npm run dev` held `target/debug/deetshealth-ehr.exe`, so the check ran from
  `target/epic-check`.
- The owner said "build it". Probed the sandbox first (about 40 reads, at no cost): the patient
  search needs family, given and birth date; phone search finds nothing; `$find` is 405; Camila
  Lopez has 261 booked visits ahead, the other six test patients none (EPIC.md §5.2).
- Built Epic mode (CHAT.md §13): `OpenCallRequest.ehr`; `Session.source`; `ehr/src/epic_ehr.rs`
  (the patient search, the next visit, the rail's list as `ehr epic-sandbox`); a sixth pass,
  `EhrPass`, held to that file; `Candidate.phone` is an `Option` (mobile only); the fake slots
  move to the next weekdays in Epic mode; the dev server's `?ehr=` on `/agent/call` and
  `/agent/sandbox`; record schema 6 (`call_start` › `ehr`); the Epic / Local button and the
  rail by source; the `--disabled-opacity` token.
- Checked: one Epic call through `ehr` with no model (Camila found, code passed, next visit
  read; Elijah Davis, no mobile number, got no code). 56 `ehr` tests, 47 core, 102 agent, the
  console type check, clippy, the docs check. Not run: the desk test with the model.
- Another session edited the console in parallel (the chat header's padding, Analytics, the
  sub-nav); those changes are not this work's.

## 2026-11-12 (Windows, night) — The Casey Morgan call review, friction at verification
- Reviewed the owner's test call as Casey Morgan (a twin record: no code goes out). The
  verification rules held and nothing showed whether the details matched. Two prompt gaps:
  `line.code_resent` said a code went out when none did, and `task.verify` sent a caller with no
  text to a person only after a failed code check.
- The owner's drafts A and B: `line.code_resent` now says a new code went out only if the details
  match; `task.verify` hands off when there is still no text after a new code, by itself.
- His idea, an instant "technical limitation" transfer for twins, would tell anyone with a name
  and date of birth that two or more records exist (SECURITY.md §4.4). He chose the same path
  for every caller, warmer: a `verification_failed` hand-off speaks `line.hand_off_code`. The
  instant transfer is a hospital setting, off by default, with its cost stated:
  FUTURE-SETTINGS.md §7 (new). The silent "no single record matched" flag raised by `ehr` is
  designed there, not built.
- The tests hung for 10 minutes: a missing recording failed the call, but the pipeline was left
  running. `TypedCall.hang_up` now cancels a failed call; `test_typed_call.run` hangs up in
  `finally`. A miss now fails in 6 s. `npm run check` builds its own ehr in `target/check`, so a
  running dev server no longer blocks it. New kept script `no_code_after_resend.txt` (the
  owner's call). All 12 scripts recorded again. 100 agent tests pass in about 40 s. When to run
  the tests: HANDOFF.md › Known gotchas.

## 2026-11-12 (Windows, night) — The Analytics page, the recovered sketches
- The owner asked for a blank page beside Settings to sketch on, then whether his design
  screenshots were saved. Only the Chat sketch was. Four more came back from the session
  transcripts (his pasted images only, no terminal or portal shots) into
  `docs/architecture/images/`, linked from CONSOLE.md §6 and CHAT.md §8.
- He narrated the Analytics page over his sketch: a nav stacked in the panel that works like
  the big one; Home first, Flags last. Built: ANALYTICS.md (desk test open), each view empty
  till he designs it.

## 2026-11-12 (Windows, night) — The API surface, the annotation gate, the drift checks
- The owner asked for the API surface, front end and back end, then for the annotate gap and
  the state of the docs; then "go ahead with all recommended fixes, ensure to account for
  possible drift in our docs checks".
- The annotate gap, found: the console's body was untyped; the gate let unknown fields into the
  committed `annotations.jsonl`; a sender could set `at` and `rubric_version`; the Review panel
  dropped the gate's reason; no test ran the routes over HTTP. Fixed: `FIELDS` and `STAMPS` in
  `catalog.py`, the `Annotation` union and `AnnotationRefused` in `api.ts`, the reason in the
  Review panel's status line, `test_dev_server.py` (the wall, the close codes, the gate).
  `/agent/call` now says "ehr is not up yet" instead of "already running" in that case.
- The Windows catalog bug in the test (HANDOFF.md › Known gotchas) is fixed: `Catalog.close`.
  The agent's catalog and dev server tests: 33 pass.
- Docs: new DEV-API.md (the route ledger, the wall, the annotation body, the watch stream, the
  seat, a map to every other API); the RPC ledger in EHR-SERVICE.md §3; README.md rewritten for
  the phone agent (it still described the patient app); CHAT.md §7's two stale lines; the six
  patient-app docs marked `legacy: patient-app` (DOCS-ORG.md §2); CONSOLE.md's Run it and its
  token counts, which check 6 found stale.
- docs:check gained checks 12, 13, 14 (facts) and 6 (a suspicion). Each was proved by breaking
  its ledger once. Check 13 at once caught the "Dev server setup" session's new
  `rpc FileCall`, which has no ledger row yet; that session was told.
- The other session's commit 2c4a624 took in this work's edits as they stood then; the rest
  (the checker, DOCS-ORG.md §4, CONSOLE.md, this entry) is in the tree, not committed.

## 2026-11-12 (Windows, late night) — The Review panel
- The Step panel passed its desk test; committed as 4b8b45d (this work's edits only; the other
  session's edits to flow.py and call.py stay in the tree).
- The owner chose A for the Review panel, the rubric itself: "each dimension can be a row with
  sliders, 0 to 5 can be five dots, a star works. Not all of it is required." Built (CHAT.md
  §12): the rail's 5th panel, sliding in when a test call or a full Demo playback ends; the
  star, a label and a note for the call; a row per rubric dimension with a Pass · Fail · Unsure
  control whose pill slides to the choice, five score dots and a note; each choice saved at
  once to `annotations.jsonl`; the model's rating shown only after the tester's verdict.
- The "Dev server setup" session is moving catalog.db to Postgres. The panel uses only the HTTP
  API; sent that session the API shapes it reads, and the `tool_calls` table and latency query
  to port.
- Checks: front-end type check, docs check. Not run: the desk test (CHAT.md §12).

## 2026-11-12 (Windows, night) — The Step panel
- Committed the Chat work as ec8c838. The other session's 38e08ee, made while this was being
  staged, had already taken the agent-side half (the fake Twilio, latency.py, record schema 4).
- The steps today: greet → verify → menu ⇄ schedule → goodbye or hand off; the global tools
  run in any step without moving the call.
- The owner chose 1A, the map, as "each step as a block and stack blocks to document the order
  the call went in", and 2A, the current step's tools and limits left. Built (CHAT.md §11):
  `StepLog` in `chat.ts`; record schema 5 with `step` events stamped at each move
  (`CallFlow._step`, `STEPS` and `CLOSING_STEPS` in `flow.py`); `left` counts on `ehr` events;
  `OpenCall`'s reply carries the limits in full; `npm run dev` remakes the gRPC code.
- Checks: ehr 41 tests and clippy clean (target/alt); front-end type check; ruff; agent test
  files all pass alone except two not from this work: the old Windows-only catalog test, and
  `test_typed_call.py`, which waits on a replay miss because the other session has edited the
  kept scripts and the prompt and not yet recorded them again. Not run: the desk test.

## 2026-11-12 (Windows, night) — Staff sign-in, on paper
- The owner asked how to set up role-based access with a hospital's security team when the
  hospital runs Active Directory. The answer: sign-in through its IdP (Entra ID), never the
  on-site directory; its groups decide who has a role, our levers decide what a role can do.
- He asked for it in ideas/ until it is built: ideas/STAFF-SIGN-IN.md. It maps HIPAA, the
  Joint Commission, ISO 27001, SOC 2, HITRUST, NIST and state rules to eight modules, and the
  steps with a hospital's security team. Facts not checked are marked *confirm*.
- Forks: F1 A (a recent sign-in with MFA before protected data), configurable by the hospital;
  F2 C (a reason for access is a hospital setting, restricted calls only by default). Both on
  FUTURE-SETTINGS.md's index, with the idle logoff. Six levers added to ACCESS-ROLES.md §3.
- He asked how hard the SSO wiring is, and said to build it with the roles. STAFF-SIGN-IN.md
  §10: the staff API (Phase 4) comes first; OIDC is days; SCIM and SAML are weeks; developer
  tenants are free. He chose OIDC only for now, with a SAML plan kept ready for a hospital
  that needs it (§8).

## 2026-11-12 (Windows, evening) — Per-tool latency, an API
- The owner asked whether a call's record keeps how long each tool took. It does: his 12:58
  test call showed the model is most of every trigger (527–1497 ms), each tool's run took
  1–8 ms on the fakes, and each answer was stamped about 400 ms late by the text harness's turn
  grace (`TURN_GRACE_S`), a wait no voice caller has.
- He chose A, "just an API for now", and a doc for the Analytics tab to build on. Built
  (CHAT.md §10): `latency.py` (one row per tool call, six spans, names and numbers only), the
  catalog's `tool_calls` table, `GET /agent/latency` (a summary per tool and model with p50,
  p90 and max; the rows on request; filters). Record schema 4: every `t_ms` to a tenth of a
  millisecond, and `first_ms` on each agent line, stamped at its first frame, so the answer
  time no longer carries the grace. Older records leave the answer spans empty, not guessed.
- The Analytics tab went into ideas/README.md, with this API as its first source.
- Checks: ruff; agent 69 of 70 tests (the same Windows-only catalog test); over the 87 records,
  390 tool calls, 310 without the replayed ones.

## 2026-11-12 (Windows, late afternoon) — The code-by-text doc; the Tools panel
- Documented the code by text in full: a new architecture doc, TEXTS.md (the path by file and
  symbol, the request, the settings, `TextPass`, the fake Twilio, the tests, the Phase 5
  steps), and CHAT.md §8 rewritten as the full UI record (§8.1–§8.5).
- The Tools panel (CHAT.md §9). The owner chose A: the time to confirm is the tool's own run,
  "to help us track latency as accurately as possible"; and a main line with bullets under it,
  not one row. Built: each tool call as its name and arguments, with "Triggered by", "Time to
  trigger" and "Time to confirm" under it, in every mode.
- For accuracy the record now keeps both times from the agent's high-resolution clock to a
  tenth of a millisecond: `trigger_ms` on `tool_call`, and `ms` on `tool_result`, which was
  whole ms (record schema 3). Older records fall back to whole-ms event times.
- Checks: front-end type check, docs check, ruff pass; agent 59 of 60 (the same Windows-only
  catalog test as before). Not run: the desk test (CHAT.md §9).

## 2026-11-12 (Windows, afternoon) — The sandbox patients in the rail; the code by text
- The owner chose fork A: a fake Twilio now, Twilio itself in Phase 5, and asked to build as
  close to the end product as is reasonable. His layout, from two screenshots: the sandbox
  patients in the rail (Test and Demo: name, date of birth, a collapsible list of visits); a
  split button where the help button was, the code on the left (it opens the text), a send
  shortcut on the right.
- Built (CHAT.md §8): `ehr/fixtures/sandbox.json`, one list for the fakes and the page;
  `ehr/src/texts.rs`, a random code sent with Twilio's Messages request on the wire's AWS-LC
  provider; a fifth pass, `TextPass`, held only by `texts.rs`; the text's words in
  AGENT-PROMPT.md › `text.code`; the fake Twilio in the dev server
  (`agent/deets_agent/fake_twilio.py`), which writes a `text` event into the test call's
  record; the rail panel and the code button in `chat.ts`. Kept tests: `ehr/tests/texts.rs`
  (the real request against a local listener), `agent/tests/test_fake_twilio.py`.
- `npm run dev` now builds `ehr` first, and on Windows ends the whole process tree on Ctrl+C.
  Found while checking: an `ehr` left behind by an earlier dev server held the binary, so no
  rebuild could replace it.
- Checks: ehr 41 tests and clippy clean (in a second target folder, because the running dev
  server holds `target/debug`'s binary); agent 59 of 60 tests pass: one catalog test fails on
  Windows (`test_annotations_survive_a_rebuild…`: it deletes a SQLite file still open; the
  catalog files are unchanged, so it was failing before this work); front-end type check and
  docs check pass. Not run: the desk test (CHAT.md §8).

## 2026-11-12 (Windows, afternoon) — The regulations map; the greeting's disclosures; confusion goes to a person
- Pulled the Mac's work. `npm run dev` failed on Windows: `spawn("npx")` gives ENOENT, because
  `npx` is a `.cmd` file. `scripts/dev.mjs` now runs Vite's own script with Node (`d14b9a5`).
- The owner's test call as Casey Morgan failed at the code three times. That was by design:
  the fakes hold Casey Morgan and Casey Morgen on one date, so no code went out. The review of
  that call led to REGULATIONS.md: HIPAA, the other federal rules, state laws, ISO, and what
  each one asks of the agent.
- The owner, 2026-11-12: test mode keeps full logs, because the data is synthetic; the call
  record splits before the first real call. "If we know there is confusion, transfer to a
  person since we know we can't help."
- Built: the greeting is a fixed line spoken by the code, with the disclosures (automated, not
  a person; transcribed; a person at any time). The failed-code line offers a person. A caller
  confused after a failed code goes to `hand_off` (a prompt rule in `task.verify`). A new kept
  script, `code_confusion.txt`. All 8 scripts recorded again (about $0.004). 56 of 57 agent
  tests pass; the catalog test fails on Windows (HANDOFF.md › Known gotchas). Desk test open.
- The owner asked for a doc on the default call flow, pointing to the code and not copying it,
  because each fixed piece will become a console setting. New: CALL-FLOW.md, and the rule in
  DOCS-ORG.md §1, rule 8. FUTURE-SETTINGS.md now points to the lines in place of quoting them. The
  emergency phrase list now lives only in `screen.py`, with each entry's examples as a comment;
  the test that matched the doc's copy to the code is gone.
- The owner's retest (Casey Morgan, chest pains in the first line): the greeting, the screen and
  the confusion hand-off worked. Two defects: the 911 line played twice, and the agent asked
  again for the name and date of birth given in the first line.
- The owner, 2026-11-12: over-warning is fine, but a caller who said yes should not be asked
  again; a reminder in neutral words, then go on. Built: `safety_reminder` (a global tool) speaks
  `line.emergency_reminder`; anything new or worse still gets `emergency`. The code holds the
  safe side: before the full line has played on the call (`CallFlow.warned`), the reminder plays
  the full line. `task.greet` now uses details already given. New kept script
  `emergency_reminder.txt`.
- Found while recording: Haiku skipped the reminder until the rule gave examples ("it still
  hurts, they will go to urgent care"); then it said the reminder twice and stopped, the
  statement-line problem of AGENT.md §3. Fixed with a note in the tool result
  (`note.after_reminder`, a new `note.*` kind of section: for the model only, never spoken).
  All 9 scripts recorded again (about $0.01). 61 of 62 tests pass (the Windows catalog bug).
- Staff flags, the owner's forks: 1A (the model's summary plus the caller's own words as its
  evidence) and 2A (silent to the caller; a flag starts work in the background: a rules
  engine, a nurse who chooses to join). Built Phase 1 (FLAGS.md): code flags on the 911 line,
  a lock and no code left; the model's `flag_for_staff`; the evidence check in code.
- Found while recording: a caller for their mother was asked for their own details, which
  would book the caller, not the mother. A rule now sends such a caller to a person. The model
  then handed off without a flag, because a hand-off ends the call; `hand_off` now carries
  the flag's fields itself. New kept script `flag_for_someone_else.txt`. All 10 scripts
  recorded again (about $0.01). 69 of 70 tests pass (the Windows catalog bug). A separate
  session wrote the flag settings, FUTURE-SETTINGS.md §5 (`d9c48b6`).
- Committed `38e08ee` (flags, the reminder, all of `agent/`) and `3376dca`: the first call
  scores in `annotations.jsonl` (the owner's four test calls, a model grader).
- The owner's run as Priya Raman (12:58) double-booked: after booking option 2, "option 3
  instead" searched again, renumbered, and booked a different time; the first visit stayed.
  The owner chose A, confirm before booking. Built: `choose_option` reads the time back and
  `book` books only that; option numbers keep their time for the whole call; `change_visit`
  names the visit that stays booked, raises a `booking_change` flag and hands off. New kept
  script `change_after_booking.txt`. 73 tests pass.
- Found: in one recording the model spent all of `MAX_TOKENS` thinking and said nothing; the
  call sat silent until the turn limit (AGENT.md §4). Not fixed: the cap is the owner's call.
- The owner: raise the cap; if a reply ever goes over, code flags the call for an analyst or
  engineer; "One moment, please." as a configurable line. Built: `MAX_TOKENS` 1,200,
  `RETRY_MAX_TOKENS` 2,400; `_CutOffGuard` (llm.py) holds back thinking until text or a tool
  starts, drops a cut-off reply, speaks `line.one_moment`, raises a `token_cap` review flag
  (FLAGS.md §8), retries once. All 12 scripts recorded again (about $0.01). 76 tests pass.
- The owner on the catalog: file each transcript into the database by a script, and shape it
  like the production store, ready for the cloud. Design on paper first (forks brought).
- The owner chose native Postgres (Docker Desktop and WSL2 are fiddly on the PC), asked what it
  takes to be demo ready, and asked `npm run dev` to start Postgres. Built: `scripts/postgres.mjs`
  (one cluster per repo in `.pg/`, run by `pg_ctl` as the user, 127.0.0.1, scram, a free port
  from 5433, kept up after Ctrl+C; `npm run db:start | db:stop | db:status`). Postgres 17 pinned
  and installed on the PC with winget. New: DATABASE.md (decisions, the Chat page's contract
  from the Review-panel session, the draft schema, the demo-ready list).
- winget's download of the installer failed twice from EnterpriseDB's CDN (403 partway, then
  very slow); no licence is involved. The owner installed 18.7 from EDB's site by hand; the
  pin moved to 18 (AWS RDS has served 18 since November 2025, support to February 2031).
- The owner confirmed `ehr` as the only writer, if it scales (DATABASE.md §2.1 says why it
  does), and asked to build the wiring meanwhile. Built: `ehr/migrations/0001_calls.sql` (the
  draft schema), `store.rs` (migrate, file a record idempotently, file annotations by line
  hash), `seal_key.rs` (core's `Cipher` on AWS-LC; the dev key in `.pg/seal.key`), the
  `ehr ingest` command, `npm run db:ingest | db:test`, and the ingest at each `npm run dev`.
  40 ehr unit tests pass. After the install: `npm run db:start` hung on Windows (the server
  `pg_ctl` starts held its pipes; fixed by giving `pg_ctl start` none). `npm run db:test`
  passed against Postgres 18.7 (it builds into `target/store-test`, because a running dev server
  holds `target/debug`'s ehr). First ingest: 128 calls and 46 annotation lines in 3.4 s; the
  second filed nothing new.
- The live path (the owner: "calls file as they end"). Built: `FileCall` in the contract (a
  client stream, one record line per message, `(phi)`); `file_call` in ehr (checks the stream
  first, then files through the store) and in the agent's client; the dev server and the typed
  harness file each call when its record closes; ehr opens the store at start from the
  settings `npm run dev` passes, or `.pg/ehr.env` for a typed call by hand; the tests' ehr has
  no store. Found: Python writes records with `\r\n` on Windows, so the live path and the ingest
  hashed one call two ways and filed it twice; the hash is now over the canonical form, and
  the store test covers it. Checked live: a typed call filed at its end, and the next ingest
  skipped it. 41 ehr unit tests, the store test, and 99 agent tests pass. Mac notes in
  DATABASE.md §4b and HANDOFF › Run it.

## 2026-11-12 — The Mac set up; the Chat page wired to the agent; the catalog of calls
- On the Mac: `npm install`, `uv` (Homebrew), the `wasm32` target, `uv sync`, `ehr` built, the
  dev certificates. The owner passed the Chat UI's desk test in Safari for now.
- He asked about the security in `ehr/`: the crypto and TLS are rustls on AWS-LC; the rules
  (sessions, verification, the edge) are ours. He asked for the text code's check to be
  constant time: built (`Shielded::ct_eq`, the `subtle` crate; EHR-SERVICE.md §6a).
- Forks: the first slice is Test and the watch stream together (1A); `npm run dev` starts both
  (2A); the catalog in SQLite (1B); ratings per dimension with an optional 0–5 score (A); a
  new dimension, intent drift. Staging after this desk test (2A), at $0 for anything but
  Anthropic credit (CLOUD.md §5a).
- Found on the way: Pipecat's browser WebSocket transport loads Daily's call-machine bundle
  from Daily's CDN, so the console uses client-js with our own text transport; Pipecat's
  serializers drop RTVI messages unless told not to; RTVI's text events do not carry the
  Flow's fixed lines, so the watch stream does.
- Built: the dev server, the launcher, the seat, the watch stream, Demo from the catalog, the
  catalog and its annotations file (CHAT.md §6, §7). A replayed booking ran end to end through
  the seat and the watch stream. 56 agent tests. Not run in a browser: the owner's desk test.
- Desk tests in Safari, the same night: the narrow canvas (the help label wrapped, the split
  ran out of the canvas; now "Help" and a fluid split); a replay miss ended a call (nothing
  lost: the record held the full error); Test made always live, replay kept for Demo (the
  owner's ask); one grid for the stage's edges, the line facts under the bubble's text.
- DeetsEngine cloned: nothing taken (a skeleton, Rust against a Python catalog, no BAA path);
  GLOBALS.md archived (the owner).
- The owner made `.env`; the `.env` deny rules in `.claude/settings.json` stay. API spend:
  about $0.001 (a few live lines).

## 2026-11-12 — Idea: a CLI and an MCP server for the console
- The owner asked for an MCP server or a CLI that lets Claude operate the console to debug,
  and later lets a hospital's analysts query logs, edit settings and run demos. Talked first,
  then wrote ideas/CONSOLE-MCP.md.
- Forks: protected data through MCP is off by default, and a hospital can turn it on for a
  client its own BAA covers (1 A); analysts get the external MCP server, and the in-console
  assistant waits until it proves useful (2 A).
- Decided inside his choices: one staff API for the console, the CLI and the MCP server; the
  CLI first; a remote MCP server; setting changes as drafts that a person applies in the
  console; the tool list is a draft for him to edit.
- No code. No API spend.
- Then the owner asked for one central list of future settings. FUTURE-SETTINGS.md held only
  the 3 with a built default; 13 more were named across the docs. Added an index by topic at
  its top, on DeetsMusic's pattern (its FUTURE-SETTINGS.md ledger): every setting on the
  index the day a doc names it, a full entry once its default is built, stable section
  numbers, a SETTINGS.md as the live table once the store is built.
- Cloned `deets-137/DeetsEngine` beside this repo on the Mac (over HTTPS: `gh` is set to SSH
  here). Its `docs/ideas/deetshealth-needs.md` (commit `adea158`, 2026-10-13) is not on GitHub;
  at the owner's word, dropped it from GLOBALS.md's sources. The docs check passes clean.
- He asked for another idea: access levels by role. I first wrote a full design (levels, a
  role table, rules, forks); he asked for it open ended instead, for his own design and build.
  ideas/ACCESS-ROLES.md now holds only his direction (custom-named roles built from levers,
  found as we build) and a list of the levers seen so far. SECURITY.md §4.10's four roles
  are now marked as examples.

## 2026-11-12 — The Chat page's UI, the wave, `main` brought up
- The owner asked to record the analyst-team demo as an idea (a Cloudflare Tunnel;
  ideas/README.md), to merge the Pipecat 1.16 branch into `conversion` and `conversion` into
  `main` (a fast-forward), and to branch `chat-what-we-doin` off `main`.
- He drew the Chat page and narrated it (the sketch: `architecture/images/`). Forks: Start |
  End call as one split button in the canvas, mirroring the help box, green and red by theme,
  End asking "Confirm?" first, and large enough that nothing moves; the help box opens and
  closes in the canvas, which keeps a row for the controls; Demo's player in the same button
  style. He confirmed my reading of "tied to Pipecat's PHI setting" (CHAT.md §2).
- Built the UI with no connection (CHAT.md §3): the toggle, the canvas, the split buttons, the
  help box, the text bar, Demo's player. Then his ask: the controls come in as one gentle
  wave on each mode change and each time the page comes into view.
- Moved the call view with a timeline out of ideas/ and into the plan (BUILD-ORDER.md §3.1).
  Wrote HANDOFF's setup for a second machine: he will wire the agent in from his Mac.
- Checks: the type check, the bundle and the docs check pass. Not opened in a browser by me.
  No API spend.

## 2026-11-11 — Pipecat 1.16
- Raised `agent/` to `pipecat-ai[anthropic]>=1.16,<2`, locked at 1.16.0 (before the browser
  caller over RTVI). The lock also moved `sentencex` 1.0.32 → 1.0.34, added
  `pipecat-resampler` 0.2.0, and removed `resampy`, `numba`, `llvmlite` and `soxr`.
- Read the 1.14, 1.15 and 1.16 changelogs and grepped our code for each deprecated name: no
  hit. Diffed Pipecat's Anthropic service, 1.13 against 1.16: only the default for an unset
  effort changed (Haiku 5.5: thinking off → `effort: low`). We already send `effort: low`.
- Ruff and the 43 tests pass in replay mode. Every replay matched, so no request changed and
  nothing was recorded again. API spend: $0.
- The words-before-a-fixed-line gap (AGENT.md §4) is unchanged: the fix only sets the effort we
  already set, and the replayed replies are the same.
- The docs check fails in a worktree under `.claude/worktrees/` only: `../DeetsMusic` and
  `../DeetsEngine` do not resolve there. With two temporary links to the sibling repos (removed
  after), `npm run check` passed all 6 steps.

## 2026-11-11 (late night) — The console's skeleton
- The owner asked for the console's skeleton in DeetsMotion's style: the look, the nav, the
  panels. Chat at the top of the nav, with a large panel and a rail of three, as in
  DeetsMotion's Visualizer; Settings at the foot, blank. He asked about OS compatibility first.
- Two forks. **1A, a browser only** (not a Tauri desktop app as well). He asked for a way to
  get a desktop feel on the hospital's own managed Edge or Chrome, as Epic does at its sites.
  The answer built: an installable web app with the window-controls overlay, which hospital IT
  can push with the `WebAppInstallForceList` policy. A native shell stays a later choice.
  **2A:** DeetsMotion's three themes and its glass look; the old themes, skins and card engine
  go to the archive.
- Built `console/` (CONSOLE.md): the top bar and its Theme menu, the nav with the sliding pill,
  the Chat stage and its three empty panels (draft names: Caller, Step, Tools), a blank
  Settings page, the three tiers of tokens, the manifest. The old front end moved to
  `archive/src/`, the primitives with it, because they import patient-only files and use the
  old tokens. `SURFACES-AND-CARDS.md` moved to `docs/archive/`.
- Checks: the type check, the bundle and the docs check pass. Not opened in a browser by me:
  the desk test is the owner's (CONSOLE.md §6). No API spend.
- **Desk test passed** (steps 1–4, in Edge). His notes, built: no "CONSOLE" eyebrow, the
  Settings panel fills the page; a Timeline panel at the top of Chat's rail, and the rail is now
  a third of the width. The icon: two rounds of trials on DeetsMusic's art. He moved the head to
  his standing sprite's (no headphones) and picked a dark fill with a smooth ring;
  `scripts/icons.py` builds it.
- He asked whether we are ready to run the typed call in the console's Chat panel. He will
  lead its design.

## 2026-11-11 (night) — Phase 0 done, the text run closed, the plan reframed
- The owner asked where Phase 0 stands and whether the patient app's Epic registration carries
  over. It was a Patients-audience app. I said the audience could not change; it can. The
  owner changed it to Backend Systems and retired the patient app's registration. The client
  IDs did not change. A future patient-facing part needs its own Patients app.
- The form dropped `Observation (Flowsheets)` for the new audience; Epic's catalog says Backend
  Systems may use it. Lab results do not need it.
- Read Epic's live catalog through the feed behind fhir.epic.com › Specifications: each API's
  allowed audiences and notes. Found: no API cancels or moves a visit (S3 and S4 hand off,
  the owner's choice); `MedicationDispense (Fill Status)` answers R3; `MedicationAdministration`
  is Dutch-only; `Patient.$match` is Epic's pick for a backend app; no outgoing API fits.
- The owner selected the core list and four held sets (a chart note, the full medicine list,
  pending lab orders, surgeries and vitals): "we can find uses for them later". All are in
  EPIC.md §2.1. He wants medication check-ins both ways: inbound now, outbound later
  (ideas/README.md).
- The owner chose Cloudflare for the non-production JWK Set URL. Built `ehr/jwks-worker/` with
  the `cf` CLI, made a P-384 key (ES384; the private key in git-ignored `dev-certs/`), deployed
  `https://health-keys.deets.solutions/nonprod/jwks.json` and checked it live (EPIC.md §2.3).
  Twilio and Cartesia accounts move to Phase 3. No API spend.
- On the owner's word: SMART v2 (narrower grants: the held note write is create only) and
  64-character FHIR IDs (the standard; we save no IDs). `epic-client.json` now says v2.
  Committed `9b10f24` (the Epic work) and `91eb92e` (the 149 replay recordings).
- **The text run is closed** (the owner). Its one open item, the call timeline, measures speed,
  which matters on a voice call, not a typed one. What it left is polish: the gaps in AGENT.md
  §4, fixed as each desk test finds them. The owner wants the timeline in the console instead: a
  chat view to see and run calls, with rows for what the agent says, what the caller hears and
  what each tool does, with their times. Recorded as an idea (ideas/README.md).
- **The plan, reframed** (the owner): no more numbered phases. Three tracks remain to the
  finish line, not three builds at once: the console, the chart and the store, the phone and the
  voice. The console starts, and each next step follows as we go, so its uses, features and
  settings grow one at a time. Then going live. BUILD-ORDER.md is rewritten; its §5 maps the
  old phase names, which older docs and this log still use. Next: the single check command,
  then the console's design on paper.
- **The single check command, built** (the owner asked for it with parameters, fast for what
  changed). `scripts/check.mjs`: six scopes (docs, web, core, ehr, agent, keys); by default it
  runs the scopes that the changed files need, against the upstream branch plus the working
  tree; named scopes, `--all`, `--full`, `--list`, `--base`, `--keep-going`. A change to core
  also runs `ehr`, and a change to `ehr` also runs `agent`, because the agent's tests run
  against a real `ehr`. The pre-push hook runs it on the commits being pushed. `--all --full`:
  12 steps, 50 s, all pass (clippy on `ehr` with warnings as errors included). Fixed on the
  way: one import order in the agent's tests (ruff).

## 2026-11-11 (evening) — the third desk test: an emergency in the middle of a booking
- The repo is confirmed private. On the owner's word, the dev call records in `agent/calls/` are
  now committed (all synthetic; every caller line checked first). `.obsidian/` is ignored.
- The third typed call: Casey Morgan (two fake patients share the name) got no code, as
  designed; the caller switched to Priya Raman and verified. "What's my disease?" got no
  answer. "Option 1. Am I dying?" got the model's own 911 line, then `hand_off`'s "connecting
  you to staff"; the booking was lost and the call ended. The fork C check flagged it.
- The owner chose A: an `emergency` tool with a fixed 911 line; the call stays open and the
  booking goes on. Built with a kept script and test (`emergency_then_book`). What follows the
  line becomes a hospital setting: new FUTURE-SETTINGS.md. The owner added the emergency flag:
  several actions at once (a nurse joined to the call for a warm hand-off, a transfer when the
  task ends, a message, a chart note), recorded there.
- The call record now has the time to the first word and to the first tool call.
- The prompt rules, with the owner: he chose 1A (a `medical_question` tool with a fixed line)
  and 2A (code screens each caller line for emergency phrases and speaks the 911 line itself),
  "tools give us a route to add more config and more levers later". Built with three more:
  the name guard in code, an off-topic prompt rule, and the invented-value check in the record
  and the tests. FUTURE-SETTINGS.md §2 and §3 hold the hospital settings for both.
- He asked about tool limits. Anthropic: accuracy degrades past 30 to 50 tools; standard
  calling fits under 10. Each node offers 5 to 7. Server-side tool search is not
  HIPAA-eligible, so it is out on the PHI path (AGENT.md §3).
- Spend: about a cent in all. 32 agent tests pass.
- Later desk test: the model still answered sums. The off-topic rule now says to give no
  answer, not even arithmetic, kindly, and steer back to visits (kept script `off_topic`).
  "ellis jordan 21 3 1984" did not verify: the fake patient is Jordan Ellis, born March 12,
  1984, so both the name order and the date were wrong, and the agent said the same words as for
  a match, as designed. Re-recording found the medical line spoken twice: it ended in a
  statement, so the model spoke next and repeated it; it now ends in a question.
  The full emergency phrase list is in FUTURE-SETTINGS.md §3, checked against the code by a
  test; DESIGN.md C5 and VOICE-AGENT.md now match option A. 34 agent tests pass.
- The owner chose A: read back the name and date of birth before the lookup. Built: `identify`
  reads back, `confirm_details` looks up. His slip now gets corrected and verifies (kept script
  `read_back_correction`).
- He asked to read and hand-edit the prompt. Every word moved to
  `agent/prompts/AGENT-PROMPT.md`, loaded and checked at start-up (`prompts.py`). Its first edit:
  "I'm just worried" had been taken as a medical question and the booking skipped; worry alone
  is not a medical question now. 43 agent tests pass; about two cents spent this round.

## 2026-11-11 (later) — the second desk test: the code step, ending a call
- The owner's second typed call ran with no crash. He stopped at the code step: no text is
  sent, and the harness did not say the fake code. The code step was wired; `ehr` accepts
  `246810`. The harness now prints the fake callers and the code when a call starts.
- From the call logs, the agent missed three chances to end a call. "End call" at the code
  step was handed off to staff. "Goodbye" at the greeting got a goodbye in the model's words,
  and the call stayed open. "Or I'm done" got an unasked goodbye in the model's words. Fixed:
  `end_call` is global, and the prompt says to use it for every goodbye (AGENT.md §3).
  Two kept scripts and tests cover it: `leave_at_greet`, `leave_at_code`.
- The model's own words before a tool's fixed line, seen as "...Goodbye. Of course. I'm
  connecting you with a member of our staff now." The owner chose C: a prompt rule, plus a
  check in code that notes it in the call record and fails the tests (AGENT.md §3). Holding
  every reply (A) was dropped: 0.4 to 0.7 seconds on each reply. The check catches the reply
  from his call when run over its recording.
- The owner offered short audio cues for call start, hand-off, goodbye and a hold sound for
  long waits. Not built; it waits for Phase 3 (voice).
- Spend: under a cent to record the three scripts twice. 24 agent tests pass.

## 2026-11-11 — the first desk test; the thinking-block crash; the dev call record
- The owner ran two typed calls: one genuine, one to break the agent. Both crashed right after
  `identify`. In the break call the agent stayed in scope, would not name its technology, and
  asked again about a birth year in the future.
- The cause, reproduced live: Haiku thought before calling `identify`; the next request, under
  the verify node's tools, sent that thinking block back, and the API refused it (400, the
  block is bound to another tool list). Pipecat's LLM service stopped and the harness timed out
  after 60 seconds. A scripted run passed only because Haiku did not think there. Fixed in our
  client (stale blocks dropped; the beta header fix is ruled out by the GA rule), with a test.
  A failed request now ends the call at once.
- The owner asked for a log of both sides and any telemetry to trace errors. Built the dev
  call record (AGENT.md §1): one JSON Lines file per typed call in `agent/calls/`, git-ignored,
  synthetic only, and `harness/show_call.py` to read it. The full text of library errors goes
  there, not to the console.
- Found while testing (AGENT.md §4): the agent rewrites `say` after `identify`; an impossible
  date of birth uses a try; the model fills a missing last name and once spoke of a
  "placeholder".
- The owner chose fork 1A: the code speaks each fixed line word for word, not the model.
  Built with `TTSSpeakFrame` (the voice path's own frame); every fixed line now ends in a
  question so the model does not speak after it (it repeated one in a test run). Also built:
  an impossible date of birth refused by the Flow and by `ehr`'s edge before a try counts; the
  name rule in the prompt; "today" set per call, pinned to the fakes' day in typed calls.
  The booking script was recorded again (the prompt changed).
- The owner asked whether to rebuild Pipecat in Rust. Answer: no, not now (the reasons are in
  this sitting's chat; the measure that would reopen it is Phase 3's latency per step).
- Spend: about two cents on Haiku in all. 18 agent tests and 32 `ehr` tests pass. The new
  recordings in `tests/replays/` are not committed; the older ones no longer match the prompt.

## 2026-10-21 — Phase 1 begins: the ehr service and its contract
- Late on 2026-10-20 the owner decided Phase 1's model: Claude through a replay cache, not a
  local model first. The API budget is the Max plan's $100 a month of credit, a hard ceiling;
  every test starts on the cheapest model (Haiku) and moves up only when a test shows it must.
  The key is in the git-ignored `.env`; `.claude/settings.json` denies Claude any read of it.
- The owner approved the repo layout (P0.2): each folder is born with its phase. He chose a
  Rust `ehr` service from the first call over Python fakes, so the patient binding is never
  retrofitted, and gRPC over HTTP with JSON as its wire (deadlines and cancels reach Rust; the
  PHI tag lives in the contract). New: [architecture/EHR-SERVICE.md](architecture/EHR-SERVICE.md).
- Installed `uv` 0.13.4 with winget. Built `ehr/`: the contract (four calls), the `(phi)` and
  `(secret)` marks with redacted debug prints made by `build.rs`, the edge, the sessions, the
  fake scheduler. Added `WirePass`, the fourth pass, to core. 15 tests pass; core's 42 still do.
- Two forks found and brought to the owner: the TLS library (the FIPS standard against what
  `tonic` and `grpcio` ship) and how the caller is identified before the two factors. He
  chose FIPS crypto at both ends (the standard still holds) and name and date of birth, then
  the code (not the calling number first). Recorded in EHR-SERVICE.md §6 and SECURITY.md §4.4.
- Built the rest of `ehr/` for Phase 1: `Identify`, `ResendCode`, `CheckCode` with the limits
  of SECURITY.md §4.4; `Shielded::same_name` in core (one slip in a name of 5 letters or more);
  four fake patients and the fixed code `246810`; booking in its own task so a dropped request
  cannot cut it off; `rustls` on AWS-LC, TLS 1.3, mTLS, a `fips` feature and a refusal to start
  in a deploy without it; the dev certificate example; a transport test against the real
  server. 26 `ehr` tests and 43 core tests pass; clippy is clean.
- Committed (`56fb877`). Built `agent/` (AGENT.md): Pipecat 1.13 with its built-in Flows (the
  separate package stopped at Pipecat 1.4); a client that drops Pipecat's beta flag and calls
  the GA endpoint, with the replay cache; the Flow (greet, verify, menu, schedule, goodbye,
  hand off); the gRPC client with mTLS and deadlines; the Python `Shielded` and the log
  processor that withholds all library log text; the typed harness, which starts `ehr` on a
  free port. Added `NextVisit` to the contract for S5 (29 `ehr` tests).
- The first live typed call on Haiku 5.5: 15 requests, well under one cent; it booked a visit
  and read back the next one. Replayed, it costs nothing. 15 agent tests pass, replay only.
  Found: the model does not always read the `say` template as written (AGENT.md §4).

## 2026-10-20 — the build order
- The owner asked for the build process and where to start. Wrote [BUILD-ORDER.md](BUILD-ORDER.md):
  six phases, each with a desk test; Phase 0 (paper) is in flight; one fork open, what goes
  first while the Epic registration waits (§3 there). Pointed to from CLAUDE.md and HANDOFF.md.
- The owner decided the fork: the typed, Epic-free call (Phase 1, the text slice) is built
  first; the Epic Backend Services sign-in (Phase 2) follows once the registration lands.
  Phase 1 starts 2026-10-21. HANDOFF.md › Open now carries the six phases.
- His working rule, recorded: always say what an option is when it comes up again; never a
  bare letter.

## 2026-10-19 — the tag pushed, from the owner's machine
- The owner's local `main` held one unpushed commit, the record view (`05e7584`). By his
  choice it went to `archive/patient-app` (`ee35b9d`, cargo check passes), not to the
  conversion. Local `main` reset to `origin/main`.
- Tagged `patient-app-final` on `ee35b9d` and pushed the branch and the tag. Corrected
  HANDOFF.md and CONVERSION.md §5.1.

## 2026-10-19 — the pivot to a hospital phone agent; conversion steps 1–3
- The owner asked for a voice-agent idea doc, then chose the phone agent run by hospitals:
  [ideas/VOICE-AGENT.md](architecture/VOICE-AGENT.md). Researched Epic's Emmie and voice roadmap
  (incoming voice AI in 2027), the vendors' BAAs, FHIR booking per EHR, Twilio
  ConversationRelay versus Media Streams, and Pipecat.
- Decided: phone agent; convert this repo; wedge W1 and W3 but every EHR from the start;
  A2 deploy behind an interface; inbound first; results on the phone with guards; Cartesia;
  Pipecat with Flows; Postgres with our sealing; no call audio by default; sandboxes with
  record and replay, no local FHIR server for now.
- Conversion steps 1–3 ([CONVERSION.md](CONVERSION.md) §5.1). The tag push was refused by
  the session's git proxy (403); the branch `archive/patient-app` stands in for it.
- Step 4, first pass, with DeetsMusic as the model (`deets-137/DeetsMusicWindows`; the public
  `deets-137/DeetsMusic` is an older Swift app): DESIGN.md, HANDOFF.md and CLAUDE.md rewritten
  in its shape; its archive rule ported.
- SECURITY.md rewritten for the phone agent (the old one archived): S1 A, the hospital's own
  single sign-on; S2 A, two factors for any chart answer, configurable above a floor, with a
  random chart fact as an option for the first (its question may never reveal the chart).
- EPIC.md (the backend app registration), DEBUGGING.md and SECURITY-BRIEF.md rewritten for the
  phone agent, the old ones archived as `PATIENT-APP-*`; a header on CORE-CRATE.md; a draft
  `ops/PROTOCOLS.md` for helpers and their agents (the owner's ask).
- One doc per vendor in `integrations/` (the owner's ask): Twilio, Cartesia, Anthropic,
  Pipecat, the cloud (fork C1 open) and the EHRs after Epic, from the day's research plus a
  search of users' lessons (Pipecat's GitHub issues on Twilio barge-in and 8 kHz pacing; Twilio's
  latency guide; Cartesia's pronunciation dictionary).

## 2026-10-13 — the globals draft, from Camila's record
- The owner asked whether Camila's exported record shows the shape of the globals to tune
  DeetsEngine towards. Answer: it shows the axes, not the proportions (one synthetic
  patient; 248 of 297 rows are vitals on one visit, and that visit was not sent).
- He asked for a draft doc, and added: prepare for panels of 8 to 15 values (lipid, CMP) and
  microbiology with more. New `architecture/GLOBALS.md`: one value, one record in `^RES`,
  with `parent`, `root` and `seq`; stubs for targets that did not arrive; a concept key and a
  date key; forks G1 (how to split the globals) and G2 (components as records or a subfile).
  He chose G1 A (one global per kind) and G2 A (one value, one record, components included).
- Questions for him in GLOBALS.md §8 (how Epic sends a culture; LOINC on lab components).
  Next data step: an Epic sandbox patient with a CMP, a lipid panel and a culture.
- No code changed.
- The repo has a remote now. The owner had the old public `deets-137/DeetsHealth` (an iOS
  Oura app) renamed to `DeetsHealthiOS`, and a new **private** `deets-137/DeetsHealth`
  created as `origin`; `main` pushed. The history was checked first: no secret or record
  file was ever committed. CLAUDE.md › Repo, HANDOFF.md and LESSONS.md §2 updated.

## 2026-10-13 — fresh eyes: a true patient portal
- The owner has a fhir.epic.com account, a new M engine (`../DeetsEngine`), and a larger
  DeetsMusic to model the look on. He asked to pick up with fresh eyes.
- He chose: read-only with a model designed for writes; Home and hub cards with levels; the
  engine set D-A + R-A + L-A (the sealed SQLite store on disk, an in-memory globals tree
  rebuilt at open, a protected mode in the engine, the tree kept while the app is open); Epic
  sandbox sign-in next. All in HANDOFF.md › Decided (2026-10-13).
- Found in the code: DeetsEngine's `NotUnique` error carries its values, and its index
  subscripts are plain in its SQLite keys, so the engine cannot hold protected data on disk.
  This is why the record stays in the sealed store and the engine runs in memory. The
  collection-card engine and Compass never came across to DeetsHealth. The core's `Cipher`
  trait is sync, but WebCrypto is async.
- New docs: `integrations/EPIC.md` (the sheet he fills in, the plan, forks E0–E3),
  `features/HOME-AND-HUBS.md` (the layout; after review, real forks H3 and H5), `ideas/IDENTITY-WORKER.md` (tiered
  access over a Cloudflare worker; after review, real forks W2 and W3). New PATIENT-APP-DESIGN.md §6.4 (web readiness).
- No code changed. Stage 7's desk test is still open, and its work is still not committed.
- The owner asked how many of the 12 layout and worker forks were real. 4 were. He asked for
  the evaluation style in writing: new LESSONS.md §5 (the fork test) and two tie-breaker rows,
  and a pointer in CLAUDE.md › Working style. PATIENT-APP-EPIC.md §2 now follows the real "Create an App"
  form, with a new fork E0 (Automatic Client Distribution).
- The fork test went into the global `~/.claude/CLAUDE.md` (Bringing forks), with a rule that a
  Claude-written or old rule may be wrong, so ask when it matters. A one-line pointer is in
  LESSONS.md §5.2.
- Incoming APIs: every R4 read and search (PATIENT-APP-EPIC.md §2, row 2). Dynamic clients: the JWT bearer
  grant (new PATIENT-APP-EPIC.md §6). SECURITY.md §5 corrected: its "refresh tokens" words were Claude's,
  and wrong for this path.
- The owner filled in the Epic form and has a non-production client ID. Built E-2:
  `core/build.rs` (reads `epic-client.json` at build time; serde_json as a build dependency,
  already in the tree), `epic-client.example.json`, `SiteEntry.scopes`, `SCOPES_V2`,
  `EPIC_SANDBOX`, the Epic sandbox at index 1 of the Connect menu, and the test
  `epic_entry_follows_the_build`. E1–E3 failed the fork test, so each follows (PATIENT-APP-EPIC.md §5).
- Checks: core 41 tests + 3 proofs (the new test also with a dummy file, no file and a bad
  file), wasm check, host `cargo check`. No live run: the owner's own terminal is needed for
  the scheme.
- The owner chose USCDI v3 (E0), and the work was committed (`e2bb0a5`). First Epic sign-in as
  Camila Lopez: the sign-in and consent worked, then the page stalled. The console showed the
  cause: Epic's CSP `form-action *` blocks the redirect to a custom scheme (PATIENT-APP-EPIC.md §3).
- The owner chose a loopback return over the scheme (Decided 2 replaced). Built `loopback.rs`
  (127.0.0.1, a port per sign-in, the `Host` check, `303 /done`, four fixed pages); `on_link`
  returns an outcome; the scheme, its registry writes and `tauri-plugin-deep-link` are gone;
  `link-register-failed` became `sign-in-port`. Checks: host 16 tests (2 new), `cargo check`,
  `tsc`. The owner added `http://127.0.0.1/callback` to the registration; at the first check
  Epic still answered `error=4` for it (not synced yet). A background probe waits for it.
- Committed `1ccf20e`. Added `scripts/epic-redirect-check.mjs` (kept). Epic accepted
  `http://127.0.0.1/callback` at 17:41 (about 25 minutes after the save), but rejected every
  other port: it matches the port exactly. `loopback::PORTS` = 80, 42813, 46219, the first
  free one wins; the script checks each. Host tests 18 (3 new). The dev app rebuilt at 17:43.
- Committed `400b5b0`. **E-3 passed:** Camila signed in through the loopback return; the fetch
  stored 296 rows of 12 types, 32 calls, 8 s, 76 warnings, 592 waiting links, CarePlan refused.
- The owner asked for DeetsMusic-grade telemetry and confirmed SECURITY.md §4.8 (sandbox data
  may be shown). Built: the call ledger (`sync::CallNote`, `fetch.rs` `call_line`),
  `fhir::issue_word` (FHIR's closed lists; digits-only detail codes), the fetch summary
  (`Summary.waiting_by_type`, `warnings_by_code`), the inspector (`core/src/dev.rs` with
  `DevPass`, feature `dev`; `src-tauri/src/dev_tools.rs`, `examples/inspect.rs`,
  `npm run inspect`). New `ops/DEBUGGING.md`. Checks: core 43 tests (2 new) + 3 proofs with
  `dev`; the inspector run on Camila's store through WMI.
- What it showed (ops/DEBUGGING.md §7): CarePlan needs one of ten categories; 17 types withhold
  Outside Record data from this client; SmartData Elements not authorized; the 592 waiting
  links name 54 distinct records.
- Committed `f42d691`. The owner: the Outside Record APIs are selected, so their suppression
  has another cause (open). Built the CarePlan category fallback (`fhir::CAREPLAN_CATEGORIES`,
  `category_fallback`; FETCH.md §5.2), one new test; not yet run against Epic (a Refresh runs it
  and shows the call ledger). Not committed. The owner asked to pause and re-align the build path.
- The owner ran a Refresh: the call ledger works (42 calls, 6 s, 300 resources, 13 types).
  CarePlan: 2 of 10 categories accepted, 8 refused `fatal:invalid`, which marked CarePlan
  incomplete and toasted. Fixed: a refused fallback category is "not served" (`Job.is_fallback`,
  outcome `category-not-served`), and the refused list no longer repeats a type. Core 44 tests.
  Not run live yet. Committed and merged into `main`; the owner opens a branch for the layout.
- DeetsEngine checked: it builds natively and for wasm; its `Store` trait is about 10 methods;
  its API takes and returns plain strings. The owner chose **R-A lite** over R-A (a generic
  value type would rework `globals.rs`, the indexes, packing and the journal), and the memory
  store and redacted reports **ahead of the engine's stage 1**. Wrote the request:
  `../DeetsEngine/docs/ideas/deetshealth-needs.md`, with a pointer at the top of its
  HANDOFF.md › Open now. DeetsHealth's own globals are designed here afterwards.
- Committed both repositories (DeetsEngine `adea158`, DeetsHealth `dfacc33`). The owner asked
  for Camila's mock record as CSV to model the layout on. Built `dev::export_csv` (sandbox
  only, enforced in the core) and `npm run inspect -- --csv <folder>`; exported 17 files to
  `dev-dumps/epic-sandbox/` (git-ignored). Core 45 tests.
- Committed `c6f610a`. The owner asked for one .xlsx in `npm run inspect`. The core now returns
  tables (`dev::export_tables`, `to_csv`); `src-tauri/src/xlsx.rs` writes the workbook by hand
  (no new crate). `--export` (or `--csv`) writes the CSV files and `record.xlsx`. A first build
  wrote 2 extra bytes per zip directory entry, which Python refused; fixed, and the test now
  walks the whole directory. Checked: Python's zip test, openpyxl reads all 18 sheets. Not
  checked: Excel and LibreOffice (the LibreOffice helper does not run on Windows).

## 2026-09-24 — stage 7: `ts-rs` and Settings › Sites
- The owner asked where the SMART work stood, then asked for stage 7.
- Built: `core/src/payload.rs` (the Sites payloads and the event types; `CardPass`'s first
  user, the site's name), `ts-rs` 12 behind the core's `ts` feature, `npm run types`
  (`scripts/types.mjs` → `src/model/`, 9 files). `Store::type_counts`. `auth::entry_for`
  and `auth::renew_site` for a saved site's Sign in. Host: `site_sign_in`, `sites_list`,
  `site_sign_out`; the fetch drops dead tokens on "sign in again". Front end: `src/sites.ts`
  and the Sites section of the Settings card. Three new error codes.
- The sign-out erase came with stage 7 (S5 says the second click erases); stage 8 keeps the
  refreshes.
- Checks: core 40 tests + 3 proofs, host 14, wasm, clippy (3 old notes in `log.rs`), `tsc`,
  `vite build`, `docs:check`. Live tests not run: the fetch itself did not change.
- Corrected HANDOFF.md: the fixes after stage 6 were committed in `0061887`.
- New crates: `ts-rs`, `ts-rs-macros` and `termcolor`, build time only (the `ts` feature).
- Open: the owner's desk test (CORE-CRATE.md §12) and the choices in HANDOFF.md › Open now 11.

## 2026-09-21 — the fixes after stage 6
- The owner said values can be padded, asked to fix the five findings, and asked that every
  failure and error route have an error the app can toast. He chose: Padmé with a 64-byte
  floor; a plain Observation search with the 9 categories as the fallback; automatic
  re-index by version; one `app-error` event with toasts now.
- Built: `seal.rs` format 2 (padding), schema 2 with an in-place migration, `CoreError::Version`,
  IMMEDIATE transactions and a 5 s busy wait; `net::on_site`; the Observation fallback; the
  `token-refresh` warning and `Summary.stopped`; `INDEX_VERSION` and `reindex_if_stale`;
  `src-tauri/src/errors.rs`, `fetch::check_sites`, a code on every sign-in, fetch and store
  failure; `src/app-errors.ts`. New `architecture/ERRORS.md`.
- Live on the SMART sandbox: the plain Observation search is accepted; 68 jobs and 70 calls
  (was 76 and 78); 163 resources, 223 links, 0 warnings. Core 37 tests, host 14, clean.
- Open: the toast wording (ERRORS.md §3) is a draft for the owner.

## 2026-09-21 — the architecture docs
- The owner chose a separate `STORE.md` (not a DATA-MODEL.md part) and a map for
  DATA-ARCHITECTURE.md (not a fold-in).
- New `architecture/FETCH.md`, `architecture/INDEX-AND-LINKS.md`, `architecture/STORE.md`,
  each written from the code.
- DATA-ARCHITECTURE.md cut to a map of the data's path, with its section numbers kept for
  old links. Refreshed CORE-CRATE.md (header, §3 layout redrawn from the code),
  DATA-MODEL.md (header, `Provenance`, fork 8), SECURITY.md (header, new §8.8 as built),
  SECURITY-BRIEF.md (§6 q3, §7, §8), CLAUDE.md's doc list.
- Found in the code, none fixed (HANDOFF.md › Open now 10): sealed lengths show which
  values are missing; the `next`-link check is a text prefix; Observations with no FHIR
  category are missed on the SMART sandbox too; a failed refresh after a 401 stops a job
  with no warning; `reindex` has no caller. Corrected the "missing" claim in CORE-CRATE.md
  §12 and in two comments in `core/src/store/`; the stage list in `core/src/lib.rs`.

## 2026-09-21 — the data model
- Laid out every FHIR R4 type by category, marked against Epic's 60 and US Core. US Core
  9.0.0 has 27 types (it added `FamilyMemberHistory`). Checked each US Core profile's
  Must Support flag for the encounter link.
- The owner described Epic's links: a lab result links to its ordering and its collecting
  encounter; medicines ordered outside a visit get a telephone encounter; problems and
  allergies carry an encounter only sometimes.
- The owner set the model: the encounter at the center, core pieces and encounter pieces, a
  unified results table with a kind column, joined at runtime. Weaving the record together
  is the app's differentiating feature.
- New `architecture/DATA-MODEL.md`. The owner closed forks 1–6 (1a 2a 3a 4a 5a 6a) and chose
  to grab the non-patient data that sits next to the record. It is sorted into `other`,
  `definition`, `site`, `site_warning`, `document`, `directory` and `vocab`. Open: fork 7
  (the `vocab` files) and fork 8 (which columns stay plain, at build).
- New `architecture/CORE-CRATE.md`, the paper design for step 3. The owner closed forks 1a
  2a 3a 4a 5a 6a 7a 9a 10a and named the wrapper `Shielded<T>`. He set the strictest standard
  for PHI (HHS's bar for covered entities), which brought Windows CNG and SChannel (10a) and
  the rule that no protected value is plain in SQLite. SECURITY.md §4.6 now has one key per
  site. Then 8a (`ts-rs`, new to the Deets apps) and 11a (one payload struct per card view).
  New SECURITY.md §8: every security measure in one place, to speak from.
- The owner accepted the Sites section (S1–S6 a) and the build order (CORE-CRATE.md §2).
- Epic has no central sign-in: each health system runs its own FHIR server and MyChart. Its
  Brands bundle lists 1,278 R4 endpoints; Epic asks apps to download it weekly and re-host
  it, never to query it at runtime.
- New `integrations/ENDPOINT-DIRECTORY.md`: the Brands bundle's shape (94.9 MB; 1,278 brands,
  95,645 locations, 820 endpoints; no logos). The owner chose a worker to re-host it weekly.
  Trimmed for the picker it is 1.06 MB compressed.
- Built stage 1 (CORE-CRATE.md §12): the workspace, `deetshealth-core`, `Shielded<T>` with a
  sealed two-pass design, the platform traits, `CoreError`. Added the wasm32 target to the
  toolchain. Checks: 3 tests, 3 compile-fail proofs, the wasm check, `cargo check`, clippy.
- Built stage 2: `seal.rs` (format byte; table, row and column in the associated data),
  `cng.rs` (CNG AES-256-GCM, RNG, SHA-256), `keystore.rs` (DPAPI-wrapped key per site).
  19 tests pass, including the GCM specification's vectors and a flipped-bit test per byte.
- PATIENT-APP-DESIGN.md §6.1–6.3: the owner's rule that every fork weighs every platform (also in
  CLAUDE.md), and notes for macOS and iPhone. The one real limit found: a Mac app has no
  uninstaller, so uninstall cannot erase there.
- Built stage 3: the store (`core/src/store/`, rusqlite 0.32 bundled behind a `sqlite`
  feature) and `src-tauri/src/sites.rs`. Every column but row keys and table names is sealed,
  a missing value included. 27 tests pass; a byte scan of the real files finds no plaintext.
- Stage 4 fork: sign-in and the fetch must send the token, the `Patient` id and the site's
  URL back to the site. The owner chose a third pass (`SitePass`, the network module only)
  over a separate credential type. SECURITY.md §4.2, §8.2, §8.5 and SECURITY-BRIEF.md updated.
- Found that SMART's launcher refuses a standalone launch on a plain base URL; it reads its
  setup from a base64url JSON array in the URL. With the patient set and login skipped, the
  authorize step returns the code at once, which makes a browser-free live test possible.
- Built stage 4: `core/src/net.rs`, `core/src/auth.rs`, `src-tauri/src/signin.rs`,
  `browser.rs`, the link wiring in `lib.rs`; `SiteRow` gained `client_id`. New crates:
  reqwest 0.12 (`native-tls`, `gzip`), `serde_json`. 33 offline tests pass; the live sign-in
  test (PKCE, state, token trade, refresh, refused refresh) passes against the sandbox.
- New `architecture/SIGN-IN.md` (the owner asked): the flow, every constant and where to
  change it, the SMART launcher's setup array, the link on Windows, log lines, fixes.
- Committed stages 1–4 (`10f8cfb`).
- Built stage 5, the fetch: `core/src/fhir.rs`, `core/src/sync.rs`, the `Store` and `Timer`
  traits, `src-tauri/src/fetch.rs`; the fetch starts after sign-in. New crate: futures-util
  (already in the build), and serde_json's `preserve_order` (adds indexmap) so raw JSON keeps the
  site's key order. Live on the SMART sandbox: 76 jobs, 78 calls, 5–7 s; paging at 20
  a page followed every `next` link on the site; a second fetch left row counts unchanged.
- Committed stage 5 (`a8f125d`).
- Stage 6 scope: the owner chose the index and links now, and the card models later, with
  each card's design (this narrows fork 9a; CORE-CRATE.md §8).
- Built stage 6: `core/src/index.rs` (index columns, every reference with its field and
  role), link writing and resolution in the fetch, `sync::reindex` (rebuild from raw, no
  fetch). Live on the SMART sandbox: 101 resources, 122 links, 4 waiting (targets the site
  never sent). 30 core tests pass.
- Committed stage 6. Listed the architecture docs to write or refresh in HANDOFF.md › Open
  now 10 (three new docs: the fetch, the index and links, the store; and five stale
  passages). The session
  ends here; stage 7 is next.

## 2026-09-21 — the empty shell
- Fixed two old doc lines: README (SMART first, Epic on hold) and SURFACES-AND-CARDS.md §2
  (the first slice is decided). Fixed three § pointers that named list items, not headings.
- The owner closed the shell forks: DeetsMusic's Midi and Max shapes, the Patient card for
  Now Playing, the Timeline card for the Queue, a new Conditions card for Max's fourth slot,
  the cog and quick panel in the title bar, and the first look Lilac × Press / Moonlight × Glass.
- Built scaffold steps 1–2 (SURFACES-AND-CARDS.md §3): the Tauri app and the engine brought
  across from DeetsMusic, each file checked against SECURITY.md §4 on the way in. Wall changes:
  hints lost row hints, the menu log lost labels, diag logs error kinds only, the panic hook
  withholds the message, the log cuts URL query strings, the CSP allows no inline script.
- Ported the scripts: `dev-app.mjs`, `docs-check.mjs` (checks 1–4, 11), `docs-move.mjs`,
  `tokens.mjs` (made `docs/architecture/TOKENS.md`).
- Checks: `tsc`, `vite build`, `cargo check` (dev and release), 3 log tests pass,
  `docs:check` clean. One dev launch: the page reported ready, no front-end error.
- The owner desk-tested the shell: passed. Next: the core crate, then Compass.

## 2026-09-21 — security rules and the probe plan
- The owner set the priority: patient experience, then privacy and security, then the rest.
- New `architecture/SECURITY.md`: protected data = PHI + PII (any identifier), the threats,
  end-to-end encryption, the fixed rules (`textContent` only, CSP, no plugins to the front
  end, PKCE + `state`, log and panic rules, keys, dev rules), and a law summary.
- The owner closed forks: cache encryption by our own crate (wrapping audited primitives
  only — noted as a rule), the wrapper type, dynamic client registration first for Epic,
  delete on sign-out and uninstall.
- Epic is on hold: the owner cannot register on fhir.epic.com yet. SMART Health IT is the only
  live site.
- Probe plan decided: a kept `scripts/fhir-probe.mjs` and `architecture/FHIR-SHAPES.md`.
- New open fork: HANDOFF.md › Open now 7, the shape of the encryption crate.
- Built `scripts/fhir-probe.mjs` and ran it on SMART's open server: 9 resource types, 300
  each, shapes only. Output: `architecture/probes/smart-open.md` (generated).
- Epic's specification page renders with JavaScript and could not be read; its sandbox
  CapabilityStatement could, and gives 60 resource types (Epic August 2026).
- New `architecture/FHIR-SHAPES.md`: findings per resource, five normalizer rules, three
  forks (HANDOFF.md › Open now 8). Biggest surprises: lab ranges missing in about half,
  no high / low flags, no allergy reactions, no dose text, and Synthea patients carry
  social security, licence and passport numbers.
- The owner closed 7a (per-value sealing, to and from SQLite), 8.2a (the site's own range;
  filling missing ranges goes to ideas), 8.3a (build dose text; his ideas at build time).
  8.1 is re-asked with "keep" defined and a new recommendation (name, birth date, sex).
- PATIENT-APP-DESIGN.md §1 now says handling messy data well is the app's point of difference.
- The owner closed 8.1b: keep name, birth date, sex; compute the rest at runtime. Listed
  SMART's Patient extension URLs and identifier types (no values) to name what is dropped.
  New open fork: which field is "sex" (FHIR-SHAPES.md §5.4).
- Sex parked with a direction: display `gender`, compute a clinical sex from several
  factors at runtime.
- Fork 6 read against DeetsMusic's imports and closed in part: 6A empty app + one-by-one
  import, 6B no tray + web-ready design (PATIENT-APP-DESIGN.md §6), 6C release path scaffolded up to
  the worker. 6D (Compass) re-opened by the owner. `frames.ts` must come with the engine.
- Cactus Needle 3 read and noted in ideas/README.md.
- The owner closed the web-ready split (a; a secure cloud provider is acceptable if a web
  version needs a server) and 6D (a: Compass shell with the scaffold). Needle planned as a
  Compass layer for ambiguity. Looked up Quest and Labcorp patient APIs; noted in ideas.

## 2026-09-21 — a second sandbox site
- The owner added SMART Health IT's public sandbox as a second site in the same app.
  Recorded in HANDOFF.md › Decided and DATA-ARCHITECTURE.md §2.
- Probed the discovery endpoints (no sign-in, no patient data): Epic's and SMART's
  `smart-configuration` and `r4.smarthealthit.org/metadata` all return 200. Epic lists
  `client-public` and `permission-offline`; its `scopes_supported` has no `patient/*`
  scopes, so Epic's resource scopes come from the app registration.

## 2026-09-21 — habits brought over from DeetsMusic
- Read DeetsMusic's CLAUDE.md, LESSONS.md, TASTE.md, DOCS-ORG.md, HANDOFF.md and the
  sign-in part of DEBUGGING.md.
- CLAUDE.md: added the full working style (forks, hand-off list, one feature in flight, the
  pre-build list with a PHI rule for hints, toasts and logs), the verify notes (no harnesses,
  the MSIX registry trap, the dev-app ready line), the explain rules, Conventions, and the
  front-matter / part-marker rules.
- New `docs/LESSONS.md`: DeetsMusic's lessons cut to this project, with a fourth stop rule
  (PHI) and a PHI tie-breaker. TASTE.md is pointed to, not copied (it is local only).
- DOCS-ORG.md: the status table, part markers, the HANDOFF / WORKLOG rule.
- HANDOFF.md: three sign-in gotchas; a note on dev vs release redirect URIs under fork 5.
- Not brought over: playback, graphics measurement, release and publishing, workers, the
  agent / MCP routes (PATIENT-APP-DESIGN.md §3 rules them out until the PHI wall is designed).

## 2026-09-21 — the project folder
- Made `Documents/DeetsHealth` with README, CLAUDE.md and the first docs, and a local git repo
  (no remote).
- Decided by the owner: a patient-facing app on Epic's FHIR API, sandbox patients first,
  structured as a 1-to-1 copy of DeetsMusic, with Midi and Max as the only surfaces.
- Open forks: HANDOFF.md › Open now.
- Later the same day, the owner closed forks 1–4: copy the skeleton, deep-link scheme,
  labs/meds/allergies for 1–2 sandbox patients, SQLite cache.
