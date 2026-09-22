---
status: sop
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — Work log

One dated `##` per sitting, newest first. Never correct an old entry; correct HANDOFF.md.

## 2026-09-21 — the fixes after stage 6
- The owner said values can be padded, asked to fix the five findings, and asked that every
  failure and error route have an error the app can toast. He chose: Padmé with a 64-byte
  floor; a plain Observation search with the 9 categories as the fallback; automatic
  re-index by version; one `app-error` event with toasts now.
- Built: `seal.rs` format 2 (padding), schema 2 with an in-place migration, `CoreError::Version`,
  IMMEDIATE transactions and a 5 s busy wait; `net::on_site`; the Observation fallback; the
  `token-refresh` warning and `Summary.stopped`; `INDEX_VERSION` and `reindex_if_stale`;
  `src-tauri/src/errors.rs`, `fetch::check_sites`, a code on every sign-in, fetch and store
  failure; `src/app-errors.ts`. New `architecture/ERRORS.md`.
- Live on the SMART sandbox: the plain Observation search is accepted; 68 jobs and 70 calls
  (was 76 and 78); 163 resources, 223 links, 0 warnings. Core 37 tests, host 14, clean.
- Open: the toast wording (ERRORS.md §3) is a draft for the owner.

## 2026-09-21 — the architecture docs
- The owner chose a separate `STORE.md` (not a DATA-MODEL.md part) and a map for
  DATA-ARCHITECTURE.md (not a fold-in).
- New `architecture/FETCH.md`, `architecture/INDEX-AND-LINKS.md`, `architecture/STORE.md`,
  each written from the code.
- DATA-ARCHITECTURE.md cut to a map of the data's path, with its section numbers kept for
  old links. Refreshed CORE-CRATE.md (header, §3 layout redrawn from the code),
  DATA-MODEL.md (header, `Provenance`, fork 8), SECURITY.md (header, new §8.8 as built),
  SECURITY-BRIEF.md (§6 q3, §7, §8), CLAUDE.md's doc list.
- Found in the code, none fixed (HANDOFF.md › Open now 10): sealed lengths show which
  values are missing; the `next`-link check is a text prefix; Observations with no FHIR
  category are missed on the SMART sandbox too; a failed refresh after a 401 stops a job
  with no warning; `reindex` has no caller. Corrected the "missing" claim in CORE-CRATE.md
  §12 and in two comments in `core/src/store/`; the stage list in `core/src/lib.rs`.

## 2026-09-21 — the data model
- Laid out every FHIR R4 type by category, marked against Epic's 60 and US Core. US Core
  9.0.0 has 27 types (it added `FamilyMemberHistory`). Checked each US Core profile's
  Must Support flag for the encounter link.
- The owner described Epic's links: a lab result links to its ordering and its collecting
  encounter; medicines ordered outside a visit get a telephone encounter; problems and
  allergies carry an encounter only sometimes.
- The owner set the model: the encounter at the center, core pieces and encounter pieces, a
  unified results table with a kind column, joined at runtime. Weaving the record together
  is the app's differentiating feature.
- New `architecture/DATA-MODEL.md`. The owner closed forks 1–6 (1a 2a 3a 4a 5a 6a) and chose
  to grab the non-patient data that sits next to the record. It is sorted into `other`,
  `definition`, `site`, `site_warning`, `document`, `directory` and `vocab`. Open: fork 7
  (the `vocab` files) and fork 8 (which columns stay plain, at build).
- New `architecture/CORE-CRATE.md`, the paper design for step 3. The owner closed forks 1a
  2a 3a 4a 5a 6a 7a 9a 10a and named the wrapper `Shielded<T>`. He set the strictest standard
  for PHI (HHS's bar for covered entities), which brought Windows CNG and SChannel (10a) and
  the rule that no protected value is plain in SQLite. SECURITY.md §4.6 now has one key per
  site. Then 8a (`ts-rs`, new to the Deets apps) and 11a (one payload struct per card view).
  New SECURITY.md §8: every security measure in one place, to speak from.
- The owner accepted the Sites section (S1–S6 a) and the build order (CORE-CRATE.md §2).
- Epic has no central sign-in: each health system runs its own FHIR server and MyChart. Its
  Brands bundle lists 1,278 R4 endpoints; Epic asks apps to download it weekly and re-host
  it, never to query it at runtime.
- New `integrations/ENDPOINT-DIRECTORY.md`: the Brands bundle's shape (94.9 MB; 1,278 brands,
  95,645 locations, 820 endpoints; no logos). The owner chose a worker to re-host it weekly.
  Trimmed for the picker it is 1.06 MB compressed.
- Built stage 1 (CORE-CRATE.md §12): the workspace, `deetshealth-core`, `Shielded<T>` with a
  sealed two-pass design, the platform traits, `CoreError`. Added the wasm32 target to the
  toolchain. Checks: 3 tests, 3 compile-fail proofs, the wasm check, `cargo check`, clippy.
