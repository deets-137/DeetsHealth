---
status: project
desk_test: none
sources: []
updated: 2026-11-11
---
# DeetsHealth — Epic: the backend app registration and the sandbox

> **Registered 2026-11-11** by converting the patient app's registration (the owner retired the
> patient app; the client IDs did not change, so `epic-client.json` still holds the right
> non-production ID). A future patient-facing part needs its own app with the Patients audience.
> The phone agent reaches Epic as a system, not as a patient: SMART Backend Services. This doc is the sheet the owner fills in on fhir.epic.com, what the sandbox can
> prove, and what each hospital must do. Epic's form and rules change: check each row on the
> form itself. The patient app's registration and its sandbox findings are
> [archive/PATIENT-APP-EPIC.md](../archive/PATIENT-APP-EPIC.md).

## 1. Terms
- **fhir.epic.com** — Epic's developer site, where an app is registered.
- **Backend Services** — SMART's sign-in for a system with no person present. Our service signs a
  short JWT with its private key and trades it for an access token (`client_credentials` with a
  `client_assertion`). No browser, no redirect URI, no refresh token.
- **JWKS URL** — a public `https` address that lists our public keys. Epic fetches it to check
  our JWT's signature. It lets us add and retire keys with no change on fhir.epic.com.
- **Non-production client ID** — works against Epic's sandbox only.
- **Production client ID** — works at a real hospital once that hospital turns it on (§4).
- **Background user** — the user account inside a hospital's Epic that our calls run as. The
  hospital makes it and sets what it may do.
- **Epic sandbox** — Epic's test FHIR server with fake patients:
  `https://fhir.epic.com/interconnect-fhir-oauth/api/FHIR/R4/`.

## 2. The sheet: what the owner fills in
Copy each ID into `epic-client.json` at the repo root (git-ignored), never into this doc or the
chat. The private key never leaves the secret manager (SECURITY.md §4.6).

