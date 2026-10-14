---
status: sop
desk_test: none
sources: []
updated: 2026-10-13
---
# DeetsHealth — Handoff

> Cold-start guide — read first. This file holds what is still true. The history of each
> sitting is [WORKLOG.md](WORKLOG.md).

## State of play (2026-10-13)
- **Fresh eyes (2026-10-13).** The owner has an Epic FHIR account, a new M engine
  (`../DeetsEngine`) and a much larger DeetsMusic to model the look on. New direction:
  Decided (2026-10-13) below. The next build is the Epic sandbox sign-in. Stage 7's desk
  test is still open (committed in `e2bb0a5` as a checkpoint).
- **Next session (the owner, 2026-10-13): the app's layout.** He wants to redo much of the UI on
  the same base (the shell, tokens, skins, the card engine). Start from
  [HOME-AND-HUBS.md](features/HOME-AND-HUBS.md) (real forks H3, H5; the shelf list is a
  draft) and the fork test (LESSONS.md §5).
- **Waiting meanwhile: Epic's sandbox sync.** Run `node scripts/epic-redirect-check.mjs`. When
  it says "accepted", the owner runs the E-3 desk test (EPIC.md §5a).

## State of play (2026-09-24)
- **The empty shell is built** (scaffold steps 1–2): the Tauri app, the title bar (title menu,
  cog and quick panel, window buttons), all themes and skins, Midi and Max, the card slots,
  swap, grow, memory, the Settings card, and six empty record cards. No record data, no
  sign-in. Desk test passed 2026-09-21 ([SURFACES-AND-CARDS.md §4](architecture/SURFACES-AND-CARDS.md)).
- Git: the remote is `origin`, the private repo `deets-137/DeetsHealth` (2026-10-13). Push
  only when the owner asks. The old iOS repo of that name is now `deets-137/DeetsHealthiOS`.
