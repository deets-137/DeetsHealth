---
status: sop
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — Docs organization

The rules are DeetsMusic's (`../DeetsMusic/docs/DOCS-ORG.md`), in short:

1. Every doc under `docs/` starts with front matter: `status`, `desk_test`, `sources`, `updated`.
2. A part of a doc with its own state carries `> **Part:** designed · YYYY-MM-DD` under its heading.
3. HANDOFF.md holds what is true now. WORKLOG.md holds each sitting. CLAUDE.md holds no state.
4. Area folders: `architecture/` (the engine), `cards/`, `features/`, `integrations/`,
   `ops/`, `ideas/` (not built). Make a folder when its first doc arrives.
5. A new doc gets one line in CLAUDE.md.
6. No real PHI in any doc. Sandbox patient names are fine; they are public fakes.

Tooling (`docs:check`, `docs-move.mjs`) is copied from DeetsMusic when the scaffold lands.
