---
status: built
desk_test: open
sources: [core/src/tree/mod.rs, core/src/tree/load.rs, core/src/tree/normalize.rs, core/src/tree/schema.rs, core/src/tree/date.rs, src-tauri/src/wipe_alloc.rs, core/src/index.rs, core/src/store/mod.rs, ../DeetsEngine/docs/ideas/deetshealth-needs.md, ../DeetsEngine/docs/architecture/IENS-AND-INDEXES.md]
updated: 2026-10-13
---
# DeetsHealth — The globals (the record view)

> **Built 2026-10-13** (core `tree/`), with its three forks decided (G1 A, G2 A, M1 A, §7).
> **Read §10, "As built", first:** where it differs from the design in §5, it is the code. No
> card reads the tree yet; the inspector builds it (`npm run inspect -- --tree`). The desk
> test is §10.7.
>
> How it came about: the first Epic fetch (Camila Lopez, the Epic sandbox) showed the shape
> of the record (§2, §3). The owner added the results Camila does not have: panels of 8 to 15
> values and microbiology with more (§4). DeetsEngine built the base layer (§6). Still open:
> the data to see in §8 (more sandbox patients).

## 1. Terms
- **Global** — one DeetsEngine tree, such as `^RES`. Here, a derived view: the core rebuilds
  it from the sealed store at each open (D-A), so it needs no migration.
- **Record** — one numbered entry in a global: `^RES(ien)`. The **IEN** is the engine's
  number for it (DeetsEngine IENS-AND-INDEXES.md §1). The FHIR id is a field, never the key.
- **Index** — a sorted list the engine keeps true on every write:
  `^RES("by-enc", 7, 120) = ""`.
- **Pointer** — a field that holds another record's IEN, plus an index on that field, so a
  read can go both ways. The engine has no declared pointer type; a field and an index are
  enough.
