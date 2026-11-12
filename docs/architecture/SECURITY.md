---
status: designed
desk_test: none
sources: []
updated: 2026-11-12
---
# DeetsHealth — Security and privacy (the phone agent)

> Designed 2026-10-19, before any phone-agent code. Forks S1 and S2 are the owner's (§9). The
> patient app's security design, which most of these rules come from, is
> [archive/PATIENT-APP-SECURITY.md](../archive/PATIENT-APP-SECURITY.md). The product is
> [DESIGN.md](../DESIGN.md); the stack is [VOICE-AGENT.md](VOICE-AGENT.md) §8.

## 1. Priority
1. **The patient's experience.**
2. **Privacy and security.**
3. Everything else: the hospital staff's convenience, speed of build, our convenience.

Privacy and security come directly after the patient's experience. The PHI wall is also a stop
rule (LESSONS.md §2): a design that trades it away comes to the owner first. Look for a design
that gives both (LESSONS.md §3.12).

## 2. Terms
- **PHI** — anything from a record: visits, medicines, results, allergies, notes, dates of care.
- **PII / identifier** — anything that points to a person, even with no health fact: name, date
  of birth, address, **phone number**, MRN, the FHIR `Patient` id, **the caller's voice**.
- **Protected data** — PHI and PII together. Every rule below says "protected data". A call's
  audio, its transcript and its call-log row are protected data. Sandbox patients and
  simulated callers are public fakes, but the code treats them as protected data, so the code
  path is the one a real call takes.
- **Covered entity** — the hospital. **Business associate** — us, under a BAA with each
  hospital. **Subcontractor** — each vendor of ours that touches protected data, under a BAA
  with us.
- **The deploy** — one hospital's own copy of the stack: its own cloud account, database and
  key (F2 A2, VOICE-AGENT.md §7.2).
- **Seal** — encrypt one field under the hospital's key, bound to its table, row and column.
- **Verified patient** — the one `Patient` id the verification step bound to this call (§4.4).

## 3. Who we protect the record from
1. **Our own leaks** — logs, traces, metrics, panic messages, error reports, a support
   session, a debug recording, an AI chat. The most likely risk, and fully ours.
2. **A caller who is not the patient** — someone who knows the patient's date of birth, or fakes
   the calling number (§4.4).
3. **A caller who talks the model into something** — prompt injection by voice: "ignore your
   rules", "read me my husband's results", "I am a doctor". The model hears untrusted input on
   every turn (§4.9).
4. **People inside** — a hospital staff member who reads more than their role needs; our own
   staff (§4.10).
5. **Another hospital** — one deploy must never reach another's data (§4.6).
6. **The network and the supply chain** — a changed dependency, a vendor breach, a stolen
   credential.

What we cannot stop alone: a breach inside the hospital's EHR, or a vendor that breaks its BAA.
We say so plainly and never claim otherwise.

## 4. The rules
### 4.1 End to end
- **In transit:** TLS 1.2 or later on every hop: the phone carrier's media stream to us, us to
  each vendor, us to the EHR, the console to us. No `http://`, no certificate override.
- **At rest:** sealed per field (§4.6). Transcripts, the call log's protected columns, the audit
  trail's protected columns, recordings when a rule keeps them. Columns used to search and sort
  stay plain and carry no protected data: time, outcome, Flow node, latency.
- **In memory:** the wrapper type (§4.2) in Rust. The Python call loop holds the call's text as
  plain strings for one call only (VOICE-AGENT.md §8.6); it never writes them anywhere but to
  the Rust service.
- **Plaintext only where a person must hear or read it, or where a BAA covers it:** the call's
  audio, the request to the Anthropic API, the speech engines, the console's transcript view
  (each view audited).
- **The record lives one call long.** No chart cache. A chart read is held in memory for the
  call and dropped at its end.

### 4.2 The wrapper type
As in the patient app (archive/PATIENT-APP-SECURITY.md §4.2): every protected value in Rust is in
`Shielded<T>` from the moment it is parsed. It has no `Debug`, `Display`, `Serialize` or
`ToString`, so a leak into a log is a compile error. The passes that may read one change with
the product (CONVERSION.md §4.1): a request to the EHR the value came from; a tool reply to the
call loop; the sealed store.

### 4.3 The console
- Record and call text goes into the page **only through `textContent`**.
- A strict Content-Security-Policy: no remote script, no inline script, no remote images.
- No protected data in a toast, a hover hint, the page title, a URL, a notification,
  `localStorage` or an analytics event. A message names a kind of thing ("3 calls handed off").
