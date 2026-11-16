---
status: built
desk_test: open
sources: [docs/architecture/images/hospital-columns-misaligned-2026-11-15.png, docs/architecture/images/hospital-visit-type-code-2026-11-15.png, docs/architecture/images/hospital-reasons-codes-2026-11-15.png, docs/architecture/images/hospital-specialties-2026-11-15.png, ehr/src/hospital.rs, ehr/fixtures/hospital.json, ehr/src/settings.rs, ehr/src/service.rs, ehr/src/scheduler.rs, ehr/src/epic_ehr.rs, ehr/src/visits.rs, ehr/src/session.rs, ehr/migrations/0009_settings_hospital.sql, ehr/proto/deets/ehr/v1/ehr.proto, agent/deets_agent/config.py, agent/deets_agent/dev_server.py, agent/deets_agent/flow.py, agent/deets_agent/ehr_client.py, console/src/pages/hospital.ts, console/src/pages/epic-oid.ts, console/src/agent/api.ts, console/src/styles/config.css, docs/integrations/EPIC.md, console/src/styles/people.css, docs/architecture/images/hospital-places-toggle-2026-11-15.png, docs/architecture/images/hospital-places-import-button-2026-11-15.png, docs/architecture/images/hospital-import-panel-margins-2026-11-15.png]
updated: 2026-11-16
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
Six tabs: **Visits**, **Times**, **Places**, **Specialties**, **Booking**, **Rules**. "Used now" means a call reads it
today. "Not used yet" means the page keeps it and no call reads it; its note on the page says
so.

### 2.1 Visit types (Epic's `service-type`) — Visits tab, used now
- Above the table: the **Visit type list**, one OID for every row (§5.1). Columns: the
  record's name, the spoken name, the ID ("R PRC 1003"), Online, Agent may book, and
  **Patients** (either, new, established; since 2026-11-16, BOOKING-RULES.md §5.1): a type for
  one kind is offered only to a caller who fits at the slot's department.
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
- Above the table: the **Diagnosis list**, the OID of the hospital's own diagnoses (§5.1).
  Columns: the caller's words, the code system by name (SNOMED CT, ICD-10-CM, the hospital's
  diagnoses), the code, the visit type, the specialty.
- Decided 2026-11-14 (EPIC.md §5.2): booking sends the reason in the caller's words only. This
  list is the map for when a hospital codes reasons. The Default row is Epic's own sample:
  SNOMED 46866001, a broken leg.
- **Still kept (2026-11-15).** The owner asked whether this section still matters. No call uses
  it, and none will while booking sends the caller's words. It stays, marked "not used", by his
  1B ("1B will let us know what we have left to build"): Epic says a hospital's Cadence rules
  may require `indications`, and then this list is the map.

  ![The owner's screenshot, 2026-11-15: Reasons as codes, the Diagnosis list not set and one SNOMED row; he asks whether the section still matters](images/hospital-reasons-codes-2026-11-15.png)

### 2.4 The time zone — Times tab, used now
- One IANA name for the hospital. Uses: the call's today and time (`OpenCallReply.local_now`),
  `$find`'s `startTime` and `endTime`, and each visit's local time. `$find` also sends each
  department's own zone with a slot, and that wins for the slot (EPIC.md §7.2).

### 2.5 Times of day — Times tab, used now
- Columns: the name ("morning"), the start (HH:MM), the length in hours.
- Uses: `find_slots`'s `time_of_day`, which may hold only these names (the model's list comes
  from the call's own version, `OpenCallReply.times_of_day`); Epic's `$find` gets
  `Timing.repeat.timeOfDay`, `duration` and `durationUnit`. The days of the week need no list.

### 2.6 Hours and closed days — Times tab, used now
- Hours: day of the week, opens, closes. Closed days: the date and a name.
- Used since 2026-11-16 by "where to go" (`place_info`, BOOKING-RULES.md §5.4): a place with no
  hours of its own says the hospital's; the next three closed days go to the model, said only
  when the caller asks about that day.
- For later: what the agent says when no person can take a hand-off now (FUTURE-SETTINGS.md
  §10), and when a nurse may join a call on a staff flag (FLAGS.md §5). `$find` returns open
  slots only, so the hours never filter slots.

### 2.7 Locations (Epic's `location-reference`) — Places tab, used now in part
- Since 2026-11-15 two lists: facilities (EAF) and their departments (DEP), shown as a card per
  facility or one table; filled by Import from Epic (§8).
- Department columns: the record's name, the spoken name, the DEP ID, the specialty, the FHIR
  id, the address, the phone, the hours.
- Used now: a visit at a department on the list says the spoken name, found by the Location's
  FHIR id first, then by name; the first department is said when a record names no place
  (`Hospital::place`). Since 2026-11-16 (BOOKING-RULES.md §5.2 to §5.5): the FHIR id in `$find`
  (`location-reference`), the specialty (the search by kind of care and the Booking rules), the
  spoken and record names, the facility's names and the city (a place the caller names), the
  address, the phone and the hours ("where to go"). Not used yet: the DEP ID and the facility's
  EAF ID.

### 2.8 Specialties (Epic's `specialties`) — Specialties tab, used now
- **Its own tab since 2026-11-15** (the owner, TABLES.md §6.3, W1 A): before, it sat under
  Locations on Places, which after an import put it about 400 rows down. A specialty is a kind
  of care, not a place: each department names one by its ID, and many share it.
- Above the table: the **Specialty list**, one OID (§5.1). Columns: the spoken name, the
  caller's words, the ID, and **Primary care** (since 2026-11-16: read by calls for the usual
  place, BOOKING-RULES.md §5.2). Since 2026-11-16 the spoken name and the caller's words are
  matched to a caller's words for a kind of care, and the ID goes to `$find` as `specialties`
  (§5.3). A specialty with no ID matches no department. Epic gives no list of a hospital's specialty codes by API.
