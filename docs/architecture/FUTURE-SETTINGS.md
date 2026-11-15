---
status: project
desk_test: none
sources: [agent/deets_agent/flow.py, agent/deets_agent/screen.py, agent/deets_agent/call.py, agent/deets_agent/config.py, agent/prompts/AGENT-PROMPT.md, ehr/src/epic_ehr.rs, ehr/src/verify.rs, ehr/src/visit_about.rs]
updated: 2026-11-15
---
# Future settings — what a hospital will configure

The agent's behavior that each hospital (or clinic) will set for itself. Today each one is a
fixed default in code. Each entry says what it controls, what the default is now, the choices
a hospital needs, and where the value will live. None of this is built: the settings store
waits on Postgres (VOICE-AGENT.md §8.7), and the console page to edit them waits on the
console's design (BUILD-ORDER.md, Phase 4). The global values that DeetsEngine carries are
drafted in [GLOBALS.md](../archive/GLOBALS.md).

**How this list works** (from DeetsMusic's ledger of the same name, the owner, 2026-11-12):
- **One central list.** Every setting a hospital will configure is on the index below, built
  or not. A setting that another doc names goes on the index the day that doc names it, with
  a link to the place it is described.
- **A full entry when its default is built.** It gets a numbered section with what it
  controls, the default now, the choices a hospital needs, and every place it runs. Its
  index line then points to that section.
- **Section numbers are stable.** Code comments and other docs refer to them (`flow.py`,
  AGENT.md, VOICE-AGENT.md). A new entry goes at the end with the next number. Never
  renumber. Read the index by topic, not by number.
- **When the settings store is built,** a SETTINGS.md becomes the live table of what is
  built, and the entries here stay as the design record.

## The index, by topic
> **Part:** project · 2026-11-12

**Safety on the call**
- After the emergency line, and the emergency flag's actions — §1 (default built).
- The medical-question line — §2 (default built).
- The emergency phrase list — §3 (default built).
- The greeting and its disclosures — §4 (default built).
- Staff flags: the kinds, the criteria for the model, the code triggers, where each kind goes
  and what it starts — §5 (default built).

**Caller verification**
- Stronger checks above the floor: a code on every call, a call back only. Never below two
  factors for a chart answer. Not built (SECURITY.md §4.4).
- The random chart questions to ask, from a list whose questions reveal nothing. Not built
  (SECURITY.md §4.4).
- When verification meets friction: how many missed codes before a person, the apology, and
  an instant transfer when no single record matches (twins, duplicates) — §7 (default built).
- Which numbers on file may receive the code: the kinds of number, ended numbers, the order,
  countries, shared numbers — §8 (default built: mobile only).
- When no code can go out (no match, twins, or no number for a code): words that are true for
  every caller, or the exact reason, which reveals that the person is a patient — §11
  (default built: option A, true for every caller; B a setting, not built).

**What the agent may say**
- The sensitive-results list: a hospital may add to it, never remove from it. Not built
  (VOICE-AGENT.md §7.1, DESIGN.md K2).
- The pronunciation dictionary: drug names, provider names, the hospital's own place names.
  Not built (CARTESIA.md).
- The holding line, spoken when a reply is slow to come because it ran into the token cap (the
  owner, 2026-11-12: configurable). Default built: `line.one_moment` (CALL-FLOW.md §3).
- What the agent says about a visit: its type, reason and online, and the sensitive list that
  keeps a visit's type and reason private — §9 (default built).
- When the agent cannot help: the words of the offer of a person, and when to offer or
  transfer at once — §10 (default built).

**The voice and the line**
- Silence and length on a call: how long a caller may say nothing before the agent asks, and
  then ends the call; the longest call — §12 (a dev guard built for test calls only; nothing
  for a real call yet).
- The voice, its speed and volume, and how Ink judges the end of a caller's turn: each lever
  and our setting today is CARTESIA.md §9. Not a setting yet.

**Booking**
- Hours, visit types and transfer numbers. Not built (DESIGN.md K2). The visit types per mode
  (in person, online) and the booking's note: §9.2.
- Which providers and locations are open to booking. Not built (OTHER-EHRS.md).
- The hospital's lists for `$find`: visit types, hours, time of day, specialties, reasons as
  codes, locations, and the time zone. On a new page, Config › Hospital — §13.1 (designed;
  ideas/HOSPITAL-CONFIG.md; the owner, 2026-11-14).
- How many slots the agent offers at a time (now 3) and how it presents them — §13.2.
- The narrowing question when a caller has several visits: on or off, the visit groups, how
  many matches start it, how many groups it names, their order; past visits; instructions;
  the CSN in flags — §13.3, §13.4.

**Records kept**
- Retention: transcripts (30 days, a draft), recordings, the call log's plain columns, within
  the law. The audit trail stays 6 years. Not built (SECURITY.md §4.7, VOICE-AGENT.md §8.7).
- Call audio: off by default; on for the whole hospital or by rule (a share of calls,
  hand-offs and errors, one line or Flow node, a debug window that ends by itself). Not built
  (VOICE-AGENT.md §8.7).

**Access and keys**
- The staff roles: custom, named by the hospital, built from levers. Open ended; the owner
  designs it. Not built (ideas/ACCESS-ROLES.md; SECURITY.md §4.10).
- Bring your own key: the hospital holds its key and may revoke it. Not built (SECURITY.md
  §4.6).
- Emergency support access for our staff, which the hospital approves. How it approves is
  open. Not built (SECURITY.md §4.10, §7).
- How recent a staff sign-in must be before a transcript, a recording or an export (the
  owner, F1 A, 2026-11-12). The default and any ceiling are open. Not built
  (ideas/STAFF-SIGN-IN.md §7).
- When staff type a reason to open a transcript: only for restricted calls (the default) or on
  every open (the owner, F2 C, 2026-11-12). Every open is audited under both. Not built
  (ideas/STAFF-SIGN-IN.md §7).
- The idle logoff for console sessions. The default is open. Not built
  (ideas/STAFF-SIGN-IN.md §5, M2).

**Ideas only** (not planned; see ideas/README.md)
- Protected data through MCP, for one client the hospital's BAA covers. Off by default
  (ideas/CONSOLE-MCP.md §4).
- The token cap for each demo call (ideas/CONSOLE-MCP.md §5.4).

## 1. After the emergency line
> **Part:** built (the default only) · 2026-11-11

**What it controls:** what the agent does after it tells a caller to hang up and call 911.

**The default now** (the owner's option A, 2026-11-11): the agent speaks a fixed line
(`line.emergency` in AGENT-PROMPT.md). The call stays open, and the task goes on: a caller who
was only worried keeps the time they chose. Code: `flow.py` `emergency_fn` (CALL-FLOW.md §3).
A caller who has said they are safe and mentions the same danger again hears a one-sentence
reminder that does not ask again, and the call goes on (the owner, 2026-11-12; `line.emergency_reminder`,
`safety_reminder_fn`). Anything new or worse gets the full line again.

**The choices a hospital needs:**
- **Stay and go on** (the default).
- **Transfer to a nurse line** after the 911 line, where the hospital staffs one (C5,
  emergency transfer).
- **End the call** after the 911 line, where the hospital's counsel wants the strongest
  message.
- **The words of the line**: the hospital may need its own (a local emergency number outside
  the US, a crisis line such as 988, its own nurse line's name).

**The emergency flag: several things at once** (the owner, 2026-11-11). The 911 line need not
be the only thing that happens. The `emergency` tool can also raise a flag on the call, and
the flag starts the hospital's chosen actions while the agent goes on with the task:
- **Pull in a nurse:** join a nurse to the call (a conference), so the nurse hears the end of
  the booking and takes over at once. This is a warm hand-off: the caller never waits in a
  queue or repeats their story.
- **Transfer when the task ends:** the agent finishes the booking, then transfers at once.
- **Send a message** to the care team or a nurse inbox (an EHR in-basket message, a page).
- **Add a note to the chart** that the caller raised a possible emergency, with the time.
- **Show it in the console:** the call is marked, so staff can listen in or take over.

This flag is now built as one kind of staff flag (`possible_emergency`); its settings,
its routing and its actions are §5. A hospital chooses one or more of these. The flag carries no protected data out of the wall:
the alert says "a caller on line N raised a possible emergency"; the details stay in the call
record and the chart (SECURITY.md §4). Open: the words that raise the flag (the model decides
today, from the prompt), and whether a staff member must acknowledge the alert.

**Every place it runs:**
- **The call:** a transfer or a joined nurse needs Twilio conferencing and a staffed number.
- **Each EHR (actions):** a chart note and an in-basket message are EHR writes (Epic first:
  `Communication` or a note, depending on what the hospital allows).
- **The console:** one control on the hospital's settings page, with an audit entry for each
  change.
- **The deploy:** A2 (our cloud) and B (the hospital's cloud) read it from the settings store
  the same way.
- **Each EHR (the line):** the line itself needs no EHR data.

## 2. The medical-question line
> **Part:** built (the default only) · 2026-11-11

**What it controls:** what the agent says when a caller asks a medical question, and what
follows.

**The default now** (the owner's option 1A, 2026-11-11): the `medical_question` tool speaks a
fixed line (`line.medical_question` in AGENT-PROMPT.md), and the task goes on. Code: `flow.py`
`medical_question_fn` (CALL-FLOW.md §3).

**The choices a hospital needs:** the words; an offer to transfer to a nurse line, where the
hospital staffs one; a message to the care team that the caller asked (an EHR in-basket
message), so a clinician sees the question before the visit. Never a symptom script unless the
hospital supplies one it has approved.

**Every place it runs:** the call (a transfer needs Twilio and a staffed number); the console
(the words and the follow-up, audited); the deploy (A2 and B the same); each EHR (only the
in-basket message writes to it).

## 3. The emergency phrase list
> **Part:** built (the default only) · 2026-11-11

**What it controls:** the phrases that make the code speak the 911 line itself, before the
model runs (the owner's option 2A, 2026-11-11: code reads the caller's words live, as a point
of customization). Code: `screen.py` `EMERGENCY_PHRASES`.

**The default now:** an English list in `screen.py` `EMERGENCY_PHRASES`, the one copy (the
owner, 2026-11-12: docs point to the code). The comment above it says how to read a pattern,
and each entry carries a comment with the lines it catches. It fires once per call.

**The choices a hospital needs:** phrases to add or remove; a list for each language it
serves; whether a match also raises the emergency flag (§1); whether it fires more than once
per call.

**Every place it runs:** the call (typed text today; speech-to-text words in Phase 3); the
console (the list, audited, with a test box that shows what a line would match); the deploy
(A2 and B the same); each EHR (none).

## 4. The greeting and its disclosures
> **Part:** built · 2026-11-12 (the default only)

**What it controls:** the first words of every call. The code speaks them word for word
before the model runs (`line.greeting`, flow.py `greet`).

**The default now (2026-11-12):** `line.greeting` in AGENT-PROMPT.md (CALL-FLOW.md §3).

**The floor (REGULATIONS.md §6):** a hospital may change the words, but every greeting must
still say three things: an automated assistant, not a person; the call is transcribed (and
recorded, when a recording rule is on); how to reach a person. How the console holds that
floor (a check that refuses a greeting without them, or a fixed sentence the hospital cannot
edit) is a fork for when the settings page is built.

**The choices a hospital needs:** its own words and the agent's name; one greeting for each
language it serves; the recording sentence, which turns on with any recording rule (§ Records
kept, VOICE-AGENT.md §8.7).

**Every place it runs:** the call (spoken first, on any phone); the console (the edit box,
audited, with the floor check); the deploy (A2 and B the same); each EHR (none).

## 5. Staff flags
> **Part:** built · 2026-11-12 (the default only)

**What it controls:** when the agent marks a call for a nurse or a staff member, what the flag
holds, and what happens next. A flag is silent to the caller (the owner, fork 2A). It carries
the model's summary and the caller's own words as evidence, and the code checks each quote
against what the caller said (fork 1A). A flag can start work in the background: a rules
engine acts on a kind, and a nurse may choose to join the call. §1's emergency flag is one kind
of staff flag.

**The default now (2026-11-12).** Every piece points to the code (DOCS-ORG.md §1, rule 8):
- **The kinds:** `FLAG_KINDS` in `agent/deets_agent/flow.py` (a draft list, the owner's to
  edit).
- **Flags the code raises,** with no model: the 911 line (`emergency_fn`, `safety_reminder_fn`,
  and `CallerScreen` in `agent/deets_agent/screen.py`, which keeps the caller's line as
  evidence); a locked verification (`_check_code_fn`); no code left to send
  (`_resend_code_fn`). Each kind is raised once per call (`CallFlow.raise_flag`).
- **Flags the model raises:** `flag_for_staff_fn`, and `hand_off_fn`, whose optional fields
  carry a flag, because a hand-off ends the call.
- **The criteria for the model:** the `flag_for_staff` rule in `role`, and `tool.flag_for_staff`,
  `arg.flag_for_staff.*` and `arg.hand_off.*`, all in AGENT-PROMPT.md.
- **The evidence check:** `CallFlow._caller_said`. A quote the caller did not say is dropped
  and counted (`StaffFlag.unverified`).
- **Silent to the caller:** nothing is spoken; `note.after_flag` tells the model to go on.
- **Where a flag goes:** `CallFlow.on_flag`. Today that is the dev call record (a `staff_flag`
  event, `agent/deets_agent/call.py`) and the catalog's label (`agent/deets_agent/catalog.py`).

**The choices a hospital needs:**
- **Its kinds.** Add, rename or turn off a kind. Each kind has a name that staff see and a
  description that the model reads.
- **The criteria for each kind,** in the hospital's own words: when the model raises it (for
  example, "a caller over 65 who mentions a fall"). The words go into the model's prompt.
- **Code triggers for each kind,** which need no model: the events that raise it (the 911
  line, a lock, every hand-off), and a phrase list for each kind, like §3's.
- **What a flag must hold:** whether a kind needs evidence before it is raised, and how long
  the summary may be. The evidence check itself is always on (the floor below).
- **What the caller hears:** nothing, by default. A hospital that staffs the action may add a
  line for a kind ("I've let our nurse know"). It must never promise an action that no one
  staffs.
- **Where each kind goes, and what it starts** (the rules engine): who receives it (a nurse
  pool, a role, an inbox); how fast (a live alert, or a queue to review); the actions (a nurse
  joins the call, a transfer when the task ends, an EHR in-basket message, a chart note, a
  call-back task); whether someone must acknowledge it; what happens if no one does within a
  set time.
- **Who may read the notes:** the roles (ideas/ACCESS-ROLES.md), and how long flags are kept
  (§ Records kept).
- **Overrides for each clinic** inside the hospital (§6).

**The floor.** A hospital may never turn these off: the evidence check by code; the rule that
an alert, a toast or a log line carries the kind only, never the summary or the evidence
(SECURITY.md §4); one audit row for each time a person reads the notes.

**Every place it runs:**
- **The call:** no words to the caller by default. A nurse who joins the call needs Twilio
  conferencing and a staffed number (Phase 5).
- **The console:** a settings page for each kind (criteria, triggers, routing, actions), every
  change audited; a flag queue; a notes view that shows the summary and the caller's lines,
  with each view audited.
- **The deploy:** A2 and B the same. The rules engine runs inside each hospital's deploy, so
  the notes never leave it.
- **Each EHR:** an in-basket message and a chart note are EHR writes (Epic first:
  `Communication` or a note, as the hospital allows). The flag itself needs no EHR.

**Open forks, for when the settings page is built:**
- Whether a hospital may turn off the safety kinds (`possible_emergency`, `safety_concern`), or
  whether they are a floor like the greeting's disclosures (§4).
- How a hospital's criteria are tested before they go live. Its words change what the model
  does, so they are a new place where the prompt can go wrong. One way: run them on the kept
  scripts and simulated callers first, and show the result before they apply.
- The flag queue in the console, and the nurse's controls (join, call back, dismiss).

## 6. Open questions
- Which settings a clinic inside a hospital may override, and which stay with the hospital.
- Who in the hospital may change a setting that touches safety (the emergency line), and
  whether a change needs a second person to approve it.

## 7. When verification meets friction
> **Part:** built (the default only) · 2026-11-12

**What it controls:** what the agent does when a caller cannot finish verification: the code
never arrives, the code does not match, or the caller is confused. The owner, 2026-11-12, after
the test call as Casey Morgan (a twin record, so no code went out): "I want to be more
proactive here", and "some hospitals won't care due to sheer scale of possible record
matches out there."

**The default now (2026-11-12).** Every piece points to the code (DOCS-ORG.md §1, rule 8):
- **One path for every caller.** No record, one record, or two near-identical records (twins,
  duplicates): the caller hears the same words at every step, so nothing shows whether a name
  and date of birth matched (SECURITY.md §4.4). The owner chose this over an instant transfer
  for twins (2026-11-12).
- **When the agent offers more tries** (since 2026-11-13, §11): only when the caller asks. A new
  code if they ask for one; their name and date of birth again if they say a detail was wrong
  (`task.verify`). `line.code_resent` says a new code is on its way only if the details match
  and a mobile number is on file.
- **When a person takes over:** at once, when the caller says no text came (§11: the code line
  promised it), when they still have no text after a new code, or when a code did not match and
  they ask why or say it is right (`task.verify`). The limits in
  `ehr/src/service.rs` (`MAX_RESENDS`, `MAX_CODE_FAILURES`, `MAX_IDENTIFIES`) hand off in code
  (CALL-FLOW.md §3, §4).
- **The apology:** a model hand-off of kind `verification_failed` speaks `line.hand_off_code`
  in place of `line.hand_off` (`hand_off_fn`).

**The choices a hospital needs:**
- **How soon a person takes over:** at the first missing code, after one new code (the
  default), or after more. Fewer tries means less waiting for a twin or a caller with an old
  number. It also means more calls for staff from patients whose text is only slow.
- **What the agent offers at a missing code:** a new code, the name and date of birth again,
  the call back to the number on file (SECURITY.md §4.4, not built), or a person.
- **The words** of the apology and of each verification line.
- **An instant transfer when no single record matches.** Off by default. When it is on,
  the caller hears an apology and goes to a person right after they confirm their details,
  without waiting for a code that cannot come. This is not hidden from the caller: anyone
  who knows a name and date of birth learns that this hospital has two or more records for
  that person. That is protected data (SECURITY.md §2), so the default never does it. A
  hospital with many duplicate records may decide the faster path is worth that cost. The
  settings page must state the cost when a hospital turns it on, and the audit trail records
  who turned it on. The words must stay true: they say that the agent cannot finish the check
  by phone, not that something is broken.

**Designed, not built: a silent flag for staff when no single record matches.** `ehr` knows
when a name and date of birth match no record or more than one; the agent and the model never
do (EHR-SERVICE.md §1). `ehr` would raise the flag itself, so the staff member who takes the
call knows at once to look for a twin, a duplicate or a typo. It needs its own design first:
where `ehr` keeps the fact after `CloseCall`, how it reaches the call store and the console
(the dev call record is written by the agent, which must not learn it), and a flag kind.

**Every place it runs:**
- **The call:** the same on any phone. A real transfer needs Twilio and a staffed number
  (Phase 5).
- **The console:** one control for each choice, every change audited; the instant transfer
  states what it reveals before it applies.
- **The deploy:** A2 and B read it from the settings store the same way.
- **Each EHR:** what counts as one match is `ehr`'s rule for each EHR. Epic's patient match
  can return more than one candidate, so each adapter must report "not exactly one" the same way.

## 8. Which numbers on file may receive the code
> **Part:** built (the default only) · 2026-11-13 — the default came with Epic mode (EPIC.md
> §5.2); the setting is designed here, not built.

**What it controls:** which of the patient's phone numbers in the chart may receive the code by
text, the second factor (SECURITY.md §4.4). The code is only as strong as the number it goes
to: a code that a co-worker or a housemate can read proves nothing about the caller.

**The rules no setting changes:**
- The code goes only to a number **on file in the chart**. Never a number the caller gives on
  the call, and never the number they call from (caller ID never ends verification, TEXTS.md
  §1).
- A patient whose chart has no allowed number **still counts in the match**. If they were left
  out, a near-twin with an allowed number would become the one match and get the code. With no
  allowed number, no code goes out, and the caller hears the same words as after any failed
  match (§7).
- Never below two factors for a chart answer.

### 8.1 The default now
Only a number marked `mobile` that has not ended, the patient's first choice by `rank`, as a US
number (+1). The code:

- **The pick** — `ehr/src/epic_ehr.rs` › `code_number` (called by `patient_fields`):
  ```rust
  r["telecom"]
      .as_array()
      .into_iter()
      .flatten()
      .filter(|t| t["system"] == "phone" && t["use"] == "mobile")
      .filter(|t| !ended(&t["period"]["end"], now))
      .filter_map(|t| Some((t["rank"].as_u64().unwrap_or(u64::MAX), e164(t["value"].as_str()?)?)))
      // min_by_key keeps the first of equal ranks, so the chart's own order breaks a tie.
      .min_by_key(|(rank, _)| *rank)
      .map(|(_, number)| number)
  ```
- **Ended** — `ended` in the same file. FHIR's `ContactPoint.period.end` marks a number the
  patient no longer uses. An end in the past is ended. A date with no time ends at the close of
  that day (UTC). **An end that cannot be read counts as ended**: a code never goes to a number
  we cannot read as current. No end means still in use.
- **The order** — FHIR's `rank`: 1 is the patient's first choice. A number with no rank comes
  after every ranked one. Equal ranks, or none: the first in the chart's list.
- **The format** — `e164` in the same file: 10 digits become `+1` and the digits; 11 digits
  that start with 1 become `+` and the digits; anything else is no number.
- **The carrier** — `ehr/src/verify.rs` › `Candidate.phone: Option<Shielded<String>>`. `None`
  means "no allowed number".
- **The use** — `ehr/src/service.rs` › `identify`: a code goes out only for one match with a
  number.
  ```rust
  if let Some(Candidate { patient, phone: Some(phone), .. }) = one_match(&claimed, found) {
      // issue and send the code
  }
  ```
- **The local fakes** (`FakePatients`) give every patient a number, from
  `ehr/fixtures/sandbox.json`.
- **The tests** — `service.rs` › `a_match_with_no_number_for_a_text_sends_no_code`;
  `epic_ehr.rs` › `an_ended_mobile_number_gets_no_code`,
  `the_code_goes_to_the_best_ranked_mobile_number` (a better-ranked work number never counts),
  `a_patient_with_only_a_work_number_has_no_number_for_a_code`.
- **`use: old`** is never a match, because only `mobile` is.

The two gaps found on 2026-11-13 (an ended number still counted; `rank` was ignored) were
fixed the same day.

### 8.2 The choices a hospital needs
| Choice | Default | Options | What each costs |
|---|---|---|---|
| Which kinds of number count | `mobile` only | add `home`; add `work`; add `temp` | `home` reaches landlines (no text) and housemates; `work` reaches shared desks. Each one added means more callers get a code, and a weaker code. |
| A number the chart marks as ended | never counts | — (a rule, not a choice) | — |
| When several numbers count | the chart's `rank`, then list order | list order only | `rank` follows the patient's own choice. |
| Countries | US only (+1) | add others | A number outside the US needs its own format check and Twilio's rules for that country. |
| A number the hospital marks "do not text" or "no consent to text" | never counts | — (a rule, if the EHR holds such a mark) | Where it is held differs by EHR and by hospital build (open: see 8.3). |
| A number shared by more than one patient (a parent's mobile on a child's chart) | counts | does not count | A parent who calls for a child is a caller for someone else, and the agent already hands that to a person (FLAGS.md). Not counting it sends more callers to staff. |

**The sketch** of the setting (not built). The adapter would read it; the service and
`Candidate` stay as they are:
```rust
/// FUTURE-SETTINGS.md §8. One per hospital, from the settings store.
pub struct CodeNumbers {
    /// The FHIR ContactPoint `use` values that count. Default: [Mobile].
    pub uses: Vec<PhoneUse>,
    /// The country codes allowed. Default: ["1"].
    pub countries: Vec<String>,
    /// Whether a number on more than one chart counts. Default: true.
    pub shared_counts: bool,
}

pub enum PhoneUse { Mobile, Home, Work, Temp }

// epic_ehr.rs: patient_fields(r, &settings.code_numbers) keeps each telecom entry with
// system = phone, a `use` in `uses`, no `period.end` in the past, and a country in
// `countries`; then takes the lowest `rank`, then the first in the list.
```

### 8.3 Open
- **Where an EHR marks a number as able to take texts, or as consented to texts.** In FHIR it
  is not a standard field. Epic may hold it in the hospital's build (a communication
  preference); the owner's Epic knowledge is the first source. Each EHR's probe should show
  it (`architecture/probes/`).
- **Other EHRs:** some FHIR servers mark a text-capable number with `system: sms`, not
  `use: mobile`. Each adapter maps its EHR's marks to the same `PhoneUse` list.
- **Consent law:** a one-time code that the caller asked for is generally allowed
  (REGULATIONS.md §4, TCPA). A hospital's own policy may be stricter.

**Every place it runs:**
- **The call:** no change in what the caller hears. A caller with no allowed number is handed
  off, the same as after any failed match.
- **The console:** one control for each choice, every change audited. Adding `home` or `work`
  states the cost before it applies.
- **The deploy:** A2 and B read it from the settings store the same way.
- **Each EHR:** each adapter applies the same setting to its own phone fields.

## 9. What the agent says about a visit: its type, its reason, and the sensitive screen
> **Part:** built (the default only) · 2026-11-13 — after the owner's test call as Camila Lopez:
> she asked twice what her visit was for and heard nothing. "We can't ignore what's on the
> record": an online visit keeps its place.

**What it controls:** what the caller hears about their next visit besides its time, clinician
and place: the visit type, the reason, whether it is online, and which visits are too sensitive
to describe by phone.

**The rules no setting changes:**
- Only what the record holds. The model never guesses what a visit is for (`task.menu`).
- **Never the scheduler's own notes** (Epic's `Appointment.comment`): they are for staff and can
  hold anything. In Epic's sandbox they hold other developers' test notes (EPIC.md §5.2).
- A sensitive type or reason is dropped **in `ehr`**, before the wire. The agent and the model
  never receive its words; they learn only that the reason is private.

### 9.1 The default now
- **The fields** (Epic): the visit type from `Appointment.serviceType`, the reason from
  `reasonCode`, each as its text or its first coding's display (`epic_ehr.rs` › `concept_words`).
  Epic's sandbox fills `serviceType` ("MyChart Video Visit") and never `reasonCode`. The local
  fixture's visit has both ("Office Visit", "Blood pressure follow-up").
- **Online:** a type that names video, virtual, telehealth, telemedicine, online or e-visit
  (`ONLINE` in `ehr/src/visit_about.rs`). The caller hears the place the record names, then
  "It is an online visit." (`line.visit_online`) in place of the type's own words.
- **The words** (AGENT-PROMPT.md, drafts): `line.next_visit` ("Your next visit is {visit}."),
  then `line.visit_online` or `line.visit_type` ("It is listed as {visit_type}."), then
  `line.visit_reason` ("The reason on record is {reason}."). The line ends without a question:
  the model goes on and answers the rest of the caller's turn (`note.after_find_visit`).
- **The model's facts:** the tool result's `reason_for_visit` is `said`, `none on record` or
  `private`. On a question the record does not answer, the model uses `offer_staff` (§10).
- **The screen** — `ehr/src/visit_about.rs` › `VisitAbout::from_record`, `SENSITIVE`: a type or
  reason that holds one of these words or phrases (whole words, any case) makes the visit
  private: behavioral health, mental health, psychiatry, psychiatric, psychology,
  psychotherapy, counseling, substance, addiction, detox, opioid, methadone, suboxone, HIV,
  pre-exposure, sexual health, STD, STI, reproductive, obstetric(s), OB, OBGYN, OB/GYN,
  prenatal, pregnancy, abortion, family planning, fertility, IVF, genetic(s), gender,
  transgender. **A draft, the owner's to edit.** Left out on purpose: "aids" (hearing aids) and
  "prep" (bowel prep).
- **The private words:** if the caller asks what a private visit is for, `offer_staff` with
  `why: private` speaks `line.offer_staff_private`: "I can't share what that visit is for by
  phone. Would you like me to connect you with a member of our staff who can?"
- **Tests:** `visit_about.rs` (the screen, whole words, online); `epic_ehr.rs` (the type from
  `serviceType`, `comment` never read); `test_gate.py` (the words and `reason_for_visit`);
  `test_typed_call.py` › `test_visit_reason_then_an_offer`.

### 9.2 The choices a hospital needs
| Choice | Default | Options | What each costs |
|---|---|---|---|
| The sensitive list | the draft above | add words; never remove the floor | More words keep more visits private; each one also hides an ordinary visit that holds the word. |
| Whether the visit type is said | yes | no | Without it, "what is this visit for?" goes to staff more often. |
| Whether the reason is said | yes | no | The same; the reason helps the caller most. |
| The spoken name of each visit type | the record's words | a hospital's own words per type ("MyChart Video Visit" → "a video visit") | Plainer words for the caller; a list to keep current. |
| Which types count as online | the word list above | the hospital's own list of type codes | Codes are exact; words can miss a type. |
| Which visit types the agent books, for in person and for online (`visit_mode`, 2026-11-14) | the fake's "Office Visit" and "Video Visit" | the hospital's own list per mode, by type code | Epic's `$find` needs one visit type per search (EPIC.md §5.2); without the list, Epic booking cannot start. |
| The note sent with a booking | the caller's reason, in their words | add a fixed prefix ("Booked by the phone agent: …"), or a length limit the site's Cadence build needs | A prefix tells the scheduler where the visit came from; a limit cuts the caller's words. |

### 9.3 Open
- **A private visit still says its time, clinician and place.** A clinician's title or a
  place's name ("Behavioral Health Clinic") can reveal the same thing. Whether the screen also
  withholds them, and what the caller hears then, is open.
- **Patient instructions** (Epic's `patientInstruction`, "Bring a list of your medicines"):
  meant for the patient, and useful on a call. Not read yet. The owner, 2026-11-14: NextVisit
  says them, through this screen. Design: EPIC.md §7.8.
- **Other EHRs:** each adapter maps its own type and reason fields to the same screen.

**Every place it runs:** the call (the same on any phone); the console (one control per choice,
every change audited); the deploy (A2 and B read it from the settings store); each EHR (the
adapter reads its fields, and the screen in `ehr` is shared).

## 10. When the agent cannot help: the offer of a person
> **Part:** built (the default only) · 2026-11-13 — the owner's option A, after Camila's call:
> the agent said nothing about her question and moved her to staff at once.

**What it controls:** what happens when the caller asks for something the agent cannot do or
see, and what the caller hears.

**The default now:**
- **The offer:** the model uses `offer_staff` (a global tool, `flow.py` › `offer_staff_fn`). It
  speaks a fixed line that says why and asks, then waits:
  - `line.offer_staff`: "I'm not able to see that here. Would you like me to connect you with a
    member of our staff who can help?"
  - `line.offer_staff_private` (a private visit, §9): "I can't share what that visit is for by
    phone. Would you like me to connect you with a member of our staff who can?"
- **A yes** is `hand_off`, with its staff flag (kind, summary, the caller's words), as before.
  **A no** goes on with the call (the kept script asks for a booking next).
- **Still at once, with no offer:** a caller who asks for a person, a caller who is upset, and a
  code that did not work (§7). `role` and `tool.hand_off` say so.
- The model says nothing in the same reply as `offer_staff` (it is a silent tool; the words
  check in `call.py` flags any).

**The choices a hospital needs:**
- **The words** of both offers. They must say that the agent cannot help with this, never that
  something is broken, and must ask before a transfer.
- **When to offer and when to transfer at once:** some hospitals may want every "cannot help"
  to go straight to a person (fewer turns, no choice for the caller), or a "no" to be asked
  once more before the call ends.
- **What a "no" offers next:** go on (the default), or offer a call back from staff (needs the
  call-back design, SECURITY.md §4.4).

**Every place it runs:** the call (the same on any phone; a real transfer needs Twilio and a
staffed number, Phase 5); the console (the words and the choice, every change audited); the
deploy (from the settings store).

## 11. When no code can go out: what the caller is told
> **Part:** designed · 2026-11-13 — after the owner's test call as Warren McGinnis
> (`agent/calls/20261112-181140-test.jsonl`): his chart has no mobile number, so no code went
> out, yet he heard "we've just sent a 6-digit code". He waited 30 seconds and hung up. The
> owner: "We shouldn't have gaslit the patient." **The owner chose option A, 2026-11-13:**
> words true for every caller, built the same day; B (the exact reason) stays a hospital
> setting, off, not built (that is option C).

**Built (option A):** `line.code_sent` and `line.code_resent` in AGENT-PROMPT.md (drafts, the
owner's to edit) say a code is on its way only if the details match **and a mobile number is on
file**, and promise a person if no text comes. `task.verify`: "no text" is `hand_off` at once
(kind `verification_failed`, so `line.hand_off_code`); a new code only when the caller asks.
Kept script `no_text_to_person.txt` (`test_no_text_goes_to_a_person_at_once`).

**What it controls:** what the caller hears after they confirm their name and date of birth,
when `ehr` knows no code will go out, and how fast they reach a person.

**Why no code goes out — three cases `ehr` knows and the agent does not:**
1. No record matches the name and date of birth.
2. Two or more records match (twins, duplicates, §7).
3. One record matches, but it has no number that may take a code (§8): Warren's case. Epic's
   sandbox has three such patients; in a real hospital, often older patients with a landline
   only.

**The tension.** Telling the caller the exact reason tells them which case it is. Case 3 says
"this person is a patient here, and their chart has no mobile number" to anyone who knows a
name and a date of birth, which are easy to find. That a person is a patient is protected data
(SECURITY.md §2). So the floor today (SECURITY.md §4.4) is that no reply says whether the details
matched. But today's words go further than the floor needs: "we've just sent a 6-digit code"
sounds like a fact even when `ehr` knows it is not one.

**The options for the words** (each one said the same way to every caller unless it says
otherwise):
| Option | What the caller hears after "Yes, that's right" | What it reveals | What it costs |
|---|---|---|---|
| **A. True for every caller** | "Thank you. If those details match our records and we have a mobile number on file, a 6-digit code is on its way by text. If no text comes in a minute, just tell me and I'll connect you with a member of our staff. Could you read me the code when it arrives?" (a draft) | Nothing: the same words in all three cases. | Longer. Warren still waits a minute before he says so; he is never told the true reason. |
| **B. Tell the exact reason** | Case 3: "I found your record, but there's no mobile number on file I can text a code to, so I can't finish the check by phone. Let me connect you with a member of our staff." Cases 1 and 2 hear A. | That this person is a patient here, and that their chart has no mobile number, to anyone with a name and date of birth. | The most honest and the fastest for Warren. A disclosure of protected data to a caller who has passed one factor only. |
| **C. Both: A by default, B as a hospital setting** | A, unless the hospital turns B on. | As A, or as B where turned on. | B's cost is stated on the settings page when a hospital turns it on, and the audit trail records who did; the same pattern as §7's instant transfer. |

**What a no-text caller gets next:** before 2026-11-13, `task.verify` offered a new code or the
details again first, then a person. Now the words promise a person, and the rule matches: a
person at once, one new code only if the caller asks for it.

**The choices a hospital needs** (once built):
- Which words: A or B (C makes this a setting).
- How fast a person takes over when no text comes (§7 holds the counts).
- **When the caller says no text came and asks for a new code in the same breath** ("I didn't
  get a code, can you send it again?"): a person at once (the default; the owner's option A,
  2026-11-14, after the kept script `no_code_after_resend` was recorded again and the model
  chose it), or one new code first and then a person (the caller's ask honoured; a minute more
  of waiting for a text that, with no mobile on file or a mismatch, never comes). Today the
  model decides from `task.verify`; as a setting, the code would decide.
- **A voice code to a landline**: `ehr` calls the home number on file and reads the code (the
  call back of SECURITY.md §4.4; §8 for which numbers count). It would get a patient with a
  landline through on their own, with no disclosure. Not for Warren: his chart has no number
  at all. Not designed yet.
- **A time to wait** before the agent offers a person on its own when the caller says nothing.

**Every place it runs:**
- **The call:** the same on any phone. A real transfer needs Twilio and a staffed number
  (Phase 5).
- **The console:** one control for the words, with B's cost stated before it applies; every
  change audited.
- **The deploy:** A2 and B read it from the settings store.
- **Each EHR:** each adapter reports case 3 the same way (a match with no number for a code;
  `Candidate.phone` is `None`, §8.1).

## 12. Silence and length on a call
> **Part:** project · 2026-11-13 — the owner chose the dev guard (fork 2A, 2026-11-13: "with
> proposed limits and a doc in future settings"). Built for voice test calls only. Nothing for a
> real call is built.

**What it controls:** what happens when the caller says nothing for a while, and the longest a
call may run. Every second of an open call costs Ink credits, silence included (CARTESIA.md §4),
and a line left open by mistake (a phone in a pocket, a dropped call that did not hang up) holds
a line and spends money with no one helped.

**The default now (dev only, voice test calls):** `config.VOICE_IDLE_S` (60 seconds with no
speech from the caller) and `config.VOICE_MAX_S` (6 minutes) end the call with no spoken line;
the record says which limit ended it (`call.py › TypedCall._watch_limits`). This guards the Free
plan's credits. It is not what a caller should hear.

**The choices a hospital needs** (a draft, for when a real call is built):
- **The first silence:** after how many seconds of silence the agent asks "Are you still
  there?" (a fixed line; its words go in AGENT-PROMPT.md).
- **The end on silence:** after how many more seconds of silence the agent says goodbye and
  ends the call. A caller who speaks again resets both.
- **While a tool runs or the agent speaks:** never counted as the caller's silence.
- **The longest call:** a ceiling, then a polite close, or a transfer to a person in place of a
  close. Off is a valid choice: some callers need time.
- **A caller who said an emergency phrase:** never ended for silence (§1, §3); the call stays
  open, or goes to a person.

**Every place it runs:**
- **The call:** the same on any phone. Silence is measured from Ink's turn events, so it works
  the same on Twilio as on the dev voice seat.
- **The console:** one control per value; every change audited.
- **The deploy:** A2 and B read it from the settings store.
- **Each EHR:** no effect.

## 13. Booking and visits: the hospital's lists, slots, finding a visit
> **Part:** designed · 2026-11-14 — the owner's asks of 2026-11-14 (EPIC.md §7.8 to §7.10;
> forks 1A, 2A, V1A, V2A). The defaults are built in two builds, in order (EPIC.md §7.10). The
> page for the lists is Config › Hospital (ideas/HOSPITAL-CONFIG.md). **Build 1's defaults are
> built** (§13.1's time zone and call time, §13.3, §13.4): `ehr/src/visits.rs` holds the
> numbers, the fixtures hold the time zone and the fake's groups. **Build 2's are built
> too** (2026-11-15): `ehr/fixtures/hospital.json` holds the times of day and the slots offered
> at once; `epic-sandbox.json` the visit types per mode. The order and the clinician's name in
> §13.2 keep their defaults in code; their other choices are not built.

**What it controls:** how the agent finds and offers open slots, how it finds one of the
caller's visits and what it says about it, and the hospital facts that Epic's `$find` needs.

**The rules no setting changes:**
- The model never names a patient, and never sees a CSN or a FHIR id. It names a visit or a
  slot by its number on this call.
- The sensitive screen (§9) covers a visit's instructions and every name of a visit group.
- `ehr` never sends a second `$book` without the caller's yes (V2A).
- No list holds protected data.

### 13.1 The hospital's facts
| Setting | Default in the build | Choices a hospital needs |
|---|---|---|
| The time zone | `ehr/fixtures/epic-sandbox.json` › `time_zone` (America/Chicago); the local fake uses the same | one IANA zone per hospital; one per location (HOSPITAL-CONFIG.md §2.7). `ehr` and the agent read the same value. |
| The call's date and time | a line the code sends at the start of each call's messages, in that zone (fork 1A); `role` keeps the date | the format of the date and the time |
| The visit types for each mode | a list in the fixture: the sandbox's types, in person and online | the hospital's codes, spoken names, lengths, new or returning (§2.1) |
| The time-of-day names | a draft list: morning (8 to 12), afternoon (12 to 5), evening (after 5) | the hospital's names and hours (§2.3) |
| Hours, specialties, reasons as codes, locations | none | each list (§2.2, §2.4 to §2.6). An empty list sends nothing to `$find`. |
| Visit groups | none in Epic mode; a draft for the fake: primary care, imaging | the hospital's groups, their words and their members (§2.9) |

### 13.2 Offering slots
| Setting | Default | Choices |
|---|---|---|
| Slots kept for the call | Epic's page (10), and its `next` link | fixed by Epic |
| Slots offered at a time | 3 (`MAX_OFFERED`) | 1 to 5 |
| Order | the soonest first | spread over different days |
| The clinician's name | said with each slot | once, when every slot offered has the same clinician |
| Times on one day | each said in full | said together ("Tuesday at 9 or at 2:30") |

### 13.3 Finding a visit
| Setting | Default | Choices |
|---|---|---|
| The narrowing question | on, when visit groups exist, and only when the caller named a day, a month, a clinician or a kind of visit ("my next visit" is always the soonest) | off |
| Matches that start it | more than 2 | any number |
| Groups one question names | 3 at most, then "or another visit" | 1 to 5 |
| Group order | the group with the soonest visit first | the list's own order |
| How far back a past visit is found | 400 days | any number of days |
| Which past statuses are said | seen, missed, cancelled | any of the three |
| Visits counted after the soonest | the next 90 days | any number of days |

### 13.4 Instructions and staff
| Setting | Default | Choices |
|---|---|---|
| Instructions said | yes: asked first; read at once when the caller asked about that visit (2A) | off; always read |
| The CSN in a flag about a visit | on, for `booking_change` | which flag kinds carry it (§5) |

**Every place it runs:**
- **The call:** the same on any phone.
- **The console:** Config › Hospital for the lists and these values; every change audited.
- **The deploy:** A2 and B read them from the settings store.
- **Each EHR:** the lists' columns are Epic's; each adapter maps its own concepts behind them
  (OTHER-EHRS.md). The local fake reads the same settings.