- **Next session starts here** (the owner's order, 2026-09-21): scaffold step 3, the web-ready
  core crate, then step 4, the Compass shell. Design each on paper first and bring the forks.
  - The core crate (Decided › Web-ready core) holds the normalizer, the health model and the
    sealing, with no Tauri dependency. It turns FHIR JSON into the model; it is not all of
    "getting the data". Sign-in, the key store, the cache and opening a browser sit behind
    small platform interfaces. **The HTTP fetch lives in the core** (the owner, 2026-09-21):
    one fetch for desktop and web.
  - The paper design for step 3 starts with the data structure: the health model types the
    cards read (`LabResult`, `Medication`, `Allergy`, `Condition`, `Encounter`, the patient),
    the wrapper type, and the sealed cache's tables. FHIR-SHAPES.md is the input.
  - **The cache's tables are on paper** in [DATA-MODEL.md](architecture/DATA-MODEL.md):
    the encounter at the center, one link table for every reference, raw resources kept
    sealed and normalized at read time. Forks 1–6 decided 2026-09-21. Open: fork 7 (the
    bundled code-name files) and fork 8 (which columns stay plain, decided at build).
  - **Step 3 is on paper** in [CORE-CRATE.md](architecture/CORE-CRATE.md): forks 1–7, 9, 10
    decided 2026-09-21 (the end state is sign-in + fetch + sealed cache on SMART, shown in
    a Settings › Sites row; `Shielded<T>`; one key and file per site; Windows CNG and
    SChannel). Forks 8a and 11a, the Sites section (S1–S6) and the build order closed the
    same day. **Stages 1–7 built** (CORE-CRATE.md §12; sign-in, a full fetch, the index and
    the links pass live tests against the SMART sandbox; reference docs: SIGN-IN.md, FETCH.md,
    INDEX-AND-LINKS.md, STORE.md). The card models wait for each
    card's design (the owner, at stage 6). **Stage 7 (2026-09-24): `ts-rs` and Settings ›
    Sites (S1–S6), built, desk test open** (CORE-CRATE.md §12, "Stage 7 desk test"). The
    sign-out erase came with it (S5), a stage early. Before the desk test the owner starts
    `npm run dev:app` from his own terminal once, so the `deetshealth-dev://` scheme lands in
    the real registry. **Next: stage 8** (refresh on open, the weekly full refresh), after
    the desk test.
  - Step 5, the release path up to (not including) the worker, comes after these two.

## Run
```
npm install
npm run dev:app       # a free port from 1420 up; the dev identifier com.deetshealth.dev
npm run dev:fresh     # the same, as a first-time user (run it from your own terminal)
npx tsc --noEmit      # front-end type check
npm run docs:check    # links, mentions, § pointers, front matter
npm run tokens        # regenerate docs/architecture/TOKENS.md after a token change
npm run types         # regenerate src/model/ (TypeScript) from core/src/payload.rs
npm run inspect -- --shapes             # dev only: what a site's sealed store holds (ops/DEBUGGING.md §5; run it outside the Claude app)
node scripts/epic-redirect-check.mjs   # does Epic's sandbox know our redirect URI yet? (EPIC.md §3)
cargo test -p deetshealth-core --features sqlite                  # core tests + compile-fail proofs
cargo test -p deetshealth --lib                                   # host tests: CNG, DPAPI, the plaintext scan
cargo test -p deetshealth --lib -- --ignored --nocapture          # live: sign-in (~7 calls) and a full fetch (~165 calls), SMART sandbox
cargo check -p deetshealth-core --target wasm32-unknown-unknown   # core stays web-ready
```
The dev app is up when its output has `start: DeetsHealth` (read it with `grep -a`).

## Decided (the owner, 2026-09-21)
1. **Scaffold:** copy DeetsMusic's Tauri skeleton, then strip what does not apply.
2. **Sign-in return:** ~~a custom deep-link scheme (`deetshealth://callback`)~~. **Replaced
   2026-10-13 (the owner): a loopback listener, `http://127.0.0.1:<port>/callback`**
   (RFC 8252 §7.3; SIGN-IN.md §6). Epic's sign-in page blocks a custom scheme in Edge
   (EPIC.md §3).
3. **First slice:** labs, meds, allergies for one or two sandbox patients (Camila Lopez first).
4. **Cache:** SQLite on disk.
- **Two sandbox sites in one app** (added the same day): Epic's sandbox and SMART Health IT's
  public sandbox (`launch.smarthealthit.org`). Both go through the same sign-in and the same
  normalizer. SMART's sandbox needs no registration, so it is the first site sign-in can
  run against end to end.
- **Security and privacy** (same day; full text [SECURITY.md](architecture/SECURITY.md)):
  priority is patient experience, then privacy and security, then the rest. The wall covers
  PII as well as PHI. Encrypted end to end. The cache is encrypted by our own crate over
  audited primitives. A wrapper type seals every protected value at parse time. Sign-out
  and uninstall delete the data. Epic sign-in: test dynamic client registration first.
- **API probe** (same day): a kept script, `scripts/fhir-probe.mjs`, prints each resource's
  shape per site. The findings go in `architecture/FHIR-SHAPES.md`, with a full list of the
  resources Epic documents on open.epic.com.

- **Encryption crate shape** (same day, fork 7a): each protected value is sealed just before
  it is written to SQLite and unsealed just after it is read back, straight into the
  wrapper type. AES-GCM per value, with the row's key as associated data. SQLite still shows
  table names and row counts; sealed columns are sorted in Rust, not SQL. SECURITY.md §4.6.
- **Labs show the site's own reference range** (same day, 8.2a). Ranges differ between
  hospitals, so a range belongs to its result, never to a test in general. When a site gives
  no range, the card shows the value without one. How to fill a missing range is later
  design work ([ideas](ideas/README.md)).
- **From `Patient`, keep only name, birth date and sex** (same day, 8.1b). All else is
  dropped at parse. Age and the like are computed at runtime, never stored.
- **Scaffold, how** (same day, 6A a): start from an empty Tauri app and bring DeetsMusic
  files in one at a time, each read and checked against SECURITY.md §4 on the way in. The
  owner is also open to copy-then-clean, with the same care. What comes across:
  - **Engine, as is:** `vite.config.ts`, `tsconfig.json`, `scripts/dev-app.mjs`, `tokens.mjs`,
    `docs-check.mjs`, `docs-move.mjs`; `palette.css`, `themes.css`, `skin.css`, fonts;
    `theme.ts`, `skin.ts`, `pop.ts`, `toast.ts`, `hint.ts` (drop its `credits` tie), `diag.ts`,
    `frames.ts` (the card engine imports it), `dropdown`, `context-menu`, `slider`; Rust
    `lib.rs` shell, single-instance and deep-link plugins, `settings.rs`, `log.rs`,
    `watchdog.rs`, `dbhealth.rs`. `diag.ts` and `log.rs` get a line-by-line wall check.
  - **Engine, music taken out:** `layout.ts`, `surface.ts` (no Mini), `card-grow`,
    `card-swap`, `card-memory`, `collection-card.ts` (imports player, queue, pins, row-drag),
    `settings-store.ts`, `settings-card.ts`; `cards.ts` and `main.ts` rewritten.
  - **Never:** every music and social module, `bridge.rs`, `query.rs`, `report.rs`, `cli/`,
    `extension/`. `apple.rs`, `provider.rs`, `model.rs`, `library.rs` are replaced by the
    FHIR fetch, the health model and the sealed cache.
- **No tray** (same day, 6B a). The app is Windows-native for now, but is designed so it
  can become a web app later (DESIGN.md §6).
- **Release path scaffolded now** (same day, 6C): the updater plugin, signing and release
  scripts come across, and stop **before** creating or deploying any worker, so the path
  can be turned on later. Making the updater's signing key pair is asked for at that step.
- **Web-ready core** (same day, 6B follow-up): the normalizer, the health model and the
  sealing live in a Rust crate with no Tauri dependency, so it can also compile to
  WebAssembly. Platform parts (key store, sign-in return, cache, opening a browser) sit
  behind small interfaces. If a web version needs a server, a secure cloud provider (Azure,
  AWS or similar) is acceptable to the owner; that is its own fork when it comes.
- **Compass shell with the scaffold** (same day, 6D a): places, Settings rows and commands,
  rebuilt on DeetsHealth's cards (DeetsMusic's `compass.ts` imports ~35 music modules and
  cannot be copied; `compass-math.ts` comes as is). Search inside the record comes after
  the first cards. Needle 3 is planned as a later layer on Compass for ambiguous asks
  (ideas/README.md).
