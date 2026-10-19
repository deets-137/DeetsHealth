---
status: project
desk_test: none
sources: [scripts/fhir-probe.mjs]
updated: 2026-09-21
---
# DeetsHealth — FHIR shapes

> What each FHIR resource looks like at each site, and what that means for the normalizer
> and the cards. Epic is on hold (HANDOFF.md › Open now 5), so the Epic half is its public
> capability list only. Open forks: §5.

## 1. How this was made
- `node scripts/fhir-probe.mjs --site smart-open --write` reads up to 300 resources of each
  type and writes [probes/smart-open.md](probes/smart-open.md): every field path, its share,
  the value[x] kinds, the code systems. It prints **no values** (PATIENT-APP-SECURITY.md §2).
- Run it once per site, and again when a site or a resource type is added. Sites that need
  sign-in (Epic, the SMART launcher) wait for the app's own sign-in.
- **Sample bias:** the probe takes the first 300 resources in server order, across many
  patients. One patient's record can differ. The shares are a guide, not a rule.
- Epic's list comes from its sandbox CapabilityStatement
  (`…/api/FHIR/R4/metadata`, no sign-in; software "Epic August 2026", FHIR 4.0.1).

## 2. What each site offers
### 2.1 Epic sandbox — 60 resource types
Every type supports read and search. Some also support create or update; **the app never
uses them** (PATIENT-APP-DESIGN.md §5). Which types a patient-facing app may read is set at app
registration, and that is not known until Epic comes back.

| Use to us | Types |
|---|---|
| First slice | `Observation` (31 search params; labs by `category`), `MedicationRequest`, `Medication`, `AllergyIntolerance` |
| Timeline and context | `Encounter`, `Condition`, `Procedure`, `Immunization`, `DiagnosticReport`, `DocumentReference`, `Binary`, `ImagingStudy`, `Appointment` |
| Medicines, more | `MedicationDispense`, `MedicationStatement`, `MedicationAdministration` |
| Care | `CarePlan`, `CareTeam`, `Goal`, `ServiceRequest`, `FamilyMemberHistory`, `ImmunizationRecommendation`, `NutritionOrder`, `Device`, `DeviceRequest`, `DeviceUseStatement`, `Specimen`, `BodyStructure`, `AdverseEvent`, `Flag`, `EpisodeOfCare`, `Task` |
| People and places | `Patient` (plus `$match`, `$summary`), `Practitioner`, `PractitionerRole`, `RelatedPerson`, `Organization`, `Location`, `Endpoint` |
| Money | `Account`, `Claim`, `Coverage`, `ExplanationOfBenefit`, `Contract` |
| Other | `Communication`, `Consent`, `Questionnaire`, `QuestionnaireResponse`, `List`, `Group` (`$group-export`), `Measure`, `MeasureReport`, `Media`, `Provenance`, `RequestGroup`, `ResearchStudy`, `ResearchSubject`, `Substance`, `ConceptMap`, `ValueSet` |

### 2.2 SMART Health IT open server
`https://r4.smarthealthit.org`, Smile CDR, FHIR 4.0.0, synthetic (Synthea) patients, no
sign-in. `_count=100` and `link[rel=next]` paging both work.

## 3. Findings, by resource (SMART, 2026-09-21)
### 3.1 Lab results — `Observation?category=laboratory`
- **Most results are numbers; some are text.** `valueQuantity` 99%, `valueString` 1%. The
  model needs both ("Positive" is a result).
- **Units:** `unit` (display text) 99%, the UCUM `code` only 69%. Show `unit`; compare two
  results by UCUM `code` only when both have one.
- **Reference range is often missing.** Present in 52%; structured low/high with UCUM in 22%.
  The card needs a calm "no range given" state. It never guesses a range.
- **No high/low flag.** `interpretation` is absent from labs (8% of vitals). See fork §5.2.
- **Two spellings of one system.** LOINC comes as `http://loinc.org` and `https://loinc.org`;
  the category system comes in an old (`hl7.org/fhir/observation-category`) and a new
  (`terminology.hl7.org/…`) form. The normalizer maps each to one canonical form.
- **Dates:** `effectiveDateTime` 100%, `issued` 42%. Sort by effective date.
- **Dense:** 300 lab results came from 22 patients. One patient's labs can be thousands of
  rows, so paging and the cache matter.

### 3.2 Vital signs — `Observation?category=vital-signs`
- 15% are `component[]` (blood pressure: two values in one resource). Some components carry
  `dataAbsentReason` instead of a value.
