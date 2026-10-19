---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# DeetsHealth — A brief for a security engineer (the phone agent)

> The one-pager to send an outside security engineer before a review. The full design is
> [SECURITY.md](SECURITY.md). The patient app's brief is
> [archive/PATIENT-APP-SECURITY-BRIEF.md](../archive/PATIENT-APP-SECURITY-BRIEF.md).

## Hi, and thank you
I am building a phone agent for hospitals, and I am not a security engineer. I have read the
standards and made a plan, but I do not know what I do not know. Please tell me where the plan
is wrong, where it is overbuilt, and where it is thin. Every question in §6 is a real question.

**No real patient data exists in this project.** Development uses public sandbox patients
(Epic, SMART Health IT) and simulated callers only. You will never need to see a real record or
hear a real call, and please do not use one.

## 1. What it is
- A hospital's patients call its number. Our agent answers, checks who the caller is, books or
  moves a visit, and answers direct questions from the chart (visits, medicines, refill state,
  results). It gives no medical advice.
- **The stack:** Twilio (phone, Media Streams) → our call loop (Pipecat, Python) with Cartesia
  (speech in and out) and the Anthropic API (the language model) → our Rust service (the FHIR
  client, sealing, the store) → the hospital's EHR (FHIR R4, SMART Backend Services).
- **A separate deploy per hospital** in our cloud: its own account, Postgres, KMS key. A web
  console for the hospital's staff, behind the hospital's own single sign-on.
- **The chart is never stored.** It is read for one call and dropped.

## 2. What I am trying to protect
Protected data is PHI and PII together: anything from the chart, and anything that points to a
person, **including the caller's phone number, voice, and the transcript** (SECURITY.md §2).

## 3. Who I think the threats are
1. Our own leaks: logs, traces, error reports, a support session.
2. A caller who is not the patient: knows the birth date, fakes the calling number.
3. **A caller who talks the model into something** (prompt injection by voice).
4. Insiders: hospital staff who read too much; our own staff.
5. One hospital's deploy reaching another's data.
6. A vendor breach or a supply-chain change.

## 4. The plan, layer by layer
- **TLS everywhere; every protected field sealed on its own** in Postgres under the hospital's
  own key (BYOK possible), bound to its table, row and column.
- **A wrapper type in Rust** (`Shielded<T>`) with no `Debug`, `Display` or `Serialize`: a leak
  into a log does not compile.
- **Two factors before any chart answer:** something known (birth date, or a chart fact whose
  question reveals nothing) and something held (a code by text, or a call back to the number on
  file) (SECURITY.md §4.4).
- **Logs, traces and metrics carry no protected data**; a redactor runs in the call pipeline.
- **Every vendor that touches protected data signs a BAA**; US-only processing.
- **Hospital staff sign in with their hospital's single sign-on**; each transcript view is
  audited; our own staff have no standing access.

## 5. The idea I most want poked at
**The model never chooses a patient.** The Rust service binds every chart tool to the patient
the verification step proved. A tool takes no patient argument, and a chart tool does not exist
in the call until verification passes (Pipecat Flows). So prompt injection can make the model
say odd things, but it cannot reach another patient's chart, and the value it speaks comes from
a template filled from the record (SECURITY.md §4.9). Where does this break?

## 6. Where I would love your help
1. Is caller verification (§4) strong enough for medicines and results by phone? Is a call back
   to the number on file a sound second factor?
2. Prompt injection by voice: what does the binding in §5 miss? Transcript text that reaches a
   tool argument? A tool reply that carries instructions?
3. The Python call loop holds the call's text as plain strings for one call. Is that a risk
   worth moving more of the loop into Rust?
4. Per-field sealing in Postgres under a per-hospital KMS key: sound, or overbuilt next to the
   cloud's own disk encryption?
5. What a hospital's security review will ask first, and what we should have ready (SOC 2 Type
   II, HITRUST, a penetration test).

## 7. What exists today
The design (SECURITY.md) and the Rust core from the patient app (the FHIR client, the
normalizer, sealing, the wrapper type). Nothing of the phone agent is built.

## 8. Where to read next
[SECURITY.md](SECURITY.md) (the rules), [VOICE-AGENT.md](VOICE-AGENT.md) §8 (the stack and the
vendors' BAAs), [DESIGN.md](../DESIGN.md) (what it does).
