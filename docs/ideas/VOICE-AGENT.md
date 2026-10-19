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
Decided 2026-10-19: **V1 B** (a phone agent), **F1 B** (convert this repo), and the stack:
Twilio, Cartesia or ElevenLabs, the Anthropic API, FHIR (§8). Then the owner chose:
- **The wedge: W1 and W3.** Non-Epic hospitals, and a better call than the rest. The owner:
  "Epic's agent factory is not quite user friendly". Then (same day): the code to call Epic's
  APIs is already built, so **build for every EHR from the start**: one `Scheduler` and reader
  interface, an adapter per EHR, Epic's first because it exists. What carries over: the FHIR
  client, the normalizer, the shapes. What does not: the sign-in. Today's code signs in as a
  patient (SMART standalone launch, PKCE). A hospital deploy signs in as a system (SMART
  Backend Services, a signed JWT), and Epic needs a backend app registration for it.
- **F3: A, then B.** Inbound first; outbound (reminders, confirmations) later.
- **F4: both.** Visits, medicines, refill state **and results**, in version 1. What follows
  from it (§7.1).

- **F2: A2, with the deploy target behind an interface** so B (the hospital's own cloud
  account) can be added later with no change to the service (§7.2).
- **F5: Cartesia** for both directions: Ink in, Sonic out.

1 real fork left: F6 (§8.6).

### 7.1 Results on the phone: what follows
- **Only results already released to the patient portal.** The agent says nothing the patient
  could not already read in their portal. Epic's Emmie keeps the same rule.
- **A stronger check before results.** A phone caller is weaker proof than a portal sign-in.
  Before any result: date of birth and a one-time code sent by text to the phone number on
  file. Visits and medicines need only the first check (§8.1).
- **Sensitive results are never read on the phone:** HIV and other STI tests, genetic tests,
  mental health and substance use (42 CFR Part 2), and a teen's confidential results. State
  laws differ; the hospital's settings can add to the list, never remove from it. The agent
  says "that result is in your portal" and offers a person.
- **The value, the reference range and the flag, read from the record.** No meaning, no advice.
  "Is that bad?" gets "your care team can explain it" and an offer to send a message or
  transfer.

### 7.2 F2 re-opened
The owner leaned to B ("best for HIPAA and ease of development"). Claude's push-back: HIPAA asks
for the same controls in either place, and B is the **harder** one to build and run: one deploy
per hospital, in clouds we do not control, often different clouds, with no view of its logs
when a call fails. What B gives is a sale: the hospital's security team sees PHI stay in its
own account. A third option gives that and keeps one way to run it:
- **A2 (recommended). A dedicated deploy per hospital, in our cloud:** a separate account,
  database and keys for each hospital; the hospital holds its own encryption key (BYOK) and
  can revoke it. One code base, one way to patch; the isolation a security team asks for.
- **A. One shared service** for all hospitals. Simplest; the weakest answer in a security review.
- **B. In each hospital's own cloud account.** Use only when a hospital requires it; plan it as
  an option for later, built on the same deploy scripts as A2.

### 7.3 Still open: F6
- **F6. How we build the call loop** (§8.6): A Pipecat, open source, run by us (recommended) ·
  B our own loop in Rust. Twilio ConversationRelay is out: it cannot use Cartesia (F5).

Every platform: the patient's side is a phone call, so it is the same on every phone. The
hospital's side is a web console; the Windows app is not part of it.

## 8. The stack
> **Part:** idea · 2026-10-19 · forks §7 F5, F6

The owner (2026-10-19): "eleven labs / Cartesia Sonic/Ink for voice, Anthropic API for llm and
FHIR for data with Twilio for phone connection".

### 8.1 The path of one call
```
caller ─► Twilio (phone number, audio 8 kHz μ-law)
            │ Media Streams WebSocket
            ▼
        our call service ──► STT (Cartesia Ink) ──► words
            │                                        │
            │          Claude (Messages API, tools) ◄┘
            │             │ tool calls: verify caller, find slots, book, read chart
            │             ▼
            │          EHR adapter (FHIR, per EHR) ──► the hospital's EHR
            ▼
        TTS (Cartesia Sonic) ──► audio back to Twilio ──► caller
```
- **Target: under about 800 ms** from the end of the caller's speech to the first sound back.
  Streaming at every step: STT partials, Claude streaming, TTS from the first sentence.
- **Claude only routes and words.** Every chart value comes from a tool result (the normalized
  model), not from the model's memory (§6). A tool returns what the template says out loud.
