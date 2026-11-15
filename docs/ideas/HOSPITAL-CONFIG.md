---
status: idea
desk_test: none
sources: [ehr/src/scheduler.rs, ehr/src/epic_ehr.rs, ehr/fixtures/epic-sandbox.json, agent/deets_agent/flow.py]
updated: 2026-11-15
---
# DeetsHealth — Config › Hospital: the hospital's own lists (idea)

> Not built. The owner, 2026-11-14: "config > Hospital we can include lists for the hospital to
> configure per their setup like the list of service types. We can also include a section for
> hospital hours, configurable list for time of day, configurable list of available
> specialties, indications, and location-references." The page design and each list's shape
> are his forks when the work starts.

**Terms.**
- **A list** is a table of rows that the hospital keeps. Each row maps the hospital's own code
  (from its EHR build) to the words the agent says and the words a caller may say.
- **Config › Hospital** is a new page under Config, beside Config › System
  ([SYSTEM-PAGE.md](../architecture/SYSTEM-PAGE.md)). Config › System holds words and
  variables. Config › Hospital holds facts about the hospital's setup.

## 1. Why
Epic's `$find` needs the hospital's own codes (EPIC.md §7.1). The visit type is required: with
no visit-type list, no Epic booking can start. The time of day, the specialty, the reason and
the location are the inputs that let a caller say "mornings at the north clinic with a skin
doctor". Each needs a list that only the hospital can fill. Today each is a fixed value in a
fixture (`ehr/fixtures/epic-sandbox.json`) or in the fake's slots (`scheduler.rs`).

## 2. The sections
Each list is a draft of its columns. The owner edits them.

### 2.1 Visit types (Epic's `service-type`)
- Columns: the code and its code system; the spoken name ("a video visit"); in person or
  online; the length; new or returning patients; whether the agent may book it; the
  specialties and locations that offer it.
- Uses: `$find`'s required `service-type`; the caller's in person or online choice
  (`VisitMode`, scheduler.rs); the visit's words on NextVisit (FUTURE-SETTINGS.md §9.2: the
  spoken name, which types count as online, which types the agent books).
- Replaces: the fake's "Office Visit" and "Video Visit" and the `ONLINE` word list in
  `visit_about.rs`, for a hospital that fills the list.

### 2.2 Hours
- Columns: the hospital's hours by day of the week; each location's own hours; the holidays
  and closed days; the hours of each staff line the agent hands calls to.
- Uses: what the agent says when no person can take a hand-off now (FUTURE-SETTINGS.md §10);
  when a nurse may join a call on a staff flag (FLAGS.md §5). `$find` already returns open
  slots only, so the hours never filter slots.

### 2.3 Time of day
- Columns: a name the caller may say ("morning", "lunch", "after work"), its first hour and its
  length.
- Uses: `$find`'s `time-of-day` input (`Timing.repeat.timeOfDay` with `duration` and
  `durationUnit`). The days of the week ("Tuesdays or Thursdays") need no list: they are
  `Timing.repeat.dayOfWeek`.
- A default list ships with the product. The hospital edits it.

### 2.4 Specialties (Epic's `specialties`)
- Columns: the code; the spoken name ("dermatology"); the words a caller may say ("a skin
  doctor"); the visit types and locations that offer it.
- Uses: `$find`'s `specialties`; the agent's words for a slot's specialty (EPIC.md §7.2).

### 2.5 Reasons as codes (Epic's `indications`)
- Columns: the code (SNOMED or the hospital's own diagnosis code); the words a caller may say;
  the visit type and specialty it leads to.
- Uses: `$find`'s `indications`. Decided 2026-11-14 (EPIC.md §5.2): booking sends the reason in
  the caller's words only, until a hospital maps reasons to codes. This list is that map.
- The caller's words still go in `appointmentNote`, as now.

### 2.6 Locations (Epic's `location-reference`)
- Columns: the Location's FHIR id; the spoken name ("the north clinic"); the address and the
  phone; the time zone; the hours (§2.2); the visit types and specialties there.
- Uses: `$find`'s `location-reference`; "the same place as last time" (the last visit's
  Location reference, EPIC.md §7.3); where to go, said after a booking.
- `Location.Read` (selected, EPIC.md §2.1) can fill the address and phone, once per location,
  when the hospital adds the row. Never once per call.

### 2.7 The time zone
- The hospital's time zone (an IANA name, such as `America/Chicago`). Each location may have
  its own (§2.6). `$find` also sends the department's zone with each slot (EPIC.md §7.2).
- Uses: the call's today, and the date and time the agent is told (EPIC.md §7.8); the
  `startTime` and `endTime` sent to `$find`.
- Replaces: `time_zone` in `ehr/fixtures/epic-sandbox.json`.

### 2.8 Offering slots
- How many slots the agent offers at a time (now `MAX_OFFERED`, 3), and how it presents them
  (EPIC.md §7.8). A setting, not a list; it sits on this page because it is how the hospital's
  schedule meets the caller.

### 2.9 Visit groups
- The owner, 2026-11-14: the agent "can ask a follow up to narrow down (ie surgical type, or
  primary type, etc)".
- Columns: the name the caller hears ("surgery", "primary care", "imaging"); the words a
  caller may say ("my operation"); the members, by visit type (§2.1), specialty (§2.4) or
  location (§2.6).
- Uses: `find_visit`'s `group`, and the narrowing question when a caller has several visits
  (EPIC.md §7.10). A group that holds a private visit, or whose name holds a sensitive word,
  is never named.
- The settings that go with it (on or off, how many matches start the question, how many
  groups it names, their order) sit beside the list.

## 3. Where the values come from
- **Until the page exists** (2026-11-15): the dev hospital's values are files that ehr builds
  in and the agent reads: `ehr/fixtures/hospital.json` (the times of day, the slots offered at
  once), `epic-sandbox.json` (the time zone, the visit types per mode, the visit groups) and
  `sandbox.json` (the fake's time zone and visit groups).
- **Typed or imported by the hospital.** Each list can take a file the hospital's Cadence team
  exports. To find out: whether Epic gives any of these lists by API. No FHIR API in our
  registration lists visit types or specialties.
- **Every change is audited** and reaches each call that starts after it, as for the agent's
  words (PROMPT-EDITOR.md).
- **No protected data.** These are facts about the hospital, not about a patient. A row never
  names a patient.

## 4. Every place it runs
- **The call:** the same on any phone. The agent reads the lists; the model sees only the
  spoken names it may offer, never the codes.
- **The console:** one page with a section per list; each row edited in place. Who may edit
  waits on the roles (ACCESS-ROLES.md).
- **The deploy:** A2 and B read the lists from the settings store (not built: DATABASE.md).
- **Each EHR:** the columns are Epic's. Each adapter maps its own concepts behind the same
  list (athenahealth's appointment types, Oracle Health's; OTHER-EHRS.md). A list an EHR does
  not use stays empty.

## 5. Open (the owner's forks when the work starts)
- The page layout and each list's columns.
- Whether the lists are typed, imported, or both.
- Whether a location may override the hospital's hours and time zone, or only add to them.
- The default time-of-day list.
