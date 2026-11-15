---
status: sop
desk_test: none
sources: []
updated: 2026-11-15
---
# DeetsHealth — Handoff

> Cold-start guide — read first. This file holds what is still true: run, **Open now**, state of
> play, gotchas, decisions, file map. Corrected in place. The record of each sitting is
> [WORKLOG.md](WORKLOG.md). The product is [DESIGN.md](DESIGN.md). The patient app's handoff
> is [archive/PATIENT-APP-HANDOFF.md](archive/PATIENT-APP-HANDOFF.md).

## Run it
**The console with the agent** ([architecture/CHAT.md](architecture/CHAT.md) §6): `npm run dev`
starts the agent's dev server and Vite, each on a free port, and prints the address (1420 or the
next free one). It needs `ehr` built, the dev certificates and `uv sync` (below). `npm run dev:web`
starts the console alone. **The typed call**: see AGENT.md §1. Nothing else runs end to end yet.

**The dev database** ([architecture/DATABASE.md](architecture/DATABASE.md) §3): install Postgres
at the pinned major version once (`PG_MAJOR` in `scripts/postgres.mjs`): on Windows, EDB's
installer from postgresql.org (winget's download of it failed, 2026-11-12); on the Mac,
`brew install postgresql@<major>`. `npm run dev` then starts the repo's own cluster in
`.pg/` if it is not up, on a free port from 5433, and files the call records into it
(`ehr ingest`). Each call is also filed as it ends (`FileCall`). **The database is added to,
never deleted or rebuilt** (DATABASE.md §4b). `npm run db:status` / `db:stop` / `db:ingest` /
`db:test`. Without Postgres, `npm run dev` says how to install it and goes on.

**The single check command** (`scripts/check.mjs`) runs only the checks that your changes need.
"Changes" means the commits since the upstream branch (else `origin/main`) plus the working
tree. It runs the docs check every time, and stops at the first failure with the tail of that
step's output.
```
npm run check                       # the checks the changes need
npm run check -- ehr agent          # named scopes only: docs web core ehr agent keys
npm run check -- --list             # say what would run and why; run nothing
npm run check:all                   # every scope plus the slow extras (vite build, ehr clippy)
```
The scopes: `web` (front-end type check), `core` (tests and the WebAssembly build), `ehr` (tests),
`agent` (ruff and pytest, replay only, against a fresh `ehr` build in `target/check`), `keys` (the key-set Worker
build), `docs`. A change to core also runs `ehr`, and a change to `ehr` also runs `agent`.
`agent/calls/` (the dev call records) runs nothing. The pre-push hook (`.githooks/pre-push`)
runs it on the commits being pushed; `npm install` turns the hook on. To skip the hook once:
`git push --no-verify`. All 12 steps took 50 s on 2026-11-11, and the agent tests are 28 s of that.

Each check by hand:
```
npm install
npx tsc --noEmit      # front-end type check (it compiles; it does not run in a browser yet)
npx vite build        # front-end bundle
npm run docs:check    # links, mentions, § pointers, front matter, the archive rule, the drift checks (DOCS-ORG.md §4)
npm run tokens        # regenerate docs/architecture/TOKENS.md after a token change
npm run types         # regenerate src/model/ (TypeScript) from core/src/payload.rs
cargo check --workspace --features deetshealth-core/dev,deetshealth-core/ts   # not --all-features: ehr's `fips` needs CMake and Go
cargo test -p deetshealth-core --features sqlite                  # core tests + compile-fail proofs
cargo test -p deetshealth-ehr                                     # the ehr service: verify, gate, booking, PHI marks, mTLS
cargo run -p deetshealth-ehr --example dev_certs                  # once: the dev CA and certificates into dev-certs/
cargo run -p deetshealth-ehr                                      # ehr on a free port (first line names it), the text run's fakes
cargo check -p deetshealth-core --target wasm32-unknown-unknown   # core stays web-ready
cd agent && uv run python harness/typed_call.py                    # the typed call (the desk test); see AGENT.md §1
cd agent && uv run pytest                                          # the agent's tests, replay only: no tokens
```
The build folder is `target/` at the repo root (it was `src-tauri/target/`).

**On another machine (a Mac, 2026-11-12).** Nothing in the build is Windows-only: the `ehr`
binary's name follows the OS (`agent/deets_agent/config.py`), `scripts/check.mjs` runs no shell,
and the pre-push hook is plain `sh`. What a fresh clone needs, in order:
1. Xcode's command-line tools, Rust (`rustup`, plus `rustup target add wasm32-unknown-unknown`
   for the core check), Node, and `uv` (it brings Python 3.12 for `agent/`).
2. `npm install` (it also turns the pre-push hook on), then `cd agent && uv sync`.
3. The dev certificates, which are git-ignored: `cargo run -p deetshealth-ehr --example dev_certs`.
4. **Keys are made per machine, never copied** (the owner, 2026-11-14). Each vendor gets a new
   key for each machine, named for it ("deets-mac", "deets-pc"): a lost machine costs only its
   own keys, and each vendor's usage shows which machine spent what. Copy `.env.example` to
   `.env` and type each value in by hand, never through a chat, a doc, a commit, a sync folder
   or a shared password manager. **`npm run env:check`** then names what this machine lacks,
   group by group, and the secret files outside `.env`. It prints names only, never a value.
   `.env.example` is the one list of names: a new setting gets its line there (its header says
   how). Replay-only tests and `npm run check` need no key.
   - **Epic:** `epic-client.json` (the client IDs) and this machine's own key:
     `cd ehr/jwks-worker && npm run make-key` adds its public key to `src/nonprod.jwks.json`
     beside the other machines'. Commit the set and deploy the Worker (EPIC.md §2.3). `ehr`
     signs with the kid of the key on disk. Only for Epic work; the build runs without it.
   - **The tunnel** runs on one machine only; its credentials file stays there (DEMO.md §7).
