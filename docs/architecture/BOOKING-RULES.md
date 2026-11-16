---
status: built
desk_test: open
sources: [ehr/src/hospital.rs, ehr/src/epic_ehr.rs, ehr/src/scheduler.rs, ehr/src/service.rs, ehr/proto/deets/ehr/v1/ehr.proto, core/src/shielded.rs, ehr/fixtures/hospital.json, agent/deets_agent/flow.py, agent/deets_agent/ehr_client.py, agent/prompts/AGENT-PROMPT.md, agent/prompts/TOOLS.md, console/src/pages/hospital.ts, docs/integrations/EPIC.md, docs/architecture/HOSPITAL-CONFIG.md]
updated: 2026-11-16
---
# DeetsHealth — Booking by place, specialty and the hospital's rules

> **Part:** built · 2026-11-16 · desk test open. Every fork below is closed. The build is §6;
> step 0 is done, and steps 1 to 5 are built (§5.1 to §5.5). Each step's as-built part is the
> code where it and the design above disagree.
> The owner asked what is left unwired after the work on Config › Hospital (Places, Specialties),
> then: "Let's chat how we want to expand existing tools or add to the tools here." His forks:
> **1A** (the default place), refined by his question on the PCP; **2A** (a specialty only from
> the caller's words); **P-A** (sooner with another clinician); **E-A** (a Booking rules table,
> the start of the rules engine); referrals "definitely hospital markable".

**Terms.**
- **A place** is a department on Config › Hospital › Places (HOSPITAL-CONFIG.md §2.7). A
  **facility** is the building or campus above it (EAF).
- **A specialty** is a row on the Specialties tab (HOSPITAL-CONFIG.md §2.8).
- **PCP** is the patient's primary care provider. Epic keeps it on the patient record; FHIR
  returns it as `Patient.generalPractitioner`.
- **New or established.** A patient is new to a specialty when no provider of that specialty in
  the same group has seen them in 3 years (the CPT rule). A new patient takes a different visit
  type, often a longer one.
- **Matched in Rust** means `ehr` compares the caller's words with a list's spoken names and
  caller's words, as `find_visit`'s `group` does today (`VisitGroups`). The model never sees a
  code or a FHIR id.
- **A booking rule** is one row of the Booking rules table (§3.5): conditions and one outcome.

## 1. Why
Before this design the agent read the visit types, the groups, the time zone, the times of
day, the booking numbers and a department's spoken name (HOSPITAL-CONFIG.md §2). A caller could
not ask for a clinic, a kind of doctor or their own doctor, and could not ask where to go. In
Epic mode `$find` searched every bookable visit type for every caller, so an established
patient could be offered "New Patient Appointment" times (`Hospital::coded_bookable`). Places
and Specialties held hundreds of rows that no call read.

## 2. The decisions (the owner, 2026-11-16)
- **1A, refined — the default place.** First shown as "offer the last place". The owner: "consider
  patient is trying to book a primary care visit with their primary provider, what then? if
  their last visit was with their primary, perfect. otherwise, this breaks right?" So the
  default is the last visit **of the same kind of care**, and the PCP for primary care (§3.2).
  Not chosen: 1B, always ask for a clinic or a town; it stays as the fallback. Dropped: search
  every place (fails the patient in a hospital with hundreds of departments).
- **2A — the specialty.** Only from the caller's words ("a skin doctor", "cardiology"). The
  reason never picks a specialty. Not chosen: 2B, the model maps the reason to a specialty and
  reads it back (a routing judgment from symptoms, close to a medical answer).
- **P-A — sooner with another clinician.** The PCP's times first; then the agent says another
  clinician at the same clinic has a sooner time. A hospital setting, on by default. Not
  chosen: P-B, the PCP only unless the caller asks.
- **E-A — the Booking rules table.** The owner: "debating if this should be the start of the
  rules engine or not to add more granularity but definitely hospital markable". Chosen: a
  table of rules on Config › Hospital, the first part of the rules engine (SYSTEM-PAGE.md §8).
  Not chosen: E-B, one "Referral needed" mark per specialty (cannot hold a rule per provider or
  per visit type).
- **Referrals.** First shown as R-A, "read the caller's open referrals". Withdrawn the same
  sitting: Epic gives no way to do it (§3.6). The hospital marks where a referral is needed
  (a rule's outcome). **The line we draw (the owner, 2026-11-16):** "Not planning on linking
  orders for the purpose of checking referrals. If we need to draw a line in our capabilities,
  that's ok." The agent never checks whether a caller has a referral; the hospital's rule says
  what happens.

## 3. The design

### 3.1 The place and the specialty in the search
- `find_slots` takes two more arguments: `place` and `specialty`, each in the caller's words.
  `find_visit` takes `place` ("my visit at the north clinic").
- `ehr` matches them in Rust:
  - A place against each department's spoken name, the caller's words, the record's name, and
    its facility's name and city.
  - A specialty against each specialty's spoken name and caller's words.
  - A fixed list of choices in the tool's schema (as `time_of_day` has) does not fit: Places
    holds up to 2,000 rows (`PLACES_MAX`).
- More than one match: one question that narrows it, naming no more items than the Booking
  tab's "groups one question names". No match: the agent says it does not know that place or
  specialty and asks once more. The words are drafts in AGENT-PROMPT.md, the owner's to edit.
- Epic's `$find` gets `location-reference` (the matched department's FHIR id; it worked in the
  sandbox's probe of 2026-11-15, EPIC.md §5.2) and `specialties` (the specialty's ID under the
  Specialty list's OID). **Step 0 found that the sandbox accepts `specialties` and ignores it**
  (EPIC.md §7.12), and a proposed time carries no specialty. So `ehr` also filters: it keeps
  only the times whose department (by its Location FHIR id) has that specialty on the Places
  list. A hospital whose rules honor `specialties` gets the same answer with less filtering.
  `$find` has no provider input; a provider is a filter on the reply's participants (EPIC.md
  §7.1).
