---
status: built
desk_test: none
sources: [core/src/index.rs, core/src/fhir.rs, core/src/sync.rs]
updated: 2026-09-21
---
# DeetsHealth — The index and the links

> What the core reads from every resource to file it, find it, order it and tie it to other
> rows. The reference for card design: which rows a card can pick, by what, and how they
> join. The design is DATA-MODEL.md §3–§6; the build history is CORE-CRATE.md §12 (stage 6);
> the rows on disk are [STORE.md](STORE.md). Where this doc and DATA-MODEL.md disagree, this
> doc is the code.

## 1. Terms
- **Row** — one stored resource, in one of 11 tables (STORE.md §3).
- **Index columns** — the facts taken from a resource to fill its row: `kind`, the code,
  `status`, the dates, `last_updated`, and for a definition its `canonical` and `version`.
  All are sealed. Every other fact stays in the raw resource.
- **Kind** — our word for what a row is (`laboratory`, `ordered`, `visit`). A card picks its
  rows by table and kind.
- **Link** — a row that ties one row to another, with a **role** (why) and a **source field**
  (the FHIR field it came from, e.g. `Observation.basedOn`).
- **Waiting link** — a link whose target the site has not sent. It keeps the reference, so a
  later fetch can resolve it.
- **Re-index** — build every row's index columns and links again from its raw resource, with
  no fetch.

## 2. Where it runs
- **At the fetch:** `fhir::read_page` calls `index::extract` and `index::references` on each
  resource, inside the same parse as its page (`net::parse_reply`). Each value is shielded
  there.
- **At a re-index:** `sync::reindex` calls the same two functions on each stored raw resource
  (§10).
- It builds no card model. The model types wait for each card's design (the owner,
  2026-09-21, CORE-CRATE.md §8).

## 3. The table of each type
`fhir::table_for`. 93 types have a table. A type with no table is not fetched, and a
reference to it makes no link.

| Table | Types |
|---|---|
| `encounter` | `Encounter`, `Appointment`, `EpisodeOfCare` |
| `result` | `Observation`, `DiagnosticReport`, `Specimen`, `ImagingStudy`, `Media`, `MolecularSequence`, `RiskAssessment`, `ClinicalImpression`, `DetectedIssue` |
| `medication` | `MedicationRequest`, `MedicationDispense`, `Immunization`, `MedicationStatement`, `MedicationAdministration`, `ImmunizationRecommendation`, `ImmunizationEvaluation` |
| `document` | `DocumentReference`, `QuestionnaireResponse`, `Composition`, `DocumentManifest`, `Communication` |
| `order_` | `ServiceRequest`, `DeviceRequest`, `NutritionOrder`, `VisionPrescription`, `CommunicationRequest`, `RequestGroup`, `Task`, `SupplyRequest` |
| `chart_item` | `Condition`, `AllergyIntolerance`, `Procedure`, `Goal`, `CarePlan`, `Device`, `FamilyMemberHistory`, `AdverseEvent`, `Flag`, `DeviceUseStatement`, `BodyStructure`, `ResearchSubject` |
| `party` | `Practitioner`, `PractitionerRole`, `RelatedPerson`, `Organization`, `Location`, `CareTeam`, `HealthcareService`, `OrganizationAffiliation`, `Endpoint`, `Person` |
| `billing` | `Coverage`, `ExplanationOfBenefit`, `Claim`, `ClaimResponse`, `Account`, `ChargeItem`, `Invoice`, `Contract`, `InsurancePlan`, `CoverageEligibilityRequest`, `CoverageEligibilityResponse`, `EnrollmentRequest`, `EnrollmentResponse`, `PaymentNotice`, `PaymentReconciliation` |
| `drug` | `Medication`, `Substance`, `MedicationKnowledge` |
| `other` | `MeasureReport`, `Consent`, `List`, `Group`, `AuditEvent`, `Linkage`, `Basic`, `GuidanceResponse`, `BiologicallyDerivedProduct`, `DeviceMetric`, `SupplyDelivery`, `AppointmentResponse`, `Provenance` |
| `definition` | `Questionnaire`, `Measure`, `ResearchStudy`, `ValueSet`, `ConceptMap`, `PlanDefinition`, `ActivityDefinition`, `ObservationDefinition` |

`Patient` has its own trimmed row, not a table here (STORE.md §3). `Provenance` sits in
`other`; its links (`provenance-of`) tie it to the rows it describes.

## 4. `kind`
`index::kind`. Every row has one.

