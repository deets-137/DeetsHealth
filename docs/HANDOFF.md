---
status: sop
desk_test: none
sources: []
updated: 2026-10-20
---
# DeetsHealth — Handoff

> Cold-start guide — read first. This file holds what is still true: run, **Open now**, state of
> play, gotchas, decisions, file map. Corrected in place. The record of each sitting is
> [WORKLOG.md](WORKLOG.md). The product is [DESIGN.md](DESIGN.md). The patient app's handoff
> is [archive/PATIENT-APP-HANDOFF.md](archive/PATIENT-APP-HANDOFF.md).

## Run it
There is no app to run yet: the conversion is in progress ([CONVERSION.md](CONVERSION.md)).
What runs today:
```
npm install
npx tsc --noEmit      # front-end type check (it compiles; it does not run in a browser yet)
npx vite build        # front-end bundle
npm run docs:check    # links, mentions, § pointers, front matter, the archive rule
npm run tokens        # regenerate docs/architecture/TOKENS.md after a token change
npm run types         # regenerate src/model/ (TypeScript) from core/src/payload.rs
cargo check --workspace --all-features
cargo test -p deetshealth-core --features sqlite                  # core tests + compile-fail proofs
cargo check -p deetshealth-core --target wasm32-unknown-unknown   # core stays web-ready
```
The build folder is `target/` at the repo root (it was `src-tauri/target/`).

## Ship it
Nothing ships yet. The deploy is A2: a separate deploy per hospital in our cloud, behind an
interface (VOICE-AGENT.md §7.2).

## Open now
### The build (BUILD-ORDER.md)
The product is built in six phases, each ending in a desk test (BUILD-ORDER.md §2):
0. **Paper** — the cost quote, the repo layout, the Epic registration, the dev accounts,
   `npm run check`. *In flight.*
1. **The text slice** — the call's shape with no audio and no Epic: the Pipecat Flow (greet,
   verify, menu, schedule, hand off), the verify gate, a fake in-memory scheduler, fake
   patients, the typed harness, the PHI-free log processor. Desk test: type a call, book a visit.
2. **The EHR** — Backend Services sign-in to Epic's sandbox, the chart reads through record and
   replay, the Rust `ehr` service. Desk test: the typed call answers "when is my next visit?"
3. **Voice** — the fake Twilio, Cartesia in and out, barge-in, fillers, the laptop microphone;
   measure latency and pick the model. Desk test: speak and book a visit.
4. **The store and the console** — Postgres with sealing, the call log, transcripts, the audit
   trail; then the console once the owner designs it.
5. **The real network** — a Twilio number, booking on Epic's sandbox, results with guards, refills.
6. **Ship** — the deploy interface, the BAAs, the compliance paperwork, the first hospital.

- **Decided 2026-10-20: the text slice (Phase 1) goes first; Backend Services (Phase 2) follows
  when the Epic registration lands.** The owner starts Phase 1 on 2026-10-21. Phase 0's paper
  items run beside it; none of them blocks it except an Anthropic dev key.
- **Phase 0, open:** the cost per call minute (Claude); the repo layout, a draft for the owner
  (CONVERSION.md §6; Phase 1 needs only `agent/`); the Epic registration E-1 (the owner, EPIC.md
  §2); dev accounts with Twilio, Cartesia and Anthropic, synthetic data only (the owner);
  `npm run check` from DeetsMusic (Claude).

### The conversion (CONVERSION.md §5)
- **Step 4, the docs.** Done: DESIGN, HANDOFF, CLAUDE.md, SECURITY, SECURITY-BRIEF, EPIC,
  DEBUGGING, a header on CORE-CRATE (its sections change with their code), the archive rule,
  and a draft of `ops/PROTOCOLS.md`. Open: LESSONS.md's patient-app lessons.
- **Epic backend registration** (EPIC.md §2, step E-1): the owner fills in the sheet.
- **Step 5, Backend Services against the Epic sandbox,** is now Phase 2 of the build: it
  follows the text slice and waits for the registration (BUILD-ORDER.md §3).