- **Stub** — a record that DeetsHealth knows only from a reference to it (its FHIR id and the
  reference's display text). The site did not send the resource itself.
- **Fan-in** — how many records point at one target.
- **Value** — one measured or reported thing: one heart rate, one sodium, one antibiotic's
  susceptibility.

## 2. The input: Camila's record
`npm run inspect -- --csv` (ops/DEBUGGING.md) wrote the record to
`dev-dumps/epic-sandbox/83e89a80/` (local only, not in git). The counts below are shapes and
numbers, not values. Camila is a public test patient.

| Fact | Count |
|---|---|
| Rows in the store | 297, of 12 FHIR types |
| Observations | 254 (86%) |
| Of those, vital signs | 248: 94 pulse, 91 blood pressure, 53 weight, 7 temperature, 3 height |
| Blood pressure rows with 2 components | 91 |
| Links (references) | 626 |
| Links that point at an encounter | 274 |
| Encounters referenced / stored | 6 / 5 |
| Fan-in of the busiest encounter | 248 (all vitals), and it is **not stored** |
| Records that point at no encounter | 23 (12 goals, 4 conditions, the medicine, the allergy, the care plan, the care team, 2 orders, 1 report, 2 observations) |
| Practitioner references that resolved | 0 of 293 |
| Distinct unresolved targets | 54 |
| Codings on one vitals row | 5 to 7 (Epic local code, flowsheet ID, LOINC, others) |
| Codings on one condition | 3 (SNOMED, ICD-9, ICD-10) |
| Rows with no start date | 9 |
| Fetch warnings | 93, many "suppressed" (Outside Record) |

**What this proves, and what it does not.** It proves the axes: the kinds of record, how they
point at each other, and which reads the hubs need. It does not prove the proportions. It is
one synthetic patient, and 245 of the vitals sit on one visit across three months. Camila has
one lab value and no microbiology. §4 covers what she lacks.

## 3. What the data shows
1. **Results are most of the record.** The large global is `^RES`. Its main read is a
   **trend**: one concept, ordered by date ("all pulse readings"). The first index to design
   is concept + date.
2. **The encounter is the center, but it may be missing.** The busiest encounter (245
   vitals) is not in the store. A pointer must be able to target a stub. The patient must
   never lose a value because its visit did not arrive.
3. **The record has two roots: the visit and the chart.** Goals, the problem list, the
   medicine, the allergy and the care plan point at no visit. They are longitudinal. One tree
   with the encounter at the top does not hold them.
4. **One record has many codes.** The by-code index is multi-valued (the engine's
   `Source::Each`). A trend needs **one concept key** per value, chosen by the normalizer
   (§5.3). Without it, one test splits across code systems.
5. **Orders form a chain, and the chain is a graph.** Camila's echo: order → child order →
   procedure → report → findings, with `hasMember` and `derivedFrom` between findings. The
   placer order number also joins the order, the procedure, the report and the imaging note
   where no FHIR reference does. So: a pointer field for each common role, with a reverse
   index, and one `^LINK` global for the rare roles.
6. **People and places are stubs.** Only display names exist. They are protected data (PII).
7. **"Not shared" is not "none".** "Suppressed" warnings say the site holds data it did not
   send. The patient must see "your health system did not share this", not an empty card.
8. **Some records have no clinical date.** An empty value gets no index entry (IENS-AND-INDEXES
   §4). A record with no date must still sort somewhere (§5.4).

## 4. Results the draft must hold that Camila lacks
> **Part:** project · 2026-10-13 · questions §8

The owner (2026-10-13): "there may be results like lipid panels or CMPs that could have
between 8 and 15 components or microbiology tests with even more, we need to prep to handle
those as well."

### 4.1 The three shapes of a result with many values
| Shape | FHIR form | Examples | Values in one group |
|---|---|---|---|
| **Components** | one `Observation` with `component[]`; one FHIR id for all | blood pressure; some sites send a urinalysis or an EKG's intervals this way | 2 to about 15 |
| **Panel** | a `DiagnosticReport` with `result[]`, each pointing at its own `Observation`; or a parent `Observation` with `hasMember[]` | CMP (14), lipid panel (4 to 8), CBC with differential (about 20), urinalysis (about 12) | 4 to about 25 |
| **Microbiology** | a `DiagnosticReport` → a culture `Observation` → an organism (isolate) `Observation` per organism → a susceptibility `Observation` per antibiotic, through `hasMember` | a urine or blood culture; 2 organisms × 20 antibiotics | 40 and more, 3 or more levels deep |

Counts are typical sizes, not measurements. **How Epic sends a culture is not yet seen**
(§8): the isolate and its susceptibilities may come as members, as components, or as text in
the report.

### 4.2 What a panel or a culture adds
- **Order inside the group.** A CMP reads in the lab's order, not by name. `result[]` and
  `hasMember[]` are ordered lists. Each child keeps its place (`seq`).
- **Depth.** A panel is one level. A culture is three or more. The draft must not fix a depth.
- **One status per value.** A culture is `preliminary` for days, then `final`. One value can
  be `corrected` or `cancelled` while the rest of the panel is final. A re-fetch replaces the
  record with the same FHIR id; the rebuild at open makes the global match.
- **Members that did not arrive.** 22 of Camila's `hasMember` targets did not resolve. A
  missing member is a stub in its place in the group, with the reference's display text. The
  group shows a calm "not sent" row, never a gap.
- **Value kinds.** A number with a unit; a number with a comparator (`<0.5`, `>60`, as eGFR
  often is); text ("Positive", an organism name); a coded answer (S, I, R); a range or ratio
  (a titer, 1:80); no value with a reason (`dataAbsentReason`).
- **Reference range and flag per value.** Low, high, the range text, and the flag (H, L, HH,
  LL, A; S, I, R for susceptibility). A range is often absent (FHIR-SHAPES.md §3.1): the card
  never guesses one.
- **Notes per value and per group.** Free text, so SECURITY.md §4.3 applies.
- **A trend across groups.** LDL from five lipid panels over ten years is one trend. So the
  trend index points at the value, not at its panel.

## 5. The draft globals
> **Part:** project · 2026-10-13 · G1 A, G2 A and M1 A decided (§7)

### 5.1 One rule for results: one value, one record
**Every value is a record in `^RES`, with a `parent` pointer and a `seq`.** A panel, a report
and a culture are records too (the group). A child points at its group. This holds for all
three shapes in §4.1:
- **Panel:** each analyte is already its own `Observation` with its own FHIR id, its own
  status and its own updates. It is its own record. The code rules out a subfile here: the
  trend index must point at the analyte.
- **Microbiology:** report → culture → isolate → susceptibility is a chain of `parent`
  pointers. Any depth works.
- **Components:** the same rule (G2 A, §7): each component becomes a child record with no
  FHIR id of its own (its key is the parent's FHIR id and its place).

The read of a whole group: walk `by-parent` from the group, recursively. To count or load a
group at once, walk `by-root`.

```
^RES(40)  = packed: kind=report  concept=loinc:24323-8  enc=7  order=12  root=40  …
^RES(41)  = packed: kind=value   concept=loinc:2951-2   parent=40  root=40  seq=1  value=…  flag=…
^RES(42)  = packed: kind=value   concept=loinc:2823-3   parent=40  root=40  seq=2  …
^RES("by-parent", 40, 1, 41) = ""
^RES("by-parent", 40, 2, 42) = ""
^RES("by-trend", "loinc:2951-2", "2024-03-05T14:00:00Z", 41) = ""
```

(Shapes only. Codes are public LOINC codes; no value is shown.)

### 5.2 The globals
> **Part:** project · 2026-10-13 · the full design; M1 A decided (§7)

Each global holds the store tables in the right column. A store table with no global stays
in the sealed store only (§5.9). Medicines have their own global, `^MED` (M1 A).

| Global | Holds (store table → kinds) | Pointers | Indexes |
|---|---|---|---|
| `^SITE` | one record per site in this tree: its name, its last fetch | — | — |
| `^PAT` | the patient, per site: name, birth date, sex (store `patient`) | site | — |
| `^ENC` | `encounter` → visit, planned (`Appointment`), episode; and stub visits | site, location, provider, parent, episode, diagnoses (list), participants (list) | by-date, by-kind (kind, date), by-diagnosis (Each), by-parent, by-handle (unique) |
| `^RES` | `result` → every result: groups (panel, report, culture, isolate) and values | site, enc, order, parent, root, specimen | by-trend (kind, concept, date), by-enc (enc, date), by-parent (parent, seq), by-root, by-order, by-date, by-code (Each: every coding), by-handle (unique) |
| `^ORD` | `order_` → orders other than medicines | site, enc, parent, requester, reasons (list) | by-enc, by-date, by-parent, by-number (placer order number), by-handle (unique) |
| `^MED` | `medication` and `drug` → ordered, filled, given, reported, vaccine, vaccine-due, drug | site, enc, drug, order, requester, reasons (list) | by-medicine (concept, date), by-kind (kind, status), by-order, by-enc, by-reason (Each), by-handle (unique) |
| `^CHART` | `chart_item` → problem, visit diagnosis, health concern, allergy, procedure, goal, care plan, device, family history | site, enc, asserter, performer | by-kind (kind, status, date), by-enc, by-date, by-handle (unique) |
| `^DOC` | `document` → note, form, message | site, enc, author, order | by-enc, by-date, by-handle (unique) |
| `^PARTY` | `party` → people, places, organizations, care teams; mostly stubs | site | by-handle (unique) |
| `^LINK` | every link role with no pointer field (§5.8) | from, to | by-from (global, IEN, role), by-to (global, IEN, role) |
| `^FETCH` | per site and FHIR type: served, none, not shared (suppressed), refused | — | — (text keys, not records) |

**Fields every record has** (the first fields of each packed format): site, FHIR type, FHIR
id, `meta.versionId` (for later writes, HOME-AND-HUBS.md §5), handle (§5.6), kind, status,
date key, date precision, date mark (§5.4), stub mark, and the reference's display text for
a stub.

**Each global's own fields** (packed, one format per global, DeetsEngine PACKING.md):
- **`^ENC`:** class, type (concept and display), start, end, location, service provider,
  parent, episode, `diagnoses[]` (`^CHART` IENs), `participants[]` (`^PARTY` IENs).
  `by-diagnosis` gives a problem's level its visits (HOME-AND-HUBS.md §4, Problems).
- **`^RES`:** concept, every coding (`codes...`), value kind, comparator, value, unit, UCUM
  code, low, high, range text, flag, enc, order, parent, root, seq, specimen. Notes are a
  subfile node (`^RES(ien,"note",n)`): free text does not go into a packed value.
  `by-trend` serves both the Results hub's list (the concepts of one kind) and a test's trend
  (its values by date), so one index does the work of two.
- **`^ORD`:** concept, intent, priority, ordered date, occurrence date, enc, parent order,
  requester, placer order number, `reasons[]`.
- **`^MED`:** concept, drug (the `Medication` record, for a medicine named by reference),
  dose quantity and unit, route, frequency text, as needed, the sig text, quantity, refills,
  days supply, the order a fill or a dose belongs to, enc, requester, `reasons[]`. Vaccines:
  the CVX concept, dose number, lot. A medicine named by reference takes its concept from its
  drug record at load, so `by-medicine` groups every order of one drug.
- **`^CHART`:** concept, clinical status, verification status, onset, abatement, recorded,
  enc, asserter. Allergies: category, criticality, `reactions[substance,manifestation,severity]`.
  Procedures: performed date, performer. Verification status is in the tree, so a problem that
  is `refuted` or `entered-in-error` can be kept off a hub (INDEX-AND-LINKS.md §6 left it out
  of the store's index).
- **`^DOC`:** type (concept and display), description, date, enc, author, order, the content
  list (content type and the `Binary` reference). The note's text is not loaded; it is fetched
  when the patient opens the note (DATA-MODEL.md §5).
- **`^PARTY`:** display name, specialty or role (concept and display).

### 5.3 The concept key
One key per record, chosen at load, for the indexes that group by concept (`by-trend`,
`by-medicine`):
- **The standard system for the global, when any coding has it:** LOINC in `^RES` and
  `^ORD`; RxNorm in `^MED`, CVX for a vaccine; SNOMED, then ICD-10-CM, in `^CHART`. These
  keys are the same at every site, so LDL from two health systems is one trend.
- **Else the first coding, prefixed by the site number:** `s<site>:<system>|<code>`. Two
  sites' local codes never merge by accident.
- Later, a bundled map of local codes to standard codes (DATA-MODEL.md §8, fork 7) joins more
  of them. Every coding stays in `codes...` and `by-code`.

### 5.4 The date key
- **One sortable key per record:** a full UTC instant, `YYYY-MM-DDTHH:MM:SSZ`. A partial date
  fills its missing parts with the start of its period (`2019` becomes
  `2019-01-01T00:00:00Z`), and the **date precision** field keeps what was sent (year,
  month, day, time), so the card shows `2019`, never "1 January 2019".
- **Why never the year alone:** the engine indexes a canonical number as a number, and
  numbers sort before every string (DeetsEngine `index.rs`, `index_value`). A key of `2019`
  would sort before every full date.
- A date with an offset is turned into UTC. A date with no time (`2019-04-05`) is not shifted.
- **The first of:** the clinical date (`start`), `recorded`, `last_updated`. The **date mark**
  says which, so the card can say "date recorded" and not "date of care".
- **No date at all:** the key `0000-01-01T00:00:00Z`. A hub that lists newest first shows
  these last, and none is lost.

### 5.5 Stubs
A reference whose target did not arrive makes a stub in the target's global, once per site,
FHIR type and FHIR id, with the reference's display text. Every pointer then holds an IEN.
When a later fetch brings the resource, the next rebuild fills the record. A stub visit can
be opened like any visit: Camila's busiest visit (248 vitals) is a stub.

### 5.6 IENs and handles
- **IENs are new at every rebuild.** The core numbers each global's records from 1 at load.
  An IEN never leaves the core and is never kept on disk.
- **The handle is what the front end holds:** a keyed hash of (site, FHIR type, FHIR id),
  made with the site's own key. It is the same across rebuilds and fetches, for a stub too,
  so a card can keep its place (card memory) after a fetch or a restart. It carries no
  identifier: without the site key it cannot be turned back into a FHIR id, and erasing the
  site's key at sign-out makes every handle meaningless.
- **`by-handle` (unique) replaces the draft's `by-fhir`.** A handle reads a record back; a
  clash report shows an opaque value, never an identifier. The FHIR id stays a field.

### 5.7 The load (a rebuild)
When: at open, and after each fetch. Off the UI thread (CLAUDE.md › Conventions).
1. **Number the records.** Read every store row's key in each table that has a global, and
   every waiting link's target. Give each row and each stub its IEN. The map from store row
   key to IEN lives only for the load, and is wiped after it.
2. **Write the records,** one global at a time, in batches of a few hundred
   (DeetsEngine request doc §8, "Batch size"): unseal the raw resource, normalize it, fill
   each pointer field from the store's links (`to` row key → IEN, or the stub's IEN), and set
   the packed record. Each record is written once, so index upkeep runs once per record.
3. **Swap.** The new tree replaces the old one in one step. A card never reads a half-built
   tree.

Cost, from the engine's `^RES`-shaped load (§6): about 0.7 s natively for 10 000 records,
before the engine's unpack-once change.

### 5.8 Pointers and `^LINK`
- **A pointer field** for each role a hub reads: a result's visit (`collected`) and order
  (`fulfils`), a group's members (`contains` → `parent`), an order's visit (`ordered`), a
  medicine's drug, order and reasons, a visit's diagnoses and people.
- **`^LINK`** holds every other role (`derived-from`, `part-of` outside results, `author`,
  `provenance-of`, …), with both ends as (global, IEN) and the source field. No hub reads it
  yet. It keeps the rule "every reference the site sent has a place" (DATA-MODEL.md §2).
- **A visit's whole view:** walk `by-enc` in `^RES`, `^ORD`, `^MED`, `^CHART` and `^DOC`.
  The results ordered at the visit but collected at another come through its orders
  (`^ORD` `by-enc`, then `^RES` `by-order`). Nothing is copied from one record to another.

### 5.9 What stays out of the tree
The tree holds only what a hub reads. `billing`, `other` and `definition` stay sealed in the
store, and a note's text stays at the site until it is opened. Reason: protected data that no
card shows is not unsealed into memory (SECURITY.md §1, second priority). A later hub adds
its global; a global is a derived view, so this needs no migration.

## 6. What to tune the engine for
> **Part:** project · 2026-10-13 · the engine's base layer is built

**The engine's base layer, as of 2026-10-13.** The memory store and redacted reports are
built (DeetsEngine `docs/ideas/deetshealth-needs.md` §8, §10). DeetsEngine measured a load
in the shape of §5.2's `^RES` (21 packed fields, 5 codings, 8 indexes with `by-code` as
`Each` and `by-fhir` unique, 500 records per transaction):

| `^RES` records | Native (Ryzen 7 5700X) | Wasm (V8, Node 24) | Heap, native / wasm |
|---|---|---|---|
| 10 000 | 0.72 s | 0.84 s | 16.8 / 11.5 MiB |
| 100 000 | 9.4 s | 12.6 s | 175 / 119 MiB |

Index upkeep was about 87% of the time, because each packed index unpacked the row again.
DeetsEngine is making each write unpack its row once (not a fork), then it measures again.
A bulk-load path is a fork only if the open is still too slow after that.

- **The bulk load.** With "one value, one record", one record is about 8 nodes (the packed
  value and about 7 index entries). A real adult record of about 10 000 values (an estimate)
  adds about 0.7 s to the open today. A long admission adds many flowsheet values to one
  visit.
- **Numeric IENs only.** A record under a text key gets no index entries and no unique
  check, with no error (DeetsEngine IENS-AND-INDEXES.md). §5 already keys every record by
  its IEN, with the FHIR id in a field.
- **Deep walks.** A culture group is a walk of 3 or more levels of `by-parent`. This is
  ordered traversal (`$ORDER`), which the engine has.
- **Unique indexes.** `by-handle` is the only unique index (§5.6). Its values are opaque,
  and any clash is redacted by `Error::redacted()` too, proved with markers (DeetsEngine
  `tests/redact.rs`). The wrapper logs only that form.
- **Nothing new is needed** for multi-value indexes, subfiles or stubs.

## 7. Forks
**Decided (the owner, 2026-10-13): "G1 A and G2 A."** One global per kind, and one value,
one record, components included. The options are kept below as the record.

**G1 — How to split the globals. Decided: A.**
- **A. One global per kind, as §5.2 (chosen).** Each kind has its own packed format and
  only the indexes it needs. It follows Epic's own split of master files, which the owner
  knows. Cost: the visit view walks `by-enc` in four globals (all in memory).
- **B. One global for every record, with a `kind` field.** One `by-enc` walk gives the whole
  visit. Cost: one packed format must hold every kind, and every index exists for every kind.

Every platform: both are the same engine calls on Windows, web, macOS and iPhone.

**G2 — Components inside one Observation (blood pressure, and any site that sends a panel as
components). Decided: A.**
- **A. One value, one record (chosen).** Each component is a child record in `^RES`.
  Ranges, flags, trends and cards work the same for a component and a panel analyte.
  Systolic pressure trends through the same index as LDL. Cost: more records (Camila's 91
  blood pressure rows become 273 records).
- **B. A subfile under the observation.** `^RES(ien,"c",n)`. Fewer records. Cost: a second
  path for ranges, flags and trends, and the trend index must also point into subfiles.

Every platform: both are the same engine calls everywhere.

**M1 — Where medicines go. Decided: A (the owner, 2026-10-13: "M1A").** §5.2 as first drafted put medicines and vaccines in
`^CHART`, and G1 A approved that table. But the store keeps `medication` apart from
`chart_item` (DATA-MODEL.md §6, decisions 4a and 5a), and a medicine's shape (dose, route,
refills, fills and doses given against an order) is not a problem's or an allergy's.
- **A. `^MED`, its own global (chosen).** Orders, fills, doses given, reported
  medicines, vaccines and drug records, with one packed format and the indexes the Medicines
  hub reads (§5.2). It follows G1's own reason: each kind has its own format. Cost: the
  visit view walks `by-enc` in five globals, not four.
