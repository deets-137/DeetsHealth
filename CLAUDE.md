# DeetsHealth — project guide for Claude

**The aim since 2026-11-22 (the owner): a portfolio piece, a plug-and-play no-code agent
harness for healthcare professionals** (`docs/DESIGN.md` §0). Its first agent is a phone agent
for a clinic's patients: it books visits and answers direct questions from the chart (Twilio,
Cartesia, Pipecat, the Anthropic API, FHIR; a Rust core; a web console where a non-coder sets it
up). Weigh each recommendation by what a healthcare professional can set up without code and
how it shows to a reviewer; the privacy rules below do not change. Being converted from a patient health-record viewer for Windows
(`docs/CONVERSION.md`). **DeetsMusic is the model for the dev process and the docs** (the owner,
2026-10-19): where this file has no rule, follow `../DeetsMusic/CLAUDE.md`.

## Start here
- **`docs/HANDOFF.md`** — cold start: run, **Open now** (everything not finished), state of
  play, gotchas, file map. Corrected in place.
- **`docs/WORKLOG.md`** — the record of each sitting, newest first. Add one dated `##` per
  sitting; never correct an old entry.
- **`docs/LESSONS.md`** — what the owner tends to want: the mission, the stop rules, the
  lessons, the tie-breakers, the fork test. It shapes the option you RECOMMEND. It never closes
  a fork. Read `../DeetsMusic/docs/TASTE.md` with it (local only, never quote it in a doc or
  commit).
- **`docs/DESIGN.md`** — product intent, the two surfaces, what is decided, the v1 scope.
- **`docs/DOCS-ORG.md`** — how the docs are organized (DeetsMusic's rules, short).

## The docs
**State lives in each doc, never in this file.** Every doc starts with front matter:
`status` (shipped · built · designed · project · parked · idea · foundation · sop · guide ·
archived), `shipped_in` (only when shipped), `desk_test`, `sources`, `updated`. A part of a doc
with its own state carries a part marker under its heading: `> **Part:** designed · 2026-09-21`.
Where a doc has an "as built" section, read it first: where it disagrees with the design
above it, it is the code. A new doc gets front matter (DOCS-ORG.md §2) and one line below.
- `npm run docs:check` after any doc edit.
- Move a doc with `node scripts/docs-move.mjs NAME=area`, never by hand. A doc no one needs
  goes to `docs/archive/` the same way (DOCS-ORG.md §5).
- Generated docs — edit the source, never the output: `architecture/TOKENS.md` by
  `npm run tokens`; `architecture/probes/<site>.md` by `node scripts/fhir-probe.mjs`.
- **Save every screenshot or sketch the owner sends, in the same sitting** (the owner,
  2026-11-12). They are the record of his decisions and of the build. On 2026-11-12 only one
  of five design screenshots was saved; four had to be dug out of the session transcripts.
  - Where: `docs/architecture/images/<page>-<what>-<YYYY-MM-DD>.png` (lowercase, hyphens; the
    date he sent it). Example: `chat-rail-alignment-2026-11-13.png`.
  - How: a pasted image is in this session's transcript
    (`~/.claude/projects/<this project>/<session id>.jsonl`) as a base64 `image` block. Decode
    it to the file. Never draw or rebuild it.
  - Link it in the doc section that the decision changes:
    an image line whose alt text reads "The owner's screenshot, <date>: <what it shows and
    asks>" and whose path is `images/<file>.png`. Add it to
    that doc's `sources`. No doc fits yet? Link it from the sitting's WORKLOG entry.
  - Name the saved files in the sitting's WORKLOG entry.
  - **Protected data never goes in.** A screenshot that shows a real patient, a real phone
    number, a key or a token is not saved; tell him why. Sandbox (synthetic) patients are
    fine.

**`docs/`** — `DESIGN.md` product intent and the v1 scope · `LESSONS.md` the owner's lessons ·
`HANDOFF.md` · `WORKLOG.md` · `DOCS-ORG.md`.
- `CONVERSION.md` — turning this repo into the phone agent: what moved to `archive/`, what
  carries forward, the steps; §5.1 is the as-built record.
- `BUILD-ORDER.md` — the phases from nothing to the first hospital, each with its desk test;
  Phase 0's checklist and the fork of what goes first.
- `PRE-LIVE-VERIFICATIONS.md` — every desk test closed on the owner's word (2026-11-22, not
  run; `desk_test: closed <date>`) and the open issues from the last runs. Each row is run
  before going live. A test closed later gets its row here.