- **Held back from the archive**, each moving with the work that replaces it (CONVERSION.md
  §5.1): `core/src/store/`, `payload.rs` (the pass redesign); `auth.rs`, `sync.rs`, `dev.rs`,
  `build.rs`, `epic-client.example.json` (Backend Services); the patient-only front-end files
  (the console's design).

### His to design or decide
- **PROTOCOLS.md** (a draft, §10 there): branch names, who merges, issues or HANDOFF,
  an agents file at the root.
- **SECURITY.md's drafts** (§7): the roles, the retention values, the list of verification
  questions that reveal nothing, how a hospital approves our emergency support access.
- **The console** (DESIGN.md §2 C, D): what it shows, staff sign-in, which looks it keeps.
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
- **`npm run check`** (tsc, tests, docs:check, cargo check, cargo test) and the pre-push hook
  that runs it (`../DeetsMusic/.githooks/pre-push`).
- **Unit tests on pure logic**: `tests/*.test.ts` on Node's own runner, no test dependency.

## State of play (2026-10-19)
- **The pivot.** DeetsHealth is now the hospital phone agent (DESIGN.md). Every fork for the
  product so far is closed (VOICE-AGENT.md §7).
- **Conversion steps 1–3 are done** (CONVERSION.md §5.1): the Tauri host and the patient-only
  scripts are in `archive/`; the workspace builds `core` only; the archived docs are in
  `docs/archive/` with status `archived`.
- **The way back is pushed:** the tag `patient-app-final` (`ee35b9d`) and the branch
  `archive/patient-app`. They also hold the record view (`core/src/tree/`, GLOBALS.md's
  build), which `archive/` here does not (CONVERSION.md §5.1).
- **Built and kept:** the core crate's FHIR client, normalizer, sealing and `Shielded<T>`
  (CORE-CRATE.md), and the front-end toolkit (tokens, themes, skins, primitives, the card
  engine).
- **Nothing of the phone agent is built yet.**

## Known gotchas
- **A cloud session cannot push a tag** (git proxy, HTTP 403, 2026-10-19). Branches push.
- **`../DeetsMusic` in a cloud session.** The Tauri DeetsMusic is the repo
  `deets-137/DeetsMusicWindows` (public). `deets-137/DeetsMusic` is an older Swift app. Clone
  DeetsMusicWindows and link it as `../DeetsMusic`, or the docs check fails the README link.
- **`../DeetsEngine` is not in a cloud session**, so `GLOBALS.md`'s two sources fail the docs
  check there. They pass on the owner's machine.
- **The front end compiles but does not run in a plain browser**: it still calls Tauri until
  the console's design untangles it (CONVERSION.md §5.1).
- **Epic's sandbox data is small and tidy.** Real site data is not.
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
  registration was not in yet (BUILD-ORDER.md §3).

## File map
- `core/` — the core crate (CORE-CRATE.md): `shielded.rs` (the wrapper and its passes),
  `net.rs` (HTTP to the EHR, `parse_reply`, the call ledger), `fhir.rs` (capability, scopes,
  one page of a search), `index.rs` (the normalizer: index columns and links), `seal.rs` (the
  sealed format), `platform.rs` (the `Cipher` trait), `error.rs`. Held back until replaced:
  `auth.rs`, `sync.rs`, `store/`, `payload.rs`, `dev.rs`, `build.rs`.
- `src/` — the front-end toolkit for the console: the primitives (`toast`, `hint`,
  `dropdown`, `slider`, `context-menu`, `pop`), the looks (`theme`, `skin`, `appearance`,
  `styles/`), settings, and the card engine (`cards`, `layout`, `card-grow`, `card-swap`,
  `card-memory`, `surface`). `src/model/` is generated by `npm run types`.
- `scripts/` — `docs-check.mjs`, `docs-move.mjs`, `tokens.mjs`, `types.mjs`, `fhir-probe.mjs`.
- `archive/` — the patient app's code (`archive/README.md`). `docs/archive/` — its docs.
