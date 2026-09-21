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
- The folder, the README, CLAUDE.md and the first docs exist. **No code.**
- Git is local only. There is no remote. Do not add one.

## Open now
Every fork below is the owner's. None is decided.

1. **Scaffold.** Copy DeetsMusic's Tauri skeleton (package.json scripts, `dev-app.mjs` with
   a free-port probe, `vite.config.ts`, token files) or start from `create-tauri-app` and
   add the pieces. Recommendation: copy, then strip.
2. **Sign-in route.** Loopback redirect (`http://127.0.0.1:<free port>/callback`) or a
   custom deep-link scheme. Recommendation: loopback — no registry write, so the MSIX
   virtualization trap from DeetsMusic does not apply. (DATA-ARCHITECTURE.md §2.)
3. **First slice.** Which resources the first card reads. Recommendation: labs
   (Observation), meds (MedicationRequest), allergies (AllergyIntolerance) for Camila Lopez.
4. **Cache or no cache.** Keep fetched data in SQLite (fast, offline, more PHI on disk) or
   in memory only (slower, nothing on disk). (DATA-ARCHITECTURE.md §4.)
5. **Epic app registration.** The owner registers a patient-facing app on fhir.epic.com
   and pastes the non-production client ID into a local, git-ignored file.

## Gotchas
- Epic's sandbox data is small and tidy. Real site data is not.
- A production client ID takes time to reach each health system after registration.
- Refresh-token lifetime is set per site. Plan for "sign in again".
- Several Deets* apps run at once on this PC. Never hard-code a dev port.

## File map
Nothing yet. The planned layout is in [README.md](../README.md).
