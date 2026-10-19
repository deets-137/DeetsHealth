---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# DeetsHealth — Debugging (the phone agent)

> Not built: nothing of the phone agent runs yet. How a call is read after it goes wrong, and
> the tools that make that possible, each one PHI-free by rule (SECURITY.md §4.5). The test
> layers that make calls on purpose are VOICE-AGENT.md §9. The patient app's tools (the fetch
> ledger, the sealed-store inspector) are
> [archive/PATIENT-APP-DEBUGGING.md](../archive/PATIENT-APP-DEBUGGING.md).

## 1. Terms
- **Call id** — an opaque random id for one call. It joins every line, trace and row of that
  call. It carries no protected data.
- **The call log** — one Postgres row per call: time, length, Flow path, outcome, latency per
  step (VOICE-AGENT.md §8.7). Its plain columns carry no protected data.
- **The call timeline** — one trace per call: every step with its start and its latency, in
  OpenTelemetry.
- **The EHR ledger** — one log line per call to the EHR (§3).
- **The redactor** — the Pipecat processor in front of every log and metric (SECURITY.md §4.5).

## 2. The tools
| Tool | What it shows | State |
|---|---|---|
| The call log | Which calls ended how; which Flow node each one left from | ⬜ |
| The call timeline | Where the time went in one call; the 800 ms target per turn | ⬜ (Jaeger in Docker, locally) |
| The EHR ledger | Each EHR call: what, status, size, time, outcome | ⬜ (the patient app's ledger carries, §3) |
| The transcript | What was said. Synthetic calls only in dev; a real one only through the console, audited | ⬜ |
| Recordings | The audio, when a rule kept it (VOICE-AGENT.md §8.7) | ⬜ |
| Record and replay | Re-run a call's EHR replies, STT results and Claude replies with no network | ⬜ (VOICE-AGENT.md §9) |

## 3. The EHR ledger
The patient app's fetch ledger (archive/PATIENT-APP-DEBUGGING.md §3) carries to the per-call reads.
One `INFO` line per HTTP call, as it ends:
```
ehr: call 7f3c… Patient?birthdate&phone p1 · 200 · 1 match · 2.1 KB · 180 ms · ok
ehr: call 7f3c… Appointment/$find · 200 · 6 slots · 4.0 KB · 640 ms · ok
ehr: call 7f3c… Observation?category=laboratory p1 · 403 · 0.4 KB · 95 ms · issues error:forbidden · refused
```
- The search names its **parameter names** only, never their values.
- `issues` are FHIR's closed words (`fhir::issue_word`), never the site's text.
- Never the patient id, a name, a phone number or a value.

## 4. Recipes
**"A call ended badly."** The call log row (outcome, last node) → the call timeline for that
call id → the EHR ledger lines with that call id → for a synthetic call, the transcript.

**"Turns feel slow."** The call timeline: which step passed its share of the 800 ms (STT end of
turn, Claude's first token, TTS's first audio, an EHR call). Then the record-and-replay run of
the same call, to see whether the cost is ours or a vendor's.

**"Callers fail verification."** The call log by outcome `verify-failed`, and the audit
trail's failure rows (the factor that failed, never the answer given). A rise after a settings
change points at the hospital's question list.

**"The agent said something wrong."** A synthetic call: replay it with the cached replies and
read the tool results against the words spoken. A value in speech that no tool returned is a
template bug (SECURITY.md §4.9).
