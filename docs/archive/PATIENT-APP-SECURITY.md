---
status: archived
desk_test: none
sources: []
updated: 2026-10-19
---
# DeetsHealth (the patient app) — Security and privacy

> **Archived 2026-10-19.** The patient app's security design, kept as a record. The phone
> agent's is [SECURITY.md](../architecture/SECURITY.md).

> The rules in §4 were fixed before any code. Most of them are now built (CORE-CRATE.md
> §12, stages 1–7); §8.8 says what is built, what is not, and what the build found. Open
> forks: §7.

## 1. Priority (the owner, 2026-09-21)
1. **Patient experience.**
2. **Privacy and security.**
3. Everything else (speed of build, convenience for us, features).

Privacy and security come directly after the patient's experience, and before everything
else. The PHI wall is also a stop rule (LESSONS.md §2, rule 4): a design that trades it away for
experience comes to the owner first. Look for a design that gives both (LESSONS.md §3.12).

## 2. Terms
- **PHI** — anything from a record: results, medicines, allergies, notes, dates of care.
- **PII / identifier** — anything that points to a person, even with no health fact in it:
  name, date of birth, address, phone, email, MRN, the FHIR `Patient` id, the MyChart user
  name, and **which real site the user signed in to** (a site's name tells where a person
  gets care). Tokens and client keys are credentials and get the same treatment.
- **Protected data** — PHI and PII together. Every rule below says "protected data".
  Sandbox patients are public fakes, but the code treats them as protected data too, so the
  code path is the one a real record takes.
- **Seal** — encrypt with a key that only this Windows user can unwrap (§5).

## 3. Who we protect the record from
1. **Our own leaks** — logs, diag rings, crash dumps, panic messages, toasts, hover hints,
   the window title, the clipboard, an AI chat. The most likely risk, and fully ours.
2. **Text inside the record** — free-text notes rendered as HTML would run script inside a
   WebView2 that can call our Rust commands.
3. **Other people and programs on the PC** — a stolen laptop, a copied backup, another user.
4. **Another program that catches the sign-in return** — since 2026-10-13 the return is a
   listener on 127.0.0.1 (SIGN-IN.md §6); before, any app could register our scheme. A program
   that reached the port first would get a code. PKCE makes that code useless without the
   verifier, which never leaves our process.
5. **The network and the supply chain** — a changed dependency, an unsigned update.

What we cannot stop: malware that runs as the same Windows user while the app is open. It
can read the screen and the process memory. DPAPI does not protect against it. We say so
plainly and never claim otherwise.

## 4. The rules
### 4.1 End to end
Protected data is encrypted at every step except the moment it is shown:
- **In transit:** HTTPS only (TLS by the site). No `http://` base URL, no certificate override.
- **At rest:** sealed. The cache, the token file, any client key, and any stored identifier
  (the chosen real site included). Nothing protected is written to disk in the clear.
- **In memory:** plaintext only inside the wrapper type (§4.2), and only as long as a card
  needs it.
- **On screen:** plaintext in the card DOM — the one place it must be readable.
- **Nowhere else:** there is no server, no sync, no telemetry, no backup of our own. The
  record never leaves the PC.