- The local fake: each of its slots has a place and a specialty, and it filters the same way,
  so the kept scripts can test the search.

### 3.2 The default place, and the PCP
When the caller names no place, the search uses the caller's last visit **of the same kind of
care**:
- **Primary care.** The PCP and the PCP's clinic. The PCP comes from the Patient read that
  verification already makes (`Patient/{id}`; no new request), if the hospital gives our
  backend user the security for "PCP details" (Epic's words on Patient.Read). The PCP's clinic
  is the place of the caller's last visit with the PCP. The search sends that clinic as
  `location-reference`; `ehr` keeps the PCP's times first (P-A, §3.3).
- **A specialty.** The place of the caller's last visit at a department of that specialty,
  found through the Places list (each visit names its department by FHIR id; each department
  names its specialty).
- **No visit of that kind:** the question the agent asks before the first search (the owner's
  1A of 2026-11-15, `line.ask_preference_*`) also asks for a clinic or a town (1B).
- The question names the default so the caller can change it: the gist is "at the north
  clinic, where you went last, or somewhere else?". The words go in AGENT-PROMPT.md.
- A hospital with one place asks nothing about the place.

**The cases a phone agent meets (the owner asked for the context, 2026-11-16):**

| The caller's situation | What the agent does | What is still needed |
|---|---|---|
| 1. PCP on file, seen recently | The PCP's times at the PCP's clinic; P-A when they are far off. | — |
| 2. PCP on file, never seen (assigned by the plan or at registration) | No clinic from the history: the place question (1B). A new-patient visit type (§3.4). | `PractitionerRole` (not registered) would give the PCP's clinic. |
| 3. PCP on file has left or retired (the field is out of date) | No times for the PCP; P-A offers others at the clinic. | A staff flag so the hospital updates the PCP. |
| 4. PCP outside this hospital | Nothing here to book: treated as no PCP. | A `Practitioner.Read` that fails says so. Not seen in the sandbox (step 0: every PCP was ours). |
| 4b. Two PCPs on file (step 0 found one test patient with two) | The one the caller saw last; with no visit with either, the place question (1B). | Epic's reply does not say which is the general PCP. |
| 5. No PCP | The place question (1B); a new-patient visit type. | The hospital's Cadence rules decide which providers take new patients. |
| 6. PCP is a resident in a teaching clinic | As case 1; residents rotate, so P-A matters more. | — |
| 7. The caller wants a new PCP | A registration task. | A hand-off to a person. |
| 8. A physical versus a sick visit | A physical goes to the PCP; a sick visit takes the soonest time at the clinic (P-A). | A booking rule per visit type (§3.5). |
| 9. A child (the caller is a parent) | The child's PCP is a pediatrician. | Proxy callers are not built; a caller for someone else is flagged today. |

### 3.3 Sooner with another clinician (P-A)
- With a PCP (or a provider the caller named), `ehr` offers that provider's times first. When
  the reply also holds a time with another clinician at the same place that is sooner than the
  provider's first, the agent says so once and offers it.
- A hospital setting, on by default; "how much sooner" is part of the setting. A booking rule
  with the outcome "Book with the PCP only" turns it off for the visits it matches (§3.5).

### 3.4 New or established
- `ehr` reads the caller's past visits once per call, when a search first needs them: one
  `Appointment.Search` over the window (a hospital setting, 3 years by default, the CPT rule).
  It is the same read `find_visit` makes for the past, with a longer window; a page past the
  first costs one more read.
- From those visits and the Places list, `ehr` knows the specialties the caller is established
  in, the last visit per specialty (§3.2), and the last visit with the PCP.
- The Visit types list gets a column **New or established** (new, established, either). `$find`
  searches only the rows that fit the caller for the specialty being booked. This closes the
  gap in §1.

### 3.5 The Booking rules (E-A)
- A table on Config › Hospital, on the console's one table (TABLES.md), kept in the settings
  store as one more hospital key, versioned as every list is.
- **Conditions** (each may be "any"): specialty, department, visit type, provider, new or
  established.
- **Outcomes** (a draft for the owner to edit):

  | Outcome | What the caller gets | Needs |
  |---|---|---|
  | Book | Times now. | — |
  | Book and flag for a referral check | Times now; staff check the referral before the visit. | A staff flag (FLAGS.md). |
  | Book with the PCP only | The PCP's times; P-A off for this rule. | The PCP (§3.2). |
  | Referral needed | The agent says this visit needs a referral from primary care, then offers a PCP visit or a person (a caller who has one goes to the person). | — (the rule, not a referral read: §3.6) |
  | Hand off | A person, with the reason in the staff summary. | — |

- The first row that matches wins. The last row is the default: Book. The rows are checked
  in Rust, inside the wall; the console shows rules, never a call's values.
- **When a rule is checked.** Specialty, department and new or established are known before the
  search: a rule on them decides the search, or that no search runs. Visit type and provider
  are known per proposed time: a time whose rule says Hand off or Referral needed is not
  offered, and when no time is left, the rule's outcome is the answer. Book checks the chosen
  time again and files a flag when its rule says so.
- **Why a rule needs no referral data in most calls.** Many specialties take patients with no
  referral (dermatology, OB/GYN, optometry, often behavioral health). Most specialty calls are
  from established patients, and §3.4 knows who they are. Many specialty clinics triage a new
  referral before anyone books it, and their own schedulers call the patient: for those, Hand
  off is the hospital's own process. Some hospitals book first and check the referral before
  the visit: Book and flag.
- **Where we draw the line:** a new patient, for a specialty that needs a referral, at a
  hospital that will not book before the referral is checked. The agent does not book that
  visit: the rule's outcome is Referral needed or Hand off, and a person books it.

### 3.6 Referrals: what Epic gives (checked 2026-11-16)
- **`ServiceRequest.Read (Referral) (R4)`** (`api:10164`) reads one referral by its FHIR id.
  There is no search by patient. The reply holds the identifier, the patient and the scanned
  documents; `status` is always "unknown" and `intent` always "proposal". No specialty, no
  referred-to provider, no open or closed state.
- **`$find`'s `referrals`** takes a referral id, and Epic then picks the visit type and the
  providers. A caller rarely knows the id.
- **Not taken (the owner, 2026-11-16):** `ServiceRequest.Search (Orders) (R4)` (`api:1054`)
  returns a patient's non-medication orders, and a referral in Epic usually starts as an order.
  We do not read orders to check referrals; the line is drawn above (§2).
- **Later, with a hospital:** Epic's HL7v2 "Outgoing Referral Notification" sends each new or
  changed referral to an outside system. It needs the hospital's interface team, and we would
  keep referral rows between calls, which "the record lives one call long" rules out today
  (VOICE-AGENT.md §8.7). The owner decides that rule when a hospital asks.
- Insurance stays with the hospital: the agent does not read the caller's plan (`Coverage`).

### 3.7 Where to go: a new tool
- A new tool (the name is a draft: `place_info`): the address, the phone and the hours of a
  place, from the hospital's lists only. For a place the caller names, or for the visit the
  caller just heard.
- It reads no record, so it works before verification too: the lists hold no protected data.
  Only "the visit you just heard" needs a verified caller. ("After verification only" gives the
  same security and a worse experience, so it was dropped.)
- Hours: the department's own; else the hospital's hours and closed days (HOSPITAL-CONFIG.md
  §2.6). This is the first call that reads them.
- A row with no address or no phone: the agent says it does not have it and offers a person.

### 3.8 Every place it runs
- **The call:** the same on any phone. The model sees spoken names only, never a code, an id
  or a rule.
- **The console:** Config › Hospital gains the Booking rules table, the New or established
  column on Visit types, and the P-A setting; Specialties gains a Primary care mark (§4). Every
  list past 200 rows is windowed (TABLES.md §6).
- **The deploy (A2, B):** the rules and lists come from the settings store, as today.
- **Each EHR:** the conditions are Epic's concepts. Another adapter maps its own (an
  athenahealth department, an Oracle Health appointment type) behind the same table
  (NON-EPIC-EHRS.md). The PCP and the past visits are standard FHIR (`generalPractitioner`,
  `Appointment`).