5. **The sibling repos** for the docs check: `../DeetsMusic` (clone `deets-137/DeetsMusicWindows`
   under that name) and `../DeetsEngine`, or the docs check (and so the pre-push hook) fails on
   their links (Known gotchas).
6. **Postgres:** `brew install postgresql@18` (the pinned major, `PG_MAJOR` in
   `scripts/postgres.mjs`). The first `npm run dev` makes the Mac's own cluster, password and
   sealing key in `.pg/` and files every committed record in `agent/calls/` into it. From then
   on it only adds: each call when it ends, and any record a pull brought in. Never copy `.pg/`
   between machines (DATABASE.md §4b).
7. `npm run check:all` proves the machine. Then `npm run dev` for the console.
8. **Claude's own setup is per machine.** The owner's global `~/.claude/CLAUDE.md` (the writing
   rules, the fork test, the end-of-turn box) and this project's memory folder under
   `~/.claude/projects/` live on the PC. Copy both to the Mac, or a session there starts
   without them.

## Ship it
Nothing ships yet. The deploy is A2: a separate deploy per hospital in our cloud, behind an
interface (VOICE-AGENT.md §7.2).

## Open now
**Start here (2026-11-15, morning).** The kept scripts pass again: all 15 recorded, every agent
test passes, and the stamp is written (`tests/RECORDED-PROMPT.sha256`). `emergency_reminder`
passed 4 of 4 fresh draws this morning; its old failing replies were deleted and it was
recorded again. The Epic code-system research is done (EPIC.md §7.11, HOSPITAL-CONFIG.md
§2.10). **Built, desk test open: the owner's A, one Epic list per list** (HOSPITAL-CONFIG.md
§5.1, desk test §6 steps 8 to 12; "R PRC 1003" on each row). Committed `74b4ecc`. **Built,
desk test open, not committed: the preference question (1A) and the note label (N2)**
(CALL-FLOW.md, HOSPITAL-CONFIG.md §2.9): book a visit on Local without saying how or when; the
agent asks once, then offers times; in Epic mode the `$book` note ends with the label.
`Location.Search (Organizational Directory) (R4)` is registered (the owner, 2026-11-15) and
tried (`ehr epic-locations`, EPIC.md §2.2): one search returns all 435 sandbox locations. The
import route is designed (HOSPITAL-CONFIG.md §8, EAF facilities over DEP departments); not
built. **The call records start fresh (the owner's A, 2026-11-15):** the 158 old records and
the annotations are in the tag `archive/calls-2026-11-15` (local, not pushed), removed from
`agent/calls/`, and cleared from the dev database. **His step, the evening of 2026-11-15:** a new
baseline of console Test calls (the draft list is in the WORKLOG entry of that day). Then the open desk
tests: booking builds 1 and 2, the request log, the EHR ledger and the repeat guard (EPIC.md
§7.10; AGENT.md §1). Restart `npm run dev` first: its `ehr` predates these sittings.
Ideas left for later: ideas/LOCAL-EHR.md; many hospitals with rules-based routing
(ideas/README.md); pass^k sampling (AGENT.md §4).

**Built, desk test open: Config › Hospital** (2026-11-15, late; architecture/HOSPITAL-CONFIG.md
§6). Committed and pushed with the checks skipped, on the owner's word ("commit and push with
no checks for the night"). The four kept scripts that missed their replays were recorded on
2026-11-15 in the morning. The lists no call reads yet (reasons as codes, hours, closed days,
specialties, a location's id, address and phone) are the work left: HOSPITAL-CONFIG.md §7.

**Morning, in this order (the owner, 2026-11-15, night):**
1. **Done: research on Epic's code systems** (EPIC.md §7.11, HOSPITAL-CONFIG.md §2.10). The
   owner: his Epic years did not cover Cadence; he knows 4-digit codes such as 1003 and 1024 as
   IDs of a facility or location. The docs say a short ID is a record ID in some master file,
   and the system names the file: here 1003's system is PRC (visit types).
2. **Done: the owner chose A** (2026-11-15, morning) and it is built: HOSPITAL-CONFIG.md §5.1.
3. Reload the console and check the column alignment fix (`config.css › .sys-table
   .prompt-control`; Config › Hospital and Config › System), then the desk test.

### The build (BUILD-ORDER.md)
**Where we stand (2026-11-11): the paper work and the text run are done.** Three tracks remain,
and the console starts; each next step follows from what the console needs (the owner,
2026-11-11). One build is in flight at a time. The whole plan is BUILD-ORDER.md.
1. **The console** (it leads) — the design on paper first, then the settings pages, staff
   sign-in, the call list, transcripts and the audit trail (BUILD-ORDER.md §3.1).
2. **The chart and the store** — the Epic sign-in, the chart reads through record and replay,
   booking on Epic's sandbox, then Postgres with sealing (BUILD-ORDER.md §3.2).
3. **The phone and the voice** — Twilio and Cartesia, speech in and out, the speed target, the
   cost per call minute, then a real number (BUILD-ORDER.md §3.3).

Then going live: the deploy, the BAAs, the paperwork, the first hospital (BUILD-ORDER.md §3.4).

- **Built, desk tests open: the console on a phone** (2026-11-14; the tracker is
  architecture/MOBILE-UI.md). The compact nav (CONSOLE.md §11: the page and view dropdowns in
  halves); Chat's toggles under the text box and Start / End call in halves (CONSOLE.md §11.4);
  the call panel first in Demo and PHI (CHAT.md §15.1); the rail panels' grow handle (CHAT.md
  §15.2). Next (MOBILE-UI.md §5): the other pages, the other controls' hover on touch, the safe
  areas, the on-screen keyboard, a run on a real phone.
- **Built, not yet run: voice on the Chat page** (2026-11-13; the owner's forks 1B with a tab
  bar, and 2A). Testing's **Text | Voice** bar (Text each time); a voice call through the laptop
  microphone, Ink in and Sonic out (`voice.py`; our own WebRTC seat, no remote code); barge-in,
  and the 911 line that plays to its end; a voice test call ends at 60 s of silence or 6 min;
  the bottom panel's **Resources** tab counts Cartesia credits this cycle. CHAT.md §17, every
  lever in CARTESIA.md §9, the real-call design in FUTURE-SETTINGS.md §12. **His step first:** a
  Free Cartesia account, then `CARTESIA_API_KEY` in `.env` (and `CARTESIA_CYCLE_DAY`, the day
  the account was opened), then restart `npm run dev` and run CHAT.md §17's desk test. Open:
  his voice (Skylar is a draft); Twilio later (BUILD-ORDER.md §3.3).
- **Built, desk test open (his, the morning of 2026-11-14): Config › Cartesia and the settings
  store** (CONSOLE.md §7). Read | Edit, the versions dropdown ("Active", "MM-DD | n"), Save, an
  older version read only with its changed values glowing; every Cartesia lever with an "i" for
  help; Credits left with a manual offset; the key's status. The store: `0002_settings.sql`
  (versions, `settings_voice`, activations; insert-only), `ehr/src/settings.rs` (every setting
  checked), three RPCs, `GET`/`POST /agent/cartesia`. Voice calls run with the active version.
  **Restart `npm run dev` first** (new `ehr`, the migration). Open: pronunciations need a
  Cartesia dictionary; Make active; Listen (CONSOLE.md §7.6).
- **Built, desk test open: Config › System, the variables and the tools** (2026-11-14; the
  owner's forks 1B, " ." opens the tool list, and 2A, analysts add their own variables).
  SYSTEM-PAGE.md: `{clinic}` and the analyst's own System variables (the `system` service,
  migration 0006, `agent/prompts/VARIABLES.md`) may go in any section; code values stay in their
  own; a tool in the words is named in backticks and checked against its steps; `{` and " ."
  lists on Config › Agent; the Tools tab reads `agent/prompts/TOOLS.md`. **Restart `npm run dev`
  first**, then SYSTEM-PAGE.md §5. Open: the existing plain tool names in the words are not in
  backticks yet (a words change: Record and test after); the rules engine (§8). Since then its
  lists are tables, each row with an "i" (SYSTEM-PAGE.md §5 steps 2, 11, 11a).
- **Built, desk test open: Config › Agent arranged by the call** (2026-11-13; the owner's 2A).
  Tabs Greet · Verify · Menu · Schedule · Anytime · Failure; each line under the tool that
  speaks it; each title says what the row is, and its "i" says when it plays. Since 2026-11-14
  (his 1A and 2B): one layout, a block per tool; Plain (new analyst) | Code (power user: the
  tool's name and each key) in place of Semantic | Flow; the lines' titles start with a verb.
  Then an AI or HC mark before every title, a Core tab before Greet (Role and rules, and the 911 tools), and
  Plain | Code animated in place.
  Names and "plays when" live in `agent/prompts/GUIDE.md`; the owner edits them there after the
  desk test. AGENT-PAGE.md §8 to §10 (the desk tests to run).
- **Built, desk test open: the demo over the tunnel** (2026-11-13/14; ops/DEMO.md). `npm run
  dev` opens `deetshealth.deets.solutions/demo/` behind Access when every guard passes (one
  connector, the main checkout, `cf` and `cloudflared` ready); it prints `[demo] on` or
  `[demo] off: <reason>`. Several calls at once, a label beside Start, PHI's Caller list, live
  reload from a watched build, TURN for guest voice. Desk test: DEMO.md §6. Then DEMO.md
  becomes `built`. A Cloudflare error 1033 at the address means no connector runs: read the
  `[demo]` line.
- **Built, desk test open: Teams › People (cards, read only) and Security › Auth (the table
  that edits them)** (2026-11-14; DEMO.md §3.5, §7). Who may sign in: the `users` table
  (migrations 0007 and 0008: email, name, first and last name, phone sealed; `user_photos`
  sealed), nine RPCs in `ehr`, and Cloudflare's Access group kept equal to the list. The token
  is in `.env`; it has read the group but **no write has been tried**: the first Add, Remove or
  email change on Security › Auth is the first. The owner has desk-tested the panel, a name and
  phone save, the Users title and Add's blank row (2026-11-14). **Not yet desk-tested:** a
  picture upload (`user_photos` is empty), adding a second person, Remove, an email change.
  Phone verification by text code is not built: every number shows the scarlet X. Role is a
  placeholder (`users.role` is not read) until roles exist. "Added by" is "First L."; the
  owner's own row says "Deets". Commits `819b7e3` and `a445276`; the full `npm run check` has
  not run on them (the dev server held the ehr exe). Org Chart is still empty.
- **Built, desk-tested 2026-11-14 (the owner): the console's read layer** (LOADING.md §2–§5). Every page
  reads through a store (`agent/store.ts`, `agent/stores.ts`): the newest answer wins, writes in
  click order, polls one at a time, retries when the agent is back; every read in the ledger
  (`agent/ledger.ts`, `deetsLedger` in dev). **And the first-load wave** (§6, the owner's fork
  A): the app hidden until its first data, then one wave top left to bottom right
  (`reveal.ts`). Desk tests LOADING.md §8 and §9: passed. Same day, also desk-tested: Security
  › Auth saves an incomplete card on a second Save (red titles, red Save; no email = no
  sign-in), and `npm run dev` waits for the agent's port before Vite starts.
- **Built, desk test open: the booking keeps the caller's reason; Local half** (2026-11-14,
  after the Priya Raman call; the owner's 1A of 2026-11-13, 1A for odd reasons, A for
  pronouns). `FindSlots` takes `visit_mode` and keeps the reason (checked to be the caller's own
  words) as the booking's note; `Book` returns the visit (type, online, reason, through the
  sensitive screen); the fake's slots have visit types, two of them online; an online time says
  "online"; `next_visit` does not read out the visit just booked again; an odd reason is booked
  and flagged `visit_reason_review`; a staff summary with a guessed pronoun goes back once.
  EHR-SERVICE.md §4, CALL-FLOW.md, FLAGS.md §3.1–3.2. Also: a saved prompt version that lacks
  a new section now takes the shipped words for it (PROMPT-EDITOR.md §9). **Recorded
  2026-11-14:** the owner's Test run recorded all 15 scripts with the new words; 2 failed, both
  resolved the same night (a wrong assertion; the resend case, his option A, FUTURE-SETTINGS.md
  §11). All 15 pass on replay, and the recordings stamp was written by hand that night (every
  reply was already recorded), so Test shows green. **His step:** call as Priya Raman on Local: "Can I book a visit for my strawberry water distillation?", choose an option,
  "What was the reason on the visit again? Is it online?". New words (his to edit):
  `arg.find_slots.visit_mode`, `line.booked_visit_asked`, `note.summary_pronoun`, a rule each
  in `role` and `task.menu`.
- **Built, desk test open: Test | cost on Config › Agent** (2026-11-14, the owner's design and
  screenshot; CONSOLE.md §9). Record and test moved to the title bar, left of Discard | Save and
  the same size: red until the recordings match, a first press for the cost ("$?.??" until
  known), a second to run with a bar that counts steps, green when passed. The cost step now
  ends as "failed" on an error, never "running" forever.
- **Next, the owner's pick in the morning: better kept-script tests** (AGENT.md §4, "What the
  kept scripts catch"). (1) show why a run failed in the Test hint; (2) several samples per
  script with a pass rate; (3) assert on events; (4) scripts for the gaps.
- **Built, desk test open: booking and visits, builds 1 and 2** (2026-11-15, EPIC.md §7.10;
  forks 1A, 2A, V1A, V2A). Build 1: `find_visit` (the next visit, or one named by day, month,
  clinician, kind or past; the narrowing question by visit group), instructions offered or read
  (`read_instructions`), the CSN in a change flag, the hospital's time at the call's start.
  Build 2 (E-4): `$find` with the patient's demographics, a last day, a time of day, days and a
  clinician; Epic's page of 10 and More; `$book` with a read after, and a lost reply asked
  about, never sent twice. The desk tests are in EPIC.md §7.10 (build 1 and build 2 as built).
- **Closed 2026-11-15 (morning): `emergency_reminder` at the code turn.** Four fresh draws
  after the Config › Hospital commit all took the code and booked. The cause of the night's
  4 of 4 was not isolated; the context changed since. Record mode keeps a reply already on
  disk, so the night's failing replies stayed until deleted: to record one script fresh, delete
  only the replies no other script uses (`typed_call.py --keys-out` per script). Seen in
  every draw, not a test failure: with "chest pains" in the first line, `find_slots` runs at
  once with that reason, and searches again when the caller says "routine check-up".
- **Built, desk test open: the request log, the EHR ledger, the repeat guard** (2026-11-15,
  AGENT.md §1, DEBUGGING.md §3). The Epic ledger is tested in ehr only: the first Epic-mode
  console call after the dev server restarts shows `epic` events (`show_call.py`).
- **Real booking in Epic mode, always** (the owner, 2026-11-15: no switch). `$find` and `$book`
  work on the sandbox (EPIC.md §7.10); each booking in Epic mode writes to Epic's shared
  sandbox. Local mode stays fake.
- **Built, desk test open: Config › Agent, the agent's words in the console** (2026-11-13; the
  owner's five tabs, Semantic | Flow, the names list). Every section of AGENT-PROMPT.md, versioned
  in the store (`prompt` service, migration 0005); a save writes the file and reaches the next
  test call; each call keeps its own copy; ehr texts the code with the call's version; Record
  and test (since 2026-11-14 the **Test** button in the title bar) re-records the kept scripts
  and runs the tests. **Restart `npm run dev` first**, then CONSOLE.md §9.5. **New rule:** after
  any edit to the words, `npm run check` fails at
  `test_the_kept_recordings_match_the_prompt_file` until Test passes. Restart `npm run dev`
  after a change to the agent's Python: it does not reload it. Open: CONSOLE.md §9.4.
- **Done: Config › Anthropic** (2026-11-13; the owner's option A, the page chooses the model;
  **desk test passed 2026-11-13** on wide screens). Model, Effort, Thinking, the token caps,
  Cache life, Budget and Dollars left; the store's `llm` service (migration 0004); test calls
  run with the active version; US-only inference sent on every live request (1.1×). Open: the
  desk test on narrow screens, both Config pages (CONSOLE.md §8.5 step 9); `strict` on tools (a
  re-recording, cost first), pre-warm, Make active (CONSOLE.md §8.4). Cartesia's unused levers:
  CONSOLE.md §7.6.
- **Done: Epic's sandbox patients on the Chat page** (branch `epic-time`; forks 1A and 2A,
  2026-11-12). The Backend Services sign-in (E-2, EPIC.md §5.1), and Epic mode, **desk test
  passed 2026-11-13** (CHAT.md §13): the Epic / Local button left of Test; `OpenCall` names the
  EHR; `ehr` finds the caller and their next visit in Epic's sandbox (EPIC.md §5.2); the rail
  lists Epic's 7 test patients and their visits. Booking: real `$find` and `$book` since
  2026-11-15 (E-4, EPIC.md §7.10). Open: the
  medicine, refill and lab reads (R2–R4), and recording Epic's replies for the tests. Epic's
  sandbox rules and the steps for a hospital: EPIC.md §3.1, §4.
