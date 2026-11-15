---
status: idea
desk_test: none
sources: [ehr/src/epic_ehr.rs, ehr/src/epic.rs, ehr/src/scheduler.rs, ehr/fixtures/sandbox.json, ehr/fixtures/hospital.json]
updated: 2026-11-15
---
# DeetsHealth — a local FHIR server in Epic's shapes (idea)

> Not built. The owner, 2026-11-15: "Write an ideas doc for A, will leave it for another time."
> Option A of the fork "the shape of the Local EHR". Option B (a database scheduler behind our own
> `Scheduler` trait) and the hybrid (B, Epic's real replies as fixtures, Epic mode for live
> checks) were the other choices. The forks below are his when the work starts.

**Terms.**
- **The local FHIR server** is a small HTTPS server, run by the dev tools, that answers the same
  requests as Epic's sandbox, in Epic's shapes, from a database we control.
- **Local mode** today is ehr's in-memory fakes (`FakePatients`, `FakeScheduler`). **Epic mode**
  is Epic's public sandbox (EPIC.md §5.2, §7.10).

## 1. Why
Testing on Epic's sandbox alone has limits (2026-11-15):
- **Data we cannot shape.** Every `$find` slot is a video visit, even for the in-person type;
  video visits hold no instructions; no test patient has a sensitive visit; the test patients'
  visits share one type, so the narrowing question never asks.
- **No undo.** FHIR has no cancel: each test booking stays until Epic's weekly wipe. Other
  developers book on the same patients.
- **Downtime.** The whole sandbox answered 503 on 2026-11-13 and 2026-11-14.
- **It changes each day,** so it cannot back the kept scripts (the replay cache).

Today's Local mode has its own limits: bookings vanish when ehr restarts; "today" is frozen at
2026-10-26 and the slots at 2026-11-02 to 11-06; nothing ages into the past.

A local FHIR server gives data we shape, that persists and resets, **and** runs the real Epic
adapter (`epic_ehr.rs`), so a test in Local mode exercises the same code as a call at a
hospital.

## 2. What it serves
Each in Epic's shape, as the probe recorded it (EPIC.md §5.2, 2026-11-15):
- `.well-known/smart-configuration` and the token endpoint: accepts our signed ES384 assertion
  (`epic.rs`), checks it against our own public key, gives a token.
- `Patient?family&given&birthdate`, `Patient/{id}`: name, birth date, `telecom` with `use`,
  `rank` and `period`, `identifier`.
- `Appointment?patient&service-category&status&date` (R4) and `Appointment/{id}`: `serviceType`,
  `reasonCode`, `description`, `patientInstruction`, `identifier` (a CSN), participants,
  statuses, and `comment` (never read; kept to prove it).
- `POST STU3 Appointment/$find`: the patient's demographics, `startTime`, `endTime`,
  `service-type` (with a dash; `serviceType` refused as Epic does), `time-of-day`; proposed
  Appointments with the timezone extension, a contained Slot, absolute participant references;
  10 a page, `total`, a `next` link with a `sessionID`; the next page by a POST of empty
  Parameters (a GET answers 403); error 4113 when the session expires.
- `POST STU3 Appointment/$book`: `patient` and `appointment` as `valueIdentifier`s,
  `appointmentNote`; the booked Appointment with a new CSN; a taken slot refused.
- OperationOutcomes in Epic's words and codes (59102, 59138, 4101, 59109) where Epic sends them.

## 3. The data
- **A `sandbox` schema in the dev Postgres** (DATABASE.md): patients, schedules (each clinician's
  weekly hours, visit types and lengths, per department and time zone), visits (with CSN,
  status, instructions), bookings. ehr stays the only writer to the store; the local server is
  its own process with its own schema, synthetic only, never in a release build.
- **Seeded** from `ehr/fixtures/sandbox.json` and `hospital.json`: the four fake patients (the
  twins, the patient with no mobile), visits of each status, a sensitive visit, in-person and
  online types, instructions, two visit groups.
- **Slots** come from the schedules, counted from the real today in each department's zone, so
  the calendar rolls forward as Epic's does. A booking takes its slot.
- **Time passes:** a visit whose time has gone keeps its status, as at Epic (`booked` with no
  outcome is never said); a Reset or a rule can mark it seen or missed.

## 4. What stays as it is
- **The kept scripts and unit tests** stay on the in-memory fixture: the replay cache needs the
  same request each run.
- **Epic mode** stays the live check against Epic itself.
- **Local calls use the real date** and the hospital's time zone, as Epic mode does.

## 5. The size (scoped 2026-11-15)
About 2 to 3 sittings and roughly 2,500 lines, then upkeep: each new Epic API we use needs its
local copy, and each new fact the probe finds needs the copy corrected. Option B is about one
sitting and 1,000 lines; the hybrid adds Epic's recorded replies as adapter fixtures to B.

## 6. Risks
- **Drift from real Epic.** A local copy can pass while Epic fails. Each reply shape is built
  from a recorded Epic reply, and `ehr epic-find` / `epic-book` are run again after each Epic
  upgrade (each November) to compare.
- **Other EHRs.** Each later EHR needs its own local server, or Local mode tests only Epic's
  adapter.
- **TLS and sign-in.** `epic.rs` refuses anything but https and checks hosts: the local server
  needs a dev certificate (`dev-certs/`) and a base URL in the Epic config.

## 7. Open (the owner's forks when the work starts)
- When the local data goes back to the seed: a Reset button on the Chat page's rail only, or
  also a weekly wipe like Epic's.
- Whether Local mode becomes the local server (and the in-memory fakes serve only the tests), or
  a third choice beside Local and Epic on the Chat page.
- Whether test patients and schedules are edited in the console, or only in the seed files.
- Which language it is written in (Rust beside ehr, or a small Node or Python server).