- **What the sandbox can give (2026-11-15).** A department's specialty, one at a time:
  `Location.Read` (registered) returns it as an extension, with its code and system (EPIC.md
  §7.11). The departments come from the Location references on the test patients' visits and on
  `$find`'s proposed slots. So the sandbox can fill this list, the Specialty list's OID and the
  Locations' FHIR ids, address and phone for the sandbox's own departments. The whole list of a
  hospital's departments and specialties comes from the hospital (`Location.Search` exists in
  Epic's catalog, `api:10084`).
- **Tried the same day** (EPIC.md §2.2, after the owner registered `Location.Search`): one
  `Location.Search` by address returns every location of the sandbox's organization, 435, in one
  page; 325 carry a specialty. The sandbox's Specialty list OID is
  `urn:oid:1.2.840.114350.1.13.0.1.7.10.686980.110` (9 Family Medicine, 4 Cardiology, 40
  Radiology). So an import from Epic is possible: one search fills Locations (DEP departments,
  each under its EAF facility) and Specialties. Not built.

  ![The owner's screenshot, 2026-11-15: Specialties, the list's OID not set and three rows with no ID; he asks whether it is usable before a hospital](images/hospital-specialties-2026-11-15.png)

### 2.9 Booking — Booking tab, used now
- Slots offered at a time (1 to 5); visits before the narrowing question; groups one question
  names; days back for a past visit; years for an established patient (1 to 10, 3 by default;
  BOOKING-RULES.md §3.4); offer a sooner time with another clinician (on; BOOKING-RULES.md
  §5.2); the search windows for the visits to come (each longer
  than the one before; one Epic read per window, then one with no end). FUTURE-SETTINGS.md
  §13.2 and §13.3 list the choices not built yet (the order, the clinician's name, times on one
  day).
- **The label after the reason in the booking note** (`booking_note_label`; the owner's N2,
  2026-11-15: "configurable (can be '(automated transcription from patient)' to start)").
  Epic's `$book` files `appointmentNote` as the caller's words, a space, then the label; the
  label is kept whole and the words are cut to fit Epic's 500 characters
  (`Hospital::booking_note`). Empty: the words alone. Epic mode only: the local fake keeps the
  words, so a read-back never says the label.

### 2.9a Booking rules — Rules tab, used now (2026-11-16)
- The rules engine's start (the owner's E-A; BOOKING-RULES.md §3.5, as built §5.5). Columns:
  Specialty, Department, Visit type, Provider, Patients (Any, New, Established), Outcome (Book;
  Book and flag for a referral check; Book with the PCP only; Referral needed; Hand off). Each
  condition empty is Any; the first row that matches wins; ↑ ↓ ✕ keep the order.
