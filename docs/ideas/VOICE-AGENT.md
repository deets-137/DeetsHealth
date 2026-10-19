---
status: idea
desk_test: none
sources: []
updated: 2026-10-19
---
# Idea — a hospital-run phone agent

> Not built, not designed: forks open (§7). Do not tell a user the app does this.
> The owner (2026-10-19): a phone agent, "changing scope of deets health entirely. Meant to be
> implemented and maintained by hospitals with us building and maintaining the tech stack",
> "dedicated to help schedule and answer direct questions from the chart". **This repo will be
> converted** (fork F1 B), but not yet: this doc is the idea only. Until the conversion starts,
> DESIGN.md, SECURITY.md and CLAUDE.md still describe the patient app and still hold.

## 1. Terms
- **Phone agent** — software that answers a patient's phone call (or makes one), speaks, and
  acts in the hospital's EHR.
- **Covered entity** — the hospital. HIPAA binds it directly.
- **Business associate** — a vendor that handles PHI for a covered entity. Under this idea,
  that is us. HIPAA then binds us too.
- **BAA** — business associate agreement: the contract HIPAA requires between the two, and
  between us and each vendor of ours that touches PHI.
- **Access center** — the hospital's call center for scheduling and general questions.
- **Cadence** — Epic's scheduling module. Its rules decide which slots a visit type may use.
- **Showroom** — Epic's catalog of outside apps (it replaced App Orchard). **Connection Hub** is
  its open list of any vendor with a live Epic integration.
- **Emmie** — Epic's own patient-facing AI assistant. **Agent Factory** — Epic's no-code tool for
  a hospital to build its own AI agents.

## 2. What the owner asked for
1. **Scheduling** on the phone: book, move and cancel visits.
2. **Direct questions from the chart**: "when is my next visit", "what was my last A1C", "is
   my refill sent".
3. **Run by the hospital, built by us.** The hospital configures it and owns the relationship
   with its patients. We build and maintain the stack.

## 3. What Epic offers (researched 2026-10-19)
All from trade press and Epic customer announcements; epic.com did not load from this
session. Each line needs a check against Epic's own materials (or the hospital's Epic
representative) before anyone relies on it.

| Epic product | What it does | State, October 2026 |
|---|---|---|
| **Emmie in MyChart** | Patient assistant. Explains results with context, answers open questions from the chart, takes actions such as scheduling. Sees only what the patient can already see in MyChart. | Live at several systems: Sutter Health first with **Ask Emmie** (chart Q&A), then Community Health Network (July 2026), Reid Health. |
| **Emmie by text (SMS)** | Confirm and reschedule visits by text. | Live. Ochsner: 14,900+ visits rescheduled, about 750 staff hours saved (vendor-reported). |
| **Schedule with Emmie** | Scheduling in Emmie. | Listed for October 2026 (Becker's). |
| **Post-discharge voice calls** | Outbound AI call after discharge; suggests follow-up visits. Built with Agent Factory. | Announced; no firm date. |
| **Incoming voice AI and voice referral scheduling** | Emmie answers the phone. | **2027** (Healthcare IT Today, UGM 2026). |
| **Agent Factory** | Hospitals build, test and watch their own agents with no code. External connectors and partner platforms are planned. | Early-adopter training from October 2026; broad release 2027. |

Two things Epic said that matter to us:
- At UGM 2025, an Epic VP said that in the future Emmie "will answer his call with voice …
  and will make outbound calls to help him schedule his appointment". So **inbound voice for
  scheduling and chart questions is Epic's own roadmap, for 2027.**
- Epic called voice "the hardest channel to get right": outbound answer rates of about 15–30
  percent, and high infrastructure cost (Health Management Academy, before UGM 2026).

**Others already in this market** (not complete):
- **Hyro** — voice agents inside Epic for access-center work and scheduling (Tampa General,
  November 2025); claims 45+ health systems.
- **RingCentral** — a voice-first AI agent platform with 80+ EHR integrations (HIMSS26).
- **Zoom Contact Center** — reported as coming to Epic's catalog; supports outside voice bots
  through SIP. Not confirmed.
- Assort Health, Hippocratic AI and others: not checked this time.

## 4. How a phone agent reaches Epic
- **Sign-in as a system, not a patient:** SMART Backend Services (a signed JWT, no user). The
  hospital approves our app in its Epic instance. Listing on Showroom / Connection Hub is how
  hospitals find and trust a vendor.
- **Scheduling:** Epic's `Appointment/$find` (the server proposes slots under Cadence rules) and
  `Appointment/$book`. Reported as STU3 only, and it works only after the hospital builds its
  Cadence rules for that visit type. Needs a check on fhir.epic.com.
- **Chart questions:** the R4 reads we already use (Observation, MedicationRequest,
  Appointment, Condition, AllergyIntolerance …). The normalizer and FHIR-SHAPES.md carry over.
- **Refill status, messages to the care team:** need a check: which of these Epic exposes to a
  backend app, and which need an Epic-side workflow (an In Basket message) instead.

## 5. The push-back: where do we fit?
**Epic plans the same product for 2027, built into the EHR the hospital already runs.** A
hospital on Epic will ask why it should not wait for Emmie, or build the agent itself in Agent
Factory. We need one clear answer before we build. The answers that hold up, best first:
1. **Hospitals not on Epic.** Oracle Health, MEDITECH, athenahealth and others get no Emmie.
   One agent over FHIR, with an adapter per EHR, serves them. Epic is about 40 percent of US
   hospitals; the rest is the larger count of sites.
2. **The window before 2027**, for Epic hospitals with long hold times now. It is real but
   short, and Epic's own product ends it.
3. **Better at the call itself:** identity checks, medical words, handoff to a person, many
   languages, latency. Hard to prove before we build, and Hyro already sells on it.
4. **Run on Agent Factory later**, when Epic opens external connectors. Then we are a partner,
   not a rival. Too early to plan on.

What carries over from this repo: the FHIR normalizer, the probe and FHIR-SHAPES.md, the data
model, the `Shielded<T>` habit, the docs system. What does not: the Tauri app, the sealed
store on the PC, DPAPI keys, the patient's SMART sign-in.

## 6. What follows (not forks)
- **HIPAA binds us.** A BAA with each hospital, and with every vendor of ours that touches PHI:
  the phone carrier, speech to text, the language model, the cloud.
- **The non-goals of the patient app go** (DESIGN.md §5): a server hears PHI, and booking is a
  write to the EHR. The conversion rewrites DESIGN.md and SECURITY.md.
- **The caller's identity is checked before the agent says any PHI.**
- **A person is always one step away.** Emergency words transfer the call at once. No
  medical advice, no triage.
- **A chart answer is read from the record, not written by the model.** The model picks what
  to look up; the value comes from the normalized model (the rule already in ideas/README for
  the on-device model). It cannot say a number the chart does not hold.
- **Recordings and transcripts are PHI.** The hospital sets how long they are kept.
- **Hospital security reviews ask for a SOC 2 Type II report**, often HITRUST too. Plan for it.

## 7. Forks (for the owner)
Decided 2026-10-19: **V1 B** (a phone agent) and **F1 B** (convert this repo). 4 real forks
left; the wedge (§5) is a question first.

- **W. The wedge** (§5): 1 non-Epic hospitals (recommended: no Emmie there, and the larger count
  of sites) · 2 the window before Epic's 2027 voice · 3 better calls. A question, not a fork:
  the answer decides which EHR adapter comes first.
- **F2. Who runs the servers:** A we host one HIPAA-grade cloud service, each hospital
  configures its own (recommended: patches reach every hospital at once) · B it runs in each
  hospital's own cloud account (Azure, AWS) and we ship updates (security teams like it;
  patches arrive slowly) · C on the hospital's own servers (not recommended).
