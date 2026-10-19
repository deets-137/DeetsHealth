---
status: project
desk_test: none
sources: [core/src/index.rs, core/src/store/mod.rs, ../DeetsEngine/docs/ideas/deetshealth-needs.md, ../DeetsEngine/docs/architecture/IENS-AND-INDEXES.md]
updated: 2026-10-13
---
# DeetsHealth — The globals: a draft

> **A draft, with its two forks decided** (G1 A, G2 A, §7). What the first Epic fetch (Camila Lopez, the Epic sandbox)
> shows about the shape of the record, and a first sketch of DeetsHealth's own globals in
> DeetsEngine. The owner asked for it on 2026-10-13, and added the results that Camila does
> not have: panels of 8 to 15 values (a lipid panel, a CMP) and microbiology with more
> (§4). The globals are designed with the owner in a later session, after the engine's base
> layer exists (HANDOFF.md › Decided, "D-A + R-A lite + L-A"). Nothing here is built. Still
> open: the questions and the data in §8, and the details inside §5.

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
- **Notes per value and per group.** Free text, so PATIENT-APP-SECURITY.md §4.3 applies.
- **A trend across groups.** LDL from five lipid panels over ten years is one trend. So the
  trend index points at the value, not at its panel.

## 5. The draft globals
> **Part:** project · 2026-10-13 · G1 A and G2 A decided, §7

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
| Global | Holds | Pointers | Indexes |
|---|---|---|---|
| `^ENC` | visits, and stub visits | — | by-date, by-fhir (unique) |
| `^RES` | every result: groups (report, panel, culture, isolate) and values | enc, order, parent, root | by-trend (concept, date), by-enc, by-parent (parent, seq), by-root, by-order, by-date, by-code (Each: every coding), by-fhir (unique) |
| `^ORD` | orders (`ServiceRequest`), with the parent order | enc, parent | by-order-number, by-enc, by-date, by-fhir (unique) |
| `^CHART` | longitudinal items: problems, visit diagnoses, allergies, medicines, vaccines, goals, care plans | enc (when it has one), addresses | by-kind (kind, status), by-enc, by-trend for medicines, by-fhir (unique) |
| `^DOC` | notes and documents | enc, order | by-enc, by-date, by-fhir (unique) |
| `^PARTY` | practitioners, locations, organizations, care teams; mostly stubs | — | by-fhir (unique) |
| `^LINK` | every link role with no pointer field | from, to | by-from (role), by-to (role) |
| `^FETCH` | per FHIR type: served, none, not shared (suppressed), refused | — | — |

A value is packed (DeetsEngine PACKING.md), one format per global. `^RES`'s format holds:
kind, concept, value kind, comparator, value, unit, UCUM code, low, high, range text, flag,
status, the date key and the date as sent, enc, order, parent, root, seq, the FHIR id, and a
stub mark. Notes are a subfile node (`^RES(ien,"note",n)`): free text does not go into a
packed value.

### 5.3 The concept key
The trend index needs one key per value. Draft rule: the LOINC code when the value has one,
else the site's own code, prefixed by the site (`site:<oid>:<code>`), so two sites' local
codes never merge. Later, a bundled map of local codes to LOINC (DATA-MODEL.md §8, fork 7)
can join more of them. Every coding stays in by-code.

### 5.4 The date key
- One sortable key per record: UTC, the precision kept (`2019`, `2019-04`, a full time).
  Text with mixed offsets does not sort in time order (INDEX-AND-LINKS.md §6), so the key is
  normalized at load.
- The first of: the clinical date (`start`), `recorded`, `last_updated`. A record that used a
  fallback has a mark, so the card can say "date recorded" and not "date of care".
- A record with no date at all is indexed under a fixed last key, so it is never lost from a
  hub.

### 5.5 Stubs
A reference whose target did not arrive makes a stub in the target's global, once per FHIR
id, with the reference's display text. Every pointer then holds an IEN. When a later fetch
brings the resource, the rebuild at open fills the record.

## 6. What to tune the engine for
- **The bulk load.** With "one value, one record", one record is about 8 nodes (the packed
  value and about 7 index entries). A real adult record of about 10 000 values (an estimate)
  is about 80 000 nodes. A long admission adds many flowsheet values to one visit. The
  engine's load measurements (needs §3: 10 000, 100 000 and 1 000 000 nodes) cover this
  range. The cost of one `set` decides the open time.
- **Deep walks.** A culture group is a walk of 3 or more levels of `by-parent`. This is
  ordered traversal (`$ORDER`), which the engine has.
- **Unique indexes on protected values.** `by-fhir` is unique. A clash report carries values
  today. The redacted reports (needs §4) are required before this index exists.
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

## 8. Questions for the owner (Epic knowledge) and data still to see
- How does Epic's FHIR send a culture: an isolate as an `Observation` with `hasMember`
  susceptibilities, as components, or only as report text?
- How often do Epic sites send LOINC on a lab component? If seldom, the concept key (§5.3)
  needs the local-code map sooner.
- Is Camila's fan-in of 248 on one visit a fair small model of an inpatient stay's flowsheet
  rows? The answer sets the sizes for the engine's load measurements.
- **Data to see:** an Epic sandbox patient with a CMP, a lipid panel and a culture. Cost: one
  sign-in and one fetch (Camila's took 32 calls). Then `npm run inspect -- --csv` shows the
  real shapes, and §4.1's open row closes.
