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
1. The user picks a site. The first two are sandboxes (the owner, 2026-09-21):
   - **Epic sandbox** — `https://fhir.epic.com/interconnect-fhir-oauth/api/FHIR/R4/`.
     Needs a registered client ID. Four test patients with MyChart logins.
   - **SMART Health IT** — `https://launch.smarthealthit.org/v/r4/fhir`. No registration;
     many synthetic (Synthea) patients with longer, messier histories.
   Real sites come later, from Epic's endpoint directory. The app never hard-codes a vendor:
   every site-specific fact comes from its `smart-configuration` or its site entry.
2. Rust reads `<base>/.well-known/smart-configuration` for the authorize and token URLs.
3. Rust makes a PKCE verifier and a `state`, opens the system browser at the authorize URL
   with scopes like `openid fhirUser patient/*.read offline_access` (exact set is a fork).
4. The user signs in with the site's MyChart login (sandbox: a test patient's login).
5. The browser returns to `deetshealth://callback` (deep-link scheme, decided 2026-09-21); Rust checks `state` and trades the code for tokens.
6. The token reply names the `patient` ID. Every read is for that one patient.

## 3. Tokens
- Access token: about one hour. Refresh before expiry, with a clock-skew margin.
- Refresh token: lifetime set by the site; may be missing. When it fails, the card shows
  "sign in again", never an error loop.
- At rest: DPAPI-sealed file per site, as DeetsMusic's `friends.json` (SECURITY.md §4.6).

## 4. Fetch, normalize, cache
- Rust fetches FHIR bundles and follows `link[rel=next]`.
- Rust turns each resource into a small model (`LabResult`, `Medication`, `Allergy`,
  `Encounter`). The front end never sees raw FHIR — the same rule as DeetsMusic's
  normalized model.
- Codes (LOINC, RxNorm, SNOMED) keep their system and code, plus the display text.
- Cache: SQLite on disk (decided 2026-09-21), encrypted by our own crate over audited
  primitives (decided 2026-09-21; its shape is SECURITY.md §7). The file holds protected
  data, so sign-out and uninstall delete it, and it never goes into a report or an export.
- Every protected value is sealed in the wrapper type the moment it is parsed (SECURITY.md §4.2).
- Every command that does I/O is `async fn` + `spawn_blocking`.

## 5. The PHI wall
The wall covers PHI **and** PII: any identifier, such as a name, an MRN, the `Patient` id
or the real site's name (SECURITY.md §2). The full rules are SECURITY.md §4.

Protected data may exist only in: the wrapper type in memory, the sealed cache, and the card DOM.
It must never reach:
- the log file, a diag ring or a `[perf]` line (log counts and resource types only);
- a bug report or crash dump;
- a toast, a hover hint, or a window title;
- an agent, MCP or HTTP bridge route;
- a SQL / export tool;
- the clipboard, unless the user copies it on purpose.

A compile-time wrapper type for protected values (not `Display`, not `Debug`-printable)
makes this checkable. Decided by the owner, 2026-09-21: SECURITY.md §4.2.