| Type | `kind` |
|---|---|
| `Observation` | the first code of its first `category` (`laboratory`, `vital-signs`, `social-history`, `survey`, `exam`, `imaging`, `procedure`, `therapy`, `activity`); `observation` when it has none |
| `DiagnosticReport` | `panel` when it has a `result` field (even an empty one); else `report` |
| `Condition` | the first code of its first `category` (`problem-list-item`, `encounter-diagnosis`, `health-concern`); `condition` when it has none |
| `Immunization` | `vaccine` |
| `ImmunizationRecommendation` | `vaccine-due` |
| `MedicationRequest` | `ordered` |
| `MedicationDispense` | `filled` |
| `MedicationStatement` | `reported` |
| `MedicationAdministration` | `given` |
| `Encounter` | `visit` |
| `Appointment` | `planned` |
| `EpisodeOfCare` | `episode` |
| `DocumentReference` | `note` |
| `QuestionnaireResponse` | `form` |
| `Communication` | `message` |
| `AllergyIntolerance` | `allergy` |
| any other type | the type in lower case (`procedure`, `servicerequest`, `practitioner`, `medication`, …) |

**For card design:**
- Pick rows by **table and kind** together. One word can mean two things in two tables:
  `procedure` is an Observation category in `result` and the `Procedure` type in `chart_item`.
- The category code is taken from the first coding, whatever its system. A site that puts
  its own category system first gives a word not in this list. The raw resource has all the
  codings.

## 5. The code
`index::code_field` names the field; `index::concept` reads it.

| Type | Field (the first present wins) |
|---|---|
| `MedicationRequest`, `MedicationDispense`, `MedicationAdministration`, `MedicationStatement` | `medicationCodeableConcept` |
| `Immunization`, `ImmunizationEvaluation` | `vaccineCode`, then `targetDisease` |
| `Encounter`, `EpisodeOfCare`, `Organization`, `Location`, `DocumentReference`, `Composition`, `Coverage`, `Claim`, `ExplanationOfBenefit`, `Account`, `Device`, `Consent`, `Flag` | `type`, then `code` |
| `Appointment` | `serviceType`, then `appointmentType` |
| `Goal` | `description` |
| `CarePlan`, `CareTeam`, `Communication` | `category` |
| `AdverseEvent` | `event` |
| `RelatedPerson`, `PractitionerRole` | `relationship`, then `code` |
| every other type | `code` |

From that field (the first entry when it is a list):
- `code_system` — the system of the **first** coding, in its canonical spelling (§5.1).
- `code` — the code of the first coding.
- `code_display` — the concept's `text`; when there is none, the first coding's `display`.

**For card design:**
- The first coding is not always the one a card wants (LOINC may come second). The raw
  resource keeps every coding; a card's normalizer can choose.
- A medicine given by reference (`medicationReference`) has **no code** in its row. Follow
  its `drug` link to the `Medication` row in `drug`; that row's `code` is the drug.

### 5.1 One spelling per code system
`index::canonical_system` (FHIR-SHAPES.md §4, rule 2). A trailing `/` is dropped first.

| Canonical | Also accepted |
|---|---|
| `http://loinc.org` | `https://loinc.org`, `urn:oid:2.16.840.1.113883.6.1` |
| `http://snomed.info/sct` | `https://snomed.info/sct`, `urn:oid:2.16.840.1.113883.6.96` |
| `http://www.nlm.nih.gov/research/umls/rxnorm` | `https://www.nlm.nih.gov/research/umls/rxnorm`, `urn:oid:2.16.840.1.113883.6.88` |
| `http://hl7.org/fhir/sid/cvx` | `urn:oid:2.16.840.1.113883.12.292` |
| `http://hl7.org/fhir/sid/icd-10-cm` | `urn:oid:2.16.840.1.113883.6.90` |
| `http://terminology.hl7.org/CodeSystem/observation-category` | `http://hl7.org/fhir/observation-category` |

Every other system is kept as the site sent it.

## 6. Status and dates
- **`status`** — the resource's `status`; when it has none, the first code of
  `clinicalStatus` (`Condition`, `AllergyIntolerance`).
  **For card design:** `verificationStatus` is not indexed. A condition or allergy that is
  `entered-in-error` or `refuted` shows only in the raw resource.
- **`start`, `end`, `recorded`** — one ordered list of FHIR fields per column, the same for
  every type. The first field present wins. Each type has only some of them.

| Column | Fields, in order |
|---|---|
| `start` | `effectiveDateTime`, `effectiveInstant`, `effectivePeriod.start`, `occurrenceDateTime`, `occurrencePeriod.start`, `performedDateTime`, `performedPeriod.start`, `onsetDateTime`, `onsetPeriod.start`, `period.start`, `start`, `started`, `whenHandedOver`, `whenPrepared`, `collection.collectedDateTime`, `collection.collectedPeriod.start`, `context.period.start`, `dispenseRequest.validityPeriod.start`, `billablePeriod.start`, `sent`, `startDate`, `createdDateTime` |
| `end` | `effectivePeriod.end`, `occurrencePeriod.end`, `performedPeriod.end`, `abatementDateTime`, `abatementPeriod.end`, `period.end`, `end`, `collection.collectedPeriod.end`, `context.period.end`, `dispenseRequest.validityPeriod.end`, `billablePeriod.end` |
| `recorded` | `issued`, `recordedDate`, `recorded`, `authoredOn`, `authored`, `dateAsserted`, `date`, `created`, `received`, `dateWritten` |