- Built stage 2: `seal.rs` (format byte; table, row and column in the associated data),
  `cng.rs` (CNG AES-256-GCM, RNG, SHA-256), `keystore.rs` (DPAPI-wrapped key per site).
  19 tests pass, including the GCM specification's vectors and a flipped-bit test per byte.
- DESIGN.md §6.1–6.3: the owner's rule that every fork weighs every platform (also in
  CLAUDE.md), and notes for macOS and iPhone. The one real limit found: a Mac app has no
  uninstaller, so uninstall cannot erase there.
- Built stage 3: the store (`core/src/store/`, rusqlite 0.32 bundled behind a `sqlite`
  feature) and `src-tauri/src/sites.rs`. Every column but row keys and table names is sealed,
  a missing value included. 27 tests pass; a byte scan of the real files finds no plaintext.
- Stage 4 fork: sign-in and the fetch must send the token, the `Patient` id and the site's
  URL back to the site. The owner chose a third pass (`SitePass`, the network module only)
  over a separate credential type. SECURITY.md §4.2, §8.2, §8.5 and SECURITY-BRIEF.md updated.
- Found that SMART's launcher refuses a standalone launch on a plain base URL; it reads its
  setup from a base64url JSON array in the URL. With the patient set and login skipped, the
  authorize step returns the code at once, which makes a browser-free live test possible.
- Built stage 4: `core/src/net.rs`, `core/src/auth.rs`, `src-tauri/src/signin.rs`,
  `browser.rs`, the link wiring in `lib.rs`; `SiteRow` gained `client_id`. New crates:
  reqwest 0.12 (`native-tls`, `gzip`), `serde_json`. 33 offline tests pass; the live sign-in
  test (PKCE, state, token trade, refresh, refused refresh) passes against the sandbox.
- New `architecture/SIGN-IN.md` (the owner asked): the flow, every constant and where to
  change it, the SMART launcher's setup array, the link on Windows, log lines, fixes.
- Committed stages 1–4 (`10f8cfb`).
- Built stage 5, the fetch: `core/src/fhir.rs`, `core/src/sync.rs`, the `Store` and `Timer`
  traits, `src-tauri/src/fetch.rs`; the fetch starts after sign-in. New crate: futures-util
  (already in the build), and serde_json's `preserve_order` (adds indexmap) so raw JSON keeps the
  site's key order. Live on the SMART sandbox: 76 jobs, 78 calls, 5–7 s; paging at 20
  a page followed every `next` link on the site; a second fetch left row counts unchanged.
- Committed stage 5 (`a8f125d`).
- Stage 6 scope: the owner chose the index and links now, and the card models later, with
  each card's design (this narrows fork 9a; CORE-CRATE.md §8).
- Built stage 6: `core/src/index.rs` (index columns, every reference with its field and
  role), link writing and resolution in the fetch, `sync::reindex` (rebuild from raw, no
  fetch). Live on the SMART sandbox: 101 resources, 122 links, 4 waiting (targets the site
  never sent). 30 core tests pass.
- Committed stage 6. Listed the architecture docs to write or refresh in HANDOFF.md › Open
  now 10 (three new docs: the fetch, the index and links, the store; and five stale
  passages). The session
  ends here; stage 7 is next.

## 2026-09-21 — the empty shell
- Fixed two old doc lines: README (SMART first, Epic on hold) and SURFACES-AND-CARDS.md §2
  (the first slice is decided). Fixed three § pointers that named list items, not headings.
- The owner closed the shell forks: DeetsMusic's Midi and Max shapes, the Patient card for
  Now Playing, the Timeline card for the Queue, a new Conditions card for Max's fourth slot,
  the cog and quick panel in the title bar, and the first look Lilac × Press / Moonlight × Glass.
- Built scaffold steps 1–2 (SURFACES-AND-CARDS.md §3): the Tauri app and the engine brought
  across from DeetsMusic, each file checked against SECURITY.md §4 on the way in. Wall changes:
  hints lost row hints, the menu log lost labels, diag logs error kinds only, the panic hook
  withholds the message, the log cuts URL query strings, the CSP allows no inline script.
- Ported the scripts: `dev-app.mjs`, `docs-check.mjs` (checks 1–4, 11), `docs-move.mjs`,
  `tokens.mjs` (made `docs/architecture/TOKENS.md`).
- Checks: `tsc`, `vite build`, `cargo check` (dev and release), 3 log tests pass,
  `docs:check` clean. One dev launch: the page reported ready, no front-end error.
- The owner desk-tested the shell: passed. Next: the core crate, then Compass.