- **B. In `^CHART`, as first drafted.** Cost: `^CHART`'s one format must hold dose, route
  and refill fields that problems and allergies never use. An inpatient stay's hundreds of
  doses given sit in the same global as a few allergies.
- **C. Epic's split.** Medicine orders go in `^ORD` with the other orders, as Epic's order
  master file holds both. Fills, doses given and reported medicines go in `^MED`. Cost: the
  Medicines hub reads two globals, and `^ORD`'s format holds both kinds of order.

Every platform: all three are the same engine calls on Windows, web, macOS and iPhone.

## 8. The owner's answers (Epic knowledge) and data still to see
The owner, 2026-10-13:
- **Cultures:** "Unsure how Epic sends a culture through FHIR. Expecting susceptibilities to
  be components." The design holds either way. As components, each susceptibility becomes a
  child record of its isolate (G2 A), with the isolate as `parent`. As `hasMember`, each is
  its own `Observation` and a child the same way (§5.1). Only report text would need more: a
  culture with no structured values shows as its report.
- **LOINC:** "Expecting LOINC will often be on a lab component." So the concept key (§5.3)
  joins most labs across sites without the local-code map. The map stays later work.
- **Size:** "We can expect other patients to be like Camila." The engine's 10 000 and
  100 000 record measurements (§6) cover it.