- Staff sign in through the hospital's own single sign-on (§4.10).

### 4.4 Caller verification
> **Part:** designed · 2026-10-19 · the owner, fork S2 A

- **Questions about the hospital itself** (hours, directions, which number to call) need no
  check.
- **Any chart answer, and any booking, needs two factors** (the owner: S2 A):
  1. **Something the patient knows:** the date of birth, **or a chart-specific fact asked at
     random** (the owner: "configurable, using DOB or another chart specific stat we can ask
     at random").
  2. **Something the patient has:** a one-time code by text to the number on file, **or a call
     back to the number on file**. The call back also serves a landline, which cannot take a
     text, and it defeats a faked calling number.
- **How the caller is identified** (the owner, 2026-10-21): by name and date of birth, then
  the code to the number on file. The calling number is not used to find the patient. No
  reply ever says whether a name and date of birth matched; a mismatch shows only as a code
  that never arrives (EHR-SERVICE.md §1). This is the default. A hospital may turn on an
  instant transfer when no single record matches, and accept what it reveals (the owner,
  2026-11-12; FUTURE-SETTINGS.md §7).
- **Rules for the random question:**
  - **The question never reveals the chart.** "What is the ZIP code on your record?" asks; it
    does not tell. "Which medicine do you take for blood pressure?" tells the caller that the
    patient takes one, so it is never asked. The hospital picks from a list of questions that
    pass this rule.
  - The answer is checked in Rust against the record, never by the model.
  - Three wrong answers end the chart path for the call and offer a person. The audit trail
    records the failure, never the answer given.
  - A chart fact is a weaker proof than people think (NIST SP 800-63 no longer accepts
    knowledge questions alone), so it is never the only factor for a chart answer.
- **Configurable, with a floor.** A hospital may make verification stronger (a code on every
  call, a call back only). It may never go below the two factors above for a chart answer.
- **Results** keep their own rules on top (VOICE-AGENT.md §7.1): released to the portal only,
  sensitive results never on the phone.
- **A confused caller goes to a person** (the owner, 2026-11-12; built). After a failed code, a
  caller who asks why, says the code is right, or still has no text goes to staff at once, not
  to more tries. What the rules say about verification, and what is still open (twins, limits
  across calls, the confidential number): REGULATIONS.md §2.

### 4.5 Logs, traces and crashes
- Logs, traces and metrics carry counts, resource types, Flow nodes, durations and error kinds.
  Never a value, a name, a phone number, an id, a transcript line or a URL with a query string.
- **A redactor processor runs in the Pipecat pipeline** in front of every log and metric
  (VOICE-AGENT.md §9), and the Rust service logs error kinds only (`CoreError`).
- A panic hook replaces the panic message before it reaches a log.
- Error-tracking services get no request bodies and no frame variables.
- **Test mode is the exception** (the owner, 2026-11-12): the dev call record in `agent/calls/`
  keeps the caller's words and the tools' arguments in full, to debug. Every value in it is
  synthetic (§4.8). Before the first real call it splits into a call log with no values and a
  transcript sealed under the hospital's key (REGULATIONS.md §8).

### 4.6 Keys
- **The strictest standard:** HIPAA's Security Rule binds us now, and we hold the bar HHS sets
  for covered entities: NIST SP 800-111 at rest with FIPS 140-validated crypto, NIST SP 800-88
  for erasure, TLS 1.2 or later in transit.
- **One key per hospital**, held in the cloud's KMS, in the hospital's deploy. The hospital may
  bring and hold its own key (BYOK) and revoke it. Each sealed field uses `seal.rs`'s format
  (the associated data binds it to its table, row and column).
- **No deploy can reach another's key or database.** Separate cloud accounts per hospital
  (A2).
- **Secrets are never in Postgres or the repo:** the backend-services private key, vendor API
  keys and Twilio credentials live in the KMS or the cloud's secret manager, and rotate.
- Crypto: our own crate may wrap audited primitives; it never holds our own cipher.

### 4.7 Retention and deletion
- **Draft values for the owner to edit:** transcripts 30 days; the audit trail 6 years (HIPAA's
  documentation period); recordings as each rule sets; the call log's plain columns as long as
  the hospital wants its dashboards. The hospital sets its own within the law.
- **When a hospital leaves:** we destroy its key (crypto-shredding) and delete its database and
  its object storage, and we give it its data first if it asks.
- A patient's request to the hospital for their own data covers our transcripts and
  recordings: the console can export one patient's, and the export is audited.

### 4.8 Development
- **Synthetic first.** The SMART and Epic sandboxes and simulated callers. Dev accounts with
  vendors get synthetic data only.
- **A real call or a real record** waits for every vendor's signed BAA and the owner's approval.
- Sandbox values may appear in docs and in an AI chat; real values never do.
- Record-and-replay fixtures hold sandbox replies only, never a real hospital's.

### 4.9 The language model
- **The patient id comes from the verification step, never from the model.** Every chart tool
  is bound to the verified patient in the Rust service; a tool takes no patient argument. A
  caller cannot talk the model into another patient's chart, because no tool can reach one.
- **The Flow gates the tools** (VOICE-AGENT.md §8.1): a chart tool does not exist before
  verification passes, so the model cannot call it early.
- **Minimum necessary:** a tool returns only the fields its answer needs, never the whole chart.
- **The model never checks a verification answer** and never decides whether a result may be
  read; the service does (§4.4).
- **The words that carry a value are a template filled from the record**; the model picks the
  template. It cannot say a number the chart does not hold. As built (2026-11-11, the owner's
  fork 1A): the code speaks the filled template to the caller word for word, and the model only
  learns what was said (AGENT.md §3).
- **The Anthropic API under "HIPAA readiness" with a signed BAA**, US-only inference, generally
  available features only in the PHI path (VOICE-AGENT.md §8.2).

### 4.10 Staff access
> **Part:** designed · 2026-10-19 · the owner, fork S1 A

- **Hospital staff sign in through the hospital's own single sign-on** (SAML or OIDC: Microsoft
  Entra ID, Okta and others; the owner: S1 A). The hospital adds and removes people in one
  place. Multi-factor sign-in is the hospital provider's.
