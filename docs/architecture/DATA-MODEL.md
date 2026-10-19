---
status: project
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — Data model

> How the sealed cache holds a patient's record, with the encounter at the center. This is
> the design. **The tables, the index columns and the links are built** (CORE-CRATE.md §12,
> stages 3 and 6): the code is [STORE.md](STORE.md) (the files and the schema) and
> [INDEX-AND-LINKS.md](INDEX-AND-LINKS.md) (kinds, codes, dates, roles); where they disagree
> with this doc, they win. Not built: `$summary`, `$docref`, `Binary`, `directory`, `vocab`.
> The input is [FHIR-SHAPES.md](FHIR-SHAPES.md); the map is
> [DATA-ARCHITECTURE.md](DATA-ARCHITECTURE.md); the wall is [SECURITY.md](SECURITY.md).
> Open forks: §8.

## 1. Terms
- **Row** — one stored record, made from one FHIR resource.
- **Kind** — a column that says what a row is ("lab", "vital", "note"). The app reads it at
  runtime to choose the format.
- **Link** — a row that ties one record to another, with a **role** (why they are tied).
- **Core** — what does not change over time. **Encounter pieces** — what adds up over time,
  or belongs to one encounter or a set of encounters.

## 2. The idea (the owner, 2026-09-21)
The app's differentiating feature is weaving the whole record together around the patient's
encounters, even when the data is messy or mismatched. We store what the site gives, as it
gives it, and clean and present at runtime. Every FHIR type has a place (§5), used now or not.

## 3. The shape
```
                      patient  (core: name, birth date, sex)
                         │
     party ──────── encounter ──────── billing
 (people, places,   (date, class, type,  (coverage, claims)
  orgs, teams)       place, status,
                     parent, episode)
                         │
                       link  (from-row → to-row, role, source field)
                         │
    ┌────────┬───────────┼────────────┬────────────┐
  result  medication  document      order      chart_item
  (kind)    (kind)     (kind)       (kind)       (kind)
```
- **`patient`** — core. Name, birth date and sex only (FHIR-SHAPES.md §5, fork 1). Very little in a
  record never changes: coverage, the care team and the problem list all change.
- **`encounter`** — the logistics of a visit: period, class, type, status, place, service
  provider, reason, discharge disposition. The encounter diagnoses and the visit's care team
  are **links** (`Encounter.diagnosis[]` → `Condition`, `Encounter.participant[]` → a party).
- **The five content tables** — `result`, `medication`, `document`, `order`, `chart_item`.
  Each holds one shape of data, with a `kind` column. Shared columns: `site`, `fhir_type`,
  `fhir_id`, `kind`, code (system, code, display), `status`, start and end time, recorded
  time, the site's `meta.lastUpdated`, and the sealed raw resource (§6.2).
- **`party`**, **`billing`**, **`drug`** — the things the rows point to.
- **`link`** — every reference between rows (§4).
- **`other`** — any patient row with no table of its own (§7).
- **Beside the record** (§7): `definition`, `site`, `site_warning`, `directory`, `vocab`.

## 4. Links
One table holds every reference between rows (fork 1a). A link to an encounter is one case
among many: result → order, medicine → the problem it treats, panel → its results. The
runtime join walks the links. Nothing is copied from one row to another.

A link holds: from-row, to-row, role, and the FHIR field it came from. When the target is
not in the cache yet, the link keeps the FHIR reference so a later fetch can resolve it.

**Encounter roles, by source field** (R4 base fields):

| Role | Source fields |
|---|---|
| ordered | `MedicationRequest.encounter`, `ServiceRequest.encounter`, and the other request types |
| collected / performed | `Observation.encounter`, `DiagnosticReport.encounter`, `Procedure.encounter`, `ImagingStudy.encounter` |
| given | `Immunization.encounter`, `MedicationAdministration.context` |
| dispensed | `MedicationDispense.context` |
| diagnosed | `Condition.encounter`, `Encounter.diagnosis[]` (the two can repeat each other; the join removes duplicates) |
| documented | `DocumentReference.context.encounter`, `Composition.encounter` |
| recorded | `encounter` on `AllergyIntolerance`, `CarePlan`, `CareTeam`, `QuestionnaireResponse`, `Communication`, `Flag`, `AdverseEvent`, `RiskAssessment`, `Media` |
| billed | `Claim.item.encounter`, `ExplanationOfBenefit.item.encounter`, `ChargeItem.context` |
| between encounters | `Encounter.partOf` (parent), `Encounter.episodeOfCare` (episode), `Encounter.appointment` (the booking) |

**Other roles, as built** (stage 6, `core/src/index.rs`): `fulfils` (`basedOn`), `contains`
(`result`, `hasMember`), `part-of`, `derived-from`, `reason` (`reasonReference`: a medicine to
the condition it treats), `drug` (`medicationReference`), `specimen`, `provenance-of`
(`Provenance.target`); on an encounter `participant`, `location`, `service-provider`; by field
name `requester`, `performer`, `author`, `organization`, `location`; any other reference
`reference`. Every link also keeps its exact FHIR field, e.g. `Observation.basedOn`.