- **Cost:** per call, at most one more `Appointment.Search` (the 3-year window, often one page)
  and none for the PCP. The model's tool schema grows by three arguments. No new vendor.

## 4. Decided inside the owner's choices (for his review)
- **A call with no specialty named is primary care.** "I need to see my doctor" searches with
  the PCP. Specialties gets a **Primary care** mark (Family Medicine in the Default; a
  hospital adds Internal Medicine, Pediatrics and the like) so `ehr` knows which specialties count.
- The PCP's clinic is the place of the last visit with the PCP, not a `PractitionerRole` read.
- A provider the caller names ("Dr. Lin") takes the PCP's role in P-A: their times first.
- The Booking rules go on a new **Rules** tab of Config › Hospital, after Booking. (A placement:
  his to move.)
- The 3-year window and P-A's "how much sooner" are hospital settings (FUTURE-SETTINGS.md).
- A booking flagged for a referral check uses the flag kinds in FLAGS.md; its words name the
  kind of check, never the caller.

## 5. As built, and the desk tests

### 5.1 Step 1: new or established (built 2026-11-16)
> **Part:** built · 2026-11-16 · desk test open. Where this part and §3.4 disagree, this part
> is the code.

- **ehr › `hospital.rs`:** `PatientKind` (either, new, established; `fits`); the Visit types
  column `patients` (a version saved before it reads as "either"); the setting
  `established_years` (1 to 10; in `KEYS` and `settings.rs › HOSPITAL`); `PlaceRef` (a place as
  the record names it: FHIR id and lower-case name, wrapped); `established(seen, at)`: seen by a
  department of the same specialty on the Places list, else at the same department;
  `sorts_patients` (any bookable type for one kind); `patients_of`; `fake_type(online,
  established)`.
