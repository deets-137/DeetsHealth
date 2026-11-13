---
status: project
desk_test: none
sources: []
updated: 2026-11-13
---
# DeetsHealth — Design Doc

> The product intent and the v1 scope of the hospital phone agent (2026-10-19). The research
> and the reason for each decision are [architecture/VOICE-AGENT.md](architecture/VOICE-AGENT.md);
> the move from the patient app is [CONVERSION.md](CONVERSION.md); the state of play is
> [HANDOFF.md](HANDOFF.md). The patient app's design is
> [archive/PATIENT-APP-DESIGN.md](archive/PATIENT-APP-DESIGN.md).
> Status legend: ✅ decided · 🔵 proposed (awaiting the owner's call) · ⬜ not yet discussed

---

## 0. What it is

A phone agent that a hospital runs for its patients. A patient calls the hospital's number;
the agent books, moves and cancels visits, and answers direct questions from the patient's
chart. We build and maintain the stack; each hospital sets it up and runs it for its own
patients. The repo keeps the name DeetsHealth, and may hold other health products later (the
owner, 2026-10-19).

**Two surfaces:**
- **The call** — the patient's side. A phone call, so it is the same on every phone. The agent
  speaks, listens, and hands off to a person at any time.
- **The console** — the hospital's side. A web app where hospital staff set the agent up,
  watch its calls and read what it did. It is built from the patient app's UI toolkit
  (tokens, themes, skins, primitives, the card engine).

**Who it is for, in order:** the patient on the phone first (LESSONS.md §5.1), the hospital's
staff second, us third.

---

## 1. Decided so far ✅

| Area | Decision |
|---|---|
| Product | A hospital-run phone agent: scheduling and chart questions (VOICE-AGENT.md §2) |
| Customer | The hospital (a covered entity); we are its business associate, under a BAA |
| Wedge | Hospitals not on Epic, and a better call than the rest; every EHR from the start, Epic's adapter first because the code exists (VOICE-AGENT.md §7) |
| Calls | Inbound first; outbound (reminders, confirmations) later |
| Chart answers | Visits, medicines, refill state and results; results with guards (VOICE-AGENT.md §7.1) |
| EHR access | FHIR R4 reads through the kept core; booking through one interface with an adapter per EHR; SMART Backend Services sign-in (a system, not a patient) |
| Phone | Twilio Programmable Voice and Media Streams |
| Voice | Cartesia: Ink in, Sonic out |
| Language model | The Anthropic API (Messages API, tools, streaming); which model is a measure (VOICE-AGENT.md §8.3) |
| Call loop | Pipecat with Pipecat Flows, run by us, open to heavy tuning (VOICE-AGENT.md §8.6) |
| Store | Postgres, one per hospital, every protected field sealed under the hospital's own key (VOICE-AGENT.md §8.7) |
| Call audio | Not stored by default; on by rule or by a hospital admin (VOICE-AGENT.md §8.7) |
| Deploy | A separate deploy per hospital in our cloud (A2), behind an interface so a hospital's own cloud account (B) can follow |
| Cloud | AWS `us-east-1`, next to Twilio's US1 media edge ([CLOUD.md](integrations/CLOUD.md)) |
| The record | Never stored: held in memory for one call only |
| Testing | Local first: the SMART and Epic sandboxes through record and replay, a fake scheduler, the call layers of VOICE-AGENT.md §9 |
| Process | As DeetsMusic: design on paper first, the owner decides every fork, one load-bearing feature in flight |

### Open technical risks
- **Latency.** The target is under about 800 ms from the end of the caller's speech to the
  first sound back. Not measured yet (VOICE-AGENT.md §8.1).
- **Epic's booking on the sandbox.** `$find` and `$book` are STU3 and need Cadence rules; their
  support on Epic's sandbox is not confirmed (VOICE-AGENT.md §8.4).
- **Spanish.** Cartesia's Ink-2 is English only; a Spanish path is not chosen (VOICE-AGENT.md
  §8.6).
- **BAAs.** Every vendor's BAA and zero-retention terms are from public pages, not signed
  (VOICE-AGENT.md §8.2).

---

## 2. Decision agenda (one at a time)

- **A. v1 feature scope** — ✅ inbound calls; scheduling and chart answers (§A below)
- **B. The call design** — ✅ the Flows node map (VOICE-AGENT.md §8.1); 🔵 the words the agent
  says, its name and its voice
- **C. The console** — ✅ where it runs: a browser only, installable as a web app (the owner,
  2026-11-11, fork 1A); ✅ the shell: DeetsMotion's top bar, nav and pages: Chat, Analytics,
  Teams and Config at the top; Security, Audit Logs and Settings at the foot; Analytics, Teams,
  Config, Security, Audit Logs and Settings each with its own nav (the owner, 2026-11-13;
  [CONSOLE.md](architecture/CONSOLE.md) §3); ⬜ what each page shows, staff sign-in
