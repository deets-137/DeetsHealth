---
status: designed
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — Data architecture

> Nothing here is built. Forks are listed in HANDOFF.md › Open now.

## 1. Terms
- **Site** — one health system's FHIR server, with its own base URL.
- **Client ID** — the ID Epic gives the app on registration. Non-production = sandbox.
- **PKCE** — the proof a public client (no secret) sends so a stolen code is useless.

## 2. Sign-in (SMART standalone launch)
1. The user picks a site (sandbox first; Epic's endpoint directory later).
2. Rust reads `<base>/.well-known/smart-configuration` for the authorize and token URLs.
3. Rust makes a PKCE verifier and a `state`, opens the system browser at the authorize URL
   with scopes like `openid fhirUser patient/*.read offline_access` (exact set is a fork).
4. The user signs in with the site's MyChart login (sandbox: a test patient's login).
5. The browser returns to the redirect; Rust checks `state` and trades the code for tokens.
6. The token reply names the `patient` ID. Every read is for that one patient.

## 3. Tokens
- Access token: about one hour. Refresh before expiry, with a clock-skew margin.
- Refresh token: lifetime set by the site; may be missing. When it fails, the card shows
  "sign in again", never an error loop.
- At rest: DPAPI-sealed file per site, as DeetsMusic's `friends.json`.

## 4. Fetch, normalize, cache
- Rust fetches FHIR bundles and follows `link[rel=next]`.
- Rust turns each resource into a small model (`LabResult`, `Medication`, `Allergy`,
  `Encounter`). The front end never sees raw FHIR — the same rule as DeetsMusic's
  normalized model.
- Codes (LOINC, RxNorm, SNOMED) keep their system and code, plus the display text.
- Cache: SQLite on disk or memory only — an open fork.
- Every command that does I/O is `async fn` + `spawn_blocking`.

## 5. The PHI wall
PHI may exist only in: the normalized models, the cache (if chosen), and the card DOM.
It must never reach:
- the log file, a diag ring or a `[perf]` line (log counts and resource types only);
- a bug report or crash dump;
- a toast, a hover hint, or a window title;
- an agent, MCP or HTTP bridge route;
- a SQL / export tool;
- the clipboard, unless the user copies it on purpose.

A compile-time type for PHI values (not `Display`, not `Debug`-printable) is one way to
make this checkable. That is a design fork for later.