| # | Field on fhir.epic.com | What to pick | Why |
|---|---|---|---|
| 0 | Application Name | `DeetsHealth` | |
| 1 | Application Audience | **Backend Systems** | A system with no person present. |
| 2 | Incoming APIs | The list in §2.1. **Read, search and the booking operations only.** | Minimum necessary (SECURITY.md §4.9). |
| 3 | Non-Production JWK Set URL | **`https://health-keys.deets.solutions/nonprod/jwks.json`** (live 2026-11-11). The form takes a URL only for this audience; there is no field for a pasted key. | Epic checks our JWT against it. §2.3. |
| 3b | Production JWK Set URL | Blank. It joins with the deploy (§4: one key per hospital's deploy). | |
| 4 | SMART on FHIR version | **R4** | |
| 5 | SMART scope version | Tell me **v1 or v2**. | Sets the scope words (`system/*.read` or `system/*.rs`). |
| 6 | Non-production client ID | Copy it to `epic-client.json`. | The sandbox. |
| 7 | Production client ID | Copy it to `epic-client.json`, unused until the first real hospital. | §4. |
| 8 | Public documentation, privacy policy, terms | Blank if the form allows. Needed before production. | |

### 2.1 The APIs selected
> **Part:** built · 2026-11-11 · selected on the form by the owner; checked against Epic's live
> catalog the same day (§2.2).

Names are exactly as the form lists them. Every one allows the Backend Systems audience in
Epic's catalog. **Core** is what the v1 call uses. **Held** is selected now, by the owner's
choice, and gets a use as we build; nothing calls it yet.

| What the call needs | API | Set |
|---|---|---|
| Find the caller | `Patient.$match (Patient Match) (R4)` (Epic recommends it for a backend app that must find one patient), `Patient.Read (Demographics) (R4)`, `Patient.Search (Demographics) (R4)` | Core |
| Visits | `Appointment.Read (Appointments) (R4)`, `Appointment.Search (Appointments) (R4)`, `Encounter.Read (Patient Chart) (R4)`, `Encounter.Search (Patient Chart) (R4)` | Core |
| Booking (S1, S2) | `Appointment.$find (STU3)`, `Appointment.$book (STU3)`, `Slot.Read (STU3)`, `Schedule.Read (STU3)` | Core |
| Medicines and their schedule | `MedicationRequest.Read (Signed Medication Order) (R4)`, `MedicationRequest.Search (Signed Medication Order) (R4)` (the dose instructions hold the timing), `Medication.Read (Organization Med List) (R4)`, `Medication.Search (Organization Med List) (R4)` | Core |
| Refill state (R3) | `MedicationDispense.Read (Fill Status) (R4)`, `MedicationDispense.Search (Fill Status) (R4)` | Core |
| Lab results (R4) | `Observation.Read (Labs) (R4)`, `Observation.Search (Labs) (R4)`, `DiagnosticReport.Read (Results) (R4)`, `DiagnosticReport.Search (Results) (R4)` | Core |
| Who and where | `Practitioner.Read (Organizational Directory) (R4)`, `PractitionerRole.Read (Organizational Directory) (R4)`, `Location.Read (Organizational Directory) (R4)`, `Organization.Read (Organizational Directory) (R4)` | Core |
| A note filed to the chart (the emergency flag's chart note, FUTURE-SETTINGS.md §1; a check-in summary) | `DocumentReference.Create (Clinical Notes) (R4)` — **a write** | Held |
| The full medicine list, with outside and patient-reported ones | `MedicationStatement.Read (STU3)`, `MedicationStatement.Search (STU3)` | Held |
| Labs ordered with no result yet | `ServiceRequest.Read (Orders) (R4)`, `ServiceRequest.Search (Orders) (R4)` | Held |
| Surgeries | `Appointment.Read (Scheduled Surgeries) (R4)`, `Appointment.Search (Scheduled Surgeries) (R4)` | Held |
| Vitals | `Observation.Read (Vital Signs) (R4)`, `Observation.Search (Vital Signs) (R4)` | Held |
| The verification text, second factor | None: the text and the call back go through Twilio to the number on file (SECURITY.md §4.4) | — |

**Outgoing APIs: none.** The form offers four: `_AssignChatIntegration` (a phone system assigns
an incoming chat to an agent), `CDS Hooks Framework` (clinician users only),
`_PayerFhirImportClinicalOutgoing` and `MyChart Mobile BLE Wayfinding`. None fits.

Not selected: every other write, DSTU2, **(Premium Billing)** (Epic charges for it),
**(Outside Record)**, **(Prior Auth)**, and `MedicationRequest (Order Template Medication)` (an
oncology plan's template, not the patient's own medicine).

### 2.2 What Epic's catalog says (2026-11-11)
Read from the feed behind fhir.epic.com › Specifications (`/Specifications/Selections` lists
every API; `/Specifications/Api?id=N` gives one API's allowed audiences, its URL and its notes).
- **No API cancels or moves a visit.** The catalog has `$find` and `$book` only. S3 (move) and
  S4 (cancel) hand off to staff until a hospital or Epic gives us one (the owner, 2026-11-11).
- **`$find` and `$book` need the hospital's Cadence build first,** and allow Backend Systems and
  clinician users only, not patients.
- **`MedicationDispense (Fill Status)` answers R3,** which was "not confirmed": whether a
  medicine was filled, with its dose, route and quantity.
- **`MedicationAdministration` is not for us:** the catalog says it serves the Dutch MedMij
  profile only.
- **The form and the catalog disagree on Flowsheets.** The catalog allows
  `Observation (Flowsheets) (R4)` for Backend Systems; the form removed it when the audience
  changed. Lab results do not need it.
- **CTI:** `StoreCallID` (Backend Systems) belongs to Epic's Computer-Telephony Integration
  group. Not selected; worth a look when the hand-off to staff is designed (Phase 3).

### 2.3 The key and the JWK Set URL
> **Part:** built · 2026-11-11 · the owner chose Cloudflare for the non-production URL.

- **The Worker:** `ehr/jwks-worker/` (the `cf` CLI), named `deetshealth-keys`, on the custom
  domain `health-keys.deets.solutions`. It serves `GET /nonprod/jwks.json` and a 404 for any
  other path, with a five-minute cache so a retired key leaves Epic's view fast. Deploy:
  `cd ehr/jwks-worker && npm install && npx cf deploy`.
- **The key:** P-384 for ES384 (Epic accepts RS384 or ES384; FIPS allows both). `kid` is the
  key's RFC 7638 thumbprint. `npm run make-key` in the Worker made it: the private key is in
  `dev-certs/epic-nonprod-es384.key` (git-ignored, never printed), the public set in
  `src/nonprod.jwks.json` (committed). The script refuses to replace a key.
- **Rotation, by hand:** add the new public key to the set and deploy; move the `ehr` service to
  the new key; drop the old key and deploy again.
- **The risk is write access, not reading.** The set is public. Whoever can deploy this Worker
  can add a key that Epic trusts. So the Cloudflare account keeps MFA and few members.
- **Production** moves to AWS with the deploy (C1, CLOUD.md); the sandbox key never signs for
  a real hospital.

## 3. The sandbox: what it can prove
- Base URL: `https://fhir.epic.com/interconnect-fhir-oauth/api/FHIR/R4/`; the token endpoint is
  in its `.well-known/smart-configuration`.
- **Test patients** are on fhir.epic.com › Documentation › Test Data. They are public fakes, so
  they may appear in docs and in chat. Camila Lopez is the one the patient app fetched
  (archive/PATIENT-APP-DEBUGGING.md §7).
- **A new or changed registration takes time to reach the sandbox.** The patient app's
  redirect URI took about 25 minutes (archive/PATIENT-APP-EPIC.md §3). An "invalid client" in
  the first hour may only mean that.
- **The data is small and tidy.** It proves the path, not the mess.
- **To find out on the sandbox:**
  - Does Backend Services work against it with a non-production client ID?
  - Does it serve `Appointment/$find` and `$book`, and with which visit types and Cadence rules?
  - Which `Patient.Search` parameters it takes (phone, birth date), and how it answers two
    patients with one phone number.
  - Which refill and message actions a backend app may use.
- **Every sandbox reply is recorded once and replayed in tests** (VOICE-AGENT.md §9), so the
  test suite puts no load on Epic.

## 4. A real hospital: what it must do
- Turn on our production client ID in its own Epic, and make the background user with the
  rights our APIs need.
- Build the Cadence rules for each visit type it lets the agent book.
- Sign its BAA with us before the first real call (SECURITY.md §5).
- Each hospital's deploy has its own key; the JWKS URL lists each deploy's public key.

## 5. The plan
1. **E-1.** The owner registers the app (§2). We make the key pair and publish the JWK Set URL.
   **Done 2026-11-11** (§2.1, §2.3), once the owner saves the form with the URL.
2. **E-2.** Backend Services sign-in against the sandbox: the JWT, the token, one `metadata`
   read, one `Patient` read. This is conversion step 5 (CONVERSION.md §5) and replaces
   `core/src/auth.rs`.
3. **E-3.** Find the caller (`Patient.Search`) and the chart reads of DESIGN.md › R1–R4, each
   recorded for replay.
4. **E-4.** Booking on the sandbox, if it serves `$find` and `$book` (§3); otherwise the fake
   scheduler carries the tests and booking waits for a hospital's test system.
