---
status: sop
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — Handoff

> Cold-start guide — read first. This file holds what is still true. The history of each
> sitting is [WORKLOG.md](WORKLOG.md).

## State of play (2026-09-21)
- **The empty shell is built** (scaffold steps 1–2): the Tauri app, the title bar (title menu,
  cog and quick panel, window buttons), all themes and skins, Midi and Max, the card slots,
  swap, grow, memory, the Settings card, and six empty record cards. No record data, no
  sign-in. Desk test passed 2026-09-21 ([SURFACES-AND-CARDS.md §4](architecture/SURFACES-AND-CARDS.md)).
- Git is local only. There is no remote. Do not add one.
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
    same day. **Stages 1–5 built** (CORE-CRATE.md §12; sign-in and a full fetch pass live
    tests against the SMART sandbox). **Next: stage 6**, index columns and links for every
    type, and the normalizer for the first-slice types. Before the desk test the owner starts
    `npm run dev:app` from his own terminal once, so the `deetshealth-dev://` scheme lands in
    the real registry.
  - Step 5, the release path up to (not including) the worker, comes after these two.

## Run
```
npm install
npm run dev:app       # a free port from 1420 up; the dev identifier com.deetshealth.dev
npm run dev:fresh     # the same, as a first-time user (run it from your own terminal)
npx tsc --noEmit      # front-end type check
npm run docs:check    # links, mentions, § pointers, front matter
npm run tokens        # regenerate docs/architecture/TOKENS.md after a token change
cargo test -p deetshealth-core --features sqlite                  # core tests + compile-fail proofs
cargo test -p deetshealth --lib                                   # host tests: CNG, DPAPI, the plaintext scan
cargo test -p deetshealth --lib -- --ignored --nocapture          # live: sign-in (~7 calls) and a full fetch (~165 calls), SMART sandbox
cargo check -p deetshealth-core --target wasm32-unknown-unknown   # core stays web-ready
```
The dev app is up when its output has `start: DeetsHealth` (read it with `grep -a`).

## Decided (the owner, 2026-09-21)
1. **Scaffold:** copy DeetsMusic's Tauri skeleton, then strip what does not apply.
2. **Sign-in return:** a custom deep-link scheme (`deetshealth://callback`), for a
   distributable app. Test the scheme through the real registry (DeetsMusic's MSIX note).
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

## Open now
5. **Epic — ON HOLD.** The owner cannot register on fhir.epic.com yet. All Epic work waits:
   sign-in, dynamic client registration, the Camila Lopez slice. SMART Health IT is the only
   live site until then, so the first slice (item 3) uses SMART's synthetic patients. When
   Epic comes back: a debug build may answer its own scheme (DeetsMusic:
   `deetsmusic-dev://`), so the registration may need two redirect URIs (fork 6).
6. **Scaffold** — steps 1–2 built and desk-tested. Steps 3–5 to come (State of play).
9. **App icon** — `app-icon.svg` is a placeholder (a plain card shape). `npx tauri icon
   app-icon.svg` makes the set; the real icon is the owner's call.
8. **Computed sex** — parked until the build. Direction set: display `gender`; calculations
   use a sex computed from several factors at runtime (FHIR-SHAPES.md §5, fork 4).

## Gotchas
- Epic's sandbox data is small and tidy. Real site data is not.
- A production client ID takes time to reach each health system after registration.
- Refresh-token lifetime is set per site. Plan for "sign in again".
- Several Deets* apps run at once on this PC. Never hard-code a dev port.
- **Registry writes from a Claude desktop session are not real** (DeetsMusic, 2026-09-13).
  The Claude app is an MSIX package, so its shells and any dev app they start write
  `HKCU\Software\Classes` into a private hive. Edge cannot see a scheme registered that way.
  This hits the `deetshealth://` scheme. Check the real registry from a WMI-started process
  (`../DeetsMusic/docs/ops/DEBUGGING.md` §Sign-in), or start the dev app from the owner's
  own terminal once.
- **No sign-in inside WebView2.** The SMART sign-in opens in the system browser, as
  DeetsMusic's does. Do not try to move it into an in-app popup.
- **A `cmd /c start` of the callback link only tests Windows → app.** It cannot prove what a
  browser sees. Quote the link: `cmd` splits an unquoted `&`.
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
  `fhir.rs` (type → table, capability, scopes, reading a page), `sync.rs` (the fetch),
  `error.rs`, and the stub `payload.rs`. The workspace `Cargo.toml` is at the repo root.
- `src-tauri/src/cng.rs` (AES-256-GCM, random, SHA-256 from Windows CNG) and `keystore.rs`
  (per-site keys wrapped by DPAPI): every `unsafe` crypto call in the app. `sites.rs`: one
  store file and one key per site; create, open, list, erase. `signin.rs`: the waiting
  sign-in, the returning link, the commands; `browser.rs`: opens https:// in the system browser.
  `fetch.rs`: runs the fetch off the UI thread after sign-in; `site_refresh`; progress events.
- `src/main.ts` — boot and the title bar. `src/layout.ts` — the slots per surface.
  `src/surface.ts` — Midi / Max and the resize bands. `src/cards.ts` — the registry.
  `src/record-cards.ts` — the empty record cards. `src/settings-card.ts` — Settings.
  `src/quick-panel.ts` — the cog. The rest of `src/` is DeetsMusic's engine, cut to fit.
- `src-tauri/src/` — `lib.rs` (setup, the scheme, commands), `log.rs` (the rolling log and its
  wall), `window.rs` (the launch show, the window position), `settings.rs`, `watchdog.rs`.
- `scripts/` — `dev-app.mjs`, `docs-check.mjs`, `docs-move.mjs`, `tokens.mjs`, and
  `fhir-probe.mjs` (prints each resource's shape, never a value; `--write` makes
  `docs/architecture/probes/<site>.md`).