- **`last_updated`** — the site's `meta.lastUpdated`.
- **`canonical`, `version`** — a definition's `url` and `version`. Empty on every other table.

**For card design:**
- A date is the FHIR text as sent: `2019`, `2019-04`, or a full time with its offset. A
  partial date keeps its precision. Text order is not time order when offsets differ, so a
  card parses before it sorts.
- `start` for a `MedicationRequest` is `dispenseRequest.validityPeriod.start` when the site
  sends it; its order date is `recorded` (`authoredOn`).
- An `onsetAge`, `onsetString` or `onsetRange` gives no `start`.

## 7. Writing and resolving links
`sync::write_resource`, `sync::resolve_links`.
1. A resource keeps its row key across fetches: the fetch finds the row by FHIR type and id
   (`Keys`) and replaces it.
2. The row's own links are deleted (`delete_links_from`); its new links are written in the
   same transaction. Links from other rows **into** this row stay, because the row key does
   not change.
3. For each reference: when the target is stored already, the link points at its row. When
   it is not, the link waits (`to_id` empty) with the reference kept.
4. When every job is done, `resolve_links` tries each waiting link again. A link to a
   resource the site never sent stays waiting. A later fetch or re-index resolves it if the
   target comes.

What a link row holds is STORE.md §3. The row keys and the two table names are plain, so
SQLite can join; the role, the source field and the reference are sealed.

## 8. Which references become links
`index::references` walks the whole resource and takes every object with a `reference`.
- **Kept:** `Type/id`, `Type/id/_history/n`, and an absolute URL ending in `Type/id`, all read
  down to the type and the id. The target type must have a table (§3).
- **Left out:**
  - a reference to `Patient` (no table: every row is this one patient);
  - a contained reference (`#id`) and a bundle reference (`urn:…`);
  - anything inside `contained`, `extension`, `modifierExtension`, `meta` and `text`;
  - a reference with only `display` or only `identifier`;
  - canonical references (plain URL strings such as `QuestionnaireResponse.questionnaire`,
    `CarePlan.instantiatesCanonical`): they are not `Reference` objects.
- **One link per reference.** The same target named twice gives two links (for example
  `Condition.encounter` and `Encounter.diagnosis`). The join at read time removes the
  duplicates (DATA-MODEL.md §4).
- **The source field** is the path with no list indexes, e.g. `Encounter.diagnosis.condition`,
  `Procedure.performer.actor`, `MedicationRequest.medicationReference`.
- **Known limit:** an absolute URL on another server is read down to `Type/id` too, so it
  can match a row of this site with the same id. No site seen so far sends one.

## 9. Roles
`index::role`. The rules are tried in this order; the first match wins. "Path" is the source
field without the type.

| # | Source type and path | Role |
|---|---|---|
| 1 | `Encounter` `diagnosis.condition` | `diagnosed` |
| 1 | `Encounter` `partOf` | `parent` |
| 1 | `Encounter` `episodeOfCare` | `episode` |
| 1 | `Encounter` `appointment` | `booking` |
| 1 | `Encounter` `participant.individual` | `participant` |
| 1 | `Encounter` `location.location` | `location` |
| 1 | `Encounter` `serviceProvider` | `service-provider` |
| 2 | `DocumentReference` `context.encounter`, `Composition` `encounter` | `documented` |
| 3 | `Claim` / `ExplanationOfBenefit` `item.encounter`, `ChargeItem` `context` | `billed` |
| 4 | `MedicationDispense` `context` | `dispensed` |
| 4 | `MedicationAdministration` `context` | `given` |
| 5 | any other `encounter` or `context` (top level): `Observation`, `DiagnosticReport`, `Specimen` | `collected` |
| 5 | … `Procedure`, `ImagingStudy` | `performed` |
| 5 | … `Immunization` | `given` |
| 5 | … `Condition` | `diagnosed` |
| 5 | … `MedicationRequest`, `ServiceRequest`, `DeviceRequest`, `NutritionOrder`, `VisionPrescription`, `CommunicationRequest`, `RequestGroup`, `Task`, `SupplyRequest` | `ordered` |
| 5 | … every other type | `recorded` |
| 6 | `basedOn` | `fulfils` |
| 6 | `result`, `hasMember` | `contains` |
| 6 | `partOf` | `part-of` |
| 6 | `derivedFrom` | `derived-from` |
| 6 | `reasonReference` | `reason` |
| 6 | `medicationReference` | `drug` |
| 6 | `specimen` | `specimen` |
| 7 | `Provenance` `target` | `provenance-of` |
| 8 | last part of the path: `requester` | `requester` |
| 8 | … `performer`, `actor`, `who`, `individual`, `member` | `performer` |
| 8 | … `recorder`, `asserter`, `author`, `enterer` | `author` |
| 8 | … `location` | `location` |
| 8 | … `organization`, `serviceProvider`, `custodian`, `managingOrganization`, `insurer`, `provider` | `organization` |
| 9 | anything else | `reference` |

