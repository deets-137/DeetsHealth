---
status: built
desk_test: open
sources: [docs/architecture/images/hospital-columns-misaligned-2026-11-15.png, ehr/src/hospital.rs, ehr/fixtures/hospital.json, ehr/src/settings.rs, ehr/src/service.rs, ehr/src/scheduler.rs, ehr/src/epic_ehr.rs, ehr/src/visits.rs, ehr/src/session.rs, ehr/migrations/0009_settings_hospital.sql, ehr/proto/deets/ehr/v1/ehr.proto, agent/deets_agent/config.py, agent/deets_agent/dev_server.py, agent/deets_agent/flow.py, agent/deets_agent/ehr_client.py, console/src/pages/hospital.ts, console/src/agent/api.ts]
updated: 2026-11-15
---
# DeetsHealth — Config › Hospital: the hospital's own lists

> **Part:** built · 2026-11-15. The owner, 2026-11-14: "config > Hospital we can include lists
> for the hospital to configure per their setup like the list of service types. We can also
> include a section for hospital hours, configurable list for time of day, configurable list of
> available specialties, indications, and location-references." His forks of 2026-11-15:
> **1B**, every section now, each list that no call reads yet marked so ("1B will let us know
> what we have left to build out and something to work towards"); **2A**, one hospital for
> every EHR source ("One hospital for now, i want to implement multiple hospitals and use rules
> based routing later": ideas/README.md).

**Terms.**
- **A list** is a table of rows that the hospital keeps. Each row maps the hospital's own code
  (from its EHR build) or the record's own name to the words the agent says and the words a
  caller may say.
- **Config › Hospital** is a page under Config, after Config › System
  ([SYSTEM-PAGE.md](SYSTEM-PAGE.md)). Config › System holds words and variables. Config ›
  Hospital holds facts about the hospital's setup.
- **The call's hospital** is the version that was active when the call opened. The call keeps
  it to its end.

## 1. Why
Epic's `$find` needs the hospital's own codes (EPIC.md §7.1). The visit type is required: with
no visit-type list, no Epic booking can start. The time of day, the specialty, the reason and
the location are the inputs that let a caller say "mornings at the north clinic with a skin
doctor". Each needs a list that only the hospital can fill. Until 2026-11-15 each was a fixed
value in one of three fixtures or a constant in the code.

## 2. The sections
Four tabs: **Visits**, **Times**, **Places**, **Booking**. "Used now" means a call reads it
today. "Not used yet" means the page keeps it and no call reads it; its note on the page says
so.

### 2.1 Visit types (Epic's `service-type`) — Visits tab, used now
- Columns: the record's name, the spoken name, the code system, the code, Online, Agent may
  book.
- A visit or a slot whose type, as the record names it, is on the list (in any case) says the
  row's spoken name and takes the row's Online mark. A spoken name that names sensitive care
  makes the visit private (FUTURE-SETTINGS.md §9). A type not on the list keeps the record's
  words, and the `ONLINE` word list in `visit_about.rs` still decides whether it is online.
- Epic's `$find` searches each row with Agent may book **and** a code, one search per row that
  fits the caller's in person or online (`Hospital::coded_bookable`). The local fake books the
  first row of each kind with Agent may book (`Hospital::fake_type`), so the order matters.
- The Default: "Office Visit" and "Video Visit" (the fake's, no code), Epic's sandbox build
  "New Patient Appointment" (1003) and "MyChart Video Visit" (1400000001), and "X-Ray Exam"
  (not booked). Epic's own `$find` sample uses "Office Visit" 95014 in the system
  `urn:oid:1.2.840.114350.1.13.0.1.7.3.808267.11`; it is not on the list, because the probe
  did not book with it (EPIC.md §5.2).

### 2.2 Visit groups — Visits tab, used now
- Columns: the name the caller hears, the caller's words, the visit types (by the record's
  name).
- Uses: `find_visit`'s `group`, and the narrowing question when a caller has several visits
  (EPIC.md §7.10). A group whose name or words name sensitive care is never named
  (`VisitGroups::new`). Since 2A the groups apply in Epic mode too.

### 2.3 Reasons as codes (Epic's `indications`) — Visits tab, not used yet
- Columns: the caller's words, the code system, the code, the visit type, the specialty.
- Decided 2026-11-14 (EPIC.md §5.2): booking sends the reason in the caller's words only. This
  list is the map for when a hospital codes reasons. The Default row is Epic's own sample:
  SNOMED 46866001, a broken leg.

### 2.4 The time zone — Times tab, used now
- One IANA name for the hospital. Uses: the call's today and time (`OpenCallReply.local_now`),
  `$find`'s `startTime` and `endTime`, and each visit's local time. `$find` also sends each
  department's own zone with a slot, and that wins for the slot (EPIC.md §7.2).

### 2.5 Times of day — Times tab, used now
- Columns: the name ("morning"), the start (HH:MM), the length in hours.
- Uses: `find_slots`'s `time_of_day`, which may hold only these names (the model's list comes
  from the call's own version, `OpenCallReply.times_of_day`); Epic's `$find` gets
  `Timing.repeat.timeOfDay`, `duration` and `durationUnit`. The days of the week need no list.

### 2.6 Hours and closed days — Times tab, not used yet
- Hours: day of the week, opens, closes. Closed days: the date and a name.
- For later: what the agent says when no person can take a hand-off now (FUTURE-SETTINGS.md
  §10), and when a nurse may join a call on a staff flag (FLAGS.md §5). `$find` returns open
  slots only, so the hours never filter slots.

### 2.7 Locations (Epic's `location-reference`) — Places tab, used now in part
- Columns: the record's name, the spoken name, the FHIR id, the address, the phone.
- Used now: a visit at a Location on the list says the spoken name; the first row is said when
  a record names no place (`Hospital::place`). Not used yet: the FHIR id (`$find`'s
  `location-reference`, "the same place as last time"), the address and phone ("where to go").
  `Location.Read` could fill the address and phone, once per row, never once per call.

### 2.8 Specialties (Epic's `specialties`) — Places tab, not used yet
- Columns: the spoken name, the caller's words, the code system, the code. Epic gives no list
  of a hospital's specialty codes by API.

### 2.9 Booking — Booking tab, used now
- Slots offered at a time (1 to 5); visits before the narrowing question; groups one question
  names; days back for a past visit; the search windows for the visits to come (each longer
  than the one before; one Epic read per window, then one with no end). FUTURE-SETTINGS.md
  §13.2 and §13.3 list the choices not built yet (the order, the clinician's name, times on one
  day).

## 3. Where the values come from
- **The store**: the `hospital` service of the settings store (`settings_hospital`, migration
  0009), one setting per list or number (`hospital.rs › KEYS`). Whole versions, insert-only;
  each save says who saved it. A save reaches each call that opens after it.
- **The Default**: `ehr/fixtures/hospital.json`. The agent seeds it as version 0
  (`config.py › hospital_defaults`). `ehr` builds the same file in and uses it when it has no
  store, when the store has no version, or when the active version cannot be read; the call
  record then says "file".
- **Typed by the hospital** on the page. Importing a file from the hospital's Cadence team is
  open (§7).
- **No protected data.** A row is a fact about the hospital, never a patient. Every error names a
  list and a row number, never a value.

## 4. Every place it runs
- **The call:** the same on any phone. The model sees only the spoken names it may offer and
  the time-of-day names, never a code.
- **The console:** one page, four tabs of tables, each row edited in place. Who may edit waits on
  the roles (ACCESS-ROLES.md). On a phone each table scrolls on its own (`.sys-table-wrap`).
- **The deploy:** A2 and B read the lists from the settings store.
- **Each EHR:** the columns are Epic's. Another adapter maps its own concepts behind the same
  lists (athenahealth's appointment types, Oracle Health's; OTHER-EHRS.md). A list an EHR does
  not use stays empty.
- **Many hospitals** (later; the owner, 2026-11-15): one deploy serving several hospitals, with
  rules that route a call to its hospital. Today there is one.

## 5. As built (2026-11-15)
> Where this section and the design above disagree, this section is the code.

- **ehr**
  - `hospital.rs`: the typed rows, `check_key` (one setting), `Hospital::from_pairs` (the whole
    set; a key a version lacks takes the file's value), `Hospital::file`, and the lookups a call
    uses: `about` and `booked` (spoken name, Online mark, the sensitive screen), `group_of`,
    `place`, `time_of_day`, `time_names`, `coded_bookable`, `fake_type`, `limits`, `tz`.
  - `settings.rs`: `HOSPITAL` (the same keys), `Kind::Hospital` (checked by name in `check`),
    and `coherent("hospital")`, which builds the whole hospital.
  - `service.rs › call_hospital`: `OpenCall` reads the active version once; the session holds
    it (`Session::hospital`). `FindSlots`, `Book` and `FindVisit` pass it to the adapters
    (`Scheduler` methods take `h: &Hospital`) and to `visits::narrow` (`Limits`).
  - `scheduler.rs`: the fake's six slots are in person or online; each takes its type from the
    list. `epic_ehr.rs`: no time zone, place, groups or types of its own; the rail's list
    (`ehr epic-sandbox`) uses the file's time zone, because it is not a call.
  - The fixtures: `sandbox.json` and `epic-sandbox.json` keep the test patients, the fake's
    visits and the rail's place only.
  - `store.rs`: `calls.hospital_version`, from the record's `hospital` event.
- **The wire**: `OpenCallReply.times_of_day` and `hospital_version` (EHR-SERVICE.md).
- **The agent**: `ehr_client.py` keeps each call's names (`call_times`) and writes the
  `hospital` event (call record schema 13); `flow.py › CallFlow.times_of_day` builds
  `find_slots`'s list; `dev_server.py`: `GET` and `POST /agent/hospital` (DEV-API.md),
  `hospital_view`.
- **The console**: `hospital.ts`, built from System's table parts (`system.ts` exports
  `table`, `row`, `field`, `heading`, `grow`); `api.ts` types; the `settings.hospital` store.
  The page checks each row as ehr does (`problem`) and shows the first problem above the tabs;
  Save waits until there is none. No new CSS.
- **The columns' alignment** (the owner, 2026-11-15): the words of an editable cell started
  about 13 px right of their header, on this page and on Config › System. Fixed in
  `config.css › .sys-table .prompt-control`: the control moves left by the box's inner padding
  and edge.

  ![The owner's screenshot, 2026-11-15: the Visits tab, each editable column's words right of its header (an orange line marks Spoken name); he asks what Code system and Code are](images/hospital-columns-misaligned-2026-11-15.png)

**Decided inside the owner's choices (for his review):**
- The tabs and columns of §2, and the spoken names of the Default ("an office visit", "a video
  visit", "a new patient visit", "an X-ray"; "EMC Family Medicine" says "the family medicine
  clinic").
- A visit type without a code may be booked by the fake only; the Office Visit and Video Visit
  rows keep the fake's bookings as they were.
- The first location is the place said when a record names none.
- The page sits after System in Config's nav.
- A list's words are typed comma separated in one cell.
- A row moves with Up; Remove takes it out. Add puts an empty row at the end to fill in.
- Closed days and hours have one zone: the hospital's.

## 6. Desk test
1. `npm run dev`. Open Config › Hospital. Read shows four tabs; Visits shows five visit types.
2. Choose Edit. On Visits, change the spoken name of "Office Visit" to "a clinic visit". Save.
   The status says the new version is active from the next call.
3. On Chat, Local, start a typed call as Jordan Ellis and book a visit. Ask what the visit is.
   The agent says "It is listed as a clinic visit."
4. On Times, add a time of day "lunch" from 11:30 for 2 hours. Save. Start a new call and ask
   for "a lunch time slot". `find_slots` gets `time_of_day: lunch`.
5. Clear a visit type's name. The problem shows above the tabs and Save waits.
6. Choose the Default in the versions list: the page shows it, read only.
7. In Epic mode, a call that asks for Camila Lopez's next visit says "the family medicine
   clinic".

## 7. Open
- **The owner's fork (2026-11-15, night; he decides in the morning): how the page shows the code
  system.** Each code column pair names one item of the hospital's build: Code is the item's ID
  (a visit type's ID in Cadence, 1003), and Code system is the list it comes from, as an OID
  (`1.2.840.114350` is Epic's root). They are sent to `$find` as one coding.
  - **A (recommended): one ID list per list, set once** above the table ("Epic visit type list
    (OID)"); each row keeps only its ID. Specialties the same. Reasons keep a system per row,
    chosen by name (SNOMED CT, ICD-10-CM, the hospital's own). Cost: a schema change; no mixing
    of two lists.
  - **B: a system on each row, with plain names** ("ID list", "ID"; a known OID shown by its
    name). Cost: the same OID typed on every row.
  - Before he decides: research Epic's documentation on these systems (HANDOFF.md, Open now).
- Import from a file (the hospital's Cadence export), as well as typing.
- Whether a location may override the hospital's hours and time zone, or only add to them.
- Each list that is not used yet waits for its feature: the specialty and location in `$find`,
  coded reasons, hours with the hand-off.
- Many hospitals and the routing rules (ideas/README.md).