**`docs/archive/`** — docs no longer needed, kept as a record (`status: archived`).
`PATIENT-APP-DESIGN.md` and `PATIENT-APP-HANDOFF.md` — the patient app's design and handoff.
`PATIENT-APP-IDEAS.md` — its idea list. `PATIENT-APP-SECURITY.md` and `PATIENT-APP-SECURITY-BRIEF.md` — its security design and brief.
`PATIENT-APP-EPIC.md` — its Epic registration and sandbox findings. `PATIENT-APP-DEBUGGING.md` —
its fetch ledger, inspector and the first Epic fetch. `HOME-AND-HUBS.md` — the patient portal's layout. `ENDPOINT-DIRECTORY.md` — Epic's Brands
bundle and the worker that re-hosts it. `IDENTITY-WORKER.md` — the patient-sharing worker idea.
`SURFACES-AND-CARDS.md` — the patient app's card engine and shell. `GLOBALS.md` — the patient
app's chart globals in DeetsEngine (archived 2026-11-12: the phone agent keeps no chart).

**`archive/`** (repo root) — the patient app's code, not built, not checked
(`archive/README.md`): the Tauri host, its scripts, its front end and the old toolkit
(`archive/src/`: six themes, five skins, the card engine, the primitives). The whole app is
the branch `archive/patient-app`.

**`console/`** — the web console (`architecture/CONSOLE.md`).

**`docs/architecture/`** — the engine
- `VOICE-AGENT.md` — the phone agent: the research (Epic's Emmie, the vendors, FHIR booking per
  EHR), every fork and its answer (§7), the stack (§8: the call path, BAAs, the model, the call
  loop, the store), the test tooling (§9).
- `CORE-CRATE.md` — the core crate's modules, `Shielded<T>`, sealing and keys, sign-in, the
  fetch, as the patient app built them (the code is still in the tree). Its header says what
  changes; each section is rewritten in the same commit as its code.
- `DATA-MODEL.md` — the normalized model, the encounter at the center, the link table, where
  every FHIR type goes (its cache tables are the patient app's).
- `INDEX-AND-LINKS.md` — the table of each type, the `kind` words, the code field, dates, code
  systems, every link role.
- `FHIR-SHAPES.md` — what each FHIR resource looks like at each site, from the probe; Epic's 60
  resource types; the normalizer rules that follow.
- `probes/<site>.md` — generated by `node scripts/fhir-probe.mjs --site <site> --write`. Edit
  the script, never the output. The probe prints shapes only, never a value.
- `SECURITY.md` — the phone agent's: the priority order, protected data, the threats, the
  rules (end to end, the wrapper type, the console, caller verification, logs, keys,
  retention, the model, staff access), vendors and BAAs, the law. S1 and S2 decided.
- `AGENT.md` — the agent (`agent/`) as built: run it, the parts, the GA-only LLM client and the
  replay cache, the decisions made while building, the known gaps.
- **`agent/prompts/AGENT-PROMPT.md`** — every word of the agent: the role prompt, each step's
  task, each tool's description, every fixed line. The owner edits it by hand or in Config ›
  Agent (a save writes the file); never put prompt words in `flow.py`. After an edit, record
  the kept scripts again: the Test button on Config › Agent (AGENT.md §3; CONSOLE.md §9).
- `EHR-SERVICE.md` — the Rust `ehr` service and its gRPC wire to the agent: the handle, the
  calls, the PHI tag in the contract, the edge, mTLS, tracing and tests.
- `CALL-FLOW.md` — the default call flow as a map into the code: each step, fixed line, limit
  and code-held rule, by file and symbol. Never copies a word or value (DOCS-ORG.md §1, rule 8).
- `DEV-API.md` — the one reference for the dev API: every route between the console and the
  agent's dev server (the route ledger), the wall, the annotation body and gate, the watch
  stream, the caller's seat, and a map to every other API. `docs:check` 12–14 keep it, the RPC
  ledger in EHR-SERVICE.md §3 and the code in step; a new route or RPC needs its row.
- `DATABASE.md` — the dev database shaped like the production store: native Postgres that
  `npm run dev` starts (`scripts/postgres.mjs`), the writer rule, the Chat page's contract, the
  draft schema, the work to be demo ready in the cloud, synthetic and real rows kept apart
  (schemas, roles, keys, id prefixes), the kept scripts' runs, and the annotations in the store.
- `FLAGS.md` — staff flags: the agent marks a call for a nurse, silent to the caller, with the
  model's summary and the caller's own words as evidence (checked by code). Settings:
  FUTURE-SETTINGS.md §5.
- `REGULATIONS.md` — the outside rules behind SECURITY.md: what HIPAA, the other federal
  rules, state laws and ISO ask of the agent, how to verify a patient, the call's disclosures,
  test mode's full logs, and the open guardrails (§10).
- `SECURITY-BRIEF.md` — the one-pager for an outside security engineer: the stack, the threats,
  the plan, the idea to poke at (the model never chooses a patient), the questions.
- `ERRORS.md` — the one `app-error` event and its codes (the patient app's; the pattern
  carries).
- `FUTURE-SETTINGS.md` — what each hospital will configure (what follows the emergency line, the
  medical-question line, the emergency phrase list), each with its default now. Not built.
- `CONSOLE.md` — the web console as built: where it runs (a browser; installable as a web
  app), the shell in DeetsMotion's look (top bar, nav, pages, Chat's stage and rail), the
  themes and the Glass skin, what moved to the archive, the desk test; Config › Voice (§7, was Config › Cartesia)
  and Config › Anthropic (§8), their settings store and shared frame (`config-kit.ts`); the
  speed rules for every page and the recipe for a slow one (§12).
