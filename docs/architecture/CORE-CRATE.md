---
status: project
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — The core crate (scaffold step 3)

> The web-ready Rust crate that signs in, fetches, seals, stores and normalizes. Nothing
> here is built. The tables it fills are [DATA-MODEL.md](DATA-MODEL.md); the rules it keeps
> are [SECURITY.md](SECURITY.md). Open forks: none.

## 1. Terms
- **Core** — the crate `deetshealth-core`. It has no Tauri dependency, so it can also compile
  to WebAssembly (DESIGN.md §6).
- **Host** — the Tauri app in `src-tauri`. It supplies the platform parts (§3).
- **Index columns** — the facts taken from a resource to fill its row and its links: kind,
  code, dates, references (DATA-MODEL.md §3). They are sealed like every other protected value.
- **Pass** — a value that only the card-payload module, the cache writer and the network module can make. It is
  the only way to read the plaintext inside `Shielded<T>`.

## 2. Where step 3 ends (fork 1a)
Sign-in, the fetch and the sealed cache work end to end against the SMART sandbox. The owner
desk-tests it through a new Settings row, **Sites**: Connect, progress, counts per kind
("1,240 results · 38 medicines"), Refresh, Sign out. Counts carry no protected data.

**The Sites section** (the owner, 2026-09-21, S1–S6 all a):
1. A new **Sites** section at the top of the Settings card, open by default.
2. A connected site is a split row: the site's name, halves **Refresh | Sign out**, and a new
   status line under the label. The name is protected data for a real site, so it goes in by
   `textContent`, never through the card's escaped markup, and never into a hover hint.
3. **Connect a site**: one pill that opens a menu of sites; a pick starts sign-in. Epic stays
   out of the menu until it comes back.
4. During a fetch the counts grow in place: "Fetching… 640 results · 12 medicines".
5. **Sign out** confirms in place: the half reads "Sign out?" and a second click within 3 s
   erases that site's data.
6. Status lines — waiting: "Finish signing in in your browser" (with a Cancel half); done:
   "1,240 results · 38 medicines · 12 allergies · 9 conditions · 41 visits · Updated 2 min
   ago"; expired: "Sign in again" (the left half becomes **Sign in**); error: "Could not
   reach this site. Refresh to try again."

**Build order** (the owner, 2026-09-21). One desk test at the end; `cargo test` and the wasm
check after each stage.
1. Workspace, core skeleton, `Shielded<T>` and the pass, the platform traits.
2. CNG cipher, DPAPI key store, the seal module, with tests.
3. The store: every DATA-MODEL.md table, sealed reads and writes, one file per site.
4. Sign-in: `smart-configuration`, PKCE and `state`, the deep link in `lib.rs`, the token
   trade, sealed tokens.
5. The fetch: `/metadata`, the plan, paging, the pace, retries, `site_warning`, progress.
6. Index columns and links for every type; the normalizer for the first-slice types.
7. `ts-rs` and the Sites section.
8. Refresh on open, the weekly full refresh, the sign-out erase.

## 3. Layout
```
Cargo workspace at the repo root: members core/ and src-tauri/

deetshealth-core (core/)
├── shielded    Shielded<T>, the pass (§4)
├── seal        AES-256-GCM per value through the Cipher trait (§5)
├── auth        smart-configuration, PKCE + state, authorize URL, token trade, refresh
├── fhir        HTTP: search, paging, _include, Retry-After, OperationOutcome capture
├── sync        the fetch plan per site, the pace, progress as counts only (§7)
├── index       raw resource → index columns + links
├── normalize   raw resource → model, at read time (DATA-MODEL.md §6, decision 2)
├── model       the types the cards read
├── store       the Store trait, the schema, the queries; SQLite behind a feature flag
└── platform    traits the host fills: Cipher, KeyStore, SignInReturn, OpenBrowser

src-tauri (host)
├── Windows CNG Cipher, DPAPI KeyStore, the deep-link SignInReturn, OpenBrowser
└── commands: sites, sign_in, sign_out, refresh; a progress event (counts only)
```
- Every command that does I/O is `async fn` + `spawn_blocking` (CLAUDE.md › Conventions).
- The web version later fills the same traits: WebCrypto, non-extractable keys, a web
  redirect, IndexedDB.