- **Dose text** (same day, 8.3a): built from the structured parts. The owner has ideas for
  it; bring them in at build time.
- **The shell** (same day): DeetsMusic's Midi and Max shapes, with the Patient card where Now
  Playing is and the Timeline card where the Queue is (pickable in Midi, anchored in Max).
  Max's fourth default card is a new Conditions card. The title bar has the cog and quick
  panel. First look: Lilac × Press (light), Moonlight × Glass (dark). SURFACES-AND-CARDS.md.

## Decided (the owner, 2026-10-13: "fresh eyes")
- **Scope: a true patient portal, read-only, with a model designed for writes later.** Every
  normalized type keeps what a later write needs (the site, the FHIR id, the version).
  Messages and appointment requests are later work. Each needs its own scopes and Epic's
  review.
- **Shell: Home and hubs.** Home unites the record for one patient. Its tiles drill into hub
  cards (visits, results, medicines, …), and each hub has levels in DeetsMusic's style (a
  hero, rows, Back). Paper design: [HOME-AND-HUBS.md](features/HOME-AND-HUBS.md), forks
  open.
- **DeetsEngine: the "D-A + R-A lite + L-A" set.** The sealed SQLite store stays the record
  on disk (D-A). At open, the core unseals it into an in-memory DeetsEngine tree of globals,
  which is a derived view rebuilt at each open, so it needs no migration. **R-A lite** (the
  owner, 2026-10-13, replacing R-A): values are plain strings inside the engine, wiped when
  they drop; the guards are DeetsHealth's typed wrapper at the boundary (`Shielded` in and
  out, a new pass) and redacted engine reports (today `NotUnique` carries its values,
  `DeetsEngine/src/error.rs`). R-A full (a generic value type in the engine) was too large a
  rework. The tree stays in memory while the app is open (L-A); an idle drop is a later
  Settings row. **DeetsEngine builds the base layer first, ahead of its stage 1:** a memory
  store and redacted reports, requested in `../DeetsEngine/docs/ideas/deetshealth-needs.md`.
  **DeetsHealth's own globals are designed here,** with the owner, once that exists. Until
  then the cards read the built store. The draft, from Camila's record and the owner's note
  on panels and microbiology: [GLOBALS.md](architecture/GLOBALS.md) (the owner, 2026-10-13: G1 A, one global per kind; G2 A, one value, one record).