**The lab case.** In Epic a result links to the encounter where it was ordered, and to the
encounter where it was collected when that is different (an orders-only encounter cannot
collect). In FHIR:

| FHIR field | Link | Role |
|---|---|---|
| `Observation.encounter` | result → encounter | collected |
| `Observation.basedOn` | result → order | fulfils |
| `ServiceRequest.encounter` | order → encounter | ordered |
| `DiagnosticReport.result[]` | panel → result | contains |

**What the owner knows of Epic's links** (2026-09-21): medicines ordered outside a visit get
a telephone encounter. Problems and allergies carry an encounter only sometimes.

**Rows with no encounter** stay unlinked (58% of SMART lab results). Any guess such as
"probably from the visit on X" is made at runtime and never stored. How to show it is later
design work.

## 5. Where every FHIR type goes

| Table | Used now | Mapped, not used yet |
|---|---|---|
| `patient` | `Patient` | — |
| `encounter` | `Encounter` | `Appointment` (kind = planned), `EpisodeOfCare` (a grouping, through links) |
| `result` | `Observation` (every category), `DiagnosticReport` (kind = panel or report; the parent of its results), `Specimen` (into the result's detail) | `ImagingStudy`, `Media`, `MolecularSequence`, `RiskAssessment`, `ClinicalImpression`, `DetectedIssue` |
| `medication` | `MedicationRequest` (ordered), `MedicationDispense` (filled), `Immunization` (kind = vaccine, fork 5a) | `MedicationStatement` (reported), `MedicationAdministration` (given), `ImmunizationRecommendation` (due), `ImmunizationEvaluation` |
| `document` | `DocumentReference` (content from `Binary`, fetched on open), `QuestionnaireResponse` (form) | `Composition`, `DocumentManifest`, `Communication` (messages) |
| `order` | `ServiceRequest` | `DeviceRequest`, `NutritionOrder`, `VisionPrescription`, `CommunicationRequest`, `RequestGroup`, `Task`, `SupplyRequest` |
| `chart_item` | `Condition` (problem or encounter diagnosis), `AllergyIntolerance`, `Procedure`, `Goal`, `CarePlan`, `Device`, `FamilyMemberHistory` | `AdverseEvent`, `Flag`, `DeviceUseStatement`, `BodyStructure`, `ResearchSubject` |
| `party` | `Practitioner`, `PractitionerRole`, `RelatedPerson`, `Organization`, `Location`, `CareTeam` (a group of parties) | `HealthcareService`, `OrganizationAffiliation`, `Endpoint`, `Person` |
| `billing` | `Coverage` | `ExplanationOfBenefit`, `Claim`, `ClaimResponse`, `Account`, `ChargeItem`, `Invoice`, `Contract`, `InsurancePlan`, `CoverageEligibility*`, `Enrollment*`, `Payment*` |
| `drug` | `Medication` | `Substance`, `MedicationKnowledge` |
| link data | `Provenance` (the source of a row; its raw resource sits in `other`, and its `provenance-of` links tie it to the rows it describes) | — |
| `other` | — | §7.1 |
| `definition`, `site`, `site_warning` | `smart-configuration`, `CapabilityStatement`, `OperationOutcome` | §7.2, §7.3 |
| `document` (summaries) | — | `$summary`, `$docref` (§7.4) |
| not stored | — | §7.6 |

## 6. Decided (the owner, 2026-09-21)
1. **Links (1a):** one table for every reference, encounter as one target type (§4).
2. **What a row stores (2a):** the index columns plus the sealed raw FHIR resource. The
   normalizer runs when a row is read. A normalizer fix reaches the whole cache without a new
   fetch, so the app calls the site less. `Patient` stays trimmed (FHIR-SHAPES.md §5, fork 1): its
   raw resource is not kept.
3. **Orders (3a):** their own table. An order is the bridge to the ordering encounter, and
   "ordered, no result yet" matters to a patient.
4. **Chart lists (4a):** one `chart_item` table with a `kind` column.
5. **Vaccines (5a):** in `medication`, kind = vaccine. Same shape as a medicine: a drug code,
   a dose, a date.
6. **Data that fits no table (6a):** new tables `other`, `definition` and `site` (§7). The
   owner also wants the non-patient data next to the record grabbed, each item sorted into
   the table that fits it best (§7).

## 7. Beyond the five tables
### 7.1 `other` — patient rows with no table
It has the same shared columns as the content tables, plus links and the sealed raw
resource. `kind` is the FHIR type. A type moves into a real table when a card needs it.

| Type | What it holds | Epic |
|---|---|---|
| `MeasureReport` (type = individual) | Whether the patient met a quality measure (for example "A1c checked this year") | ✓ |
| `Consent` | Sharing and privacy choices; some advance-directive data | ✓ |
| `List` | A curated set with its own title and date. The members are links; the list is the row | ✓ |
| `Group` | Groups the patient belongs to | ✓ |
| `AuditEvent` | Who opened or changed the record | — |
| `Linkage` | Duplicate or merged patient records | — |
| `Basic` | The FHIR catch-all for data with no resource type | — |
| `GuidanceResponse` | A decision-support result for the patient; has an encounter | — |
| `BiologicallyDerivedProduct` | A blood product given in a transfusion | — |
| `DeviceMetric` | A device's readings and settings | — |
| `SupplyDelivery` | Supplies delivered to the patient | — |
| `AppointmentResponse` | The patient's answer to an appointment | — |

### 7.2 `definition` — what patient rows point to
No patient data is in a definition, but a row cannot be read in full without it. The key is
the site plus the canonical URL and version. The row holds the sealed raw resource. It is
sealed because the set of definitions a record points to is itself a health fact.

| Type | Pointed to by | When it is fetched | Epic |
|---|---|---|---|
| `Questionnaire` | `QuestionnaireResponse.questionnaire` | When a form row points to one not yet cached | ✓ |
| `Measure` | `MeasureReport.measure` | The same | ✓ |
| `ResearchStudy` | `ResearchSubject.study` | The same | ✓ |
| `ValueSet`, `ConceptMap` | Codes in any row, including the site's own codes | When a code has no display text | ✓ |
| `PlanDefinition`, `ActivityDefinition` | `CarePlan.instantiatesCanonical` | When a site offers them | — |
| `ObservationDefinition` | A lab test's defined ranges (see the missing-range idea) | When a site offers them | — |

Cost: one read per distinct definition, shared by every patient at that site.

### 7.3 `site` and `site_warning` — facts about the site and the fetch
- **`site`** — one row per connected site: the base URL, the `smart-configuration`, the
  `CapabilityStatement` (sealed raw), the site's software version, and when each was read.
  The fetch asks only for the types the site serves, so there are fewer failed calls. Cost:
  `smart-configuration` is free (sign-in reads it); `/metadata` is 1 call per sign-in, with
  no token. The row is sealed: the site's name is protected data (PATIENT-APP-SECURITY.md §2).
- **`site_warning`** — one row per `OperationOutcome` issue found inside a search reply: site,
  FHIR type, severity, issue code, the sealed text, and when. It is free: the warning comes
  in the same reply. It lets a card say "the site withheld some results", so a gap looks
  like a gap.

### 7.4 Whole-record summaries — into `document`
- **`Patient/$summary`** (the IPS Bundle) — kind = summary. It is a second view of the whole
  record, for checking gaps. Stored raw.
- **`DocumentReference/$docref`** (a CCD) — kind = ccd. The C-CDA XML comes from its
  `Binary` and is stored raw. It can hold data that the FHIR types do not expose. Reading
  C-CDA XML needs its own parser, later.
- Cost: about 3 calls (1 for the summary, 1 for the CCD reference, 1 for its content), on the
  first load and on a full refresh. Not on the small update.

### 7.5 `directory` and `vocab` — from outside the site
Neither holds patient data, so neither is sealed. Each comes as a whole file, bundled in the
installer or downloaded whole. **Never a per-code lookup:** that would send a health fact
and the user's IP address outside the wall (PATIENT-APP-SECURITY.md §4).
- **`directory`** — Epic's endpoint directory, the list for the site picker
  (DATA-ARCHITECTURE.md §2). A worker re-hosts it weekly; its shape is
  [ENDPOINT-DIRECTORY.md](../archive/ENDPOINT-DIRECTORY.md).
- **`vocab`** — plain names for codes: RxNorm (drugs), LOINC (tests; it has a consumer-name
  field), UCUM (units), and MedlinePlus health-topic text. Which files, and how: fork 7.

### 7.6 Not stored
No patient data, and nothing a row points to: the conformance types, `CodeSystem`,
`NamingSystem`, the evidence types, the `MedicinalProduct*` and `Substance*` specification
types, `Library`, `EventDefinition`, `SpecimenDefinition`, `DeviceDefinition`,
`ChargeItemDefinition`, `Schedule`, `Slot`, `Bundle` (only as a carrier), `Parameters`,
`MessageHeader`, `Subscription`, `CatalogEntry`, `VerificationResult`, `TestScript`,
`TestReport`.

## 8. Open forks
7. **The `vocab` files.** What is known:
   - RxNorm: the full release needs a free UMLS licence. The "Current Prescribable Content"
     subset needs none.
   - LOINC: free, under its own licence terms, with attribution. Consumer names cover only
     part of LOINC.
   - UCUM: free.
   - MedlinePlus: the health-topic XML is public. How a code maps to a topic without the
     per-code web service is not yet checked.
   - Open: which files, bundled in the installer or downloaded on first run, and how they
     are updated. Bring this with the first card that shows a code's plain name.
8. **Which columns stay plain in SQLite** (PATIENT-APP-SECURITY.md §4.6). Narrowed 2026-09-21 by the
   strictest-standard rule: no protected value is plain, the index columns included. Only
   opaque row keys and table names stay plain; core filters and sorts in Rust. **Built in
   stage 3, for the owner to confirm** (CORE-CRATE.md §12): plain are the table names, the row
   keys, a link's two row keys and their table names (so SQL can join), and the schema
   version. A missing value is sealed too. Every value is padded before it is sealed, so the
   file does not show which values are missing. What it still shows (row counts, the link
   graph, a size class per value) is listed in STORE.md §4.

