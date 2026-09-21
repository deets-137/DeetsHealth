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
- The docs exist, and one dev script: `scripts/fhir-probe.mjs`. **No app code.**
- Git is local only. There is no remote. Do not add one.
- **Next session starts here:** write the first build step as a plan on paper for the
  owner to check before any code — the empty Tauri app; the first imports, each with its
  wall check; the core crate's outline (web-ready); the Compass shell; the release path up
  to, not including, the worker. Every scaffold fork is closed (Decided below).

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

## Open now
5. **Epic — ON HOLD.** The owner cannot register on fhir.epic.com yet. All Epic work waits:
   sign-in, dynamic client registration, the Camila Lopez slice. SMART Health IT is the only
   live site until then, so the first slice (item 3) uses SMART's synthetic patients. When
   Epic comes back: a debug build may answer its own scheme (DeetsMusic:
   `deetsmusic-dev://`), so the registration may need two redirect URIs (fork 6).
6. **Scaffold** — all decided (see Decided). Next: the first build step as a plan on paper.
8. **Computed sex** — parked until the build. Direction set: display `gender`; calculations
   use a sex computed from several factors at runtime (FHIR-SHAPES.md §5.4).

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

## File map
- `scripts/fhir-probe.mjs` — reads a FHIR server and prints each resource's shape, never a
  value. `--write` makes `docs/architecture/probes/<site>.md`.
- The rest of the planned layout is in [README.md](../README.md).