- **Next: Epic sandbox sign-in**, before card work. The fill-in sheet and the plan:
  [EPIC.md](integrations/EPIC.md).
- **Planned, paper only: a Cloudflare identity worker** for tiered access (parent and
  child, read-only and read-write): [ideas/IDENTITY-WORKER.md](ideas/IDENTITY-WORKER.md).
- **Web:** how ready the plan is for a web copy is DESIGN.md §6.4.

## Open now
5. **Epic — back on** (2026-10-13). The registration form is filled in, and the owner has a
   non-production client ID. **E-2 is built** (the Epic sandbox in the Connect menu, from
   `epic-client.json` at build time; [EPIC.md](integrations/EPIC.md) §5a). Next: the owner
   writes `epic-client.json`, then runs the E-3 desk test (§5a), which also runs the first
   Epic fetch. The registration lists both redirect URIs.
   **E-3, first try (2026-10-13):** the sign-in and consent worked, but Epic's page could not
   hand the browser to `deetshealth-dev://` (its CSP `form-action *` blocks a custom scheme;
   EPIC.md §3). **The loopback return is built** (`loopback.rs`; the scheme, its registry
   writes and the deep-link plugin are gone; SIGN-IN.md §6), not yet desk-tested. The owner
   added `http://127.0.0.1/callback` to the registration; Epic accepted it at 17:41, but only
   with no port: **Epic matches the port exactly** (EPIC.md §3). So the app tries ports 80,
   42813, 46219 in order (`loopback::PORTS`). Port 80 works now; the owner registers the two
   fallback URIs. **E-3 passed (2026-10-13):** Camila signed in; the first Epic fetch stored 296
   rows of 12 types in 32 calls. **E-4 tools built** (the call ledger, the fetch summary,
   `npm run inspect`; ops/DEBUGGING.md). What the fetch showed, and the fixes it points to
   (CarePlan's categories, Outside Record APIs, 54 missing referenced records):
   ops/DEBUGGING.md §7. The CarePlan category fallback is built (FETCH.md §5.2); a refused
   category is "not served", not a failure, which a Refresh has not yet confirmed live. All
   merged into `main` (2026-10-13). Open from the fetch: why Outside Record data is
   suppressed although its APIs are selected; the 54 missing referenced records (stage 8).
6. **Scaffold** — steps 1–2 built and desk-tested. Steps 3–5 to come (State of play).
9. **App icon** — `app-icon.svg` is a placeholder (a plain card shape). `npx tauri icon
   app-icon.svg` makes the set; the real icon is the owner's call.
8. **Computed sex** — parked until the build. Direction set: display `gender`; calculations
   use a sex computed from several factors at runtime (FHIR-SHAPES.md §5, fork 4).
10. **The fixes after stage 6 — built, for the owner's desk test** (2026-09-21;
    CORE-CRATE.md §12 › Fixes after stage 6): padding and schema 2 (STORE.md), the
    Observation fallback and the `next`-link check (FETCH.md), automatic re-index by version
    (INDEX-AND-LINKS.md §10), and one `app-error` event with a toast for every failure
    (ERRORS.md). **Open: the toast wording in ERRORS.md §3 is a draft for the owner** (three
    more codes since stage 7). Committed in `0061887`.
11. **Stage 7 — built, for the owner's desk test** (2026-09-24). Choices made inside S1–S6,
    for the owner to confirm: the Connect pill reads **Choose**; the five counts are
    results = `Observation`, medicines = `MedicationRequest` + `MedicationStatement`,
    allergies, conditions, visits = `Encounter`; a kind with 0 is left out of the line;
    **Sign in** on a dead site renews it in place for the same patient and makes a new site
    for another patient; Sign out is refused while the site fetches; a success toast after
    Sign out. Not committed.

