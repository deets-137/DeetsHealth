# DeetsHealth — project guide for Claude

A lightweight patient health-record viewer for Windows 11 (Tauri v2 + WebView2, vanilla TS
front end, Rust back end). A sibling of `../DeetsMusic`: copy its structure, its docs
system and its habits unless a doc here says otherwise.

## Start here
- **`docs/HANDOFF.md`** — cold start: state of play, **Open now**, gotchas.
- **`docs/WORKLOG.md`** — one dated `##` per sitting, newest first. Never correct an old entry.
- **`docs/DESIGN.md`** — product intent, the two surfaces, what differs from DeetsMusic.
- **`docs/DOCS-ORG.md`** — how the docs are organized (a short copy of DeetsMusic's rules).

## The docs
State lives in each doc, never in this file. Every doc starts with front matter:
`status` (shipped · built · designed · project · parked · idea · foundation · sop · guide),
`desk_test`, `sources`, `updated`.

**`docs/`** — `DESIGN.md` product intent · `HANDOFF.md` · `WORKLOG.md` · `DOCS-ORG.md`.

**`docs/architecture/`**
- `DATA-ARCHITECTURE.md` — SMART sign-in, the FHIR fetch, the normalized model, the cache, the PHI wall.
- `SURFACES-AND-CARDS.md` — Midi and Max, and the first cards.

**`docs/ideas/`** — not built; never tell a user the app does these.

## PHI rules (this project only — read before any code)
- **Sandbox first.** Build against Epic's sandbox test patients. A real record is a later,
  owner-approved step.
- **PHI never leaves the PHI wall** (DATA-ARCHITECTURE.md §5): not the log, not a diag
  ring, not a bug report, not an agent / MCP route, not a SQL export, not a toast, not a
  hover hint, not a crash dump.
- Tokens at rest go through DPAPI, as `friends.json` does in DeetsMusic.
- Never paste real record content into a doc, a commit or this chat.

## How to verify your work
- The owner runs the app and tests. Do not build throwaway harnesses or mock pages.
- Cheap checks worth running: `npx tsc --noEmit`, `npx vite build`, `cargo check`.

## Working style (same as DeetsMusic)
- Design on paper first. Bring the real forks, short, recommendation first. **He decides
  every fork.**
- One load-bearing feature in flight at a time.
- Everything is token-based: no raw color, px, font or motion value in a component.
- A synchronous `#[tauri::command]` runs on the UI thread: make I/O commands `async fn` +
  `spawn_blocking`.
- Do not delegate to subagents.

## How to explain things to me
ASD-STE100: short sentences, active voice, one idea per sentence. Name the exact control.
Test each option against the code before showing it.

## Repo
Local git only. **Never add a remote and never push.** A GitHub repo named DeetsHealth
already exists for something else.
Commit only when the user asks. Trailer: `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