## 4. The wrapper type (fork 2a)
- **`Shielded<T>`** (the owner's name, 2026-09-21). No `Debug`, no `Display`, no `Serialize`,
  no `ToString`. A log line that tries to print one is a compile error.
- The plaintext comes out only with the pass, and only two modules can make a pass: the card
  payload and the cache writer (SECURITY.md §4.2).
- **A third pass** (the owner, 2026-09-21, at stage 4): `SitePass`, held by the network module
  (`core/src/net.rs`) only. It reveals a value only to put it into a request to the site it
  came from (URL, `Authorization` header, form field), and it reads each reply inside one
  parse call where every kept value is wrapped. Chosen over a separate credential type, so
  there is one wrapper type to keep tight.
- The value is wiped from memory when it drops (`zeroize`).

## 5. Sealing and keys (forks 4a, 10a)
- **Strictest standard** (the owner, 2026-09-21): the app holds itself to the bar HHS sets
  for covered entities, although HIPAA does not bind it (SECURITY.md §6): NIST SP 800-111 at
  rest with FIPS 140-validated crypto, NIST SP 800-88 for erasure, TLS 1.2 or later.
- **Provider:** Windows CNG for AES-256-GCM, random numbers and SHA-256; SChannel for TLS
  (through `reqwest` with `native-tls`). Both are FIPS 140-validated modules in the OS. No
  third-party crypto crate. The `unsafe` glue sits in one file, with tests.
- **One SQLite file and one data key per site.** The key is made from `BCryptGenRandom` and
  wrapped by DPAPI (`CurrentUser`). Sign-out deletes the file, the tokens and the key. With
  the key gone, a stray copy of the file cannot be opened: cryptographic erase (SP 800-88).
- **Per value:** each protected value is sealed just before its write and unsealed just after
  its read. The nonce is a random 96-bit value. The associated data is the table and the
  row key, so a value moved to another row fails to open.
- **No protected value is plain in SQLite**, and that includes the index columns (dates of
  care, codes). Only opaque row keys and table names are plain. Core filters and sorts in
  Rust. This settles most of DATA-MODEL.md §8 fork 8.
- **What remains:** a deleted key file may linger on an SSD. A leftover copy still needs the
  same Windows user to unwrap it. Windows Device Encryption (BitLocker) covers freed blocks.

## 6. Sign-in (SMART sandbox)
1. Settings › Sites › Connect, and the user picks the SMART sandbox.
2. Core reads `<base>/.well-known/smart-configuration`, makes a PKCE verifier (S256) and a
   `state`, and holds both in memory only. One sign-in may wait at a time.
3. The host opens the system browser at the authorize URL. Scopes: `openid fhirUser
   launch/patient offline_access patient/*.read` (every patient read scope the site offers).
4. The sandbox shows a patient picker, and may show a practice login.
5. The browser asks to open DeetsHealth; the `deetshealth-dev://callback` link reaches the
   running app through the single-instance callback in `lib.rs`.
6. Core checks `state`, trades the code for tokens, and reads the patient from the reply. A
   link with no sign-in waiting, or the wrong `state`, is dropped and logged as ignored.
7. The tokens are sealed with the site's key. The first fetch starts.

**Keeping signed in (fork 7a):** a public client with PKCE. When the refresh token is missing
or fails, the Sites row says to sign in again. The per-install key (`private_key_jwt`,
SECURITY.md §5) comes with Epic.

**Registering the dev scheme:** a debug build registers `deetshealth-dev://` on each start.
From a Claude session the write lands in the MSIX private hive, so the owner starts
`npm run dev:app` from his own terminal once.

## 7. Fetch
- **Plan:** read `/metadata` once per sign-in; fetch every type the site serves and the
  scopes allow (DATA-MODEL.md §5, §7). `Observation` is fetched once per category. Each type
  is `?patient=<id>&_count=100`, following `link[rel=next]`, with `_include` for the
  references a card shows.
- **Pace (fork 5a):** up to 3 calls at once per site. Each type is stored as it arrives.
  Retry-After is honoured; 429 and 503 back off. The number is a stewardship choice, not a
  technical limit: pages inside one type run in order, so the longest page chain (lab
  results) sets the floor, and more than 3 at once gains little. Measure after the build.
- **Refresh (fork 6a):** on app open when the last fetch is more than 1 hour old, plus a
  Refresh command. The small update asks each type for `_lastUpdated` after the last fetch.
  A full refresh once a week catches deletions.
- **Warnings:** every `OperationOutcome` issue in a reply goes to `site_warning`.
- **Logs:** counts, resource types, durations and error kinds only (SECURITY.md §4.5).

## 8. Reading FHIR and the normalizer (forks 3a, 9a)
- **Our own lenient reader** over plain JSON. It reads only the fields we use; a malformed
  field loses one value, not the resource. Custom tooling is built as needed (the owner).
- **Step 3 builds:** index columns and links for every type. **The model types wait for the
  card-by-card design** (the owner, 2026-09-21, at stage 6: this narrows 9a). Stages 7–8 need
  counts only, so no card-facing model is built before its card is designed.
- **Later, card by card** (the owner, 2026-09-21): when the owner designs each card, we go
  through how its raw data looks and decide what to normalize on top.
- Fixed by earlier decisions: a partial date keeps its precision; every field below 100% in
  the probe is optional; code systems are mapped to one canonical URL (FHIR-SHAPES.md §4); a
  resource that cannot be parsed is kept raw and only counted.

## 9. The front-end contract (forks 8a, 11a, the owner, 2026-09-21)
- **Payload structs (11a):** the front end receives one payload struct per card view, with
  plain values. It is built from the model only in the card-payload module, with the pass.
  The model never goes to the front end, so the compile-time wall holds. Each payload is
  designed with its card; that is where "clean and present at runtime" happens.
- **Generated types (8a):** the payload structs derive `TS`; `npm run types` writes them into
  `src/model/`, which is never edited by hand. The cheap checks become `npm run types`, then
  `npx tsc --noEmit`. New for the Deets apps: DeetsMusic writes its types by hand.
- **Order for a new module:** the Rust shape, then generate, then the front end. Every change
  to what the front end receives shows as a diff in `src/model/` at hand-off.
- The security summary of the whole path is SECURITY.md §8.

## 10. Open forks
None.

## 11. New crates (SECURITY.md §4.8)
| Crate | Job | Third-party? |
|---|---|---|
| `windows` (more features) | CNG, DPAPI | Microsoft; already in the tree |
| `reqwest` 0.12 (`native-tls`, `gzip`) | HTTP; TLS by SChannel on Windows, by the browser on the web | third-party |
| `serde_json` | reading FHIR and SMART replies | third-party |
| `rusqlite` (bundled) | SQLite; holds sealed values only | third-party over public-domain SQLite |
| `zeroize` | memory wipe | RustCrypto |
| ~~`base64`~~ | not needed: base64url is ours (`auth::b64url`, RFC 4648 tests) | — |
| `ts-rs` | TypeScript types, build time only | third-party; build time only (fork 8a) |

Ours: `Shielded<T>`, the seal module, PKCE and `state`, the FHIR reader, index, normalizer,
sync, the store schema, the CNG and DPAPI glue.

## 12. As built
> **Part:** built · 2026-09-21 · stages 1–6 of 8

Where this part and the design above disagree, this part is the code.
- **Workspace:** `Cargo.toml` at the repo root (members `core`, `src-tauri`). `Cargo.lock`
  moved to the root. `.cargo/config.toml` keeps the build folder at `src-tauri/target`, so
  the old cache and the `.gitignore` line still apply. `src-tauri` depends on the core.
- **`core/src/shielded.rs`:** `Shielded<T: Zeroize>`, wiped on drop. It has `Clone`, and
  `Eq` / `Ord` so core can sort and filter in Rust: a comparison says "same?" or "which
  first?", never the value. The pass is a sealed trait: `shielded.rs` itself names the only
  two pass types, `store::CachePass` and `payload::CardPass`, and each has a private field,
  so only its own module can make one. A third reader means editing `shielded.rs`.
- **Proofs** (doc tests that must fail to compile): printing a `Shielded` with `{:?}`,
  passing one to a `Serialize` bound, and making a pass outside its module.
- **`core/src/platform.rs`:** `Cipher` and `KeyStore` hold the key as an associated type,
  never bytes the core sees (on the web it will be a non-extractable `CryptoKey`). The nonce
  is made inside `Cipher::seal` and prepended to its output. `OpenBrowser`. `SiteKey`: the
  app's own random name for a site; no `Debug`, no `Display`.
- **Differs from §3:** the sign-in return is not a trait. The platform pushes the callback
  link to the core (stage 4), because the link arrives from outside.
- **Known later item:** `Cipher` and `KeyStore` are synchronous. WebCrypto is async, so the
  web version will need async forms of both.
- **`core/src/error.rs`:** `CoreError`, kinds with fixed text only, so any error may be logged.
- **Checks:** `cargo test -p deetshealth-core` (3 unit tests, 3 compile-fail doc tests),
  `cargo check -p deetshealth-core --target wasm32-unknown-unknown`, `cargo check`, `cargo
  clippy`: all clean.

**Stage 2** (built 2026-09-21): the cipher, the key store, the seal format.
- **`core/src/seal.rs`:** the format around the cipher. A sealed value is a format byte (1)
  then the cipher's output. The associated data is a domain string plus the **table, row key
  and column**, each length-prefixed: a value copied to another row, or to another column of
  the same row, fails to open (stricter than §5's "table and row key"). It takes and returns
  bytes; it holds no pass. `open_text` refuses bytes that are not UTF-8.
- **`src-tauri/src/cng.rs`:** `Cng` implements `Cipher` with CNG's pseudo-handles:
  AES-256-GCM (`BCRYPT_AES_GCM_ALG_HANDLE`), `BCryptGenRandom` (system RNG), SHA-256. Layout:
  nonce (12) ‖ ciphertext ‖ tag (16). `CngKey` holds only the CNG key handle, behind a mutex,
  destroyed on drop; the raw bytes are wiped right after import.
- **`src-tauri/src/keystore.rs`:** `DpapiKeyStore` implements `KeyStore`. A key is 32 CNG
  random bytes; on disk only a DPAPI blob (`CurrentUser`, app entropy, empty description
  because DPAPI stores it in the clear) at `<dir>/<site key>.key`. Written whole (temp file,
  flush, rename); never replaces an existing key; destroy overwrites the file, then deletes it.
  The folder is chosen in stage 3.
- **`SiteKey::generate`:** 128 random bits as hex; `from_stored` refuses anything else, so a
  site key is always a safe file name.
- **Checks:** core 7 unit tests + 3 compile-fail proofs; host 12 tests, including the GCM
  specification's test cases 13 and 14 (proof of real AES-256-GCM), the SHA-256 standard
  vector, a flipped-bit test on every byte, and DPAPI create / open / destroy. Wasm check,
  clippy (new code clean; 3 old notes in `log.rs`), `cargo check --release`.

**Stage 3** (built 2026-09-21): the store.
- **`core/src/store/mod.rs`:** the platform-free row types: `ResourceRow` (14 sealed
  columns, the same on every resource table), `PatientRow` (name as the `HumanName` JSON,
  birth date, sex, the `Patient` id), `SiteRow` (the site's name, base URL, SMART config,
  capability, scopes, tokens, fetch times), `WarningRow`, `LinkRow`; `Table` (the 11
  resource tables); `RowId` (128 random bits, hex); `CachePass`.
- **`core/src/store/sqlite.rs`** (feature `sqlite`, off for wasm): `SiteStore`, rusqlite 0.32
  with SQLite bundled. **Plain:** table names, row keys, a link's `from_id` / `to_id` and
  their table names (so SQL can join), `meta.schema_version`. **Sealed, `NOT NULL`:** every
  other column; a missing value is sealed as "missing", so the file does not show which
  fields a record lacks. One generic write and one generic read seal and unseal every
  column. WAL, `secure_delete` on. A file with a newer schema is refused. SQL names:
  `order_` and `end_` (keywords).
- **Lookups by FHIR id** read the table into Rust; there is no blind index. Cost to measure
  in stage 5.
- **`src-tauri/src/sites.rs`:** `SiteFiles`: `<app data>/sites/<site key>.db` and
  `<app data>/keys/<site key>.key`; create, open, list, erase. Erase destroys the key first,
  then the file and its `-wal` / `-shm`.
- **Checks:** core 13 tests + 3 proofs (round trip, "missing" kept, a value moved to another
  row fails, links join, a transaction is all or nothing, a newer file is refused). Host 14
  tests, including **a byte scan of the real `.db` and `-wal` files, open and closed, for a
  marker string written into every sealed column: not found**, and an erase test: after
  erase, a copy of the file taken before cannot be opened. Wasm, clippy, release check clean.

**Stage 4** (built 2026-09-21): sign-in.
- **`core/src/net.rs`:** `SitePass` (the third pass) and `Net`, a reqwest 0.12 client: HTTPS
  only, TLS 1.2 or later through `native-tls` (SChannel), **no redirects**, gzip, 30 s
  timeout. `get_json` / `post_form` take shielded URLs and tokens; a reply's body is
  shielded on arrival (`Reply`, with its status and `Retry-After`). `parse_reply` and
  `parse_link` are the only places a reply or a returning link is read; `open_in_browser`
  hands the authorize URL to the platform, `https://` only.
- **`core/src/auth.rs`:** `begin` (reads `smart-configuration`, needs `https://` endpoints;
  PKCE verifier from 32 random bytes, S256 challenge; `state` from 16 random bytes) →
  `check_link` (`Ours` / `Refused` / `NotOurs`; the `state` compare is on shielded values) →
  `exchange` (needs `token_type` Bearer and a `patient`; returns the `SiteRow`) → `refresh`
  (`Renewed`, or `SignInAgain` on 400 / 401 or no refresh token) and `needs_refresh` (2 min
  early). The core reads no clock: callers pass `now`. Base64url is ours (RFC 4648 tests);
  the `base64` crate was not needed.
- **The SMART sandbox entry:** the launcher takes its setup from a base64url JSON array in
  the URL. Ours: patient standalone launch with the patient picker, public client, PKCE
  always required (`auth::SMART_LAUNCH`). A plain `/v/r4/fhir` base refuses a standalone launch.
- **`SiteRow` gained `client_id`** (the refresh needs it).
- **Host:** `src-tauri/src/signin.rs` holds the one waiting sign-in (10 minutes), the
  commands `sign_in_start(site)` and `sign_in_cancel`, and `on_link`: the link is shielded on
  arrival; a wrong `state` is ignored and the wait goes on; our link trades the code, then
  `SiteFiles::create` makes the key and the file and seals the site row. Events: `sign-in`
  (a fixed word: waiting, connected, refused, expired, failed, cancelled) and
  `sites-changed` (no payload). `browser.rs`: `ShellExecuteW`, `https://` only. `lib.rs`
  passes both link paths (a second launch, the first launch) to `on_link`.
- **Checks:** core 19 tests + 3 proofs (base64url, link matching, token replies, expiry,
  refresh timing). Host 14 tests, plus **a live test against the SMART sandbox**
  (`cargo test -p deetshealth --lib -- --ignored`, about 5 calls): with the launcher set to
  pick a patient and skip its login, it runs `begin`, reads the authorize redirect, refuses a
  wrong `state`, trades the code (PKCE checked by the launcher), refreshes, and gets
  `SignInAgain` for a bad refresh token. Passed. Wasm, clippy, release check clean.
- The whole flow, its constants, log lines and troubleshooting: [SIGN-IN.md](SIGN-IN.md).

**Stage 5** (built 2026-09-21): the fetch.
- **`core/src/fhir.rs`:** `table_for` (every FHIR type → its table, DATA-MODEL.md §5 and §7);
  `read_capability` (the types the site can search by patient: it picks `patient`, then
  `subject`, `beneficiary`, `member`; notes `category` support); `scope_allows` (SMART v1
  and v2 scopes); `read_page` (a searchset: matches, `_include`s, `OperationOutcome` issues,
  the `next` link, `total`); `read_error`; `read_patient` (keeps name, birth date, sex; drops
  the rest at the parse).
- **`core/src/sync.rs`:** `full_fetch`: refresh the token if needed → `/metadata` (no token)
  → the patient (`Patient?_id=…&_elements=name,birthDate,gender`) → one job per type served
  and allowed (`Observation` once per each of FHIR's 9 categories when the site supports
  `category`) → 3 workers (`futures_util::join_all`, one task, no threads) → each page in one
  transaction. `_include`s only from `WANTED_INCLUDES` and only when the site lists them in
  `searchInclude`. Retries: network error, 429, 500, 502, 503, 504, up to 4 tries, waiting
  `Retry-After` (at most 60 s) or 1, 2, 4 s. A 401 renews the token once and retries. Any
  other 4xx: the job stops, a warning `http-<status>` is stored, the type is in `skipped`.
  A `next` link that leaves the site stops the job (`next-link-off-site`): the token never
  goes to another host. Progress after every page (counts only).
- **Stage 5 stores** each resource's type, id, a first `kind` (the Observation category, or
  the type in lower case) and the raw resource; the other index columns wait for stage 6.
  A resource replaces its earlier row (same FHIR type and id). An included resource goes to
  its own type's table. `Provenance` waits in `other` until stage 6 makes links from it.
- **`Store` trait** (`core/src/store/mod.rs`): what the fetch needs from a store; `SiteStore`
  fills it. `keys(table)` reads two sealed columns per row, not the raw resource.
- **`Timer` platform trait**: `now` and `sleep`; the core still reads no clock.
- **Host:** `src-tauri/src/fetch.rs`: `start(site)` runs the fetch on a blocking thread (one
  per site at a time) right after sign-in; `site_refresh(site)` command; events
  `fetch-progress` and `fetch-done` (§ header of the file). Log: `fetch: started`, `fetch:
  done: N resources, T types, C calls, S s, W warnings, key reads K ms`, `fetch: the site
  refused: <types>`.
- **Measured on the SMART sandbox** (2026-09-21): 76 jobs; 78 calls at 100 a page; 5–7 s
  for 94–397 resources; key reads under 1 ms. The site serves 74 patient-searchable types;
  66 of them have a table.
- **Known limits:** on a site that needs `category` (Epic), an Observation in none of the 9
  categories is not fetched. The raw JSON keeps the site's key order (serde_json
  `preserve_order`, which adds `indexmap`). A full fetch does not yet remove rows the site deleted
  (stage 8).