- A rule must name rows that are on the other lists; the page and ehr refuse it by row number.
  The Default has no rules.

### 2.10 What a code and its system mean (Epic's docs, 2026-11-15)
> **Part:** research · 2026-11-15. The whole record, with Epic's document ids: EPIC.md §7.11.

- **The code system names the list; the code names the row in it.** In
  `urn:oid:1.2.840.114350.1.13.0.1.7.2.808267`: `1.2.840.114350` is Epic, `.1.13` is Epic's
  EDI application, `0` is the organization (the sandbox), `1` the environment (Epic's own; a
  hospital's production is 2, its test is 3), `.7` HL7 use, and `.2.808267` the records of the
  PRC master file (P R C in ASCII: 80 82 67), Cadence's visit types.
- **All of one hospital's visit types share one system.** The system changes between the
  hospital's test and production Epic, and between hospitals.
- **A code alone does not say what it is.** 1003 is a visit type here because its system is
  PRC. Departments and facilities also have short record IDs (Epic's sample: department "1",
  system `.2.686980`, DEP).
- **A hospital may name its visit types two ways:** by record ID (`.2.808267`, as the sandbox
  does) or by the value of PRC item 11 (`.3.808267.11`, as Epic's `$find` sample does).
- **Reasons (`indications`):** the hospital's own diagnosis list (EDG, `.2.696871`) and SNOMED
  CT; when both are sent, the hospital's code wins. Epic's sample also sends ICD-10-CM.
- **Specialties:** a category list (`.10.…`), small codes ("3" = Allergy in Epic's sample).
- **Locations:** `$find` takes a reference to a Location, not a code. `Location.Read` gives the
  name, address, phone, hours and specialty.
- **No API lists these.** The hospital's Epic team gets the OIDs from Epic's OID Generator
  utility and the codes from its Cadence build.

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
- **The console:** one page, six tabs of tables (Visits, Times, Places, Specialties, Booking, Rules),
  each row edited in place; a search box on the tab bar's line and long lists windowed
  (TABLES.md §6). Who may edit waits on
  the roles (ACCESS-ROLES.md). On a phone each table scrolls on its own (`.dt-wrap`, TABLES.md).
- **The deploy:** A2 and B read the lists from the settings store.
- **Each EHR:** the columns are Epic's. Another adapter maps its own concepts behind the same
  lists (athenahealth's appointment types, Oracle Health's; NON-EPIC-EHRS.md). A list an EHR does
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
- **The console**: `hospital.ts`, built from the console's one table (`data-table.ts`:
  `table`, `row`, `field`, `titleLine`, `listTools`; TABLES.md, 2026-11-15; before, System's
  parts in `system.ts`); `api.ts` types; the `settings.hospital` store. The page checks each
  row as ehr does (`problem`) and shows the first problem above the tabs; Save waits until
  there is none.
- **The columns' alignment** (the owner, 2026-11-15): the words of an editable cell started
  about 13 px right of their header, on this page and on Config › System. First fixed with a
  negative margin on the text boxes; that left the selects out, so since TABLES.md the header
  starts at the box's inner padding instead (TABLES.md §3.3).

  ![The owner's screenshot, 2026-11-15: the Visits tab, each editable column's words right of its header (an orange line marks Spoken name); he asks what Code system and Code are](images/hospital-columns-misaligned-2026-11-15.png)

### 5.1 One Epic list per list (the owner's A, 2026-11-15)
> **Part:** built · 2026-11-15. The owner: "Let's go with A and I'll iterate for usability
> afterwards. Code system wouldn't make sense to an analyst in the scope of their work, so
> translating to master files, id types, and records works best. and no inventing or guessing,
> we respect Epic's decision and listen to their guidance here."

- **ehr › `hospital.rs`:** three new keys, each one OID or empty: `visit_type_list`,
  `diagnosis_list`, `specialty_list` (`check_key` takes `urn:oid:` and dotted numbers). The
  visit type and specialty rows lose `system`; `code` is the row's ID in its list. A reason's
  `system` is a name from `REASON_SYSTEMS`: `snomed`, `icd10cm` or `hospital`. `from_pairs`
  refuses a row with an ID when its list is empty, and a "hospital" reason when the diagnosis
  list is empty; the error names the row.
- **Versions saved before A** are read in today's shape (`hospital::upgrade`, called by
  `settings::get` for the hospital): rows that all name one OID give it to the list, when the
  version has no list key; a reason's SNOMED or ICD-10-CM OID becomes its name; another OID
  becomes "hospital" and the diagnosis list. Rows that name two OIDs stay as they are, so the
  version does not fit and the call uses the file (as for any version that does not fit).
- **`$find`** sends `visit_type_list` as the system of every visit type (`epic_ehr.rs`). The
  request to Epic is unchanged.
- **The console:** each list has a one-row table above its rows: the OID (edited in place) and
  "What the OID says". A row's ID shows its list's prefix in gray, "R PRC 1003"; the prefix
  changes as the OID is typed. A reason's code system is a dropdown; a reason on the hospital's
  list shows the diagnosis list's prefix. `console/src/pages/epic-oid.ts` reads an OID.
- **What the page reads from an OID, and only this** (Epic says to treat an OID as opaque,
  open.epic tutorial:2659): Epic's documented pattern `1.2.840.114350.1.13.<CID>.<EnvType>.7.…`,
  with EnvType in Epic's words (1 Epic's own environment, 2 production, 3 non-production), and
  two forms that Epic's own examples name: `.7.2.<file>`, a master file's records (Epic's AIP
  example: the letters as ASCII codes), shown as "R PRC"; and `.7.5.737384.<n>`, an ID type
  (Epic's MRN example), shown as "R IIT n". Any other OID says "Not a form Epic's documentation
  names: kept as entered", and its rows show no prefix. The page never builds an OID.
- **The Default:** `visit_type_list` is the sandbox's PRC OID; `diagnosis_list` and
  `specialty_list` are empty; the reason row is `snomed`.

**Decided inside the owner's choices (for his review):**
- A: the list's OID sits in a one-row table above its rows, titled "Epic's OID for the list"
  and "What the OID says". An OID that is not in Epic's documented forms gets no label and no
  prefix, never a guess. A specialty list's OID (a category list, `.10.…`, in Epic's sample)
  gets no label for that reason.
