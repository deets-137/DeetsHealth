---
status: guide
desk_test: none
sources: [core/src/shielded.rs, core/src/seal.rs, core/src/platform.rs, src-tauri/src/cng.rs, src-tauri/src/keystore.rs, src-tauri/tauri.conf.json]
updated: 2026-09-24
---
# DeetsHealth — A brief for a security engineer

> Written for someone who is kind enough to look over my shoulder. The rules themselves live
> in [SECURITY.md](SECURITY.md); this page is the tour and the ask.

## Hi, and thank you
I am building a health-record app on my own, and I am not a security engineer. I have read
the standards and made a plan, but I do not know what I do not know. I would love a more
experienced set of eyes and hands: tell me where the plan is wrong, where it is overbuilt,
and where it is thin.

Nothing here is precious. If a decision looks naive, please say so. Every question in §6 is
a real question, not a rhetorical one.

**No real patient data exists in this project.** Development uses public sandbox patients
(SMART Health IT) only. You will never need to see a real record, and please do not use one.

## 1. What the app is
- A Windows 11 desktop app that lets a patient see their own health record.
- Tauri v2: a Rust back end and a WebView2 front end (plain TypeScript, no framework).
- It signs in to a health system's FHIR server with SMART on FHIR, downloads the patient's
  record, and keeps a cache on the PC.
- **There is no server of ours.** No sync, no telemetry, no backup, no analytics. The record
  goes from the health system to this PC, and nowhere else.

## 2. What I am trying to protect
- **Protected data** = PHI plus PII. PII includes any identifier, even with no health fact in
  it: the name, the MRN, the FHIR `Patient` id, and **which real health system** the user
  signed in to (that alone tells where a person gets care). Tokens and keys get the same
  treatment.
- **The bar.** HIPAA does not bind an app that a patient chooses for their own record. I want
  to hold the HHS bar for covered entities anyway: NIST SP 800-111 at rest with
  FIPS 140-validated crypto, SP 800-88 for erasure, TLS 1.2 or later in transit.
- **Priority.** Patient experience first, privacy and security second, everything else after.

## 3. Who I think the threats are
Most likely first:
1. **My own leaks.** Logs, panic messages, crash dumps, toasts, the window title, the
   clipboard, an AI coding tool reading a log. I think this is the biggest risk by far.
2. **The record's own text.** A free-text clinical note rendered as HTML could run script
   inside a WebView that can call my Rust commands.
3. **Other people on the PC.** A stolen laptop, a copied backup, another Windows account.
4. **Another app that claims my URL scheme** (`deetshealth://`), to catch the sign-in code.
5. **The network and the supply chain.** A changed dependency, an unsigned update.

**What I do not try to stop:** malware that runs as the same Windows user while the app is
open. It can read the screen and the process memory. I plan to say that plainly to users.

## 4. The plan, layer by layer

| Layer | What I do |
|---|---|
| **In transit** | HTTPS only, through Windows SChannel (`reqwest` + `native-tls`). No `http://` base URL, no certificate override. |
| **Sign-in** | SMART standalone launch in the **system browser**, never inside WebView2. PKCE (S256) and a `state` check. One sign-in may wait at a time; a deep link with no sign-in waiting, or the wrong `state`, is dropped. |
| **In memory** | Every protected value lives in a wrapper type, `Shielded<T>` (§5). |
| **At rest** | SQLite, but every protected value is sealed on its own: AES-256-GCM from Windows CNG, a random 96-bit nonce, and the table + row key + column as associated data, so a value copied to another cell fails to open. Each value is padded first (Padmé, 64-byte floor). A format byte leads each value. Index columns (dates, codes) are sealed too; only opaque row keys and table names are plain. Filtering and sorting happen in Rust. |
| **Keys** | One data key and one SQLite file per health system. The key is 32 bytes from `BCryptGenRandom`, wrapped with DPAPI (`CurrentUser`) plus app-specific entropy. The raw key goes into a CNG key handle and the bytes are wiped. |
| **Erasure** | Sign-out deletes that site's file, tokens and key: a cryptographic erase. Uninstall deletes all of them. |
| **Front end** | Record text enters the page only through `textContent`, never `innerHTML`. A strict Content-Security-Policy. The front end can call named Tauri commands only: no file, shell, HTTP or opener plugin. No DevTools in a release build. |
| **Logs and panics** | Logs carry counts, resource types, durations and error kinds only. A panic hook replaces the panic message. Every error type has fixed text, so any error is safe to log. |

**Crypto policy.** Every primitive comes from the OS's FIPS 140-validated modules (CNG and
SChannel). No third-party crypto crate, and no cipher, mode or key derivation of my own. All
the `unsafe` FFI sits in two files: [cng.rs](../../src-tauri/src/cng.rs) and
[keystore.rs](../../src-tauri/src/keystore.rs).

## 5. The one idea I am proudest of (please poke at it)
[core/src/shielded.rs](../../core/src/shielded.rs)