- `CHAT.md` — the Chat page (built, wired to the agent's dev server): the owner's sketch (`images/`), the canvas, the
  bubbles and their line facts, the PHI / Test / Demo toggle, the text box, the connection, the
  catalog of calls, the rail's sandbox patients and the code button (§8); its forks.
- `MOBILE-UI.md` — the console on a phone: how a page knows it is narrow (width, pointer,
  hover; no user-agent checks), the width tokens, what is built page by page, what is open.
  Add a row and a log line with each piece built.
- `TABLES.md` — the console's one table (`data-table.ts`, `data-table.css`), as `tab-bar.ts` is
  for tab bars: column kinds as width tokens, one cell box, words under the headers, ↑ ↓ ✕ at
  the right edge, Add on the title line, the row motion, Read as words. Every table uses it.
  §6: long lists windowed (`list-window.ts`, above 200 rows) and the search box with Ctrl+F.
- `LOADING.md` — the console's reads: one store per kind of data (the newest answer wins, writes
  in click order, polls one at a time, retry when the agent is back), the ledger of every read
  for debugging and health later, and the first-load wave (the owner's fork A: hold, then one
  wave top left to bottom right).
- `ANALYTICS.md` — the Analytics page: the owner's sketch, its own nav inside the panel (Home,
  Outcome, Cost, Logs, Speed, Flags), the reader each view is for, the shared filter bar; Logs
  (§3.1) and the views on the sums (§3.2: ehr's `CallStats`, our own charts, Flags' words with
  PHI View) built.
- `TEXTS.md` — the code by text: the random code, Twilio's Messages request from `ehr`, the
  settings, `TextPass`, the fake Twilio in the dev server, the sandbox fixture, the tests, the
  Phase 5 steps.
- `PROMPT-EDITOR.md` — editing the agent's words in the console (built as Config › Agent,
  CONSOLE.md §9): how a save reaches every call that starts after it (versions, a call keeps
  its own copy, the file written back), Record and test, and the analyst's test path later.