- **ehr › `scheduler.rs`:** each `Slot` carries `place_ref` and `kind`; `SlotWish.seen`. The fake
  picks each slot's type for the caller at its one place, and books the type it offered.
- **ehr › `service.rs`:** `seen`: at the first `FindSlots` of a call that needs it, one visit
  search over the window, the seen visits' places kept in `Session::seen`; `keep_fitting` drops
  the slots whose type does not fit the caller at the slot's place, on a new search and on
  "more". The same for every EHR. No search when every bookable type is "either".
- **Epic:** the search is `EpicScheduler::visits` with a range (EPIC.md §6, its new row). The
  slot's kind comes from the type `$find` returns, not the type searched.
- **The Default** (`ehr/fixtures/hospital.json`): Office Visit established, New Patient
  Appointment new, the video types and X-Ray Exam either; 3 years.
- **The agent:** `config.py › HOSPITAL_KEYS` gains `established_years`. No tool, word or wire
  change: the agent sees the same slots, fewer of them.
- **The console (`hospital.ts`):** the Visit types table's **Patients** column (Either, New,
  Established, with an "i" for each); Booking › **Years for an established patient**;
  `withRowDefaults` fills both for a version saved before them.
- **Tests:** `hospital.rs` (the rule by specialty and by department; the Default's types; an old
  version reads as "either"; the checks); `service.rs ›
  a_new_patient_books_the_new_patient_type_and_an_established_one_does_not`.

**Decided inside the design (for the owner's review):**
- The Default marks Office Visit "established" and New Patient Appointment "new". Before, the
  local fake booked Office Visit for everyone.
- When a slot's department is on no list, "established" means seen at that same department (by
  FHIR id, else by name).
- The window counts seen visits only (`fulfilled`, `arrived`, `checked-in` in the past), not a
  no-show or a cancelled one.
- The window ends today in the hospital's time zone.

**One kept script to record again.** `booked_reason_asked` calls as Priya Raman, who is now a
new patient: her booking reads back as "a new patient visit", so its stored replies no longer
match (`ReplayMiss`, 2026-11-16). Every other agent test passes (252). Record it again with the
Test button on Config › Agent (AGENT.md §3).

**What the sandbox cannot show.** Epic's sandbox returns every proposed time as a MyChart Video
Visit ("either"; EPIC.md §5.2), so in Epic mode no time is dropped. Local mode shows the rule.

**Desk test** (restart `npm run dev` first: a new `ehr`).
1. Config › Hospital › Visits: the Visit types table has a **Patients** column. The Default reads
   Established for Office Visit, New for New Patient Appointment, Either for the rest. Its "i"
   says what each means.
2. Booking: **Years for an established patient** reads 3, and takes 1 to 10 only.
3. Chat, Local, Test: call as **Jordan Ellis** (seen in October). Book an in-person visit. The
   read-back says "an office visit".
4. Call as **Priya Raman** (no visits). Book an in-person visit. The read-back says "a new
   patient visit".
5. Edit Visit types: set Office Visit to Either and save. Call as Priya again: the in-person
   times read back as "an office visit" (the first type that fits). Set it back.
6. Chat, Epic, as Camila Lopez: book as before. The request log shows one more Appointment search
   at the first slot search, and none at the second.

### 5.2 Step 3: the usual place, the PCP and P-A (built 2026-11-16)
> **Part:** built · 2026-11-16 · desk test open. Where this part and §3.2, §3.3 disagree, this
> part is the code.

- **The PCP:** `verify.rs › Candidate::pcp`, from Epic's Patient search (`epic_ehr.rs › pcps`:
  each `Practitioner/` reference in `generalPractitioner`, by FHIR id and display; no new
  request) or the fake's file (`sandbox.json`: Priya Raman's PCP is Dr. Sam Okafor). Kept in
  `Pending`, then `Session::pcp` when the code passes.
- **The seen visits** (`service.rs › seen`, the step 1 read): now `SeenVisit` (place, its words,
  clinician, start), newest first. Read when a type sorts patients, or when a usual place can
  matter (the caller has a PCP, or a specialty is marked Primary care).
- **The usual place** (`usual_place`): the last seen visit with a PCP on file (two PCPs: the one
  seen last, case 4b), else the last at a department of a Primary care specialty. Epic's `$find`
  gets its FHIR id as `location-reference`; ehr keeps only that place's slots either way, on
  every page.
- **The PCP first, and P-A** (`pcp_first`): the PCP's times first; with Booking › Offer a sooner
  time with another clinician on (the Default), the soonest time with another clinician on an
  earlier day than the PCP's first goes in as the last option of the first offer
  (`Shielded::earlier_day`, core). The others follow, for "more".