- A reason's systems are named SNOMED CT, ICD-10-CM and "The hospital's diagnoses"; "None" is
  allowed for a row with no code.
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
1. `npm run dev`. Open Config › Hospital. Read shows five tabs; Visits shows five visit types.
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
8. **One list per list (§5.1).** Restart `npm run dev` first (a new `ehr`). On Visits, the
   Visit type list shows the OID and "Records of the master file PRC (organization 0, Epic's
   own environment)". New Patient Appointment's ID reads "R PRC 1003"; Office Visit's ID is
   empty and has no prefix.
9. Choose Edit. Clear the Visit type list's OID. The problem "Visit types, row 3: an ID needs
   the visit type list's OID" shows and Save waits. Type the OID back: the prefixes come back.
10. On a reason row, choose "The hospital's diagnoses": the problem asks for the Diagnosis
    list's OID. Choose SNOMED CT again.
11. Choose an older version (saved before today): it shows in the new shape, each ID in its
    list.
12. In Epic mode, ask to book a visit: `$find` still returns times (the same request as before).
13. **Places (§8).** Restart `npm run dev` (a new `ehr`). Open Places. Cards shows one card, "Not
    in a facility", with five departments. Press it: the cards fade out, then "← All facilities",
    the title and the departments' rows rise in, one after another. Press "← All facilities":
    the cards come back the same way.
14. Choose Table: the same departments in one table under a "Not in a facility" row. Choose
    Cards again. Reload the page: it opens on the view you chose last.
15. In Read, "Import from Epic" on the Locations title line is hidden, as Add is. Choose Edit
    and press it: the panel
    drops with the instruction. Type `wis`: Wisconsin (WI) is suggested; press Enter: a "WI" chip.
    Type `Springfield, ZZ` and press Enter: the line says what to type, and nothing is added.
    Press Import. The panel closes, and the line under the Locations heading counts
    Epic's facilities and departments (the sandbox: about 34 and 400), what is new and changed,
    the other locations left out, and 1 read. The cards fill: one per facility, with counts and
    specialties. "EMC Family Medicine" keeps "the family medicine clinic" and is marked
    Changed; "Main Street Clinic" is marked Not in Epic.