- **Built, desk test open: what a visit is, and the offer of a person** (2026-11-13, after
  Camila's call; the owner's option A). `next_visit` says the visit type, the reason and
  "online" from the record (never the scheduler's notes), and its line ends without a question
  so the model answers the rest of the turn. A sensitive type or reason is dropped in `ehr`
  (`visit_about.rs`, a draft list). When the agent cannot help, `offer_staff` says so and asks;
  a yes is `hand_off`, a no goes on. FUTURE-SETTINGS.md §9, §10. Desk test: Epic, Test, call as
  Camila, ask "when is my next visit and what is it for?", then something it cannot do. (The
  owner's 18:22 call, where a feedback request went to `hand_off`, ran the old code: not a test
  of this.)
- **Built, desk test open: the code step's words are true for every caller** (2026-11-13, after
  Warren's call; the owner's option A, FUTURE-SETTINGS.md §11). The code is "on its way" only
  if the details match and a mobile number is on file, and a person is promised if no text
  comes; "no text" goes to a person at once. Nothing says whether the details matched. Desk
  test: Epic, Test, call as Warren McGinnis (May 24, 1952), say "I didn't get a text".
- **The Chat page's 2026-11-13 round is on `main`** (`63142a8`; CHAT.md §9, §12–16): the
  page's own nav (Testing, Demo, PHI), the rail as two tabbed panels, the Tools tab for
  analysts, Demo's recorded calls with a play button per row and a 0.5× | 1× | 2× speed bar,
  our own scrollbars (CONSOLE.md). **Desk tests passed 2026-11-13**, all but the Review tab's
  last three steps (CHAT.md §12: a dimension's note, a Demo replay keeps the choices, the
  model's line). **Next, the owner's:** the Timeline and Caller tabs are "Empty for now".
- **In flight: the Chat page** (CHAT.md). The UI passed its first
  desk test (2026-11-12). **The connection is built, desk test open** (CHAT.md §6): `npm run
  dev` starts both; Test calls the agent through client-js and our own text transport (not
  Pipecat's WebSocket transport, which loads remote code); the watch stream draws Test, PHI
  and Demo with one code path; Demo plays any call in the catalog. **The catalog** (CHAT.md
  §7): SQLite, rebuilt from the records, with labels and ratings in the committed
  `agent/calls/annotations.jsonl`. **Next, the owner's to design:** the rail panels
  (Timeline, Caller, Step, Tools), where the rating controls sit, the model and code graders.
  Then the staging deploy (2A: after this desk test; at $0, so the Docker Compose stand-in
  first, CLOUD.md). Polish from every desk test is always on (BUILD-ORDER.md §2; AGENT.md §4).
- **Also in flight: the sandbox patients and the code by text** (CHAT.md §8 for the page,
  TEXTS.md for the text path, 2026-11-12).
  Built, desk test open: the rail's Sandbox patients panel (Test and Demo) from
  `ehr/fixtures/sandbox.json`; Test calls send a random code through `ehr`'s real Twilio
  request (`ehr/src/texts.rs`) to a fake Twilio in the dev server; the code button at the
  canvas's bottom left opens the text or sends the code. `npm run dev` now builds `ehr` first.
  Twilio itself is Phase 5; start the sender registration early (TWILIO.md §6). **The Tools
  panel** (CHAT.md §9) is built too, desk test open: each tool call with its trigger line, the
  time to trigger and the time to confirm, exact to a tenth of a ms (record schema 3); rebuilt
  for analysts and desk test passed 2026-11-13 (CHAT.md §9.1). **The
  Step panel** (CHAT.md §11), desk test passed 2026-11-12: a block per step, stacked in call order, the
  current one with its tools and the limits left, the steps ahead faint (record schema 5:
  `step` events at each move; `OpenCall` now carries the limits). **Per-tool latency**: `GET
  /agent/latency` (CHAT.md §10).
- **Also in flight: the dev API reference and the annotation gate** (DEV-API.md, 2026-11-12).
  Built, desk test open (DEV-API.md §8): the gate refuses a field its kind does not take, and the
  catalog alone sets `at` and `rubric_version`; the console's `Annotation` type has one shape
  per kind; the Review panel shows the gate's reason; `test_dev_server.py` runs every route.
  docs:check 12–14 compare the route and RPC ledgers and the console's types with the code.
- **Also in flight: the call guardrails** (REGULATIONS.md, 2026-11-12). Built, desk test open:
  the greeting is a fixed line with the disclosures (automated, transcribed, a person at any
  time; §6), and a caller confused after a failed code goes to a person at once (§2.1; the kept
  script `code_confusion.txt`). Also built: the safety reminder after a warning, and details
  already given are never asked for again (`emergency_reminder.txt`). Staff flags, Phase 1
  (FLAGS.md; forks 1A and 2A): code and the model raise them, silent to the caller, with the
  caller's own words as checked evidence; a hand-off carries one; a caller for someone else
  goes to a person. Desk test open. Next for flags: the kinds list (the owner's), and the
  console's view of a flag. Confirm before booking (choice A), stable option numbers and
  `change_visit` are built (CALL-FLOW.md §5). A reply cut off by the token cap now gets the
  holding line, a review flag and one retry (FLAGS.md §8). **Next: the production-shaped dev
  database** (DATABASE.md): native Postgres, which `npm run dev` now starts (§3). `ehr` is the
  only writer (confirmed; §2.1 says why it scales). Open: the draft schema (§4, the owner's),
  then the store in `ehr` and the ingest. Open, in order: REGULATIONS.md §10 (twins, limits across
  calls, the confidential number, the code by keypad, and more). The map of every step, fixed
  line and limit, by file and symbol: CALL-FLOW.md.
- **Gate on the first real call: split the call record** (the owner, 2026-11-12). Test mode
  keeps full logs on purpose; before a real call, a log with no values and a sealed transcript
  (REGULATIONS.md §8, SECURITY.md §4.5).
- **Done, 2026-11-11:** the console's skeleton (CONSOLE.md; desk test passed in Edge); Pipecat
  1.16.0 (AGENT.md); `main` holds both.
- **Paper, done:** the repo layout; the Anthropic dev workspace, "DeetsHealth Dev", key in the
  git-ignored `.env`; the Epic registration (2026-11-11: the patient app's registration became
  the backend app with the same client IDs; the APIs checked against Epic's live catalog; SMART
  v2; the key set live, EPIC.md §2).
- **The text run, closed 2026-11-11.** Decided: Claude through a replay cache, cheapest model
  first (Haiku), a hard $100-a-month API budget; the Rust `ehr` service from the first call;
  gRPC between the agent and `ehr`; mTLS with FIPS crypto at both ends; the caller identified
  by name and date of birth, then the code. Built: `ehr/` for Phase 1 — the contract (eight
  calls, `NextVisit` for S5), verification, the gate, the fake scheduler, the PHI marks, mTLS
  on a free port, 29 tests (EHR-SERVICE.md §6a). `agent/` — the Flow, the typed harness, the
  GA-only LLM client with the replay cache, the PHI-free logs, 15 tests (AGENT.md). A scripted
  call books a visit end to end. **Desk test, 2026-11-11:** both of the owner's typed calls
  crashed after `identify` (stale thinking blocks, AGENT.md §3). Fixed, and each typed call now
  writes a dev call record (AGENT.md §1). From the test, built the same day: the code speaks
  each fixed line word for word (fork 1A), an impossible date of birth costs no try, a name
  rule in the prompt, and "today" set per call (AGENT.md §3). **Second desk test, 2026-11-11:**
  no crash; he stopped at the code step because the harness did not show the fake code. Built
  the same day: the harness prints the test data (the code is `246810`); `end_call` is global,
  so a caller can leave at any step without a hand-off; fork C, a prompt rule plus a check that
  flags the model's words before a closing tool (AGENT.md §3). **Third desk test, 2026-11-11:**
  an emergency during a booking lost the booking; now an `emergency` tool speaks a fixed 911
  line and the call goes on (option A; FUTURE-SETTINGS.md §1 holds the hospital settings and
  the emergency flag). Then the prompt rules moved into code where they could (1A, 2A): a
  fixed medical-question line, an emergency phrase screen on every caller line, a name guard,
  and an invented-value check (AGENT.md §3; FUTURE-SETTINGS.md §2, §3). Then the read-back before
  the lookup (option A), and every prompt word moved to `agent/prompts/AGENT-PROMPT.md` for the
  owner to edit. 43 agent tests. The owner closed the text run here: what is left is polish.
  The call timeline is now the Chat page's Timeline panel (CHAT.md); his audio cues wait for the voice
  work (BUILD-ORDER.md §3.3).
- **Next sitting (the owner, 2026-10-21): start with a full rundown** before building more:
  every library we use and how we use it (Rust: tonic, prost, rustls/AWS-LC, tokio, rcgen,
  zeroize, getrandom; Python: Pipecat 1.16 and its Flows, the Anthropic SDK, grpcio, protobuf,
  loguru, uv, ruff, pytest), what exists in each that we do not use yet, the levers each
  gives (useful or not, e.g. Pipecat's evals and simulated callers, its observers, turn
  strategies, context strategies, Anthropic effort and caching), and links to each one's docs.

### The conversion (CONVERSION.md §5)
- **Step 4, the docs.** Done: DESIGN, HANDOFF, CLAUDE.md, SECURITY, SECURITY-BRIEF, EPIC,
  DEBUGGING, a header on CORE-CRATE (its sections change with their code), the archive rule,
  and a draft of `ops/PROTOCOLS.md`. Open: LESSONS.md's patient-app lessons.
- **Epic backend registration** (EPIC.md §2, step E-1): done 2026-11-11.
- **Step 5, Backend Services against the Epic sandbox,** is now the Epic sign-in on the chart
  track (BUILD-ORDER.md §3.2). **Built 2026-11-12** (EPIC.md §5.1): `ehr epic-check` signs in
  and reads `metadata` and one `Patient`.
- **Held back from the archive**, each moving with the work that replaces it (CONVERSION.md
  §5.1): `core/src/store/`, `payload.rs` (the pass redesign); `auth.rs`, `sync.rs`, `dev.rs`,
  `build.rs`, `epic-client.example.json` (Backend Services). The front end moved on
  2026-11-11 (CONSOLE.md §4).

### His to design or decide
- **PROTOCOLS.md** (a draft, §10 there): branch names, who merges, issues or HANDOFF,
  an agents file at the root.
- **SECURITY.md's drafts** (§7): the roles, the retention values, the list of verification
  questions that reveal nothing, how a hospital approves our emergency support access.
- **The console** (DESIGN.md §2 C): what each page shows (the views of Analytics, Teams,
  Config, Security, Audit Logs and Settings are all empty), staff sign-in. The look is decided
  (DESIGN.md §2 D); the 16 px radius is a trial for the owner to keep or end (CONSOLE.md §3).
  The 2026-11-13 pages' desk test is open (CONSOLE.md §6).
- **The repo layout** (CONVERSION.md §6, a draft).
- **The agent's words, name and voice** (DESIGN.md §2 B).
- **Transcript retention**, 30 days by default (a draft value, VOICE-AGENT.md §8.7).
- **ERRORS.md to the archive?** Every source of it is archived (docs:check 6 lists it). It stays
  in `architecture/` while the console has no error event of its own, because the pattern
  carries. His call when to move it.

### To find out
- Epic's sandbox: which `$find` inputs does it need, and does `$book` work? It serves STU3
  `$find` (400 to a bare request). `ehr epic-find` answers it (EPIC.md §5.2).
- **Epic's docs over MCP:** the repo's `.mcp.json` adds `open-epic-mcp`
  (`https://mcp.open.epic.com`): search and read every open.epic spec, tutorial and FAQ. No
  sign-in; never put protected data in a query.
- Which refill and message actions Epic exposes to a backend app (VOICE-AGENT.md §4).
- Cartesia's multilingual speech-in for Spanish (VOICE-AGENT.md §8.6).
- Each vendor's BAA and zero-retention terms, in writing (VOICE-AGENT.md §8.2).
- **Before the first real hospital** (SECURITY.md §6): a written risk analysis, named security
  and privacy officers, written policies, workforce training for everyone with access.
- The cost per call minute (DESIGN.md §2 I).

### Tooling to bring across from DeetsMusic
- **Unit tests on pure logic**: `tests/*.test.ts` on Node's own runner, no test dependency.

## State of play (2026-11-11)
- **The pivot.** DeetsHealth is now the hospital phone agent (DESIGN.md). Every fork for the
  product so far is closed (VOICE-AGENT.md §7).
- **Conversion steps 1–3 are done** (CONVERSION.md §5.1): the Tauri host and the patient-only
  scripts are in `archive/`; the workspace builds `core` only; the archived docs are in
  `docs/archive/` with status `archived`.
- **The way back is pushed:** the tag `patient-app-final` (`ee35b9d`) and the branch
  `archive/patient-app`. They also hold the record view (`core/src/tree/`, GLOBALS.md's
  build), which `archive/` here does not (CONVERSION.md §5.1).
- **Built and kept:** the core crate's FHIR client, normalizer, sealing and `Shielded<T>`
  (CORE-CRATE.md).
- **The phone agent's text run is built and closed (2026-11-11):** `agent/` and `ehr/`, a typed
  call that verifies and books against fakes (AGENT.md, EHR-SERVICE.md). No voice, no Epic
  reads, no store.
- **The console's skeleton is built (2026-11-11), desk test open:** `console/`, a browser page
  in DeetsMotion's look: the top bar and its Theme menu, the nav (Chat, Settings), Chat's
  stage and three-panel rail, all empty; installable as a web app (CONSOLE.md). The old
  front-end toolkit (six themes, five skins, the card engine, the primitives) is in
  `archive/src/` (CONSOLE.md §4).

## Known gotchas
- **A sign-in made from Claude's shell is not the owner's** (2026-11-14). Claude Code's Bash
  runs in a sandbox with its own `AppData`. `cf`'s OAuth token (made there on 2026-11-13 for
  the tunnel) was in the sandbox only, so `npm run dev` in his terminal had no sign-in and the
  demo stayed off (error 1033 at the address). Fix: `cf auth login` in his own terminal. Test
  what his shell sees in a Terminal panel tab, not in Bash. `~/.cloudflared/<id>.json` and
  `.env` were on the real disk. ~/.claude/CLAUDE.md has the rule for every project.
- **Epic's sandbox goes down** (2026-11-14: 503 `OltpServerUnavailable` on every FHIR request).
  An Epic test call then ends "the EHR did not answer"; `ehr` says so in the terminal and signs
  in again every 30 s (CHAT.md §13). Check with `deetshealth-ehr.exe epic-check`. Use Local
  meanwhile.
- **`cf` signed out crashes on exit** (`cf` 1.0.0-beta.10, Node 24): it prints "No
  authentication token found", then a libuv assertion, exit 0xC0000409. `dev.mjs` reads the
  message and says to run `cf auth login`.
- **A cloud session cannot push a tag** (git proxy, HTTP 403, 2026-10-19). Branches push.
- **`../DeetsMusic` in a cloud session.** The Tauri DeetsMusic is the repo
  `deets-137/DeetsMusicWindows` (public). `deets-137/DeetsMusic` is an older Swift app. Clone
  DeetsMusicWindows and link it as `../DeetsMusic`, or the docs check fails the README link.
- **Epic's sandbox data is small and tidy.** Real site data is not.
- **Windows holds `catalog.db` open** (2026-11-12): `Catalog.build()` deletes the file, which
  fails with WinError 32 while any process has it open. A leftover agent from a closed terminal
  tab does that (stop the run with Ctrl+C, not by closing the tab). The test
  `test_annotations_survive_a_rebuild_and_the_latest_rating_wins` failed for the same reason
  until 2026-11-12: it now lets go of the first catalog first (`Catalog.close`). The leftover
  process is still a cause.
- **The agent's tests: when to run them, and what to watch** (the owner, 2026-11-12, after a run
  hung for 10 minutes):
  - **After a prompt or Flow edit, record first, then test.** An edit to `AGENT-PROMPT.md` or
    `flow.py` changes the model's requests, so their recordings miss. Record the kept scripts
    that the edit touches (`harness/typed_call.py --no-record --script …`, about a tenth of a
    cent each), then run `pytest`. A run before the recording can only fail.
  - **Run only the scope that changed.** `npm run check` picks it. Use
    `uv run pytest tests/test_<file>.py` while you work, and the whole suite once before a
    commit. The whole agent suite takes about 40 s (100 tests, 2026-11-12). Over 2 minutes
    means a hang: stop it and find why. Do not wait.
  - **A missing recording fails at once** (`CallFailed: ReplayMiss`). Before 2026-11-12 it
    left the call's pipeline running, and pytest hung at the end of the run. `TypedCall.hang_up`
    now cancels a failed call, and `test_typed_call.run` hangs up in `finally`.
  - **The check builds its own ehr** in `target/check` (2026-11-12): a running dev server holds
    `target/debug/deetshealth-ehr.exe`, and Windows will not replace a running .exe. The first
    check after a clean takes a few minutes to build it.
- **Size each check to the change** (the owner, 2026-11-13: "feels like a small change since the
  last test"). While working:
  - a console CSS or page tweak: `npx tsc -p console --noEmit` only;
  - one agent module: `ruff` and its one test file;
  - Rust: `cargo test -p deetshealth-ehr --lib <module>`, and `npm run db:test` for a store
    change;
  - docs only: `npm run docs:check`.

  The full `npm run check` runs once: before a commit or push, or at the end of a round that
  touched the proto, the store schema or more than one side. Not after each small fix in a
  feedback loop.
- **Windows only: `npm run dev` locks `target/debug/deetshealth-ehr.exe`,** so the full check (its
  `ehr` tests) and the pre-push hook fail with "Access is denied (os error 5)". Ask the owner to
  stop the dev server, then run the check. Never build in another `CARGO_TARGET_DIR` to get
  around it, and never start or stop his dev server unless he says so. macOS does not lock a
  running program, so this does not happen there.
- **Casey Morgan never verifies, on purpose.** The fakes hold Casey Morgan and Casey Morgen on
  one date of birth, so no code goes out. Use Jordan Ellis, 1984-03-12, code `246810`.
- **Several Deets* apps run at once on the owner's PC.** Never hard-code a dev port; Vite takes
  the next free one from 1420.
- **The wall in the engine** (SECURITY.md §4): hover hints are written hints only; the context
  menu logs a row's place, never its label; diag logs an error's kind, never its message. Keep
  all of them in the console.

## Key decisions (the "why")
- **The phone agent** (2026-10-19): the owner's call; the research is VOICE-AGENT.md §3–§5.
- **Every EHR from the start**: the Epic code exists, and the wedge is non-Epic hospitals
  (VOICE-AGENT.md §7).
- **A2 deploy, swappable to B**: one way to patch, the isolation a security review asks for
  (VOICE-AGENT.md §7.2).
- **Pipecat, our own loop on Media Streams**: Twilio ConversationRelay has no Cartesia, and the
  caller notices turn-taking first (VOICE-AGENT.md §8.6).
- **Postgres with our sealing**: it runs under every large cloud's BAA, so the deploy can move
  (VOICE-AGENT.md §8.7).
- **The record lives one call long**: no chart cache to protect (VOICE-AGENT.md §8.1).
- **The typed, Epic-free call is built before the Epic sign-in** (2026-10-20): the patient
  feels the call's shape first, text proves the Flow for Claude tokens only, and the Epic
  registration was not in yet (BUILD-ORDER.md §4).
- **The console starts; each next step follows as we go** (2026-11-11): the owner leads the
  console first, so its uses, features and settings grow one at a time (BUILD-ORDER.md §4).

## File map
- `core/` — the core crate (CORE-CRATE.md): `shielded.rs` (the wrapper and its passes),
  `net.rs` (HTTP to the EHR, `parse_reply`, the call ledger), `fhir.rs` (capability, scopes,
  one page of a search), `index.rs` (the normalizer: index columns and links), `seal.rs` (the
  sealed format), `platform.rs` (the `Cipher` trait), `error.rs`. Held back until replaced:
  `auth.rs`, `sync.rs`, `store/`, `payload.rs`, `dev.rs`, `build.rs`.
- `console/` — the web console (CONSOLE.md): `index.html`, `vite.config.ts`, `src/main.ts`
  (boot), `nav.ts`, `theme.ts`, `pages/` (one file per page; `subnav.ts` is the
  shared page nav), `styles/` (`palette`, `themes`, `skin`, `app`, `chat`, `subnav`), `public/` (`prepaint.js`, the web-app manifest, the icons).
- `archive/src/` — the patient app's front end and the old toolkit (CONSOLE.md §4).
  `archive/src/model/` is generated by `npm run types`.
- `ehr/jwks-worker/` — the Cloudflare Worker that serves the Epic sandbox's public key set
  (EPIC.md §2.3).
- `scripts/` — `docs-check.mjs`, `docs-move.mjs`, `tokens.mjs`, `types.mjs`, `fhir-probe.mjs`.
- `archive/` — the patient app's code (`archive/README.md`). `docs/archive/` — its docs.