- **The caller is verified by a tool** (date of birth plus a match on the calling number or a
  code by text) before any chart tool unlocks. The model cannot skip it: the service does not
  register chart tools until the check passes.
- **One FHIR fetch per call**, after verification, for what the call needs; held in memory for
  the call only (LESSONS.md §3.3).

### 8.2 Each vendor and its BAA
Every row needs a signed BAA before the first real call. All facts from vendor pages and
third-party summaries; confirm each with the vendor in writing.

| Part | Vendor | HIPAA path | Watch |
|---|---|---|---|
| Phone | Twilio | BAA on the Security or Enterprise Edition. Programmable Voice, Media Streams and ConversationRelay are listed HIPAA-eligible. | Only eligible products are covered. Recording and transcription need their own controls. |
| STT and TTS | Cartesia (Ink, Sonic) | Enterprise plan: BAA, and zero data retention for STT and TTS. | No voice cloning: it is outside ZDR. Confirm ZDR covers the exact Ink model. |
| STT and TTS | ElevenLabs | Enterprise plan: BAA, with Zero Retention Mode on. | Its agent platform limits LLMs and tools in that mode (F6). |
| Language model | Anthropic API | "HIPAA readiness": sign the BAA in the Claude Console. The Messages API is eligible. | **Beta features are reported as outside the BAA**: keep the PHI path on GA features only. Claude Fable 5.1 needs 30-day retention; not needed here. |
| Data | the hospital's EHR | Covered by the hospital's own BAA with us. | §8.4. |

### 8.3 The language model
- The Messages API with our own tools (find slots, book, read visits, read medicines, verify
  caller, hand off to a person), streaming, prompt caching on the fixed system prompt.
- **Which model is a measure, not a fork now:** voice needs a fast first token. Start with
  Claude Opus 5.5 at `low` effort (its thinking cannot be turned off), and measure the time to
  first token and the tool accuracy against Claude Sonnet 5.5 and Claude Haiku 5.5 on the same
  recorded test calls. The owner picks from the numbers.
- Server-side refusal fallbacks are a beta feature: check whether the BAA covers them before
  turning them on.

