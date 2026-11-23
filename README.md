# DeetsHealth

A plug-and-play, no-code agent harness for healthcare professionals. A clinic manager or a
nurse lead sets up a patient-facing agent in a web console: its words, the clinic's lists and
hours, the booking rules, the voice, the model and who on staff may do what. Then they watch
every call, review it and score it. No code.

The first agent is a phone agent. A patient calls the clinic's number. The agent books visits
and answers direct questions from the patient's chart, after it verifies the caller with two
factors. The model never chooses a patient: every tool is bound to the verified patient in Rust.

**Status: a portfolio piece, before v1.** Synthetic callers and sandbox patients only (Epic's
public sandbox and a local one). No real record and no real call. Going live with a real clinic
would need every vendor's BAA signed first (SECURITY.md §5). The aim is
[docs/DESIGN.md](docs/DESIGN.md) §0; the plan is [docs/BUILD-ORDER.md](docs/BUILD-ORDER.md).

The repo was a patient health-record viewer for Windows until 2026-10-19. That app's code is in
`archive/` and on the branch `archive/patient-app`; the move is
[docs/CONVERSION.md](docs/CONVERSION.md).

## Terms
- **The agent** (`agent/`): the Python call loop. Pipecat runs the call; Claude is the model.
- **`ehr`** (`ehr/`): the Rust service. It verifies the caller, binds every tool to the
  verified patient, and talks to the EHR. The agent reaches it over gRPC with mTLS.
- **The console** (`console/`): the staff web page. Vanilla TypeScript and Vite, no framework.
- **The core crate** (`core/`): Rust code shared from the patient app (the wrapper type for
  protected data, sealing, FHIR).
- **PHI / PII:** protected data. Any identifier counts: a name, a phone number, the call's
  audio, the transcript.
- **Sandbox:** fake patients (Epic's and SMART's sandboxes, and `ehr/fixtures/sandbox.json`).

## Stack
Twilio (the phone line) · Cartesia (speech) · Pipecat (the call loop) · the Anthropic API (the
model) · FHIR (the EHR; Epic first) · Rust (`ehr`, the core crate) · Postgres (the store; dev
cluster in `.pg/`) · a web console. The reasons are
[docs/architecture/VOICE-AGENT.md](docs/architecture/VOICE-AGENT.md).

## Run it
```
npm install
cd agent && uv sync && cd ..
cargo run -p deetshealth-ehr --example dev_certs   # once: the dev certificates
npm run dev                                        # the agent's dev server + the console
npm run check                                      # the checks your changes need
```
`npm run dev` prints the console's address. The full setup, every check and the gotchas are
[docs/HANDOFF.md](docs/HANDOFF.md) › Run it.

## Layout
```
DeetsHealth/
  agent/      the Python call loop, its dev server, tests and dev call records
  ehr/        the Rust service, its gRPC contract (proto/), the JWKS worker
  core/       the shared Rust crate
  console/    the staff web console
  scripts/    dev, check, docs-check, docs-move, tokens, types, postgres, fhir-probe
  docs/       HANDOFF, WORKLOG, DESIGN, LESSONS, DOCS-ORG and the area folders
  archive/    the patient app's code: not built, not checked
```

## The APIs
The dev API between the console and the agent, and a map to every other API (gRPC to `ehr`,
the model's tools, Twilio, the JWKS worker), is
[docs/architecture/DEV-API.md](docs/architecture/DEV-API.md).

## Repo
The public GitHub repo `deets-137/DeetsHealth` (public since 2026-11-22). Synthetic data only:
never commit a key, a token or a real record.

## Start here
[docs/HANDOFF.md](docs/HANDOFF.md), then [docs/DESIGN.md](docs/DESIGN.md).