## Gotchas
- Epic's sandbox data is small and tidy. Real site data is not.
- A production client ID takes time to reach each health system after registration.
- Refresh-token lifetime is set per site. Plan for "sign in again".
- Several Deets* apps run at once on this PC. Never hard-code a dev port.
- **Registry writes from a Claude desktop session are not real** (DeetsMusic, 2026-09-13).
  The Claude app is an MSIX package, so its shells and any dev app they start write
  `HKCU\Software\Classes` into a private hive. Edge cannot see a scheme registered that way.
  Sign-in no longer uses a scheme (2026-10-13), so this no longer touches it. Check the real
  registry from a WMI-started process (`../DeetsMusic/docs/ops/DEBUGGING.md` §Sign-in).
- **A sign-in return must be `http` or `https`.** Epic's sign-in page sets `form-action *`,
  which blocks a redirect to any custom scheme in Chromium browsers (EPIC.md §3).
- **No sign-in inside WebView2.** The SMART sign-in opens in the system browser, as
  DeetsMusic's does. Do not try to move it into an in-app popup.
- **No inline script.** The CSP (`tauri.conf.json`) allows none, so the theme pre-paint is a
  file, `public/prepaint.js`. Keep its defaults in step with `theme.ts` and `skin.ts`.
- **The wall in the engine** (SECURITY.md §4). Hover hints are written hints only (no row
  hints); the context menu logs a row's place, never its label; diag logs an error's kind,
  never its message; the Rust panic hook withholds the message; the log cuts every URL's
  query string. Keep all five when a card fills.

## File map
- `core/` — the web-ready core crate (CORE-CRATE.md): `shielded.rs` (the wrapper and the two
  passes), `platform.rs` (Cipher, KeyStore, OpenBrowser, SiteKey), `seal.rs` (the sealed
  format), `store/` (row types; `sqlite.rs`, the sealed SQLite store, feature `sqlite`),
  `net.rs` (the third pass and the HTTP client), `auth.rs` (SMART sign-in and refresh),
  `fhir.rs` (type → table, capability, scopes, reading a page), `index.rs` (index columns and
  references from a raw resource), `sync.rs` (the fetch, link resolution, `reindex`),
  `error.rs`, and `payload.rs` (`CardPass`; the Sites payloads and the event types, with
  `ts-rs` under the `ts` feature). The workspace `Cargo.toml` is at the repo root.
- `src-tauri/src/cng.rs` (AES-256-GCM, random, SHA-256 from Windows CNG) and `keystore.rs`
  (per-site keys wrapped by DPAPI): every `unsafe` crypto call in the app. `sites.rs`: one
  store file and one key per site; create, open, list, erase; the `sites_list` and
  `site_sign_out` commands. `signin.rs`: the waiting sign-in (Connect, or a saved site's
  Sign in), the returning link, the commands; `browser.rs`: opens https:// in the system browser.
  `fetch.rs`: runs the fetch off the UI thread after sign-in; `site_refresh`; progress events;
  `check_sites` at start. `errors.rs`: the one `app-error` event (ERRORS.md); the page side
  is `src/app-errors.ts`.
- `src/main.ts` — boot and the title bar. `src/layout.ts` — the slots per surface.
  `src/surface.ts` — Midi / Max and the resize bands. `src/cards.ts` — the registry.
  `src/record-cards.ts` — the empty record cards. `src/settings-card.ts` — Settings (Sites
  first). `src/sites.ts` — the Sites state, commands and events. `src/model/` — generated
  by `npm run types`; never edit it by hand.
  `src/quick-panel.ts` — the cog. The rest of `src/` is DeetsMusic's engine, cut to fit.
- `src-tauri/src/` — `lib.rs` (setup, the scheme, commands), `log.rs` (the rolling log and its
  wall), `window.rs` (the launch show, the window position), `settings.rs`, `watchdog.rs`.
- `scripts/` — `dev-app.mjs`, `docs-check.mjs`, `docs-move.mjs`, `tokens.mjs`, and
  `fhir-probe.mjs` (prints each resource's shape, never a value; `--write` makes
  `docs/architecture/probes/<site>.md`).