- **The wire:** `FindSlotsRequest.any_place`; `FindSlotsReply.place_default` (phi); `Slot.with_pcp`,
  `Slot.sooner`.
- **The agent:** `find_slots` takes `any_place`; with a usual place, the times start with
  `line.slots_usual_place` ("These are at {place}, where you were seen last. If you'd like
  somewhere else, just say so."); a sooner time adds `line.slot_sooner` ("Option {number} is
  sooner, with another clinician at the same place."). Drafts, his to edit.
- **The console:** Specialties gains **Primary care** (on for Family Medicine in the Default);
  Booking gains **Offer a sooner time with another clinician** (on).
- **Tests:** `service.rs › the_pcp_s_times_come_first_with_one_sooner_time_from_another_clinician`,
  `the_usual_place_is_the_last_with_a_pcp_else_the_last_primary_care_place`.

**Decided inside the design (for the owner's review):**
- **The usual place is named with the times, not asked before them.** §3.2 said the question
  before the first search names it ("at the north clinic, or somewhere else?"). Built instead:
  the search runs at the usual place and the times start by naming it, with "just say so" to
  look elsewhere (`any_place`). One question fewer for the caller who wants their usual place;
  no new RPC to learn the place before the search. **Kept (the owner, 2026-11-16): "Keep it
  for now, would rather be safe than sorry with the user not getting booked at the wrong
  place."** The caller hears the place with the times and again in each option and the
  read-back before anything is booked.
- With no usual place (cases 2 and 5), the search runs everywhere, as before: asking for a
  clinic or a town (1B) needs step 2's place matching.
- "Sooner" is any earlier day than the PCP's first time; a later time the same day is not.
  A setting for "how much sooner" waits until a hospital asks.
- The sooner time is the last option of the first offer, so the caller hears their PCP first.
- A PCP with no times in the search (left the practice, case 3): the times keep their order;
  no staff flag yet.

**What the sandbox and the fake can show.** Local: Priya Raman (PCP Dr. Okafor, never seen) gets
Dr. Okafor's times, then Dr. Lin's on an earlier day marked sooner; no usual place (the fake's
one clinic has no specialty). Epic: Camila Lopez (PCP on file, 3 visits with them) gets her
usual place named with the times, and her PCP's times first.

**Desk test** (restart `npm run dev` first: a new `ehr`).
1. Config › Hospital › Specialties: a **Primary care** column, on for Family Medicine. Booking:
   **Offer a sooner time with another clinician**, on.
2. Chat, Local, Test: call as **Priya Raman**; give a reason. Since step 2 she is asked where
   too (she has no usual place, §5.3): ask for in person, any time, at **Main Street**. The
   first times are Dr. Okafor's (Nov 3, Nov 5), then Dr. Lin's Nov 2, and the agent says that
   option is sooner, with another clinician.
3. Turn the Booking switch off and save; call as Priya again: Dr. Okafor's times, then Dr. Lin's,
   with no "sooner". Turn it back on.
4. Call as **Jordan Ellis** (no PCP on file): the times in date order, as before. Since step 2
   the Default marks Main Street Clinic as family medicine, so his times start by naming it
   ("where you were seen last").
5. Chat, Epic, as **Camila Lopez**; ask for online or either: the times start by naming her
   usual place. Say "somewhere else": the next search names none.

### 5.3 Step 2: the place and the specialty in the search (built 2026-11-16)
> **Part:** built · 2026-11-16 · desk test open. Where this part and §3.1, §3.2 disagree, this
> part is the code.

- **The matcher (core › `shielded.rs`):** `Shielded::names_terms`. The words that name no one
  place are left out ("the", "clinic", "doctor", "care" and the like). A term whose every word
  the caller said is a full match; when there is one, only full matches count ("EMC cardiology"
  names EMC Cardiology, not EMC Cardiology Procedures). Else a term that holds every word the
  caller said ("cardiology" names both). It answers with positions only.
- **The pick (ehr › `hospital.rs`):** `Hospital::pick(place, specialty)`. A specialty by its
  spoken name and the caller's words. A department by its spoken name, its record's name, its
  facility's names and its city (from the address: "…, Verona, WI 53593"). No specialty named
  is primary care: the departments of a primary care specialty, and those with no specialty
  on the list. A place named with no specialty that is not primary care keeps its own kind of
  care ("EMC Cardiology" searches cardiology). Several departments: a facility or a town that
  holds them all is searched whole; else the caller is asked which. Outcomes: search, unknown
  place, unknown specialty (also: no department has it), not at that place, which place.
- **The search (`service.rs › find_slots`):** the pick, then the usual place (§3.2, now also
  for a specialty: the last seen visit at one of its departments), then the question 1B: no
  place named, no usual place, and the care at more than one place. It names the places when
  they are no more than Booking › Groups one question names, else asks for a clinic or a town.
  The search keeps only the slots at its places (`Hospital::keeps`; for primary care with no
  place named, a slot at a department on no list is kept too). Epic's `$find` gets
  `location-reference` when one place is searched and `specialties` when one specialty with an
  ID was named. `FindVisit` keeps the visits at every place the words name.
- **One question, not two:** before the preference question (1A) the agent asks ehr where a
  search would look (`plan_only`: no `$find`). When ehr would ask where, the same turn asks it
  after the preference question. Asked once a call: a later search that names no place looks
  anywhere.
- **The wire:** `FindSlotsRequest.place`, `.specialty` (phi), `.plan_only`;
  `FindSlotsReply.place_ask`, `.places` (phi), `.other_places`; `FindVisitRequest.place` (phi);
  `FindVisitReply.unknown_place`.
- **The agent:** `find_slots` takes `place` and `specialty` (2A: `arg.find_slots.specialty` says
  never to choose one from the reason); `find_visit` takes `place`. New lines, drafts, his to
  edit: `line.ask_place`, `line.which_place`, `line.another_place`, `line.unknown_place`,
  `line.unknown_specialty`, `line.not_at_place`, `note.after_ask_place`,
  `note.after_place_unknown`.
- **The Default** (`ehr/fixtures/hospital.json`): a facility, Example Medical Center (Madison),
  holding EMC Family Medicine, EMC Cardiology and EMC Cardiology Procedures; Main Street Clinic
  (Verona) and EMC Family Medicine are family medicine; the X-ray department is radiology, with
  hours of its own (Saturday morning). Addresses and phone numbers are synthetic (555-01xx).
- **The fake:** three more slots, at EMC Cardiology (Dr. Rosa Patel, Nov 3 and Nov 5) and EMC
  Family Medicine (Dr. Lee Moreno, Nov 4). Each slot has its place.
- **Tests:** `shielded.rs › names_terms_prefers_a_full_match…`; `service.rs ›
  with_no_usual_place_the_caller_is_asked_where…`, `a_place_and_a_specialty_narrow_the_search_or_ask`,
  `a_visit_is_found_by_its_place…`; `test_gate.py ›
  test_with_no_usual_place_the_preference_question_also_asks_where`.

**Decided inside the design (for the owner's review):**
- **No "caller's words" column on Places.** §3.1 named one. Built instead: a department is
  matched by its spoken name, its record's name, its facility's names and its city; a facility's
  spoken name is a second name for every department in it. A Words column on a list of up to
  2,000 rows waits until a hospital needs it.
- **The where question lists the places** when they fit one question, as the visit groups do,
  and asks for "a clinic or a town" past that.
- **A town searches every department in it** (no question), as a facility does.
- **Several specialties named** ("heart" in two names): the union is searched.
- **A place named, no list:** "I don't know that place". A specialty named, no list: no filter.

**What the sandbox can show.** Epic's sandbox returns one department whatever is asked (step 0),
so a search across places shows only in Local. In Epic mode, with Places imported, a caller with
no usual place is asked where.

**Desk test** (restart `npm run dev` first: a new `ehr` and agent).
1. Chat, Local, Test, as **Priya Raman**: ask to book, give a reason. The agent asks in person
   or online, a day or time, and "Would you like Main Street Clinic or the family medicine
   clinic?" in one turn. Say "the family medicine clinic, in person, any time": Dr. Moreno's Nov
   4 time only.
2. As **Jordan Ellis**: "I'd like to see a heart doctor". He is asked EMC Cardiology or EMC
   Cardiology Procedures; say "EMC Cardiology": Dr. Patel's two times.
3. As Jordan: "an appointment in Verona": Main Street Clinic times. "at the medical center":
   Dr. Moreno's time (its primary care department).
4. As Jordan: "a skin doctor": the agent says it does not see that kind of care (dermatology has
   no ID). "an appointment in Chicago": it does not know that place.
5. As Jordan: "cardiology at Main Street": that care is not at that place.
6. As Jordan: "when is my visit at Main Street?": his next visit there.

### 5.4 Step 5: where to go (built 2026-11-16)
> **Part:** built · 2026-11-16 · desk test open. Where this part and §3.7 disagree, this part is
> the code.

- **ehr:** `PlaceInfo(handle, place)` (`service.rs › place_info`, `hospital.rs ›
  Hospital::place_info`). An open handle is enough. The words are matched as in §5.3. A
  facility or a town that holds every department named answers as the facility (its name,
  address and phone, the hospital's hours). One department: its spoken name; its address and
  phone, else its facility's; its own hours, else the hospital's. Several: which one. With every
  answer, the hospital's next three closed days.
- **The agent:** `place_info` is in every step (`global_functions`). With no place named, the
  place of the visit the caller just booked or heard; with none, it asks which place. It says
  the address (or that it has none), the phone (or that it has none) and the hours, days in a
  row with the same hours said together. The closed days go to the model only
  (`note.after_place_info`: say one only when asked about that day). New lines, drafts:
  `tool.place_info`, `arg.place_info.place`, `line.place_address`, `line.place_no_address`,
  `line.place_phone`, `line.place_no_phone`, `line.place_hours`, `line.place_info_which`.
- **Tests:** `service.rs › place_info_answers_from_the_lists_before_verification`;
  `test_gate.py › test_where_to_go_is_open_before_verification_and_reads_the_lists`.

**Decided inside the design (for the owner's review):**
- The address, phone and hours are said together in one turn; the closed days only when asked.
- A department with no address or phone takes its facility's before the agent says it has none.
- The model is told to use `offer_staff` when the caller needs an address or phone not on file
  (§3.7 says "offers a person").

**Desk test** (restart `npm run dev` first).
1. Chat, Local, Test, before verifying: "Where is the X-ray place?" The agent says EMH X-Ray
   Imaging's address and phone, and "Monday to Friday, 7 AM to 7 PM, and Saturday, 8 AM to 12
   PM".
2. "Are you open Saturday?" about Main Street Clinic: Monday to Friday only.
3. "What's the phone number for EMC Cardiology Procedures?": the medical center's number (it has
   none of its own).
4. Verify as Jordan, book a visit, then "Where do I go?": Main Street Clinic's address.

### 5.5 Step 4: the Booking rules (built 2026-11-16)
> **Part:** built · 2026-11-16 · desk test open. Where this part and §3.5 disagree, this part is
> the code.

- **ehr › `hospital.rs`:** `BookingRule` (specialty ID, department record name, visit type
  record name, provider name, patients, outcome; each condition empty for any) and
  `RuleOutcome` (book, book_and_flag, pcp_only, referral_needed, hand_off). The key
  `booking_rules` (in `KEYS` and `settings.rs › HOSPITAL`); a rule that names a row not on its
  list is refused, by row number. `rule_for` (a time: the first row whose every condition
  matches), `rule_before_search` (a department: known when the first row whose place conditions
  match names no visit type or provider).
- **ehr › `service.rs`:** before the search, when every place it would look at holds the visit
  back, nothing is searched and `FindSlotsReply.rule` says why (`held_back_everywhere`). After
  the search, each time is checked (`keep_by_rules`): Referral needed and Hand off drop it;
  Book with the PCP only drops another clinician's time; Book and flag marks it
  (`Session.flag_on_book`). When the rules dropped every time, the first one's outcome is the
  answer. "More" checks each new page the same way. `Book` says `check_referral` for a marked
  time. Each slot now carries its visit type as the record names it (`Slot::visit_type_ref`).
- **The agent:** Referral needed says `line.referral_needed` and the model offers a primary care
  visit or a person (`note.after_referral_needed`; a caller who has a referral goes to a person).
  Hand off says `line.rule_hand_off` and hands off, with a code flag `booking_rule_hand_off`
  whose evidence is the caller's reason, checked as theirs. A booking with `check_referral`
  files a code flag `referral_check` with the visit's CSN; the caller hears nothing. The flag
  kinds are drafts (FLAGS.md §3).
- **The console:** Config › Hospital › **Rules** (after Booking): the one table, with Specialty,
  Department, Visit type, Provider, Patients and Outcome; ↑ ↓ ✕ (the order matters); each
  outcome's "i" says what it does. The page checks a rule's rows as ehr does.
- **The Default:** no rules (every visit is Book).
- **Tests:** `hospital.rs › a_booking_rule_must_name_rows_on_the_lists`; `service.rs ›
  booking_rules_hold_back_hand_off_refer_flag_and_keep_to_the_pcp`.

**Decided inside the design (for the owner's review):**
- **Department and Provider are typed, not chosen.** Places holds up to 2,000 departments: a
  select of every one in each rule row breaks the speed rules (CONSOLE.md §12). The page refuses
  a department that is not on Places; a provider is compared as a clinician the caller names is.
- **Book with the PCP only, for a caller with no PCP on file:** every time is offered.
- **Patients "Any"** on the Rules tab is the Visit types column's "Either".
- **Flag kinds:** `referral_check` and `booking_rule_hand_off` (drafts, FLAGS.md §3).

**Desk test** (restart `npm run dev` first).
1. Config › Hospital › **Rules**, Edit. Add three rules and Save:
   1. Specialty Cardiology, Patients New, Outcome Hand off.
   2. Provider "Dr. Rosa Patel", Outcome Referral needed.
   3. Visit type Office Visit, Outcome Book and flag for a referral check.
2. Chat, Local, Test, as **Jordan Ellis**: "a heart doctor at EMC Cardiology". Rule 1 (he is new
   to cardiology): the agent says staff book this visit and hands off; the call has a
   `booking_rule_hand_off` flag. No search runs (the request log shows none).
3. Move rule 2 above rule 1 (↑) and Save. Call again: rule 2 matches each of Dr. Patel's times:
   the agent says a referral is needed and offers primary care or a person.
4. As Jordan, book an in-person visit at his usual place: the booking reads back as usual, and
   the call has a `referral_check` flag (rule 3). An online visit has none.
5. Type a department that is not on Places in a rule: the page names the row and does not save.

### 5.6 The later steps
None: steps 0 to 5 are built. §7 holds what is open.

## 6. The build
> **Every step built, 2026-11-16** (the owner: "Let's finish building out the booking rules /
> steps"): step 3 before step 2, then 2, 5 and 4. Open: his desk tests of steps 1 to 5 (§5.1 to
> §5.5), then every kept script recorded again (the Test button on Config › Agent).

One load-bearing step at a time; each ends with the owner's desk test. Each step changes the
words in AGENT-PROMPT.md and TOOLS.md where a tool changes, records the kept scripts again
(the Test button on Config › Agent), and adds its rows to DEV-API.md and EHR-SERVICE.md §3.

**Step 0 — the sandbox look. Done 2026-11-16** (`ehr epic-booking`; EPIC.md §7.12).
- The PCP: on 3 of 7 test patients, a relative `Practitioner/` reference that
  `Practitioner.Read` answers (ours); Camila Lopez has 3 of 8 visits with hers (case 1),
  Desiree Powell 0 of 2 (case 2); Warren McGinnis has two PCPs (case 4b).
- `specialties`: accepted, ignored by the sandbox's rules, and no specialty on the reply. So
  `ehr` filters by the Places list's specialties (§3.1). Step 2 needs no change in order.
- One finding for step 1 still to see on a real search: the sandbox returned one department
  even with no `location-reference`, so the sandbox cannot show a search across places.

**Step 1 — new or established, and the past visits read** (§3.4). **Built 2026-11-16, desk
test open: §5.1.** It fixes a gap that books today.
- `ehr`: the past-visit read once per call (the window a hospital setting); the established
  specialties, the last visit per specialty, from Places. `hospital.rs`: the Visit types
  column, its check, `coded_bookable` by the caller's status. The Default: "New Patient
  Appointment" new, the others either.
- Console: the column on Visit types; the window on the Booking tab.
- Tests: `ehr` unit tests for the status and the visit-type choice; the fake marks its slots.
- Desk test: on Epic, an established sandbox patient is not offered new-patient times.

**Step 2 — the place and the specialty in the search** (§3.1). **Built 2026-11-16, desk test
open: §5.3.**
- Proto: `FindSlotsRequest.place`, `.specialty`; `FindVisitRequest.place` (all tagged `phi`).
- `hospital.rs`: the place and specialty matchers. `epic_ehr.rs`: `location-reference` and
  `specialties` in `$find`. `scheduler.rs`: the fake's slots carry a place and a specialty.
- `flow.py`: the arguments; the narrowing question; the no-match line.
- Kept scripts: a caller who names a clinic; one who names a specialty; two clinics that match.
- Desk test: on Local and on Epic, "a cardiology visit at the north clinic" offers only those
  times.

**Step 3 — the default place, the PCP and P-A** (§3.2, §3.3). **Built 2026-11-16, desk test
open: §5.2.** Taken before step 2, with step 2's `location-reference` only.
- `ehr`: the PCP from the Patient read, kept in the session (`Shielded`); the default place by
  kind of care; the PCP's times first; the sooner time at the same place. The Specialties
  Primary care mark. The P-A setting.
- `flow.py` and AGENT-PROMPT.md: the place question names the default; the sooner line.
- Kept scripts: "the same place as last time"; a primary care booking with the PCP far off;
  a caller with no visits.
- Desk test: the cases in §3.2 that the sandbox can show (1, 5, and 4 if step 0 finds one).

**Step 4 — the Booking rules** (§3.5). **Built 2026-11-16, desk test open: §5.5.**
- `hospital.rs`: the rule rows, their check, the first match; the key in `KEYS` and
  `settings.rs › HOSPITAL`. `service.rs`: the rules before the search, per proposed time, and
  at Book; the flag on Book and flag.
- Console: the Rules tab, the table with ↑ ↓ ✕ (rule order matters), each outcome with an "i".
- Kept scripts: a rule that hands off a new neurology patient; Book and flag; Book with the
  PCP only.
- Desk test: write three rules on the page, Save, and call each case on Local.

**Step 5 — where to go** (§3.7). **Built 2026-11-16, desk test open: §5.4.** Small; may run
beside step 3 or 4 (it touches no shared primitive).
- A new RPC (the name is a draft: `PlaceInfo`), open before verification, that returns the
  spoken address, phone and hours of a matched place, or of the visit just heard after
  verification. `flow.py`: the tool, in every step. The hours read the department's, then the
  hospital's hours and closed days.
- Kept scripts: "Where do I go?" before verification; "Are you open Saturday?"; a place with no
  phone on file.
- Desk test: both questions on Local, before and after the code.

## 7. Open
- The words of every new line (AGENT-PROMPT.md): the owner's, after each step's desk test.
- The outcome list (§3.5) and the two flag kinds (§5.5) are drafts.
- Every kept script misses its replay (new tools and words in every step): record them again
  after the desk tests. New kept scripts the build list names (a caller who names a clinic, one
  who names a specialty, two clinics that match; the three rules; "Where do I go?" before
  verification) are not written yet.
- The Places "caller's words" column (§5.3) waits for a hospital that needs it.
- "How much sooner" for P-A, and a staff flag for a PCP who has left (case 3): when a hospital
  asks.
- Case 2 without a past visit needs `PractitionerRole` (not registered): when a hospital needs it.
- A referral feed from the hospital (§3.6): later, with the rule on keeping records.
- Who may edit the rules: waits on the roles (ACCESS-ROLES.md).