- **Checks:** core 24 tests + 3 proofs (table map, capability, scopes, a page with matches,
  includes and an outcome, the trimmed patient). Host 14 tests, plus **a live full fetch**
  (`cargo test -p deetshealth --lib fetch -- --ignored --nocapture`): sign in, fetch at 20 a
  page (so `next` links are followed; none left the site), check the patient row, the
  capability and the fetch times, then fetch again at 100 a page: every table's row count
  unchanged. About 165 calls in all. Wasm, clippy, release check clean.

**Stage 6** (built 2026-09-21): the index and the links. No card model (the owner, at stage
6: the models wait for each card's design; §8).
- **`core/src/index.rs`:** `extract(type, resource)` gives the index columns:
  - `kind` (our word per type: the Observation or Condition category code; `panel` or
    `report`; `vaccine`, `ordered`, `filled`, `given`, `reported`, `visit`, `planned`,
    `note`, `form`, `message`, `allergy`; otherwise the type in lower case);
  - the code (one field per type: `code`, `medicationCodeableConcept`, `vaccineCode`,
    `type`, …): its first coding's system (one spelling per system: LOINC, SNOMED, RxNorm,
    CVX, ICD-10-CM, the Observation category), its code, and `text` before `display`;
  - `status` (or `clinicalStatus`); `start`, `end`, `recorded` from one ordered list of
    FHIR date fields for all types (the first present wins; dates kept as written);
    `last_updated`; `canonical` and `version` for definitions.
- **`references(type, resource)`:** every reference to a type with a table, with its field
  (`Observation.basedOn`) and its role. References to the patient, `#contained`,
  `urn:` and those inside `contained`, `extension`, `meta` and `text` are left out.
  Absolute URLs and `_history` are read down to `Type/id`. Roles: DATA-MODEL.md §4.
- **At the fetch:** each resource is indexed in the same parse as its page. Its row and its
  links are written together; a row's old links are removed first. A link whose target is
  stored already points at it; the rest are resolved once every job is done
  (`resolve_links`). A link to a resource never fetched stays waiting, with the reference
  kept.
- **`sync::reindex(store)`:** re-index every row from its raw resource, with no fetch: run
  it when the index rules change (DATA-MODEL.md §6, decision 2).
- **Measured on the SMART sandbox:** 101 resources, 122 links, 4 waiting.
- **Checks:** core 30 tests + 3 proofs (a lab result's columns, per-type fields, references
  and roles, the encounter roles of DATA-MODEL.md §4, reference forms; a re-index of raw
  rows builds columns and links, resolves them, and a second re-index replaces them).
  The live fetch asserts links. Wasm, clippy, release check clean.