- **Data to see:** "We can test other patients / pull their data to see into a spreadsheet."
  Sign in as more Epic sandbox patients (Test Data list: EPIC.md §3), fetch each, and run
  `npm run inspect -- --export dev-dumps\epic-sandbox` (ops/DEBUGGING.md) to see each record
  as a workbook. Look first for a patient with a CMP, a lipid panel and a culture: that
  closes §4.1's open row and shows Epic's real culture shape. Cost: one sign-in and one fetch
  per patient (Camila's fetch took 32 calls).

## 10. As built (2026-10-13)
> **Part:** built · 2026-10-13 · desk test open (§10.7)

Where this section and §5 disagree, this section is the code.

### 10.1 Where it lives
| File | What |
|---|---|
| `core/src/tree/mod.rs` | `Tree`: `build`, the typed reads, `TreePass`, `Global`, `TreeSource` |
| `core/src/tree/schema.rs` | every packed format and `File` declaration (§5.2) |
| `core/src/tree/load.rs` | the load: number, plan, finish, write (§5.7) |
| `core/src/tree/normalize.rs` | each global's own fields from the raw resource, the concept key (§5.3), values |
| `core/src/tree/date.rs` | the date key (§5.4) |
| `core/src/tree/tests.rs` | a small record shaped like Camila's (§10.5) |
| `src-tauri/src/wipe_alloc.rs` | the zeroing allocator |
| `core/src/dev.rs` `tree_report`, `export_tree` | the inspector's view (ops/DEBUGGING.md §5) |

The engine comes in by path (`almirah-engine`, `../../DeetsEngine`, no default features). The
core still builds for `wasm32-unknown-unknown`.

### 10.2 The wall
- **`TreePass`** is the fourth pass (`shielded.rs`; R-A lite's "new pass"). Only `tree/` makes
  one. The load reveals each value with it to write it; every read wraps the value again.
- **An IEN comes back plain:** it is the engine's number, not record data.
- **An engine error** becomes `CoreError::Tree(code)`, the code of its redacted report. The
  rest of the error is dropped.
- **The zeroing allocator** wipes every freed block in the app, the inspector and the tests:
  serde's buffers, the engine's short-lived copies, everything no wrapper owns. A `realloc`
  moves the block by hand, so the old block is wiped too. Measured (release, Ryzen 7 5700X):
  20 000 parses of a lab result, 147 ms with the wipe and 147 ms without it. No test proves
  the wipe itself; the engine's own scan test covers its store.

### 10.3 Differences from §5
1. **`by-row` in place of `by-handle` (§5.6).** The unique index is (site, store row key). A
   row key is already random, opaque and kept across fetches (INDEX-AND-LINKS.md §7), so it
   is a stable handle for every stored record with no new key. A stub and a component have
   no row key, so no entry. **Their handle is chosen with the first card that opens one** (a
   stub visit, a blood pressure's systolic): a keyed hash (§5.6), or a store row for each
   stub. Not built: no card needs it yet.
2. **`by-kind` is (kind, date) in `^MED` and `^CHART`,** not (kind, status). An index entry
   needs every field, and many records have no status, so they would drop out of the hub.
   A card filters by status as it reads (Active | Past, Active | Resolved).
3. **`^LINK` keeps only links whose target is in the tree.** A link to billing or a
   definition stays in the store's link table (§5.9).
4. **A number keeps the digits the site wrote** (`5.10`, not `5.1`): the core's JSON parser
   now keeps numbers as text (`serde_json` `arbitrary_precision`). This affects every parse
   in the core; every core and host test passes with it.
5. **A record's `version`** is `meta.versionId`, for later writes (HOME-AND-HUBS.md §5).
6. **A component** copies its parent's head (site, FHIR type and id, kind, status, date) and
   visit, and has an empty `row`.
7. **A result's group:** when a result is a member of two groups (a panel `Observation` and
   its report), the `Observation` is its parent, since the report holds that `Observation`.
8. **`^FETCH(site, type)`** holds the rows stored and each warning's code. The words "not
   shared" and "refused" are made from these codes by the card, later.
9. **Not wired into the app yet.** No card reads the tree, so the app does not build it at
   open or after a fetch: that would unseal the whole record into memory for nothing (§5.9).
   The first card's work adds the build (off the UI thread) and its rebuild after a fetch.

### 10.4 Reading it
`Tree` (all values come back shielded):
- `field(global, ien, name)`, `pointer(global, ien, name)`, `list(global, ien, name)`,
  `notes(ien)`;
- `walk(global, index, prefix, newest_first)`: the IENs under a prefix, in index order. For
  example `walk(Res, "by-trend", [kind, concept], true)` is one test's values, newest first;
- `values(global, index, prefix)`: the distinct values at the next level. For example
  `values(Res, "by-trend", [kind])` is every test of one kind;
- `by_row(global, site, row)`, `site_field(Patient | Site, site, name)`, `fetch(site, type)`;
- `counts()`, `index_size`, `records(global)` (the inspector).

### 10.5 Tests
`cargo test -p deetshealth-core --features sqlite tree` (18 tests): the date key (UTC across
a day, a month, a year and a leap day; partial dates; never a number); the concept key
(LOINC second still wins; a local code per site); values (trailing zeros, comparators,
ranges, ratios, coded answers, absent values); and a record shaped like Camila's: a stub visit
with 9 values on it, blood pressure components in order and trending by their own code, a
panel in the lab's order with a member that did not arrive, the visit's diagnoses and people,
a medicine's concept from its drug, two sites that never share a record or a local code.

### 10.6 Camila's record through the tree (Claude, 2026-10-13, through WMI)
`npm run inspect -- --tree` on the Epic sandbox site. Built in **313 ms** (a debug build).

| Global | Records | Stubs | Index entries |
|---|---|---|---|
| `^ENC` | 7 | 2 | by-date 7, by-kind 7, by-diagnosis 0, by-parent 0, by-row 5 |
| `^RES` | 467 (182 components) | 27 | by-trend 439, by-enc 437, by-parent 211, by-root 467, by-order 3, by-date 467, by-code 1679, by-row 258 |
| `^ORD` | 5 | 1 | by-enc 2, by-date 5, by-parent 1, by-number 4, by-row 4 |
| `^MED` | 3 | 1 | by-medicine 1, by-kind 3, by-order 0, by-enc 1, by-reason 0, by-row 2 |
| `^CHART` | 28 | 6 | by-kind 28, by-enc 4, by-date 28, by-row 22 |
| `^DOC` | 5 | 0 | by-enc 5, by-date 5, by-row 5 |
| `^PARTY` | 18 | 17 | by-row 1 |
| `^LINK` | 294 | — | by-from 294, by-to 294 |

What it shows, for the card work:
- **No visit has a diagnosis list** (`by-diagnosis` 0): Camila's encounters send no
  `Encounter.diagnosis`. A problem's visits come from `Condition.encounter` (`^CHART`
  `by-enc`, 4) until a site sends more.
- **Her medicine's drug is a stub.** The order names its drug by reference, and the site did
  not send the `Medication`. So the order has no concept and is not in `by-medicine`. The
  Medicines hub must show it by the stub's display text. A choice for the Medicines card.
- **People are almost all stubs** (17 of 18): names come from the references' display text.
- **294 links have no pointer field**, mostly a result's `performer`. No hub reads them yet.

### 10.7 Desk test (the owner)
Run from your own terminal (the Claude app sees a private AppData):
1. `npm run inspect -- --tree`. Expect, for Camila: in `^RES`, her stored results (254
   Observations and her reports), 182 components (91 blood pressures × 2) and about 22 stubs
   (the `hasMember` targets that did not arrive); in `^ENC`, 6 visits, 1 of them a stub; the
   build time in milliseconds; and every index's size.
2. `npm run inspect -- --export dev-dumps\epic-sandbox`. `record.xlsx` now ends with one
   `^` sheet per global. Check that the busiest visit (the stub) holds the vitals, and that
   each blood pressure's systolic and diastolic follow it in `^RES`.