### 4.2 The wrapper type (decided, the owner, 2026-09-21)
Every protected value in Rust is sealed in a wrapper type the moment it is parsed, before
it goes anywhere. The type has no `Debug`, no `Display`, no `Serialize` to a log, no
`ToString`. The only ways out are (a) the card payload to the front end, (b) the sealed
cache writer, and (c) a request to the site the value came from: the site's URL, the
access token, the `Patient` id, a form field (the owner, 2026-09-21: a third pass, held by
the core's network module only). A leak into a log is then a compile error, not a review
note. It covers PII as well as PHI: a patient name is wrapped exactly as a lab value is.
- **The moment of parsing:** a site's reply is read only inside the network module's parse
  call, and the code there wraps every value it keeps. That call is the one place a reply's
  plaintext exists outside the wrapper (CORE-CRATE.md §12, stage 4).

### 4.3 The front end
- Record text goes into the page **only through `textContent`**. Never `innerHTML`, never a
  template string turned into HTML.
- A strict Content-Security-Policy: no remote script, no inline script, no remote images.
- The front end gets named Tauri commands only. No file, shell, HTTP or opener plugin is
  exposed to it; Rust opens the browser for sign-in.
- No protected data in a toast, a hover hint, the window title, the tray, a notification or
  `localStorage`. A message names a kind of thing ("3 lab results"), never a value.
- The clipboard only when the user copies on purpose.
- Release builds have no DevTools.

### 4.4 Sign-in
- PKCE with S256 and a `state` check on every sign-in.
- One sign-in may wait at a time. A returning link with no sign-in waiting, or with the wrong
  `state`, is dropped and logged as "ignored" with no values (DeetsMusic's nonce pattern).
- The return listener binds 127.0.0.1 only, lives only while a sign-in waits, checks `Host`,
  and answers with fixed pages (SIGN-IN.md §6).
- Sign-in runs in the system browser, never in WebView2.

### 4.5 Logs, panics and crashes
- Logs carry counts, resource types, durations and error kinds. Never a value, a name, an id,
  a URL with a query string, or a real site's name.
- A panic hook replaces the panic message before it reaches the log or stderr.
- No crash dump of our own. Windows Error Reporting dumps are out of our hands; the release
  notes do not ask users to send them.

### 4.6 Keys
- **The strictest standard** (the owner, 2026-09-21): we hold ourselves to the bar HHS sets
  for covered entities, although HIPAA does not bind the app (§6). At rest: NIST SP 800-111
  with FIPS 140-validated crypto. Erasure: NIST SP 800-88. In transit: TLS 1.2 or later.
- **One data key per site** (fork 4a, 2026-09-21; it replaces "one per install"), with one
  SQLite file per site. The key is made from the OS random source and sealed with DPAPI
  (`CurrentUser`). Sign-out deletes the file, the tokens and the key: cryptographic erase.
  Nothing is derived from a password we make up.
- **Provider** (fork 10a, 2026-09-21): Windows CNG for AES-256-GCM, random numbers and
  SHA-256; SChannel for TLS. Both are FIPS 140-validated. No third-party crypto crate. Our
  own crate may wrap them; it never contains our own cipher, mode or key derivation. Every
  nonce is random or a counter that never repeats under one key. The web version plugs
  WebCrypto into the same trait (CORE-CRATE.md §5).
- **No protected value is plain in SQLite**, the index columns included. Only opaque row
  keys and table names are plain.
- **Known remainder:** a deleted key file may linger on an SSD; a copy still needs the same
  Windows user to unwrap it. Windows Device Encryption covers freed blocks.
- Sealing is per value (fork 7a): each protected value is sealed just before its SQLite
  write and unsealed just after its read, straight into the wrapper type. The row's key is
  the associated data, so a sealed value moved to another row fails to open.

### 4.7 Deletion (decided, the owner, 2026-09-21)
- **Sign-out** deletes that site's cache and tokens.
- **Uninstall** deletes all of them: the uninstaller removes the data folder and the sealed key.

### 4.8 Development
- Sandbox data only. A real record is a later, owner-approved step (stop rule 4).
- Client IDs live in a git-ignored file. Cache and key files are git-ignored by pattern.
- Sandbox values may appear in docs and in an AI chat; real values never do.
- Dependencies stay few; each new crate or package is named at hand-off.

## 5. Keeping a user signed in (decided, the owner, 2026-09-21: 3a)
Test Epic's dynamic client registration first: each install gets its own key pair, sealed as
in §4.6, so the app can renew access with no new sign-in. SMART Health IT's launcher accepts
`private_key_jwt` with no registration, so the per-install key half can be built and tested
there first.

**Corrected 2026-10-13.** Epic is back on (the owner has a fhir.epic.com account). The words
"so Epic can issue refresh tokens" were Claude's, and they were wrong for this path. Epic's
form asks how a dynamic client gets a new access token. The answer is the **JWT bearer grant**
(`urn:ietf:params:oauth:grant-type:jwt-bearer`), not the refresh token grant: each new access
token comes from a short JWT that the install signs with its own key. No refresh token is
stored, so the only long-lived secret is a key that never leaves its key store. That is both
more secure and fewer sign-ins (PATIENT-APP-EPIC.md §6).

## 6. The law (a summary, not legal advice)
- **HIPAA** binds covered entities (providers, health plans, clearinghouses) and their
  business associates. An app that a patient chooses to receive their own record is neither,
  so HIPAA does not govern the copy the app holds. (HHS guidance on the right of access and
  third-party apps, 2019.)
- What can apply instead: the **FTC Act** (privacy claims must be true), the **FTC Health
  Breach Notification Rule** (health apps count; a disclosure the user did not authorize is a
  breach), and state laws such as **Washington's My Health My Data Act** and California's.
- The design in §4 keeps the exposure small: no server, no sharing, nothing leaves the PC.
  Check this again before the app goes to anyone other than the owner.

## 7. Open forks
1. ~~The shape of the encryption crate~~ — decided 2026-09-21 (7a): per value. Seal just
   before the SQLite write, unseal just after the read, straight into the wrapper type.
   AES-GCM per value, the row's key as associated data (a value cannot be moved to another
   row). Table names and row counts stay visible; sealed columns sort in Rust.
2. ~~The wrapper type's name~~ — decided 2026-09-21: `Shielded<T>` (CORE-CRATE.md §4).
3. ~~What we keep from `Patient`~~ — decided 2026-09-21: name, birth date, sex only;
   derived values computed at runtime, never stored (FHIR-SHAPES.md §5, fork 1).

## 8. The security measures, in one place
> **Part:** project · 2026-09-21 · built through stage 6; payloads and erase wait for stages 7–8

A summary to speak from, for a user, a reviewer or a health system. Each line points to its
rule. Where this section and §4 disagree, §4 is the rule. What is built is §8.8. For a
security engineer who helps us, the tour and the open questions are
[SECURITY-BRIEF.md](../architecture/SECURITY-BRIEF.md).

### 8.1 The standard we hold
- HIPAA does not bind an app that a patient chooses (§6). We hold the bar HHS sets for
  covered entities anyway: NIST SP 800-111 at rest with FIPS 140-validated crypto, NIST
  SP 800-88 for erasure, TLS 1.2 or later in transit (§4.6).
- Protected data is PHI and PII together: every value from the record and every identifier,
  the real site's name included (§2).
- There is no server. The record goes from the health system to the user's PC and nowhere
  else: no sync, no telemetry, no backup of ours, no AI route (§4.1).

### 8.2 The path of one value
```
site ──TLS (SChannel)──► core parses it ──► Shielded<T> in memory
     ◄──TLS── pass (site): URL, token, Patient id  │
              go back to that same site only      │ pass (cache writer)
                                                  ▼
                        sealed: AES-256-GCM (CNG), per value, row key as associated data
                                                  │ SQLite file for this site only
                                                  ▼
                        read back ──► unsealed straight into Shielded<T>
                                                  │ pass (card payload)
                                                  ▼
                        payload struct, plain values, one per card view
                                                  │ Tauri IPC (in process)
                                                  ▼
                        card DOM, through textContent only
```
1. **In transit:** HTTPS only, through Windows' SChannel (FIPS 140-validated). No `http://`
   site, no certificate override (§4.1).
2. **In memory:** every protected value lives in `Shielded<T>` from the moment it is parsed.
   The type has no `Debug`, `Display`, `Serialize` or `ToString`, so a log line that tries to
   print one does not compile. It is wiped from memory when it drops (`zeroize`) (§4.2).
3. **The only three ways out** of `Shielded<T>` need a pass, and only three modules can make
   one: the cache writer, the card payload, and the network module, which may put a value
   only into a request to the site it came from (CORE-CRATE.md §4). `shielded.rs` names all
   three; a fourth is a visible change to that file.
4. **At rest:** each value is sealed on its own, with AES-256-GCM from Windows CNG
   (FIPS 140-validated), a random 96-bit nonce, and the table plus row key as associated
   data, so a value copied to another row fails to open. No protected value is plain in the
   file, the index columns (dates, codes) included. Only opaque row keys and table names
   show (§4.6).
5. **To the screen:** a card receives a payload struct built for that view, with plain values.
   Its TypeScript type is generated from Rust (`ts-rs`), so the two cannot drift. Record text
   enters the page only through `textContent` (§4.3).

### 8.3 Keys and erasure
- One data key per site, made by CNG's random source and wrapped by DPAPI for this Windows
  user only. One SQLite file per site (§4.6).
- **Sign-out** deletes that site's file, tokens and key. Without the key a leftover copy
  cannot be opened: cryptographic erase under NIST SP 800-88. **Uninstall** deletes all of
  them (§4.7).
- Crypto comes only from the OS's validated modules. Our code wraps them; it holds no cipher,
  mode or key derivation of its own.

### 8.4 What never carries protected data
The log, a diag ring, a `[perf]` line, a panic message (the hook replaces it), a crash dump
of ours, a toast, a hover hint, the window title, a notification, `localStorage`, the
clipboard (unless the user copies on purpose), a bug report, an agent or MCP route, an
export (§4.3, §4.5). Logs carry counts, resource types, durations and error kinds only.

### 8.5 Sign-in
- In the system browser, never in WebView2. PKCE (S256) and a `state` check on every sign-in;
  one sign-in may wait at a time; a stray or mismatched link is dropped (§4.4).
- Tokens are sealed with the site's key. Another app that claims our link scheme gets a
  code it cannot use without the verifier, which never leaves our process (§3).
- The returning link holds the code, so it is shielded the moment it arrives and never
  logged. A waiting sign-in lasts 10 minutes. A link with the wrong `state` leaves the
  waiting sign-in in place, so a stray link cannot cancel the user's sign-in.
- The HTTP client follows no redirects, so a token never goes to another host; it requires
  HTTPS and TLS 1.2 or later. The browser is opened by Rust for `https://` only.

### 8.6 The front end
A strict Content-Security-Policy (no remote or inline script, no remote images); named
commands only, with no file, shell, HTTP or opener plugin exposed; no DevTools in a release
build (§4.3).

### 8.7 What we say plainly
- Malware running as the same Windows user while the app is open can read the screen and
  the process memory. No app can stop that, and we do not claim to (§3).
- A deleted key file may linger on an SSD. A copy still needs the same Windows user to
  unwrap it; Windows Device Encryption covers freed blocks (§4.6).
- Development uses sandbox patients only. A real record is a later, owner-approved step (§4.8).

### 8.8 As built
> **Part:** built · 2026-09-24 · stages 1–7 of 8

Where this part and §8.1–§8.7 disagree, this part is the code.

**Built and tested:**
- `Shielded<T>` and its three passes (`CachePass`, `CardPass`, `SitePass`), with
  compile-fail proofs (CORE-CRATE.md §12). `CardPass` has one user since stage 7:
  `payload::site_view` reveals the **site's name** for its Settings › Sites row. The page
  puts it in with `textContent` after the markup is built, and the row has no hover hint
  (`src/settings-card.ts`, `fillNames`). No log line, event, toast or diag entry carries it.
- The payload and event types are generated from Rust (`ts-rs`, `npm run types`), so the
  page cannot expect a field Rust does not send (§4.3).
- AES-256-GCM, random numbers and SHA-256 from Windows CNG; the GCM specification's test
  vectors pass. The associated data is the table, the row key **and the column**: stricter
  than §4.6. Every value is padded before it is sealed (STORE.md §5).
- One data key per site, DPAPI-wrapped with app entropy; one SQLite file per site. A byte
  scan of the real files finds no plaintext (STORE.md §12).
- HTTPS only, TLS 1.2 or later through SChannel, no redirects; a `next` link must stay on
  the site's base URL (FETCH.md §13).
- Sign-in: PKCE S256, `state`, one waiting sign-in, 10 minutes, the link shielded on arrival
  (SIGN-IN.md §10).
- The log's wall: redaction inside the writer (JWTs, `Bearer`, URL query strings, the
  Windows account name) and a panic hook that withholds the message (`src-tauri/src/log.rs`).
  Every `CoreError` has fixed text. Events carry counts and fixed words only.