- **F3. Call direction for version 1:** A inbound: the patient calls, the agent schedules and
  answers (recommended: what the owner asked; hold time is the patient's pain) · B outbound:
  reminders and confirmations (simpler, but Epic says answer rates are 15–30 percent).
- **F4. Chart answers in version 1:** A the visit list, medicines and refill state only, no
  results (recommended first) · B results too, read to a verified caller (helps the patient
  more; brings clinical risk and the question of how a result is said with no clinician).

Every platform: the patient's side is a phone call, so it is the same on every phone. The
hospital's side is a web console; the Windows app is not part of it.

## 8. Sources
- [Epic — Emmie for Patients](https://www.epic.com/software/emmie/)
- [Epic on X — Introducing Emmie](https://x.com/HeyEpic/status/1957844340882936118)
- [Becker's — Community Health Network launches Emmie](https://www.beckershospitalreview.com/healthcare-information-technology/ehrs/community-health-network-launches-epics-emmie-in-mychart/)
- [Becker's — 34 new Epic features coming, and when](https://www.beckershospitalreview.com/healthcare-information-technology/innovation/34-new-epic-features-coming-and-when/)
- [Healthcare IT Today — Epic UGM 2026](https://www.healthcareittoday.com/2026/08/19/epic-ugm-2026-judy-faulkner-keynote-and-cool-stuff-ahead/)
- [Healthcare IT Today — Inside Agent Factory](https://www.healthcareittoday.com/2026/09/03/inside-epics-push-to-help-customers-build-their-own-ai-agents-using-agent-factory/)
- [Distilled Post — Inside Epic UGM 2026](https://www.distilledpost.com/post/inside-epic-ugm-2026-the-biggest-reveals-from-cool-stuff-ahead)
- [Health Management Academy — Emmie and the digital front door](https://hmacademy.com/insights/all-access/health-technology/epics-emmie-and-the-new-race-to-control-healthcares-digital-front-door)
- [Fierce Healthcare — HIMSS26: Epic previews Agent Factory](https://www.fiercehealthcare.com/ai-and-machine-learning/himss26-epic-expands-ai-roadmap-previews-factory-build-and-orchestrate-ai)
- [HIT Consultant — HIMSS26 wrap-up](https://hitconsultant.net/2026/03/10/himss26-tuesday-wrap-up/)
- [MobiHealthNews — AI voice tools (Hyro and Tampa General)](https://www.mobihealthnews.com/tag/ai-backed-healthcare-voice-tool)
- [Nirmitee — FHIR scheduling APIs in Epic, Oracle, athenahealth](https://nirmitee.io/blog/fhir-scheduling-api-appointment-slot-epic-oracle-athenahealth/)
- [FHIR chat — Appointment $find vs Slot.search](https://chat-archive.fhir.org/stream/179166-implementers/topic/Appointment.20.24find.20vs.20Slot.2Esearch.html)