### 8.4 FHIR: reads are common, writes are not
"FHIR for data" holds for **reads**: the R4 resources we already normalize. **Booking is
different for each EHR** (secondary sources; confirm on each vendor's developer portal):
- **Epic:** `Appointment/$find` and `$book`, STU3, after the hospital builds its Cadence rules.
- **Oracle Health:** R4 `POST /Appointment` and status updates; the hospital picks which
  providers and locations are open to booking.
- **athenahealth:** booking is its own REST API, not FHIR.
- **MEDITECH Expanse:** scheduling APIs listed; write support not confirmed.

So the core holds one `Scheduler` trait, and each EHR gets an adapter, as the patient app holds
its platform traits. This bears on the wedge (§5): non-Epic hospitals mean more adapters.

### 8.5 What it costs to run
Not quoted yet. Before the first build, quote a cost per call minute: Twilio per minute, STT
and TTS per minute, Claude tokens per turn with caching. Hospitals buy per call or per minute.

### 8.6 The call loop (F6): what the digging found
> **Part:** idea · 2026-10-19 · forks §7 F6

**Twilio ConversationRelay is out.** Its speech-out engines are Google, Amazon and ElevenLabs;
its speech-in engines are Google and Deepgram. **No Cartesia**, so it breaks F5. It also adds
about $0.07 a minute; Media Streams adds $0.0044. With ConversationRelay, Twilio decides
turn-taking and interruption; we get text only, and cannot tune either. ElevenLabs' agent
platform is out too (§7, before).

So the loop is ours, on **Twilio Media Streams** (a two-way WebSocket of 8 kHz μ-law audio,
about $0.013 a minute for a US inbound call with the voice rate). The real fork is how to build it.

**The hard parts of a call loop**, in order of what the caller notices:
1. **Turn-taking:** knowing the caller has finished, not just paused. Too early and the agent
   talks over them; too late and the line feels dead. Cartesia's Ink-2 has turn detection built
   in.
2. **Barge-in:** the caller talks while the agent speaks. Stop the audio at once, clear
   Twilio's buffer, cancel the Claude stream and the TTS stream, keep what was already said in
   the history.
3. **Latency:** stream every step, start TTS on the first sentence, host near Twilio's media
   region (US1).
4. **Fillers and silence:** "One moment, I am checking the schedule" while a slow EHR call runs.
5. **Phone audio:** 8 kHz μ-law in and out; resample with care. One guide reports that
   Pipecat's Smart Turn v3 fails silently at 8 kHz; Ink-2's own turn detection avoids it.

| | A. Pipecat (recommended) | B. Our own loop in Rust |
|---|---|---|
| What it is | Open-source Python framework for voice agents (from Daily). Has a Twilio Media Streams serializer, Cartesia STT and TTS, Anthropic, interruption handling. | A tokio service: the WebSocket, the audio, turn-taking, barge-in, all ours. |
| The caller's experience | Turn-taking and barge-in already tested by many teams. | As good as we make it; months to reach the same polish. |
| PHI wall | Runs in our A2 deploy; no Pipecat company in the path (do not use Pipecat Cloud: another vendor and BAA). Python has no `Shielded<T>`: the loop holds PHI in plain strings. | One language with the core; `Shielded<T>` from the socket to the EHR. |
| EHR code | The adapters stay in the Rust core, as a small internal service the Pipecat tools call. Two languages, one network hop inside the deploy. | One process. |
| Fits F2 (swap the deploy) | Yes: a container. | Yes: a container. |
| Risk | A dependency on a young framework; pin it and read each upgrade. | All the bugs are ours. |

Recommended A, because the caller notices turn-taking first, and Pipecat is ahead there. The
PHI held in Python is in memory only, for one call, inside our deploy; the log rules hold for
it as they do for the core (SECURITY.md §4.5).

**One gap for F4 and the patient:** Ink-2 is English only (Cartesia changelog, 2026). Many
callers will want Spanish first. Check Cartesia's multilingual STT (Ink-Whisper) for Spanish
before version 1, or plan a second STT for it.

## 9. Sources
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
- [Twilio — ConversationRelay is now HIPAA eligible](https://www.twilio.com/en-us/changelog/conversationrelay-is-now-hipaa-eligible)
- [Twilio — ConversationRelay docs](https://www.twilio.com/docs/voice/twiml/connect/conversationrelay)
- [Twilio — HIPAA eligible products (PDF)](https://twilio-cms-prod.s3.amazonaws.com/documents/HIPAA-eligible-products-and-services.pdf)
- [Cartesia — Zero Data Retention](https://docs.cartesia.ai/enterprise/zero-data-retention)
- [Cartesia — Sonic](https://www.cartesia.ai/sonic) · [Ink](https://www.cartesia.ai/ink)
- [ElevenLabs on X — Conversational AI is HIPAA compliant](https://x.com/ElevenLabs/status/1900226974175351055?lang=en)
- [ElevenLabs — Privacy and compliance (DeepWiki summary)](https://deepwiki.com/elevenlabs/elevenlabs-docs/5.2-privacy-and-compliance)
- [Anthropic — API and data retention](https://platform.claude.com/docs/en/manage-claude/api-and-data-retention)
- [IntuitionLabs — Is Claude HIPAA compliant](https://intuitionlabs.ai/articles/is-claude-hipaa-compliant)
- [Oracle Health — FHIR R4 Appointment](https://docs.oracle.com/en/industries/health/millennium-platform-apis/mfrap/api-appointment.html)
- [athenahealth — Appointment](https://mydata.athenahealth.com/static/apidoc/ig-ge-appointment.html)
- [Taction — athenahealth API guide](https://www.tactionsoft.com/blog/athenahealth-api-integration-guide/)
- [Twilio — ConversationRelay voice configuration](https://www.twilio.com/docs/voice/conversationrelay/voice-configuration)
- [Twilio — ConversationRelay best practices](https://www.twilio.com/docs/voice/conversationrelay/best-practices)
- [Twilio — ElevenLabs voices in ConversationRelay](https://www.twilio.com/en-us/blog/integrate-elevenlabs-voices-with-twilios-conversationrelay)
- [Twilio — Voice pricing (Media Streams)](https://www.twilio.com/en-us/voice/pricing/gb)
- [Cartesia — Twilio pricing 2026](https://www.cartesia.ai/learn/twilio-pricing)
- [Zian AI — Twilio vs Telnyx costs](https://zian.ai/twilio-vs-telnyx-ai-voice-agents/)
- [Evalgent — Pipecat and Twilio](https://www.evalgent.com/blog/pipecat-twilio-telnyx-phone-calls)
- [Cartesia — Changelog 2026](https://docs.cartesia.ai/changelog/2026)
- [Pipecat in production guide](https://luonghongthuan.com/en/blog/pipecat-voice-agent-production-scalable-guide/)