- `AGENT-PAGE.md` — Config › Agent arranged by the call (built; `agent/prompts/GUIDE.md` holds
  each section's tab, name and "plays when"): the tabs Greet … Failure,
  each line under the tool that speaks it, Semantic for the new analyst and Flow for the power
  user, every section's "plays when".
- `SYSTEM-PAGE.md` — Config › System: the System variables (the clinic's, and the
  analyst's own), the code values, the tools; the `{` and " ." lists on Config › Agent; the
  checks; the rules engine idea.
- `ACCESS-ROLES.md` — access by role (built 2026-11-18, desk test passed): the owner's levers
  and role templates, his forks, the rules (`ehr/src/access.rs`), the audit trail's design
  (§5.1), each route's lever, and what was decided inside each of the three hand-overs. The
  owner designs the roles; a new route or RPC names its lever there.
- `AUDIT.md` — the staff audit trail and the Audit Logs page (built 2026-11-18, desk test
  passed): what a row holds and never holds, every action word and what writes it (a new
  change or look adds its row there), the hash chain and its check, reading it, what is open
  (the page's look; retention; export).
- `HOSPITAL-CONFIG.md` — Config › Hospital (built): the hospital's own lists and numbers
  (visit types, groups, reasons as codes, the time zone, times of day, hours, closed days,
  locations, specialties, how slots are offered and visits found), one hospital for every EHR
  source, a copy per call; which lists no call reads yet.
- `BOOKING-RULES.md` — booking by place, specialty and rule (designed): the place and the
  specialty in `find_slots`, the default place and the PCP, sooner with another clinician, new
  or established, the Booking rules table (the rules engine's start), what Epic gives on
  referrals, the where-to-go tool, and the build in steps.
- `VOICE-VENDORS.md` — the `vendors` crate (built 2026-11-23): a vendor for each direction of the
  voice (steps 1–5) and the model vendors (§10: the model catalog, its rules and cost): the
  `vendors` crate holds the catalog of vendors and their levers, a Python adapter per vendor,
  one voice version, the voice's cost as crate functions (§9), Config › Voice; ElevenLabs as the
  second vendor (not built).
- `TOKENS.md` — every token (generated by `npm run tokens`; never edit it by hand).
- `DEPENDENCIES.md` — every crate and package used directly: its source, the locked and latest
  versions, the major updates not taken, the trust review, how to refresh and audit. A commit
  that adds, removes or moves a dependency to a new major version changes its row.
- The patient app's sign-in, fetch, store and data map, kept until their code moves to the
  archive: `SIGN-IN.md`, `FETCH.md`, `STORE.md`, `DATA-ARCHITECTURE.md`.

**`docs/integrations/`** — outside sources
- `EPIC.md` — the backend app registration sheet (Backend Services, a JWKS URL, the APIs to
  pick), what the sandbox can prove and Epic's rules for it, what a hospital must do (a demo
  in its own system too), the plan E-1 to E-4 with the as-built sign-in and Epic mode, and
  every request we send to Epic (§6).
- One doc per vendor, each with what we use, the BAA, configuration, cost, what users learned,
  and what to find out: `TWILIO.md` (the phone line, Media Streams, `clear` and `mark`),
  `CARTESIA.md` (Ink and Sonic, ZDR, the pronunciation dictionary; ElevenLabs as the fallback),
  `ANTHROPIC.md` (HIPAA readiness, the models and prices, every lever and its setting, caching for voice), `PIPECAT.md` (the
  call loop; known issues to test first), `CLOUD.md` (what a deploy needs; C1 decided: AWS `us-east-1`),
  `NON-EPIC-EHRS.md` (the shape for every EHR: one standard FHIR core with a variant per EHR,
  `Source` as EHR and environment, the SMART sandbox; then Oracle Health, athenahealth, MEDITECH).

**`docs/ops/`** — build, ship, diagnose
- `DEBUGGING.md` — how a call is read after it goes wrong: the call log, the call timeline, the
  EHR ledger, record and replay, recipes. Not built.
- `DEMO.md` — the demo over a Cloudflare Tunnel at `deetshealth.deets.solutions/demo/`:
  Access in front, the agent's second door (the Access token), the built console with live
  reload, several calls at once, TURN for voice and its cost, the guest list from the console.
- `PROTOCOLS.md` — a draft: how the owner, helpers and their agents work here (the rules that
  never bend, joining and leaving, branches and PRs, the review checklist, agents, vendors,
  incidents).

**`docs/ideas/`** — not built; never tell a user the app does these.
- `README.md` — the short list of ideas without a doc of their own.
- `CONSOLE-MCP.md` — a CLI and a remote MCP server on the console's staff API: one API, three
  clients, the same roles and audit; protected data off unless a hospital's BAA covers the
  client; setting changes as drafts.
  Custom-named roles built from levers; a list of the levers seen so far.
- `STAFF-SIGN-IN.md` — staff sign-in through the hospital's IdP (Entra ID, Okta): what HIPAA,
  the Joint Commission, ISO 27001, SOC 2, HITRUST and NIST ask of staff access; the modules
  (sign-in, roles, provisioning, audit, emergency access, reviews, change control); the work
  with a hospital's security team; F1 and F2 decided (step-up and reason for access, both
  hospital settings).
- `LOCAL-EHR.md` — a local FHIR server in Epic's shapes, backed by a database we shape and
  reset, so Local mode runs the real Epic adapter (option A; left for later).

## Privacy rules (this project only — read before any code)
The full rules are SECURITY.md (the phone agent's, 2026-10-19). In short:
- **Priority: patient experience first, privacy and security second, everything else after.**
- **Protected data = PHI + PII.** Any identifier counts, even with no health fact: a name, an
  MRN, the `Patient` id, **the caller's phone number, the call's audio, the transcript**
  (SECURITY.md §2).
- **Synthetic first.** Build against the SMART and Epic sandboxes and simulated callers. A real
  record or a real patient's call is a later, owner-approved step, after every vendor's BAA is
  signed. Dev accounts with Cartesia, Anthropic and Twilio get synthetic data only.
- **Protected data never leaves the wall** (SECURITY.md §4): not the log, not a diag ring, not
  a trace or a metric, not a panic message, not a bug report, not an agent / MCP route, not a
  SQL export, not a toast, not a hover hint, not a crash dump, **not a vendor without a signed
  BAA**.
- **Encrypted end to end:** TLS in transit; sealed at rest, field by field, under the
  hospital's own key; the wrapper type in memory. Plaintext only where a person must hear or
  read it, or where a BAA covers it: the call's audio, the console's transcript view (each view
  audited), the request to the Anthropic API.
- **A chart answer needs two factors** (SECURITY.md §4.4): something the caller knows (date of
  birth or a random chart fact whose question reveals nothing) and something they have (a
  code by text, or a call back to the number on file). **Tools are bound to the verified
  patient in Rust; the model never names a patient** (SECURITY.md §4.9).
- **The record lives one call long.** No chart cache. The store holds settings, the call log,
  transcripts, the audit trail and, when a rule turns it on, audio (VOICE-AGENT.md §8.7).
- Crypto: our own crate may wrap audited primitives; it never holds our own cipher.
- Never paste real record content into a doc, a commit or this chat.

## How to verify your work
- **The owner runs it and tests your changes** and gives feedback. Do NOT build throwaway test
  harnesses, mock pages or one-off tooling to verify UI behavior. Make the change, check that
  it compiles, then hand it over with the desk test written into the doc.
- **Kept test tooling is not a harness** (LESSONS.md §3.11): the layers in VOICE-AGENT.md §9
  (record and replay, the text call harness, simulated callers, the fake Twilio) are built to
  stay. Use them once they exist.
- **`npm run check`** after a change: it runs the checks the changed files need (HANDOFF.md ›
  Run it). `npm run check:all` runs everything. Also `npm run types` after a change to
  `core/src/payload.rs`.
- If something cannot be reasoned through and the owner is away, ask him to test.

## Working style (the owner directs the architecture)
- For non-trivial features, **design on paper first**, bring the real forks (he responds
  well to multiple choice), confirm, then build.
- **He decides every fork.** A default, a wording, a placement, a motion value, a schema
  shape, a scope list, an architecture: bring him the fork before you build it. Short
  options, the real ones only, your recommendation first. Reason: in DeetsMusic a badge he
  never saw shipped to live users (2026-09-18).
  - `docs/LESSONS.md` and TASTE are how you pick what to RECOMMEND, not permission to skip
    the question.
  - Inside a fork he HAS chosen, build the full, polished thing without asking again.
  - At hand-off, list what you decided inside his choice, so nothing ships unseen.
  - **Run the fork test before you show a fork** (`docs/LESSONS.md` §5, the owner,
    2026-10-13). The patient is the most important user. Hold every option to maximum
    security that still gives a great experience. Hard or complex work is never a cost. Drop
    every option that a rule, his own words, the code or that standard rules out. One option
    left = not a fork: state it in one line. Lists are drafts, and later choices wait for
    their work. Open with the count: "N real forks; M things follow".
    On 2026-10-13, 8 of 12 forks failed this test.
  - **Push back** where a choice costs more than it gives (the owner's own ask).
- **Every fork weighs every place it runs** (was "every platform", 2026-10-19): the call (any
  phone), the console (a web browser), the deploy targets (A2 in our cloud; B in the hospital's
  own cloud account), and each EHR (Epic first, then others). Each option names what it costs,
  or rules out, in each. Deploy facts go behind the deploy interface; EHR facts behind the EHR
  adapter.
- **One load-bearing feature in flight.** Hand it over, he desk-tests it, then the next one
  starts. Two small features at once is fine; two that both touch the schema, the PHI wall
  or a shared primitive is not.
- **Everything in the console is token-based**: no raw color, px, font or motion value in a
  component. Color → theme role; geometry, type, spacing, motion → skin token.
- **Before you build a new console panel, row, button or card**, walk DeetsMusic's list (its
  CLAUDE.md › Working style) with one change for this project: hints, toasts and log lines
  **may never carry protected data (PHI or PII)**. A hint or toast names a kind of thing
  ("3 calls handed off"), never a value from a record or a call. Then walk the speed rules
  (`architecture/CONSOLE.md` §12): plan for the longest list a real hospital has, window a list
  past 200 rows, give it a search box, never read layout in a write loop, and run key-press work
  once a frame.
- He values polish and good stewardship: keep calls to each health system's FHIR server
  low, and quote a cost before a fetch-heavy or minute-heavy approach (FHIR calls, Twilio,
  Cartesia and Claude minutes).
- **Do not delegate to subagents** (the `Agent` tool). Explore, read and edit yourself.
  Only exception: he asks for one.

## How to explain things to me
- Write in ASD-STE100 (Simplified Technical English): short sentences, active voice, one
  idea per sentence, plain approved words, no metaphor.
- Name the exact control or gesture. Define the terms once, at the start.
- Test each option against the code BEFORE you show it to me. Discard the options that the
  code already rules out. Show the real forks only.
- A question about the console needs the front-end state, not only the Rust.

## Run
See HANDOFF.md › Run it. Nothing runs end to end yet; the checks do.

## Conventions
- The console only ever sees the normalized model (`LabResult`, `Medication`, …), never raw
  FHIR. Normalization lives in Rust.
- **The EHR adapters, sealing and the store live in Rust.** The Pipecat call loop (Python)
  calls the Rust service for every chart read, booking and write; it never opens the database
  (VOICE-AGENT.md §8.7).
- Never hard-code a dev-server port. Vite takes the next free one from 1420.
- **One Rust build folder: `target/`** (the owner, 2026-11-16). Every cargo command builds into
  `target/` (`.cargo/config.toml`). Never pass `--target-dir`. Never set `CARGO_TARGET_DIR`.
  Never build Rust in a scratchpad or a temp folder. Reason: each side folder is a full build of
  4–16 GB. On 2026-11-16, ten of them filled C: (85 GB in six days, the third time).
  - The dev server does not lock `target/debug`: it runs a copy of ehr from `target/run/`. If a
    build still fails with "Access is denied (os error 5)", an old ehr is still running. Ask the
    owner to stop it. Do not build around it.
  - If you make another build folder anyway, delete it before your turn ends, and say so in the
    hand-off. `npm run check` names any stray folder in `target/`.

## Repo
The remote is `origin`: the **public** GitHub repo `deets-137/DeetsHealth` (public since
2026-11-22, on the owner's word after a scan of the whole history for secrets; private
before). Everything pushed is public: never commit a key, a token, a real record or a local-only
file (TASTE.md). The old iOS app (Oura) that had this name is now
`deets-137/DeetsHealthiOS`.
Commit and push only when the user asks. Never add another remote and never force-push.
Co-author trailer — name the model that did the work
(today `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`); update this line when the
model changes.