- One non-standard code system appears (a Cerner code set). Real data mixes vendors.
- `note[].text` and narrative HTML appear (8%): free text, so PATIENT-APP-SECURITY.md §4.3 applies.

### 3.3 Lab panels — `DiagnosticReport`
- `result[]` links each panel (a CBC, a metabolic panel) to its `Observation`s, 100%.
  The Labs card can group by panel through this link, with no extra fetch when read with
  `_include`.

### 3.4 Medicines — `MedicationRequest`
- `medicationCodeableConcept` 100% (RxNorm), never `medicationReference` here. Epic may use
  `medicationReference` to a `Medication` resource; the normalizer handles both.
- **No human dose text.** `dosageInstruction[].text` is absent; structured dose and timing
  appear in 37–49%. The card may have to build "1 tablet, 2 times a day" from parts, or show
  the medicine without dose (fork §5.3).
- No `dispenseRequest` and no validity period: "active" comes from `status` only.
- `requester` (the prescriber) is always a reference, so the name costs a `Practitioner`
  read. `_include=MedicationRequest:requester` gets it in the same call.
- `reasonReference` (61%) links a medicine to the `Condition` it treats.

### 3.5 Allergies — `AllergyIntolerance`
- Very even: code, `criticality`, clinical and verification status, `type`, `category`: 100%.
- **No `reaction[]` at all**, so no reaction and no severity. PATIENT-APP-DESIGN.md §4 plans "substance,
  reaction, severity". The card must work with the substance and criticality only.
- The patient link is `patient.reference`, not `subject.reference` (as in Immunization).

### 3.6 Encounters, conditions, immunizations
- `Encounter`: period start and end, type and class 100%; reason 41%. Where and with whom
  are references (`serviceProvider`, `participant`), so they cost reads or an `_include`.
- `Condition` and `Immunization`: complete and even. Condition dates 100%; abatement 54%.

### 3.7 The patient — `Patient` (PII)
- Everything that identifies a person is there: name, birth date, address with geolocation,
  phone, identifiers, and extensions (race, ethnicity, birthplace, mother's maiden name).
- `identifier[]` carries several ID types (v2-0203 codes). In Synthea these include social
  security, driver's licence and passport numbers.
- Narrative HTML (`text.div`) is on 100% of patients. It is never rendered (PATIENT-APP-SECURITY.md §4.3).
- See fork §5.1: what we keep.

## 4. Rules for the normalizer, from the above
1. Every model has a "missing" state for every field below 100% here, and the card shows it
   calmly. A missing value is never an error.
2. Code systems are mapped to one canonical URL each before any compare.
3. The patient link is read from `subject` or `patient`, whichever the resource uses.
4. References that a card shows (prescriber, place) are fetched with `_include`, never one
   read per row (LESSONS.md §3.3).
5. Free text (`note`, narrative, `valueString`) is protected data and plain text only.

## 5. Open forks
1. ~~What we keep from `Patient`~~ — decided 2026-09-21 (b): **name, birth date and sex.**
   "Keep" means **held** (it survives the parse into the model and the wrapper type) and
   **stored** (sealed into the cache). Everything else is dropped during the parse. The site
   may still send it; `_elements=name,birthDate,gender` asks for less (SMART supports it;
   Epic is untested). Anything derived — age, the range that fits an age and sex — is
   computed at runtime from the kept fields and never stored (the owner, 2026-09-21).
   Dropped: identifiers (on SMART: MR, SS, DL, PPN and one untyped), address with
   geolocation, phone, race, ethnicity, birthplace, mother's maiden name, marital status,
   language, multiple birth, date of death, and the narrative.
4. **Which field is "sex"** — parked until the build (the owner, 2026-09-21), with a set
   direction: **`gender` is what the app displays.** Calculations (reference ranges) use a
   **computed sex** built from several factors, the way Epic computes its own: listed
   gender, sex at birth (`us-core-birthsex`, on 100% of SMART patients), organ inventory and
   others. Those inputs are pulled and computed at runtime, never stored (§5.1).
2. ~~High / low marks~~ — decided 2026-09-21 (a): show the range and any mark **the site
   gives**. Ranges vary between hospitals, so a range belongs to its result, and a result is
   never compared against another site's range. A missing range is shown as missing; how to
   fill one is later design work (ideas/README.md).
3. ~~Dose text~~ — decided 2026-09-21 (a): built from the structured parts. The owner has
   ideas for the wording; bring them in at build time.
