---
status: sop
desk_test: none
sources: []
updated: 2026-11-11
---
# DeetsHealth — Work log

One dated `##` per sitting, newest first. Never correct an old entry; correct HANDOFF.md.

## 2026-11-11 (later) — the second desk test: the code step, ending a call
- The owner's second typed call ran with no crash. He stopped at the code step: no text is
  sent, and the harness did not say the fake code. The code step was wired; `ehr` accepts
  `246810`. The harness now prints the fake callers and the code when a call starts.
- From the call logs, the agent missed three chances to end a call. "End call" at the code
  step was handed off to staff. "Goodbye" at the greeting got a goodbye in the model's words,
  and the call stayed open. "Or I'm done" got an unasked goodbye in the model's words. Fixed:
  `end_call` is global, and the prompt says to use it for every goodbye (AGENT.md §3).
  Two kept scripts and tests cover it: `leave_at_greet`, `leave_at_code`.
- The model's own words before a tool's fixed line, seen as "...Goodbye. Of course. I'm
  connecting you with a member of our staff now." The owner chose C: a prompt rule, plus a
  check in code that notes it in the call record and fails the tests (AGENT.md §3). Holding
  every reply (A) was dropped: 0.4 to 0.7 seconds on each reply. The check catches the reply
  from his call when run over its recording.
- The owner offered short audio cues for call start, hand-off, goodbye and a hold sound for
  long waits. Not built; it waits for Phase 3 (voice).
- Spend: under a cent to record the three scripts twice. 24 agent tests pass.

## 2026-11-11 — the first desk test; the thinking-block crash; the dev call record
- The owner ran two typed calls: one genuine, one to break the agent. Both crashed right after
  `identify`. In the break call the agent stayed in scope, would not name its technology, and
  asked again about a birth year in the future.
- The cause, reproduced live: Haiku thought before calling `identify`; the next request, under
  the verify node's tools, sent that thinking block back, and the API refused it (400, the
  block is bound to another tool list). Pipecat's LLM service stopped and the harness timed out
  after 60 seconds. A scripted run passed only because Haiku did not think there. Fixed in our
  client (stale blocks dropped; the beta header fix is ruled out by the GA rule), with a test.
  A failed request now ends the call at once.
- The owner asked for a log of both sides and any telemetry to trace errors. Built the dev
  call record (AGENT.md §1): one JSON Lines file per typed call in `agent/calls/`, git-ignored,
  synthetic only, and `harness/show_call.py` to read it. The full text of library errors goes
  there, not to the console.
- Found while testing (AGENT.md §4): the agent rewrites `say` after `identify`; an impossible
  date of birth uses a try; the model fills a missing last name and once spoke of a
  "placeholder".