- The CSP in `tauri.conf.json`; no DevTools feature in the Tauri build.

- The sign-out erase: a row's **Sign out**, pressed twice, destroys the site's key, then
  its files (STORE.md §8). Not while the site fetches.

**Not built yet:**
- The card payloads, and so no record text reaches the page yet (card by card).
- The uninstaller's delete comes with the release path (STORE.md §8).

**Found in the build, and fixed** (2026-09-21, the owner's choices):
- **Sealed lengths.** A sealed value was its plaintext length plus 30 bytes, so the file
  showed which values were missing and how long each note was. **Fixed:** every value is
  padded before it is sealed (Padmé, 64-byte floor): every short value (a missing one
  included) has one length, and a long one shows only a size class within about 12%. Old
  files are migrated in place (STORE.md §4–§6).
- **The `next`-link check** was a text prefix, so a base URL with no path accepted a link to
  a look-alike host. **Fixed:** `net::on_site` compares scheme, host, port and the path at a
  `/` (FETCH.md §13).
- **Every failure now reaches the user** as a toast with a fixed code, never a value
  (ERRORS.md).

**Still true, said plainly:**
- **The link graph is plain** so SQLite can join: the file shows which row points to which,
  with no values (STORE.md §4).
- **Roaming profile.** The data folder is under `%APPDATA%` (Roaming). A domain roaming
  profile copies it to a server; the key still needs the same Windows user (STORE.md §2).
