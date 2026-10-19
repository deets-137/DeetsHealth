---
status: sop
desk_test: none
sources: []
updated: 2026-10-19
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
### The conversion (CONVERSION.md §5)
- **Push the tag** `patient-app-final` from the owner's machine. This session's git proxy
  refused it (HTTP 403); the branch `archive/patient-app` stands in (CONVERSION.md §5.1).
- **Step 4, the docs.** Done: DESIGN, HANDOFF, CLAUDE.md, SECURITY, SECURITY-BRIEF, EPIC,
  DEBUGGING, a header on CORE-CRATE (its sections change with their code), the archive rule,
  and a draft of `ops/PROTOCOLS.md`. Open: LESSONS.md's patient-app lessons.
- **Epic backend registration** (EPIC.md §2, step E-1): the owner fills in the sheet.
- **Step 5, the first feature:** SMART Backend Services against the Epic sandbox. Epic needs a
  backend app registration.
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