16. Open a facility. Change a department's spoken name. Press ✕ on another facility's row:
    it and its departments are gone. Save. The status says the new version is active.
17. Choose Table: every department, a heading row per facility. Narrow the window to a phone's
    width: the cards stack in one column; the table scrolls on its own.
18. In Epic mode, ask Camila Lopez's next visit: the place is the department's spoken name.
19. With reduced motion on (Windows: Settings › Accessibility › Visual effects › Animation
    effects off), a drill shows the next view at once.
20. Press "Import from Epic" again: the cities from the imported addresses are suggested with
    the box empty ("Verona, WI", …). Choose one and press Import: one more read, the same
    facilities found as "changed" or not at all. Press Cards | Table: the pill slides first,
    then the view changes.

## 7. Open
- **Decided 2026-11-15: A, one Epic list per list** (built, §5.1). Not chosen: B, an OID on
  each row. The owner will iterate on the page's usability after his desk test. The research
  behind it: §2.10, EPIC.md §7.11.

  ![The owner's screenshot, 2026-11-15: the Visit types table, the Code 1003 boxed; he asks for A in detail and to show it as "R PRC 1003"](images/hospital-visit-type-code-2026-11-15.png)
- A label for an OID form that Epic's documentation does not name (an item, `.3.<file>.<item>`,
  as Epic's `$find` sample sends; a category list, `.10.…`): only from Epic's own words, when a
  hospital needs it.
- Import from a file (the hospital's Cadence export), as well as typing. Places can come from
  Epic itself: §8.
- Whether a location may override the hospital's hours and time zone, or only add to them.
- Each list that is not used yet waits for its feature: the specialty and location in `$find`,
  coded reasons, hours with the hand-off. **Designed 2026-11-16:** the place and the specialty
  in the search, the default place and the PCP, new or established on Visit types, the Booking
  rules, and the address, phone and hours by a new tool: [BOOKING-RULES.md](BOOKING-RULES.md).
- Many hospitals and the routing rules (ideas/README.md).

## 8. Import Places from Epic (built)
> **Part:** built · 2026-11-15 (as built: §8.3). Designed the same day. The owner: "Since an import is possible, let's doc the route
> first, can help with fast onboarding for a clinic / hospital." And: "EAF / DEP split is perfect,
> helps distinguish higher level locations with sublocations/departments and EAF / DEP are very
> familiar terms for hospital folks." The facts behind it: EPIC.md §2.2 (the sandbox try).

**Terms.** An **EAF** is a facility record ("EHS Hospital", "EHS Clinic"): a higher-level
location. A **DEP** is a department record ("EMC Cardiology"): the place a visit is booked in,
`partOf` one facility. Both are master files; the page names them "R EAF n" and "R DEP n"
(epic-oid.ts).

### 8.1 The route, for a new hospital
1. **Before:** the hospital adds our client to its Epic (`Location.Read` and `Location.Search
   (Organizational Directory) (R4)` are in our registration, EPIC.md §2.1) and names its time
   zone. Nothing else is needed for Places: the search's reply carries its own systems.
2. **One search:** `Location?address=<the hospital's state>`. On the sandbox this returned every
   location of the organization (435) in one page. A hospital in several states: one search per
   state. A `next` link is followed when a page is cut. Cost: 1 to a few reads per import, never
   per call. No protected data: a location is a fact about the hospital.
3. **Split by kind:** an EAF has an identifier in `….7.2.<EAF>` and no `partOf`; a DEP has an
   identifier in `….7.2.<DEP>` and `partOf` an EAF. Anything else (rooms, billing offices) is
   shown apart, not imported unless chosen.
4. **Places › Locations:** one row per DEP, grouped under its EAF: the record's name, the FHIR id,
   the DEP ID, the address, the phone, the hours (`hoursOfOperation`). The spoken name stays empty
   for the analyst to write; empty means the record's words.
5. **Places › Specialties:** the distinct specialties across the DEPs. Their one system becomes
   the Specialty list's OID; each code becomes a row's ID; Epic's display becomes a draft spoken
   name. The analyst adds the caller's words ("skin doctor").
6. **A draft, never live:** the import fills the page in Edit, with each new or changed row
   marked. Save makes the version, as for any edit. A later import of the same hospital shows
   what is new, gone or changed since the active version (a refresh).
7. **Not from this search:** visit types (PRC) and reasons (EDG). They come from the hospital's
   Cadence team, or, for visit types, from the visits and `$find` replies seen.

### 8.2 The page (decided 2026-11-15)
> **Part:** designed · 2026-11-15. The owner: "the import creating a list and before deploying
> into production, analysts would remove what's unnecessary. So that said, we can import all for
> now and we'll have to build better filters and UI to handle filtering things out which we can
> deal with when we get there." Then, of the page: "i don't think we can handle the two tiers.
> Would it make sense to maybe make cards of every location / facilities like we do for teams
> to better organize? (and a table view as well to get more info laid out, albeit cluttered)".
> His forks: **1A**, one card per facility; the Table view groups the DEPs under their EAF;
> **2A**, a card opens by drilling in, "ensure to have a gentle animation in line with the rest
> of DeetsHealth"; **3A**, two lists.

- **Import all.** Every EAF and DEP the search returns comes in. The analysts remove what the
  agent does not need before the hospital goes live. Better filters come later.
- **Two lists (3A).** `facilities`: one row per EAF (the record's name, the spoken name, the FHIR
  id, the EAF ID, the address, the phone). `locations` keeps its name and holds the departments;
  each gains `facility` (its EAF's FHIR id; empty: no facility), `code` (the DEP ID),
  `specialty` (its ID in the specialty list) and `hours` (the days and times, from
  `hoursOfOperation`; not used by calls yet). A department's facility must be on the list. A
  version saved before this reads as it was: its rows are departments with no facility.
- **Places › Locations, Cards | Table.** A toggle above the list; Cards is the default, and the
  page remembers the last choice in this browser.
  - **Cards (1A):** one card per facility, in rows that wrap, built from the People card
    (`people.css`: the same edge, corners and band; no picture). Each card: the name, "R EAF n",
    the address, the count of departments, the specialties. Departments with no facility share
    one last card, "Not in a facility".
  - **Drill in (2A):** a press on a card replaces the cards with that facility: "← All
    facilities", its own fields (the record's name, the spoken name, the address, the phone,
    ✕), then its departments' table, edited in place as every table on the page. The ✕ on
    the facility's row takes out the facility and its departments (TABLES.md §3.11).
  - **The motion:** the console's own, no new value. The cards fade out as a page does when the
    nav changes (`--dur-leave`); then the facility's header and rows rise and fade in as one wave,
    top to bottom, as Chat's controls do after a mode change (`--dur-wave-in`, `--wave-step`,
    `--wave-rise`, `--ease-wave`). Back runs the same way. Reduced motion: no motion.
  - **Table:** every department in one table, a facility heading row before each group, so the
    columns line up across facilities. All the columns at once: the cluttered view.
  - **The toggle slides** (the owner, 2026-11-15: "can we have the selection slide please?"):
    the pill moves to the new choice at once (`--dur-pill`), the stage fades out under it, and
    the redraw comes when the pill has landed. The "R DEP" prefix shows only beside an ID.

    ![The owner's screenshot, 2026-11-15: the Cards | Table toggle boxed and one "Not in a facility" card; he asks for the selection to slide](images/hospital-places-toggle-2026-11-15.png)
- **An Import button beside the tabs** (the owner, 2026-11-15): "Let's implement the import
  button in the orange box ... Clicking it can just import the EAFs/DEPs and their info as sent
  by Epic, later we can have the import button wire into more places." Epic refuses a search
  with no input (error 4110), so the button needs what to search by. His **B**: "can dropdown a
  small panel with a cities/states box and explicit instruction to enter city, state or just
  state. Can we wire in any guardrails / auto-suggest user cities/states?" Not chosen: A, a
  saved setting.

  ![The owner's screenshot, 2026-11-15: Places in Table view, an orange box right of the Visits, Times, Places, Booking tabs where the Import button goes](images/hospital-places-import-button-2026-11-15.png)
- **Import from Epic** (Edit only): "Import from Epic" on the Locations title line, left of Add,
  in the same tab bar frame (since 2026-11-15, TABLES.md §2; first just right of the tabs). It
  drops a panel:
  - The instruction: a city and its state ("Verona, WI"), or just a state ("WI" or
    "Wisconsin"); Enter adds each, up to 5. Since the owner's note the same evening ("neaten
    this box please, use our standard sized text box, styling, rounding, and move the help text
    to an info button to the left of the textbox. Make the placeholder text City, State ... for
    cancel/import, use the save/discard style of buttons"): the words sit behind an "i" left of
    the Config tables' own text box (placeholder "City, State"), and Cancel | Import is Discard
    | Save's bar, Import in Save's colors and grey while there is nothing to search. Then
    ("use the blue lines here to make the margins consistent throughout the panel"): one margin,
    `--canvas-pad` (the Config frame's edge to its tab bar), inside the panel and between its
    rows.

    ![The owner's screenshot, 2026-11-15: the Import panel open under the tabs, blue lines at the Config frame's corner marking the margin the panel should use](images/hospital-import-panel-margins-2026-11-15.png)
  - **When Epic's sandbox is down** (2026-11-15 evening: its `smart-configuration` answered HTTP
    503 while its website answered): the panel says "Epic's directory did not answer". ehr
    asks Epic again at each press, and an ehr that started while Epic was down signs in again
    every 30 s (`EpicSandbox::start`, `SANDBOX_RETRY`); press Import again later.
  - **Guardrails:** each entry is checked as it is added and turned into the one form ehr takes:
    a USPS state code (a name becomes its code), or "City, ST" (a city of letters, spaces, `.`,
    `'` and `-`; "verona wi" becomes "Verona, WI"). Anything else gets a line that says what to
    type; a repeat and a sixth entry are refused. ehr checks the same rules again
    (`hospital.rs › PlaceSearch`) and refuses anything else.
  - **Suggestions:** as the analyst types, up to 6 rows: the cities already in the page's
    addresses (after a first import, the next is a pick), the states whose code or name starts
    with what is typed, and the typed entry itself once it reads as a search. Arrow keys move,
    Enter adds, Escape closes, Backspace in an empty box takes the last entry off.
  - **The search Epic gets:** a state is `Location?address=WI`; a city and its state is
    `Location?address=Verona&address-state=WI` (Epic searches by `address` and filters by
    `address-state` after, EPIC.md §2.2).
  - The panel shows "Importing from Epic", and a failure in Epic's or ehr's own words. On
    success it closes, and the line under the Locations heading counts what came in. The
    reply fills the draft:
  - A facility or department already on the list is matched by its FHIR id, then by the
    record's name. Epic's facts replace ours (the record's name, the IDs, the facility, the
    address, the phone, the hours, the specialty); the spoken name stays.
  - New rows and changed rows are marked until Save or Discard. A row on the list that Epic did
    not return stays, marked "not in Epic". The import never removes a row.
  - Specialties: the departments' specialty system becomes the Specialty list's OID when it is
    empty; each specialty code not on the list becomes a row, its spoken name Epic's display in
    lower case. A different OID already set: the import stops and says so.
  - Nothing is saved until Save, as for any edit.
- **Sizes.** A list of facilities or departments holds up to 2,000 rows (the other lists keep
  200). The wire's message limit grows from 64 KB to 4 MiB, gRPC's own default, so a whole
  hospital fits in one GetSettings.
- **A visit's place** is matched by the Location's FHIR id first (exact once the import has
  run), then by the record's name.
- **The Default today** (2026-11-15, from the probe's printout, no new search): the Specialty
  list's OID and IDs 9 (family medicine), 4 (cardiology), 40 (radiology, a new row); the
  sandbox's four departments on Camila Lopez's visits by name, with no facility. An import fills
  the rest.

### 8.3 As built (2026-11-15)
> **Part:** built · 2026-11-15. Where this part and §8.1–§8.2 disagree, this part is the code.

- **ehr › `hospital.rs`:** `FacilityRow`; `LocationRow` gains `facility`, `code`, `specialty`,
  `hours`; the key `facilities` (in `KEYS` and `settings.rs › HOSPITAL`); `PLACES_MAX` (2,000)
  for both lists; `twice_place` (the same FHIR id, else the same name with no id, is one place
  twice); `from_pairs` refuses a department whose facility is not on the list, and a specialty ID
  with no Specialty list. `place(id, record)` matches the FHIR id first. `Places`: what an
  import offers.
- **ehr › `epic_ehr.rs`:** `EpicEhr::places` (the searches and their pages, at most 50) and
  `places_from` (the split: a DEP identifier `….7.2.686980` is a department, an EAF identifier
  `….7.2.696570` a facility, anything else counted; the address, the first phone, the hours, the
  specialty; a facility the search did not return is cleared from its departments). A visit's
  place and a `$find` slot's place pass the Location's FHIR id (`actor_id`).
- **The wire:** `rpc ImportPlaces` (EHR-SERVICE.md §3); the message limit is 4 MiB.
- **The agent:** `ehr_client.py › import_places` (10 s: ehr's own cap on any request);
  `POST /agent/hospital/import-places` (DEV-API.md); `config.py`: `facilities` in
  `HOSPITAL_KEYS`, `places` in `HOSPITAL_LIMITS`.
- **The console (`hospital.ts`):** `places()`: the import block (Edit only), the Cards | Table
  toggle (`deets.console.hospital.places`), the stage. `cards()`, `facility()` (the drill-in),
  `placesTable()` (one table, a group row per facility: `data-table.ts › groupRow`). `mergePlaces` merges an import
  into the draft and returns the status line (counts only). The page's state across redraws is
  `PlacesUi`: the view, the facility open, the import's marks (new, changed, not in Epic; kept
  until Save, Discard or a version pick), the motion. `config.css › Places › Locations`: the
  card on People's rules, the chips, the marks, the motion. `withRowDefaults` fills the new
  fields of rows saved before them, as ehr's serde defaults do (found at the first desk test:
  without it the page's checks threw and the page stayed on "Loading").
- **Limits found while building:** ehr caps every request at 10 s (`main.rs › REQUEST_LIMIT`),
  so an import of several large searches may not finish; the sandbox's one search does. A
  hospital with thousands of departments may need the import to run as a job: when one does.
- **The Import button and panel (his B, the same evening):** `hospital.ts › importControl`
  (appended to the tab bar's row on Places); `console/src/pages/us-states.ts` (the states and
  territories by USPS code, `placeSearch`, `citiesIn`); `hospital.rs › US_STATES` and
  `PlaceSearch` (the same codes, the same city rule); `EpicEhr::places` takes `PlaceSearch`es.
  `config.css › Import from Epic`: the panel on the console's `.menu`, its rows on `.word-row`.

**Decided inside the owner's choices (for his review):**
- The button reads "Import from Epic". In Read it is grey, with the hint "Edit only".
- Suggestions: the cities in the page's addresses first (shown with the box empty), then the
  states, then the typed entry; at most 6. A city's words are capitalized ("verona" →
  "Verona").
- A city with no state is refused: Epic's sandbox matched "Verona" alone to every location, so
  a city needs its state to narrow the search.
- The searches typed stay in the panel until the page is left, so a second import is one press.
- The card shows the spoken name, else the record's name; "R EAF n"; the address (a dim
  "Address" when there is none); the count; at most 3 specialties, then "n more".
- Departments with no facility, or whose facility is not on the list, share one last card,
  "Not in a facility".
- The facility's band is a quarter of the People band (no picture); a facility an import added
  or changed, or one an older version changed, has the accent edge.
- A department's specialty is a dropdown of the Specialties list (by ID); its hours are read
  only, written as "Mon–Fri 08:00–17:00".
- Up moves a department above the one before it in the same facility.
- A facility added by hand needs its FHIR id typed before departments can be added to it.
- The wave's last step is the 13th part; the rows after it come in with it.
- An import whose Specialty list OID differs from the page's stops and says so.
- A specialty an import adds takes Epic's words in lower case as its spoken name.

### 8.4 Open
- Who may run an import: waits on the roles (ACCESS-ROLES.md).
- Filters for a list of hundreds (by specialty, by facility, by city), and a way to remove many
  rows at once: when the owner designs them.
- Whether a department's hours override the hospital's hours (§2.6): with the hand-off.
- Another EHR's import: behind its own adapter (NON-EPIC-EHRS.md).
