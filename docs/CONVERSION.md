---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# The conversion — from the patient app to the hospital phone agent

> Paper only: no file has moved. The owner (2026-10-19): "plan out the conversion, what
> existing files should be moved into archive/ … keep UI pieces for the management interface
> and the API calls / learnings to carry forward". The product is
> [ideas/VOICE-AGENT.md](ideas/VOICE-AGENT.md).

## 1. Terms
- **Archive** — `archive/` at the repo root. It holds the patient app's files at their old
  paths (`src-tauri/x.rs` → `archive/src-tauri/x.rs`), so links between archived files still
  work. It is not built and not checked.
- **Carry** — a file that stays in the tree and is used by the new product, as is or changed.
- **Console** — the web app hospital staff use to set up and watch the agent (F2 A2: a web
  page, not a Windows app).
- **The tag** — `patient-app-final`, a git tag on the last commit of the patient app.

## 2. Two ways back
1. **The tag** is the real way back: `git checkout patient-app-final` gives the patient app
   exactly as it was, and it builds.
2. **`archive/`** is for reading and for copying a piece back. Its files do not build where
   they sit (the workspace no longer lists them).

## 3. What moves to archive/
### 3.1 The Windows host — all of `src-tauri/`
The Tauri app, CNG and DPAPI keys (`cng.rs`, `keystore.rs`), the loopback sign-in return
(`loopback.rs`), the site list (`sites.rs`), the fetch commands (`fetch.rs`), the window, the
watchdog, the log (`log.rs`), the xlsx export, the icons, `tauri.conf.json`, `capabilities/`,
`gen/`, `examples/inspect.rs`. Nothing in the new product runs a Tauri window.

### 3.2 The patient half of the core
| File | Why it goes | What carries forward |
|---|---|---|
| `core/src/auth.rs` | Patient sign-in: standalone launch, PKCE, a person in a browser. | The `smart-configuration` read; the token refresh rules. The new sign-in is SMART Backend Services (a signed JWT, no person). |
| `core/src/sync.rs` | Fetches **everything** for one patient, with three workers. A call fetches only what it needs. | Retries, stops and warning codes (FETCH.md), copied into the new per-call reads. |
| `core/src/store/` | One sealed SQLite file per site, on a PC. | The rule "every value sealed on its own, bound to its row and column" (§4). |
| `core/src/dev.rs` | The inspector reads the SQLite store. | Its rule: shapes for any site, values for a sandbox only. |
| `core/src/payload.rs` | Card payloads for the patient's cards. | The pattern: one module builds what the front end sees, `ts-rs` writes the TS types. |
| `core/build.rs` | Reads `epic-client.json`, the patient client id. | — |

### 3.3 Front-end files that only the patient app needs
`src/main.ts` (Tauri entry, window buttons), `src/boot-cover.ts` (hidden-window launch cover),
`src/browser-defaults.ts` (turns off browser behavior a web page should keep), `src/frames.ts`
and `src/telemetry-on.ts` (dev frame telemetry through Rust), `src/sites.ts` (the patient's
sites), `src/record-cards.ts` (the patient's cards), `src/model/` (generated from
`payload.rs`).

### 3.4 Scripts and config
`scripts/dev-app.mjs` (launches the Tauri dev app), `scripts/epic-redirect-check.mjs` (the
patient redirect), `epic-client.example.json`, `app-icon.svg`.

### 3.5 Docs that describe archived code or the patient product
`docs/DESIGN.md`, `architecture/DATA-ARCHITECTURE.md`, `SIGN-IN.md`, `FETCH.md`, `STORE.md`,
`SECURITY.md`, `SECURITY-BRIEF.md`, `CORE-CRATE.md`, `features/HOME-AND-HUBS.md`,
`integrations/ENDPOINT-DIRECTORY.md`, `integrations/EPIC.md`, `ops/DEBUGGING.md`,
`ideas/IDENTITY-WORKER.md`, and the patient items in `ideas/README.md`.
`DESIGN.md`, `SECURITY.md`, `SECURITY-BRIEF.md`, `CORE-CRATE.md`, `EPIC.md` and
`DEBUGGING.md` are then **written again** for the new product; the old text stays in the
archive, so a rewrite never loses a learning.