## 2026-09-21 — security rules and the probe plan
- The owner set the priority: patient experience, then privacy and security, then the rest.
- New `architecture/SECURITY.md`: protected data = PHI + PII (any identifier), the threats,
  end-to-end encryption, the fixed rules (`textContent` only, CSP, no plugins to the front
  end, PKCE + `state`, log and panic rules, keys, dev rules), and a law summary.
- The owner closed forks: cache encryption by our own crate (wrapping audited primitives
  only — noted as a rule), the wrapper type, dynamic client registration first for Epic,
  delete on sign-out and uninstall.
- Epic is on hold: the owner cannot register on fhir.epic.com yet. SMART Health IT is the only
  live site.
- Probe plan decided: a kept `scripts/fhir-probe.mjs` and `architecture/FHIR-SHAPES.md`.
- New open fork: HANDOFF.md › Open now 7, the shape of the encryption crate.
- Built `scripts/fhir-probe.mjs` and ran it on SMART's open server: 9 resource types, 300
  each, shapes only. Output: `architecture/probes/smart-open.md` (generated).
- Epic's specification page renders with JavaScript and could not be read; its sandbox
  CapabilityStatement could, and gives 60 resource types (Epic August 2026).
- New `architecture/FHIR-SHAPES.md`: findings per resource, five normalizer rules, three
  forks (HANDOFF.md › Open now 8). Biggest surprises: lab ranges missing in about half,
  no high / low flags, no allergy reactions, no dose text, and Synthea patients carry
  social security, licence and passport numbers.
- The owner closed 7a (per-value sealing, to and from SQLite), 8.2a (the site's own range;
  filling missing ranges goes to ideas), 8.3a (build dose text; his ideas at build time).
  8.1 is re-asked with "keep" defined and a new recommendation (name, birth date, sex).
- DESIGN.md §1 now says handling messy data well is the app's point of difference.
- The owner closed 8.1b: keep name, birth date, sex; compute the rest at runtime. Listed
  SMART's Patient extension URLs and identifier types (no values) to name what is dropped.
  New open fork: which field is "sex" (FHIR-SHAPES.md §5.4).
- Sex parked with a direction: display `gender`, compute a clinical sex from several
  factors at runtime.
- Fork 6 read against DeetsMusic's imports and closed in part: 6A empty app + one-by-one
  import, 6B no tray + web-ready design (DESIGN.md §6), 6C release path scaffolded up to
  the worker. 6D (Compass) re-opened by the owner. `frames.ts` must come with the engine.
- Cactus Needle 3 read and noted in ideas/README.md.
- The owner closed the web-ready split (a; a secure cloud provider is acceptable if a web
  version needs a server) and 6D (a: Compass shell with the scaffold). Needle planned as a
  Compass layer for ambiguity. Looked up Quest and Labcorp patient APIs; noted in ideas.

## 2026-09-21 — a second sandbox site
- The owner added SMART Health IT's public sandbox as a second site in the same app.
  Recorded in HANDOFF.md › Decided and DATA-ARCHITECTURE.md §2.
- Probed the discovery endpoints (no sign-in, no patient data): Epic's and SMART's
  `smart-configuration` and `r4.smarthealthit.org/metadata` all return 200. Epic lists
  `client-public` and `permission-offline`; its `scopes_supported` has no `patient/*`
  scopes, so Epic's resource scopes come from the app registration.

## 2026-09-21 — habits brought over from DeetsMusic
- Read DeetsMusic's CLAUDE.md, LESSONS.md, TASTE.md, DOCS-ORG.md, HANDOFF.md and the
  sign-in part of DEBUGGING.md.
- CLAUDE.md: added the full working style (forks, hand-off list, one feature in flight, the
  pre-build list with a PHI rule for hints, toasts and logs), the verify notes (no harnesses,
  the MSIX registry trap, the dev-app ready line), the explain rules, Conventions, and the
  front-matter / part-marker rules.
- New `docs/LESSONS.md`: DeetsMusic's lessons cut to this project, with a fourth stop rule
  (PHI) and a PHI tie-breaker. TASTE.md is pointed to, not copied (it is local only).
- DOCS-ORG.md: the status table, part markers, the HANDOFF / WORKLOG rule.
- HANDOFF.md: three sign-in gotchas; a note on dev vs release redirect URIs under fork 5.
- Not brought over: playback, graphics measurement, release and publishing, workers, the
  agent / MCP routes (DESIGN.md §3 rules them out until the PHI wall is designed).

## 2026-09-21 — the project folder
- Made `Documents/DeetsHealth` with README, CLAUDE.md and the first docs, and a local git repo
  (no remote).
- Decided by the owner: a patient-facing app on Epic's FHIR API, sandbox patients first,
  structured as a 1-to-1 copy of DeetsMusic, with Midi and Max as the only surfaces.
- Open forks: HANDOFF.md › Open now.
- Later the same day, the owner closed forks 1–4: copy the skeleton, deep-link scheme,
  labs/meds/allergies for 1–2 sandbox patients, SQLite cache.
