# DeetsHealth

A lightweight patient health-record viewer for Windows 11. It reads **your own** record from
Epic (and other SMART on FHIR servers) and shows it in cards you can arrange.

**Status: project — the empty shell is being built.** The first target is SMART Health IT's
public sandbox and its synthetic patients, never a real record. Epic's sandbox (Camila Lopez
first) is on hold until the app can be registered there (docs/HANDOFF.md › Open now).

It is a sibling of [DeetsMusic](../DeetsMusic). The stack, the folder layout, the docs
system and the working style are copied from it on purpose, so one set of habits works in
both. The differences are listed in [docs/DESIGN.md](docs/DESIGN.md) §3.

## Terms
- **FHIR** — the standard health-data format and REST API. Epic serves FHIR R4.
- **SMART on FHIR** — the OAuth 2.0 sign-in layer on top of FHIR.
- **Sandbox** — Epic's non-production FHIR server with fake patients (fhir.epic.com).
- **PHI** — protected health information: anything in a real record.

## Stack (same as DeetsMusic)
- **Tauri v2 + WebView2** shell.
- **Vanilla TypeScript** front end, Vite build, no framework.
- **Rust** back end: OAuth, FHIR fetch, normalization, SQLite cache.
- Token tiers: palette → theme → skin, the same as DeetsMusic.

## Surfaces
Two only: **Midi** and **Max**. There is no Mini (DeetsMusic's Mini is a player; a health
record has nothing to "play").

## Layout
```
DeetsHealth/
  CLAUDE.md          project guide for Claude
  README.md          this file
  docs/              HANDOFF, WORKLOG, DESIGN, DOCS-ORG + area folders
  src/               TS front end (cards, surfaces, tokens)       — the empty shell
  src-tauri/         Rust back end (auth, fhir, normalize, db)    — the shell; no FHIR yet
  scripts/           dev-app, docs-check, docs-move, tokens, fhir-probe
```

## Repo
Local only. There is **no remote** and nothing is pushed: a GitHub repo named DeetsHealth
already exists for something else. If this project goes on, the owner decides where it lives.

## Start here
[docs/HANDOFF.md](docs/HANDOFF.md).