## 4. What carries forward
### 4.1 The core (Rust) — the API calls and the PHI wall
| File | Use in the new product | Change later |
|---|---|---|
| `shielded.rs` | The wrapper type, from the EHR reply to the spoken answer. | The three passes become: `SitePass` (kept), a pass for the agent's tool replies, a pass for the sealed store. |
| `net.rs` | HTTP to the EHR, `parse_reply`, the call ledger. | Backend Services tokens. |
| `fhir.rs` | `CapabilityStatement`, scopes, one page of a search. | System scopes (`system/*.read`), not patient scopes. |
| `index.rs` | The normalizer: one raw resource → the columns and links. | It feeds the agent's tools, not a cache. |
| `seal.rs` | Field-level sealing with associated data. Server data (transcripts, audit, settings) is sealed the same way. | The `Cipher` behind it becomes the cloud KMS, with the hospital's own key (F2 A2, BYOK). |
| `platform.rs` | The `Cipher` trait. | Filled by KMS, not CNG. The deploy-target interface (F2) joins it. |
| `error.rs`, `lib.rs` | Errors as kinds only; the module list. | `lib.rs` drops the archived modules. |

### 4.2 The front end — the console's toolkit
Stays and moves into the console: the primitives (`toast`, `hint`, `dropdown`, `slider`,
`context-menu`, `pop`), themes and skins (`theme`, `skin`, `skin-settings`, `appearance`,
`ambient`, `storm`, `styles.css`, `styles/` with the fonts, `public/prepaint.js`), settings
(`settings-store`, `settings-card`, `quick-panel`), the card engine (`cards`, `layout`,
`layout-bus`, `card-grow`, `card-swap`, `card-memory`, `surface`), `diag`, `app-errors`,
`index.html`, `vite.config.ts`, `tsconfig.json`. Each Tauri call in them (`ambient`, `diag`,
`app-errors`, `surface`, `theme`) is swapped for its web form when the console is built; the
files keep their names and history.

### 4.3 Scripts
`docs-check.mjs`, `docs-move.mjs`, `tokens.mjs`, `types.mjs`, `fhir-probe.mjs` (it probes a new
EHR's shapes, values never printed).

### 4.4 Docs
`LESSONS.md`, `DOCS-ORG.md`, `WORKLOG.md`, `HANDOFF.md` (rewritten in place),
`FHIR-SHAPES.md` and `probes/`, `INDEX-AND-LINKS.md`, `DATA-MODEL.md` (the normalized model
and the links; the cache-table part becomes history), `ERRORS.md` (the pattern; the codes
change), `TOKENS.md`, `SURFACES-AND-CARDS.md` (the card engine is kept), `GLOBALS.md` (it waits
for the server store choice), `ideas/VOICE-AGENT.md` (moves to `docs/` as the design doc).

## 5. The steps, in order
Each step is one commit; the tree builds after each one.
1. **Merge `docs/voice-agent-idea` into `main`.** Then tag the last patient-app commit
   `patient-app-final` and push the tag.
2. **Move only.** `git mv` every file in §3 to `archive/` at its old path. No edits, so git
   keeps each file's history. Add `archive/README.md`: what it is, the tag, "not built".
3. **Make the rest build.** Workspace members `["core"]`; drop the `sqlite`, `dev` features and
   the archived modules from `core`; `package.json` loses `@tauri-apps/*`, `tauri`, `dev:*`
   and `inspect`; a plain web entry replaces `main.ts`; `docs-check` skips `archive/`.
   Checks: `cargo check`, `npx tsc --noEmit`, `npx vite build`, `npm run docs:check`.
4. **The docs.** New `DESIGN.md`, `SECURITY.md`, `CLAUDE.md` and `HANDOFF.md` for the hospital
   product, each with forks first; `VOICE-AGENT.md` to `docs/` with `docs-move`.
5. **The first feature** (one load-bearing feature in flight): SMART Backend Services against
   the Epic sandbox, with `net.rs` and `fhir.rs`. Epic needs a backend app registration.

## 6. The new layout (a draft for the owner to edit)
```
core/       Rust: the kept core (§4.1) — the FHIR client, normalizer, sealing, Shielded<T>
ehr/        Rust: a small internal service over core; the agent's tools call it (new)
agent/      Python: the Pipecat call loop and Flows (new)
console/    TypeScript: the web console, from today's src/, index.html, vite (§4.2)
deploy/     the deploy-target interface: A2 first, B later (new)
archive/    the patient app (§3)
docs/       as today
scripts/    as today, less §3.4
```

## 7. Open
- **The product name and the repo name.** DeetsHealth stays, or a new name? It does not block
  steps 1–3.
- **The server store** (transcripts, audit, settings): DeetsEngine or a database such as
  Postgres. It decides `GLOBALS.md`. It waits for its work.
- In flight on the patient app, closed by the conversion: the E-3 desk test (Epic patient
  sign-in) and the layout work in HOME-AND-HUBS.md.
