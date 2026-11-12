---
status: sop
desk_test: none
sources: []
updated: 2026-11-12
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
`agent` (ruff and pytest, replay only, against a fresh `ehr` build), `keys` (the key-set Worker
build), `docs`. A change to core also runs `ehr`, and a change to `ehr` also runs `agent`.
`agent/calls/` (the dev call records) runs nothing. The pre-push hook (`.githooks/pre-push`)
runs it on the commits being pushed; `npm install` turns the hook on. To skip the hook once:
`git push --no-verify`. All 12 steps took 50 s on 2026-11-11, and the agent tests are 28 s of that.

Each check by hand:
```
npm install
npx tsc --noEmit      # front-end type check (it compiles; it does not run in a browser yet)
npx vite build        # front-end bundle
npm run docs:check    # links, mentions, § pointers, front matter, the archive rule
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
4. **The git-ignored secrets do not travel with the repo.** `.env` (the Anthropic key): make it
   again on that machine, by hand, never through a chat, a doc or a commit; replay-only tests
   and `npm run check` need no key. `epic-client.json`: only for Epic work; the build runs
   without it.
5. **The sibling repos** for the docs check: `../DeetsMusic` (clone `deets-137/DeetsMusicWindows`
   under that name) and `../DeetsEngine`, or the docs check (and so the pre-push hook) fails on
   their links (Known gotchas).
6. `npm run check:all` proves the machine. Then `npm run dev` for the console.
7. **Claude's own setup is per machine.** The owner's global `~/.claude/CLAUDE.md` (the writing
   rules, the fork test, the end-of-turn box) and this project's memory folder under
   `~/.claude/projects/` live on the PC. Copy both to the Mac, or a session there starts
   without them.

## Ship it
Nothing ships yet. The deploy is A2: a separate deploy per hospital in our cloud, behind an
interface (VOICE-AGENT.md §7.2).

## Open now
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

- **In flight: the Chat page** (branch `chat-what-we-doin`, CHAT.md). The UI passed its first
  desk test (2026-11-12). **The connection is built, desk test open** (CHAT.md §6): `npm run
  dev` starts both; Test calls the agent through client-js and our own text transport (not
  Pipecat's WebSocket transport, which loads remote code); the watch stream draws Test, PHI
  and Demo with one code path; Demo plays any call in the catalog. **The catalog** (CHAT.md
  §7): SQLite, rebuilt from the records, with labels and ratings in the committed
  `agent/calls/annotations.jsonl`. **Next, the owner's to design:** the rail panels
  (Timeline, Caller, Step, Tools), where the rating controls sit, the model and code graders.
  Then the staging deploy (2A: after this desk test; at $0, so the Docker Compose stand-in
  first, CLOUD.md). Polish from every desk test is always on (BUILD-ORDER.md §2; AGENT.md §4).
- **Also in flight: the call guardrails** (REGULATIONS.md, 2026-11-12). Built, desk test open:
  the greeting is a fixed line with the disclosures (automated, transcribed, a person at any
  time; §6), and a caller confused after a failed code goes to a person at once (§2.1; the kept
  script `code_confusion.txt`). Open, in order: REGULATIONS.md §10 (twins, limits across
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
  track (BUILD-ORDER.md §3.2).
- **Held back from the archive**, each moving with the work that replaces it (CONVERSION.md
  §5.1): `core/src/store/`, `payload.rs` (the pass redesign); `auth.rs`, `sync.rs`, `dev.rs`,
  `build.rs`, `epic-client.example.json` (Backend Services). The front end moved on
  2026-11-11 (CONSOLE.md §4).

### His to design or decide
- **PROTOCOLS.md** (a draft, §10 there): branch names, who merges, issues or HANDOFF,
  an agents file at the root.
- **SECURITY.md's drafts** (§7): the roles, the retention values, the list of verification
  questions that reveal nothing, how a hospital approves our emergency support access.
- **The console** (DESIGN.md §2 C): what Chat and Settings show, staff sign-in. The look is
  decided (DESIGN.md §2 D).
- **The repo layout** (CONVERSION.md §6, a draft).
- **The agent's words, name and voice** (DESIGN.md §2 B).
- **Transcript retention**, 30 days by default (a draft value, VOICE-AGENT.md §8.7).

### To find out
- Epic's sandbox: does it serve `Appointment/$find` and `$book`? (VOICE-AGENT.md §8.4)
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
- **A cloud session cannot push a tag** (git proxy, HTTP 403, 2026-10-19). Branches push.
- **`../DeetsMusic` in a cloud session.** The Tauri DeetsMusic is the repo
  `deets-137/DeetsMusicWindows` (public). `deets-137/DeetsMusic` is an older Swift app. Clone
  DeetsMusicWindows and link it as `../DeetsMusic`, or the docs check fails the README link.
- **Epic's sandbox data is small and tidy.** Real site data is not.
- **Windows holds `catalog.db` open** (2026-11-12): `Catalog.build()` deletes the file, which
  fails with WinError 32 while any process has it open. A leftover agent from a closed terminal
  tab does that (stop the run with Ctrl+C, not by closing the tab), and
  `test_annotations_survive_a_rebuild_and_the_latest_rating_wins` fails on Windows for the same
  reason. Not fixed.
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
  (boot), `nav.ts`, `theme.ts`, `pages/` (one file per page), `styles/` (`palette`,
  `themes`, `skin`, `app`), `public/` (`prepaint.js`, the web-app manifest, the icons).
- `archive/src/` — the patient app's front end and the old toolkit (CONSOLE.md §4).
  `archive/src/model/` is generated by `npm run types`.
- `ehr/jwks-worker/` — the Cloudflare Worker that serves the Epic sandbox's public key set
  (EPIC.md §2.3).
- `scripts/` — `docs-check.mjs`, `docs-move.mjs`, `tokens.mjs`, `types.mjs`, `fhir-probe.mjs`.
- `archive/` — the patient app's code (`archive/README.md`). `docs/archive/` — its docs.
