---
status: project
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — Security and privacy

> Nothing here is built. The rules in §4 are fixed before any code. Open forks: §7.

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
4. **Another app that claims `deetshealth://`** — any app can register a scheme. PKCE makes a
   stolen code useless without the verifier, which never leaves our process.
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
`ToString`. The only ways out are (a) the card payload to the front end and (b) the sealed
cache writer. A leak into a log is then a compile error, not a review note. It covers PII as
well as PHI: a patient name is wrapped exactly as a lab value is.

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
- One sign-in may wait at a time. A deep link with no sign-in waiting, or with the wrong
  `state`, is dropped and logged as "ignored" with no values (DeetsMusic's nonce pattern).
- Sign-in runs in the system browser, never in WebView2.

### 4.5 Logs, panics and crashes
- Logs carry counts, resource types, durations and error kinds. Never a value, a name, an id,
  a URL with a query string, or a real site's name.
- A panic hook replaces the panic message before it reaches the log or stderr.
- No crash dump of our own. Windows Error Reporting dumps are out of our hands; the release
  notes do not ask users to send them.

### 4.6 Keys
- One data key per install, made from the OS random source, sealed with DPAPI
  (`CurrentUser`). Nothing is derived from a password we make up.
- Crypto comes from audited primitives only (AES-GCM or ChaCha20-Poly1305 from RustCrypto,
  or Windows CNG). Our own crate may wrap them; it never contains our own cipher, mode or
  key derivation. Every nonce is random or a counter that never repeats under one key.
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
in §4.6, so Epic can issue refresh tokens to a public app. **On hold:** the owner cannot
register on fhir.epic.com yet, so all Epic work waits. SMART Health IT's launcher accepts
`private_key_jwt` with no registration, so the per-install key half can be built and tested
there first.

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
2. **The wrapper type's name** — decided at build time.
3. ~~What we keep from `Patient`~~ — decided 2026-09-21: name, birth date, sex only;
   derived values computed at runtime, never stored (FHIR-SHAPES.md §5, fork 1).