`Shielded<T>` wraps every protected value the moment it is parsed.
- It has no `Debug`, `Display`, `Serialize` or `ToString`. A log line or a JSON reply that
  tries to carry one **does not compile**.
- The plaintext comes out only through `reveal()`, which needs a `Pass`. `Pass` is a sealed
  trait with exactly three implementations: the sealed-cache writer, the card-payload
  builder, and the network module (which may put a value only into a request to the site it
  came from: the URL, the token, the `Patient` id). Each pass has a private field, so only
  its own module can make one.
- It is zeroized on drop. It allows `Eq` and `Ord`, so the core can sort and filter: a
  comparison says "same?" or "which first?", never "what is it?".
- Compile-fail doc tests prove each rule.
- The front end never sees the model. It gets one small payload struct per card view, with
  TypeScript types generated from Rust (`ts-rs`).

The goal: a PHI leak into a log becomes a compile error, not a code-review comment.

## 6. Where I would love your help
Each of these is a place where I suspect the plan is thin. I would value your honest read:
is it a real risk, is it worth fixing, and how would you do it?

1. **Plaintext before the wrapper.** The HTTP response body and the parsed JSON hold PHI
   before `Shielded::new` runs. `zeroize` does not reach reqwest's buffers or serde's
   temporaries. Is that worth chasing, or is it noise next to the process being open at all?
2. **Plaintext after the wrapper.** The payload structs, Tauri's IPC JSON and the WebView DOM
   hold plain values. I do not control those copies. Is there anything sensible to do here?
3. **SQLite metadata.** Table names and row counts stay visible. The link table's row keys
   are plain too, so SQLite can join: the file shows which row points to which, with no
   values. Every value is padded before it is sealed (Padmé, 64-byte floor): every short
   value, a missing one included, has one sealed length, and a long one shows only a size
   class within about 12%. Is that padding enough? Could someone still learn something real,
   such as the number of visits from the row counts or the link graph?
   ([STORE.md §4–§5](STORE.md))
4. **DPAPI's limit.** Any process that runs as the same user can call `CryptUnprotectData`.
   My app entropy stops accidents, not an attacker. Is there a better Windows option for a
   desktop app (a TPM-backed CNG key, Windows Hello, something else)? Is it worth the
   friction for the patient?
5. **SSD remnants.** A deleted key file may linger on an SSD. I lean on BitLocker / Device
   Encryption for freed blocks. Is that fair to say, or should I do more?
6. **The CSP allows `style-src 'unsafe-inline'`**
   ([tauri.conf.json](../../src-tauri/tauri.conf.json)). How much does that matter when no
   script can run inline? Should I remove it?
7. **Refresh tokens for a public client.** Until a health system lets me register a key per
   install (`private_key_jwt`), the app is a public client with PKCE. How much should that
   worry me?
8. **The `unsafe` code.** A plain review of [cng.rs](../../src-tauri/src/cng.rs) and
   [keystore.rs](../../src-tauri/src/keystore.rs) would mean a lot. It is my first real FFI
   to CNG and DPAPI.

And the open-ended one: **what did I miss entirely?**

## 7. What exists today
Honest status, so you know what is real and what is paper:
- **Built and tested (stages 1–7 of 8):** the workspace, `Shielded<T>` and its three
  passes, the platform traits, the error type; the CNG cipher (with the GCM specification's
  test vectors), the DPAPI key store, the seal format; the sealed SQLite store (a byte scan
  of the real files finds no plaintext); SMART sign-in with PKCE; the full fetch (HTTPS
  only, no redirects, a `next` link must stay on the site); the index and the links between
  rows. Sign-in and the fetch are tested live against the SMART sandbox. The log's
  redaction and the panic hook (it withholds the message) are built. Settings › Sites: the
  one protected value on screen so far is the site's name, put in by `textContent`; the
  Sign out button calls the erase.
- **Designed only:** the normalizer and the card payloads (no record text reaches the page
  yet), the uninstaller's delete.
- The build plan has 8 stages; the list is in [CORE-CRATE.md §2](CORE-CRATE.md). What the
  build found is [SECURITY.md §8.8](SECURITY.md).

## 8. Where to read next
1. [SECURITY.md §8](SECURITY.md) — the one-page summary of the rules. §4 has every rule in
   full.
2. [CORE-CRATE.md](CORE-CRATE.md) §4, §5 and §12 — the wrapper, sealing and keys, and what is
   built. [STORE.md](STORE.md) — the files on disk and what they show;
   [SIGN-IN.md](SIGN-IN.md) §10 and [FETCH.md](FETCH.md) §13 — the network side.
3. The code, in this order: [shielded.rs](../../core/src/shielded.rs),
   [seal.rs](../../core/src/seal.rs), [platform.rs](../../core/src/platform.rs),
   [cng.rs](../../src-tauri/src/cng.rs), [keystore.rs](../../src-tauri/src/keystore.rs).

Checks you can run: `cargo test -p deetshealth-core`, `cargo check`, `cargo clippy`.

Thank you. Any time you give this is a gift, and I will take every note seriously.