- The owner chose fork 1A: the code speaks each fixed line word for word, not the model.
  Built with `TTSSpeakFrame` (the voice path's own frame); every fixed line now ends in a
  question so the model does not speak after it (it repeated one in a test run). Also built:
  an impossible date of birth refused by the Flow and by `ehr`'s edge before a try counts; the
  name rule in the prompt; "today" set per call, pinned to the fakes' day in typed calls.
  The booking script was recorded again (the prompt changed).
- The owner asked whether to rebuild Pipecat in Rust. Answer: no, not now (the reasons are in
  this sitting's chat; the measure that would reopen it is Phase 3's latency per step).
- Spend: about two cents on Haiku in all. 18 agent tests and 32 `ehr` tests pass. The new
  recordings in `tests/replays/` are not committed; the older ones no longer match the prompt.

## 2026-10-21 — Phase 1 begins: the ehr service and its contract
- Late on 2026-10-20 the owner decided Phase 1's model: Claude through a replay cache, not a
  local model first. The API budget is the Max plan's $100 a month of credit, a hard ceiling;
  every test starts on the cheapest model (Haiku) and moves up only when a test shows it must.
  The key is in the git-ignored `.env`; `.claude/settings.json` denies Claude any read of it.
- The owner approved the repo layout (P0.2): each folder is born with its phase. He chose a
  Rust `ehr` service from the first call over Python fakes, so the patient binding is never
  retrofitted, and gRPC over HTTP with JSON as its wire (deadlines and cancels reach Rust; the
  PHI tag lives in the contract). New: [architecture/EHR-SERVICE.md](architecture/EHR-SERVICE.md).
- Installed `uv` 0.13.4 with winget. Built `ehr/`: the contract (four calls), the `(phi)` and
  `(secret)` marks with redacted debug prints made by `build.rs`, the edge, the sessions, the
  fake scheduler. Added `WirePass`, the fourth pass, to core. 15 tests pass; core's 42 still do.
- Two forks found and brought to the owner: the TLS library (the FIPS standard against what
  `tonic` and `grpcio` ship) and how the caller is identified before the two factors. He
  chose FIPS crypto at both ends (the standard still holds) and name and date of birth, then
  the code (not the calling number first). Recorded in EHR-SERVICE.md §6 and SECURITY.md §4.4.
- Built the rest of `ehr/` for Phase 1: `Identify`, `ResendCode`, `CheckCode` with the limits
  of SECURITY.md §4.4; `Shielded::same_name` in core (one slip in a name of 5 letters or more);
  four fake patients and the fixed code `246810`; booking in its own task so a dropped request
  cannot cut it off; `rustls` on AWS-LC, TLS 1.3, mTLS, a `fips` feature and a refusal to start
  in a deploy without it; the dev certificate example; a transport test against the real
  server. 26 `ehr` tests and 43 core tests pass; clippy is clean.
- Committed (`56fb877`). Built `agent/` (AGENT.md): Pipecat 1.13 with its built-in Flows (the
  separate package stopped at Pipecat 1.4); a client that drops Pipecat's beta flag and calls
  the GA endpoint, with the replay cache; the Flow (greet, verify, menu, schedule, goodbye,
  hand off); the gRPC client with mTLS and deadlines; the Python `Shielded` and the log
  processor that withholds all library log text; the typed harness, which starts `ehr` on a
  free port. Added `NextVisit` to the contract for S5 (29 `ehr` tests).
- The first live typed call on Haiku 5.5: 15 requests, well under one cent; it booked a visit
  and read back the next one. Replayed, it costs nothing. 15 agent tests pass, replay only.
  Found: the model does not always read the `say` template as written (AGENT.md §4).

## 2026-10-20 — the build order
- The owner asked for the build process and where to start. Wrote [BUILD-ORDER.md](BUILD-ORDER.md):
  six phases, each with a desk test; Phase 0 (paper) is in flight; one fork open, what goes
  first while the Epic registration waits (§3 there). Pointed to from CLAUDE.md and HANDOFF.md.
- The owner decided the fork: the typed, Epic-free call (Phase 1, the text slice) is built
  first; the Epic Backend Services sign-in (Phase 2) follows once the registration lands.
  Phase 1 starts 2026-10-21. HANDOFF.md › Open now carries the six phases.
- His working rule, recorded: always say what an option is when it comes up again; never a
  bare letter.

## 2026-10-19 — the tag pushed, from the owner's machine
- The owner's local `main` held one unpushed commit, the record view (`05e7584`). By his
  choice it went to `archive/patient-app` (`ee35b9d`, cargo check passes), not to the
  conversion. Local `main` reset to `origin/main`.
- Tagged `patient-app-final` on `ee35b9d` and pushed the branch and the tag. Corrected
  HANDOFF.md and CONVERSION.md §5.1.

## 2026-10-19 — the pivot to a hospital phone agent; conversion steps 1–3
- The owner asked for a voice-agent idea doc, then chose the phone agent run by hospitals:
  [ideas/VOICE-AGENT.md](architecture/VOICE-AGENT.md). Researched Epic's Emmie and voice roadmap
  (incoming voice AI in 2027), the vendors' BAAs, FHIR booking per EHR, Twilio
  ConversationRelay versus Media Streams, and Pipecat.
- Decided: phone agent; convert this repo; wedge W1 and W3 but every EHR from the start;
  A2 deploy behind an interface; inbound first; results on the phone with guards; Cartesia;
  Pipecat with Flows; Postgres with our sealing; no call audio by default; sandboxes with
  record and replay, no local FHIR server for now.
- Conversion steps 1–3 ([CONVERSION.md](CONVERSION.md) §5.1). The tag push was refused by
  the session's git proxy (403); the branch `archive/patient-app` stands in for it.
- Step 4, first pass, with DeetsMusic as the model (`deets-137/DeetsMusicWindows`; the public
  `deets-137/DeetsMusic` is an older Swift app): DESIGN.md, HANDOFF.md and CLAUDE.md rewritten
  in its shape; its archive rule ported.
- SECURITY.md rewritten for the phone agent (the old one archived): S1 A, the hospital's own
  single sign-on; S2 A, two factors for any chart answer, configurable above a floor, with a
  random chart fact as an option for the first (its question may never reveal the chart).
- EPIC.md (the backend app registration), DEBUGGING.md and SECURITY-BRIEF.md rewritten for the
  phone agent, the old ones archived as `PATIENT-APP-*`; a header on CORE-CRATE.md; a draft
  `ops/PROTOCOLS.md` for helpers and their agents (the owner's ask).
- One doc per vendor in `integrations/` (the owner's ask): Twilio, Cartesia, Anthropic,
  Pipecat, the cloud (fork C1 open) and the EHRs after Epic, from the day's research plus a
  search of users' lessons (Pipecat's GitHub issues on Twilio barge-in and 8 kHz pacing; Twilio's
  latency guide; Cartesia's pronunciation dictionary).

## 2026-10-13 — the globals draft, from Camila's record
- The owner asked whether Camila's exported record shows the shape of the globals to tune
  DeetsEngine towards. Answer: it shows the axes, not the proportions (one synthetic
  patient; 248 of 297 rows are vitals on one visit, and that visit was not sent).
- He asked for a draft doc, and added: prepare for panels of 8 to 15 values (lipid, CMP) and
  microbiology with more. New `architecture/GLOBALS.md`: one value, one record in `^RES`,
  with `parent`, `root` and `seq`; stubs for targets that did not arrive; a concept key and a
  date key; forks G1 (how to split the globals) and G2 (components as records or a subfile).
  He chose G1 A (one global per kind) and G2 A (one value, one record, components included).
- Questions for him in GLOBALS.md §8 (how Epic sends a culture; LOINC on lab components).
  Next data step: an Epic sandbox patient with a CMP, a lipid panel and a culture.
- No code changed.
- The repo has a remote now. The owner had the old public `deets-137/DeetsHealth` (an iOS
  Oura app) renamed to `DeetsHealthiOS`, and a new **private** `deets-137/DeetsHealth`
  created as `origin`; `main` pushed. The history was checked first: no secret or record
  file was ever committed. CLAUDE.md › Repo, HANDOFF.md and LESSONS.md §2 updated.

## 2026-10-13 — fresh eyes: a true patient portal
- The owner has a fhir.epic.com account, a new M engine (`../DeetsEngine`), and a larger
  DeetsMusic to model the look on. He asked to pick up with fresh eyes.
- He chose: read-only with a model designed for writes; Home and hub cards with levels; the
  engine set D-A + R-A + L-A (the sealed SQLite store on disk, an in-memory globals tree
  rebuilt at open, a protected mode in the engine, the tree kept while the app is open); Epic
  sandbox sign-in next. All in HANDOFF.md › Decided (2026-10-13).
- Found in the code: DeetsEngine's `NotUnique` error carries its values, and its index
  subscripts are plain in its SQLite keys, so the engine cannot hold protected data on disk.
  This is why the record stays in the sealed store and the engine runs in memory. The
  collection-card engine and Compass never came across to DeetsHealth. The core's `Cipher`
  trait is sync, but WebCrypto is async.
- New docs: `integrations/EPIC.md` (the sheet he fills in, the plan, forks E0–E3),
  `features/HOME-AND-HUBS.md` (the layout; after review, real forks H3 and H5), `ideas/IDENTITY-WORKER.md` (tiered
  access over a Cloudflare worker; after review, real forks W2 and W3). New PATIENT-APP-DESIGN.md §6.4 (web readiness).
- No code changed. Stage 7's desk test is still open, and its work is still not committed.
- The owner asked how many of the 12 layout and worker forks were real. 4 were. He asked for
  the evaluation style in writing: new LESSONS.md §5 (the fork test) and two tie-breaker rows,
  and a pointer in CLAUDE.md › Working style. PATIENT-APP-EPIC.md §2 now follows the real "Create an App"
  form, with a new fork E0 (Automatic Client Distribution).
- The fork test went into the global `~/.claude/CLAUDE.md` (Bringing forks), with a rule that a
  Claude-written or old rule may be wrong, so ask when it matters. A one-line pointer is in
  LESSONS.md §5.2.
- Incoming APIs: every R4 read and search (PATIENT-APP-EPIC.md §2, row 2). Dynamic clients: the JWT bearer
  grant (new PATIENT-APP-EPIC.md §6). SECURITY.md §5 corrected: its "refresh tokens" words were Claude's,
  and wrong for this path.
- The owner filled in the Epic form and has a non-production client ID. Built E-2:
  `core/build.rs` (reads `epic-client.json` at build time; serde_json as a build dependency,
  already in the tree), `epic-client.example.json`, `SiteEntry.scopes`, `SCOPES_V2`,
  `EPIC_SANDBOX`, the Epic sandbox at index 1 of the Connect menu, and the test
  `epic_entry_follows_the_build`. E1–E3 failed the fork test, so each follows (PATIENT-APP-EPIC.md §5).
- Checks: core 41 tests + 3 proofs (the new test also with a dummy file, no file and a bad
  file), wasm check, host `cargo check`. No live run: the owner's own terminal is needed for
  the scheme.
- The owner chose USCDI v3 (E0), and the work was committed (`e2bb0a5`). First Epic sign-in as
  Camila Lopez: the sign-in and consent worked, then the page stalled. The console showed the
  cause: Epic's CSP `form-action *` blocks the redirect to a custom scheme (PATIENT-APP-EPIC.md §3).
- The owner chose a loopback return over the scheme (Decided 2 replaced). Built `loopback.rs`
  (127.0.0.1, a port per sign-in, the `Host` check, `303 /done`, four fixed pages); `on_link`
  returns an outcome; the scheme, its registry writes and `tauri-plugin-deep-link` are gone;
  `link-register-failed` became `sign-in-port`. Checks: host 16 tests (2 new), `cargo check`,
  `tsc`. The owner added `http://127.0.0.1/callback` to the registration; at the first check
  Epic still answered `error=4` for it (not synced yet). A background probe waits for it.
- Committed `1ccf20e`. Added `scripts/epic-redirect-check.mjs` (kept). Epic accepted
  `http://127.0.0.1/callback` at 17:41 (about 25 minutes after the save), but rejected every
  other port: it matches the port exactly. `loopback::PORTS` = 80, 42813, 46219, the first
  free one wins; the script checks each. Host tests 18 (3 new). The dev app rebuilt at 17:43.
- Committed `400b5b0`. **E-3 passed:** Camila signed in through the loopback return; the fetch
  stored 296 rows of 12 types, 32 calls, 8 s, 76 warnings, 592 waiting links, CarePlan refused.
- The owner asked for DeetsMusic-grade telemetry and confirmed SECURITY.md §4.8 (sandbox data
  may be shown). Built: the call ledger (`sync::CallNote`, `fetch.rs` `call_line`),
  `fhir::issue_word` (FHIR's closed lists; digits-only detail codes), the fetch summary
  (`Summary.waiting_by_type`, `warnings_by_code`), the inspector (`core/src/dev.rs` with
  `DevPass`, feature `dev`; `src-tauri/src/dev_tools.rs`, `examples/inspect.rs`,
  `npm run inspect`). New `ops/DEBUGGING.md`. Checks: core 43 tests (2 new) + 3 proofs with
  `dev`; the inspector run on Camila's store through WMI.
- What it showed (ops/DEBUGGING.md §7): CarePlan needs one of ten categories; 17 types withhold
  Outside Record data from this client; SmartData Elements not authorized; the 592 waiting
  links name 54 distinct records.
- Committed `f42d691`. The owner: the Outside Record APIs are selected, so their suppression
  has another cause (open). Built the CarePlan category fallback (`fhir::CAREPLAN_CATEGORIES`,
  `category_fallback`; FETCH.md §5.2), one new test; not yet run against Epic (a Refresh runs it
  and shows the call ledger). Not committed. The owner asked to pause and re-align the build path.
- The owner ran a Refresh: the call ledger works (42 calls, 6 s, 300 resources, 13 types).
  CarePlan: 2 of 10 categories accepted, 8 refused `fatal:invalid`, which marked CarePlan
  incomplete and toasted. Fixed: a refused fallback category is "not served" (`Job.is_fallback`,
  outcome `category-not-served`), and the refused list no longer repeats a type. Core 44 tests.
  Not run live yet. Committed and merged into `main`; the owner opens a branch for the layout.
- DeetsEngine checked: it builds natively and for wasm; its `Store` trait is about 10 methods;
  its API takes and returns plain strings. The owner chose **R-A lite** over R-A (a generic
  value type would rework `globals.rs`, the indexes, packing and the journal), and the memory
  store and redacted reports **ahead of the engine's stage 1**. Wrote the request:
  `../DeetsEngine/docs/ideas/deetshealth-needs.md`, with a pointer at the top of its
  HANDOFF.md › Open now. DeetsHealth's own globals are designed here afterwards.
- Committed both repositories (DeetsEngine `adea158`, DeetsHealth `dfacc33`). The owner asked
  for Camila's mock record as CSV to model the layout on. Built `dev::export_csv` (sandbox
  only, enforced in the core) and `npm run inspect -- --csv <folder>`; exported 17 files to
  `dev-dumps/epic-sandbox/` (git-ignored). Core 45 tests.
- Committed `c6f610a`. The owner asked for one .xlsx in `npm run inspect`. The core now returns
  tables (`dev::export_tables`, `to_csv`); `src-tauri/src/xlsx.rs` writes the workbook by hand
  (no new crate). `--export` (or `--csv`) writes the CSV files and `record.xlsx`. A first build
  wrote 2 extra bytes per zip directory entry, which Python refused; fixed, and the test now
  walks the whole directory. Checked: Python's zip test, openpyxl reads all 18 sheets. Not
  checked: Excel and LibreOffice (the LibreOffice helper does not run on Windows).

## 2026-09-24 — stage 7: `ts-rs` and Settings › Sites
- The owner asked where the SMART work stood, then asked for stage 7.
- Built: `core/src/payload.rs` (the Sites payloads and the event types; `CardPass`'s first
  user, the site's name), `ts-rs` 12 behind the core's `ts` feature, `npm run types`
  (`scripts/types.mjs` → `src/model/`, 9 files). `Store::type_counts`. `auth::entry_for`
  and `auth::renew_site` for a saved site's Sign in. Host: `site_sign_in`, `sites_list`,
  `site_sign_out`; the fetch drops dead tokens on "sign in again". Front end: `src/sites.ts`
  and the Sites section of the Settings card. Three new error codes.
- The sign-out erase came with stage 7 (S5 says the second click erases); stage 8 keeps the
  refreshes.
- Checks: core 40 tests + 3 proofs, host 14, wasm, clippy (3 old notes in `log.rs`), `tsc`,
  `vite build`, `docs:check`. Live tests not run: the fetch itself did not change.
- Corrected HANDOFF.md: the fixes after stage 6 were committed in `0061887`.
- New crates: `ts-rs`, `ts-rs-macros` and `termcolor`, build time only (the `ts` feature).
- Open: the owner's desk test (CORE-CRATE.md §12) and the choices in HANDOFF.md › Open now 11.

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
- PATIENT-APP-DESIGN.md §6.1–6.3: the owner's rule that every fork weighs every platform (also in
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
- PATIENT-APP-DESIGN.md §1 now says handling messy data well is the app's point of difference.
- The owner closed 8.1b: keep name, birth date, sex; compute the rest at runtime. Listed
  SMART's Patient extension URLs and identifier types (no values) to name what is dropped.
  New open fork: which field is "sex" (FHIR-SHAPES.md §5.4).
- Sex parked with a direction: display `gender`, compute a clinical sex from several
  factors at runtime.
- Fork 6 read against DeetsMusic's imports and closed in part: 6A empty app + one-by-one
  import, 6B no tray + web-ready design (PATIENT-APP-DESIGN.md §6), 6C release path scaffolded up to
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
  agent / MCP routes (PATIENT-APP-DESIGN.md §3 rules them out until the PHI wall is designed).

## 2026-09-21 — the project folder
- Made `Documents/DeetsHealth` with README, CLAUDE.md and the first docs, and a local git repo
  (no remote).
- Decided by the owner: a patient-facing app on Epic's FHIR API, sandbox patients first,
  structured as a 1-to-1 copy of DeetsMusic, with Midi and Max as the only surfaces.
- Open forks: HANDOFF.md › Open now.
- Later the same day, the owner closed forks 1–4: copy the skeleton, deep-link scheme,
  labs/meds/allergies for 1–2 sandbox patients, SQLite cache.