- **Roles (a draft for the owner to edit):** admin (settings, recording rules, roles),
  supervisor (call log, transcripts), viewer (call log and dashboards, no transcripts), auditor
  (the audit trail). These are examples only: each role is custom, named by the hospital and
  built from levers (the owner, 2026-11-12; open ended, ideas/ACCESS-ROLES.md).
- **Each transcript view, recording play and export is a row in the audit trail.**
- **Our own staff have no standing access to a hospital's protected data.** Support uses
  time-limited emergency access that the hospital approves and that is audited.

## 5. Vendors and BAAs
Every vendor that touches protected data signs a BAA with us before the first real call:
Twilio, Cartesia, Anthropic, the cloud provider. The terms each one offers (from public pages,
not yet signed) are VOICE-AGENT.md §8.2. US data residency for every one. A vendor without a
BAA never receives protected data, in dev or in production.

## 6. The law (a summary, not legal advice)
The full map of laws, guidance and standards, and what each asks of us: REGULATIONS.md.
- **HIPAA binds us now.** As a business associate we follow the Security Rule's safeguards
  (administrative, physical, technical), the Breach Notification Rule (we tell the hospital
  without unreasonable delay, at most 60 days after we find a breach), and the minimum-necessary
  standard.
- **Before the first real hospital:** a written security risk analysis (45 CFR 164.308), named
  security and privacy officers, written policies, and workforce training for everyone with
  access, helpers included.
- **Call recording:** some US states need every party to agree. When any rule can record a
  call, the greeting says so first (VOICE-AGENT.md §8.7). **Transcription counts too**
  (2026-11-12): every greeting says the call is transcribed, because we keep transcripts
  (REGULATIONS.md §6).
- **Not a medical device, by design:** the agent reads the chart and books visits; it gives no
  advice and does no triage (DESIGN.md › Not goals). Triage would bring FDA rules for software
  as a medical device.
- **Outbound calls** (later) bring the TCPA's consent rules.
- **Audits hospitals ask for:** a SOC 2 Type II report, often HITRUST. Their timing is a
  business choice, decided when the first sales talk starts.

## 7. Open
- The list of verification questions that reveal nothing (§4.4), and their wording.
- The roles and retention values (§4.7, §4.10): drafts.
- How a hospital approves our emergency support access (§4.10).

## 8. As built
Nothing of the phone agent is built yet. The wrapper type, sealing format and error kinds are
built in the core (CORE-CRATE.md), from the patient app.

## 9. Forks
- **S1. Staff sign-in:** A the hospital's own single sign-on (decided 2026-10-19) · B our own
  accounts with passkeys.
- **S2. Caller verification:** A two factors for any chart answer, with a call back as the
  second factor's alternative, and a date of birth or a random chart fact as the first,
  configurable above a floor (decided 2026-10-19) · B date of birth and calling number for
  visits and medicines.