Rules 5 and 6 match the top-level field only. A nested field falls through to rule 8 or 9.

**The links a card joins most** (DATA-MODEL.md §4, the lab case):

| From | Role | To |
|---|---|---|
| result (Observation) | `collected` | encounter |
| result (Observation) | `fulfils` | order (ServiceRequest) |
| order | `ordered` | encounter |
| result (DiagnosticReport, `panel`) | `contains` | result (Observation) |
| medication (MedicationRequest) | `drug` | drug (Medication) |
| medication (MedicationRequest) | `reason` | chart_item (Condition) |
| encounter | `diagnosed` | chart_item (Condition) |
| encounter | `participant`, `location`, `service-provider` | party |

## 10. Re-index
`sync::reindex(store)` rebuilds every row's index columns and links from its raw resource,
in one transaction, with **no call to the site** (DATA-MODEL.md §6, decision 2).
- **When:** after a change to any rule in this doc (§4–§9). A change to the rules reaches
  the whole cache without a new fetch.
- **What it does:** for each row of each of the 11 tables, it reads the type from the raw
  resource, extracts the index columns and the references, rewrites the row and its links,
  then runs `resolve_links` once.
- **What it cannot do:** move a row to another table. A change to `fhir::table_for` needs a
  new fetch.
- **When it runs** (the owner, 2026-09-21: automatic, by version): `index::INDEX_VERSION`
  names the rules; each site file stores the version its rows were built with
  (`meta.index_version`, STORE.md §3). `sync::reindex_if_stale` re-indexes a file whose
  number is older, and marks it current in the same transaction. The host calls it every
  time it opens a site for work (`fetch::open_current`): at app start for every site
  (`fetch::check_sites`, off the UI thread) and before each fetch. There is no UI; a failure
  sends `reindex-failed` (ERRORS.md), the old index stays, and the next open tries again.
- A new file is current from the start. A file migrated from schema 1 is re-indexed once.
- **Cost:** it reads, opens and seals every row once, with no call to the site. Not
  measured on a large record.

## 11. Card design: what is there, and what is not
**There now:**
- Rows by table, with a kind, a code, a status and three dates, all sealed.
- Links with a role and a source field, both ways (`links_from`, `links_to`).
- The raw resource of every row except the patient.
- The site's warnings per type (FETCH.md §7).

**Not there yet:**
- No model types and no normalizer: each card's design adds them (CORE-CRATE.md §8).
- No payload structs: `core/src/payload.rs` is a stub (CORE-CRATE.md §9).
- No query by kind or by date in the store. Core reads a whole table and filters in Rust
  (STORE.md §6). The cost on a large record is not measured.
- No plain name for a code: that waits for the `vocab` files (DATA-MODEL.md §8, fork 7).

## 12. What to change, and where
| To change | Where |
|---|---|
| The table of a type | `fhir::table_for` (then a new fetch: §10) |
| The kind words | `index::kind` |
| The code field of a type | `index::code_field` |
| A code system spelling | `index::canonical_system` |
| The date fields and their order | `index::START`, `index::END`, `index::RECORDED` |
| The roles | `index::role` |
| What the walk leaves out | `index::walk` (the skipped keys) and `index::split_reference` |

After any change: **raise `index::INDEX_VERSION`**, update this doc, and run the core tests.
Every site file then re-indexes itself at the next app start.

## 13. Tests
`cargo test -p deetshealth-core --features sqlite`:
- `index::tests`: a lab result's columns (kind, one spelling of LOINC, text before display,
  a partial date, the recorded date); the per-type fields (a medicine, a condition, an
  encounter, a panel, a definition); references and roles (the patient, `#contained`,
  `contained` and `extension` left out; an absolute `_history` URL read down); the encounter
  roles of DATA-MODEL.md §4; the reference forms.
- `store::sqlite::tests::reindex_builds_index_and_links_from_raw`: rows with only a raw
  resource get their columns and links; a link to a stored row points at it; a link to a
  resource never fetched waits; a second re-index replaces the links and does not add them.
- `store::sqlite::tests::reindex_runs_only_when_stale`: a new file is current; a stale one
  is re-indexed and marked current.
- The live fetch (FETCH.md §14) asserts that links were written.