- **D. Visual identity of the console** — ✅ DeetsMotion's look: three themes (Dark, Light,
  Moonlight) and one skin, Glass; the patient app's six themes, other skins and card engine
  are archived (the owner, 2026-11-11, fork 2A; CONSOLE.md §3, §4)
- **E. Data model** — ✅ Postgres with sealing, what it holds (VOICE-AGENT.md §8.7); ⬜ the
  schema
- **F. Technical architecture** — ✅ the stack (§1); 🔵 the repo layout (CONVERSION.md §6, a
  draft)
- **G. Security and compliance** — ✅ [SECURITY.md](architecture/SECURITY.md): S1 the hospital's own
  single sign-on, S2 two factors for any chart answer; 🔵 its drafts (§7 there)
- **H. Vendors and credentials** — ⬜ the accounts, the BAAs, the Epic backend app
  registration
- **I. Cost** — ⬜ the cost per call minute, quoted before the first build

---

## A. v1 feature scope ✅

**Nucleus: one inbound call that books a visit.** We grow outward feature by feature, the
owner directing priority.

### Feature backlog

Status: ✅ done · 🔵 designing · ⬜ todo · ❄️ deferred to later

**The call**
| # | Feature | Status |
|---|---|---|
| C1 | Answer and greet; the recording notice when any rule can record | ⬜ |
| C2 | Verify the caller, factor 1: date of birth, or a random chart fact whose question reveals nothing (SECURITY.md §4.4) | ⬜ |
| C3 | Verify the caller, factor 2: a code by text or a call back to the number on file; needed for any chart answer or booking | ⬜ |
| C4 | Hand off to a person, from any node | ⬜ |
| C5 | Emergency words: a fixed 911 line at once, spoken by code (a phrase screen, then the model's tool); the call goes on. The transfer, a nurse joined to the call, and the emergency flag are hospital settings (FUTURE-SETTINGS.md §1; the owner's option A, 2026-11-11) | 🔵 (the line built in Phase 1; the transfer waits on Phase 3) |
| C6 | Turn-taking and barge-in | ⬜ |
| C7 | Filler speech while a slow EHR call runs | ⬜ |
| C8 | Spanish | ❄️ (the Spanish path is an open risk, §1) |

**Scheduling**
| # | Feature | Status |
|---|---|---|
| S1 | Find open slots for a visit type | ⬜ |
| S2 | Book | ⬜ |
| S3 | Move | ⬜ (Epic has no API for it: hands off to staff for now, EPIC.md §2.2) |
| S4 | Cancel | ⬜ (Epic has no API for it: hands off to staff for now, EPIC.md §2.2) |
| S5 | "When is my next visit?" | ⬜ |

**Chart answers**
| # | Feature | Status |
|---|---|---|
| R1 | Visits, past and coming | ⬜ |
| R2 | Medicines | ⬜ |
| R3 | Refill state | ⬜ (`MedicationDispense (Fill Status)` serves it to a backend app, EPIC.md §2.2) |
| R4 | Results: released to the portal only, sensitive results never, the value, range and flag only (VOICE-AGENT.md §7.1) | ⬜ |

**The console**
| # | Feature | Status |
|---|---|---|
| K1 | Staff sign-in: the hospital's own single sign-on (SAML or OIDC) | ⬜ (SECURITY.md §4.10) |
| K2 | The hospital's settings: hours, visit types, transfer numbers, the sensitive-results list | ⬜ |
| K3 | The call log and the latency of each step | ⬜ |
| K4 | Read a transcript (each view is audited) | ⬜ |
| K5 | Recording rules and retention | ⬜ |

**Cross-cutting (not screens, but real work)**
| # | Feature | Status |
|---|---|---|
| X1 | The EHR interface and the Epic adapter | ⬜ |
| X2 | SMART Backend Services sign-in | ⬜ (step 5 of the conversion) |
| X3 | The store, sealing and the KMS key | ⬜ |
| X4 | The deploy interface, A2 first | ⬜ |
| X5 | The test layers: record and replay, the text harness, simulated callers, the fake Twilio | ⬜ |
| X6 | PHI-free logging and the call timeline | ⬜ |
| X7 | The audit trail | ⬜ |

### Not goals
- **No medical advice, no triage.** The agent says what the chart says, and nothing about
  what it means. A question about meaning goes to the care team.
- **No chart cache.** The record lives in memory for one call.
- **No call audio by default.**
- **No outbound calls in v1.** They come after inbound works.
