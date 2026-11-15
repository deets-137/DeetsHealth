---
status: project
desk_test: none
sources: [ehr/src/epic.rs, ehr/src/epic_ehr.rs, ehr/fixtures/epic-sandbox.json]
updated: 2026-11-15
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
| 4 | SMART on FHIR version | **R4** | The sign-in's version. The API list, not this field, lets us call the STU3 booking APIs. |
| 5 | SMART scope version | **SMART v2** (the owner, 2026-11-11) | v2's CRUDS letters grant less: the held note write is `DocumentReference.c` (create only), where v1's `.write` also allows update. Epic grants what we ask for, cut to the selected APIs. The `ehr` sign-in asks with v2 words. |
| 5b | FHIR ID Generation Scheme | **64-Character-Limited FHIR IDs** (the owner, 2026-11-11) | The FHIR standard caps an ID at 64 characters. Epic keeps longer IDs only for apps that saved old ones; we save none (the record lives one call long). |
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
- **One key per machine** (the owner, 2026-11-14; HANDOFF.md › On another machine): the private
  key never leaves the machine that made it. On a new machine, `npm run make-key` adds its
  public key to the set beside the others; commit the set and deploy. `ehr` computes the kid
  from the key on disk (`thumbprint` in `ehr/src/epic.rs`), and a test checks that every kid in
  the set is its key's thumbprint. A lost machine: drop its key from the set and deploy.
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
  - ~~Does Backend Services work against it with a non-production client ID?~~ Yes
    (2026-11-12, §5.1).
  - Does it serve `Appointment/$find` and `$book`, and with which visit types and Cadence rules?
    `$find`: served on the STU3 address; the parameters it needs are open (2026-11-13, §5.2).
  - Which `Patient.Search` parameters it takes (phone, birth date), and how it answers two
    patients with one phone number. Family, given and birth date together; not phone
    (2026-11-12, §5.2).
  - Which refill and message actions a backend app may use.
- **Every sandbox reply is recorded once and replayed in tests** (VOICE-AGENT.md §9), so the
  test suite puts no load on Epic.

### 3.1 Epic's rules for the sandbox (read 2026-11-13)
From fhir.epic.com's Terms of Use and its "FHIR Developer Sandbox" section:
- **Sample or synthetic data only.** "You agree not to put individually identifiable
  information in any Epic-provided sandboxes."
- **The sandbox is shared.** Every developer reads the same test patients. Anything written is
  seen by others until the reset.
- **It resets every week:** "The Sandbox is refreshed every Sunday at approximately 8:00 PM
  Central Time", and "any data you wrote to the Sandbox from the prior week will be erased".
  Calls near that time may fail. Some developers run a script each Monday to make the data
  they need for the week.
- **Not for load or security tests:** the site "should not be used for performance,
  scalability, or security testing". So the simulated callers (VOICE-AGENT.md §9) never run in
  bulk against it, and no security probe is pointed at it.
- **Writes:** Epic gives no general list. Some writes need a ticket (a flowsheet row needs a
  Sherlock ticket). The terms do not forbid a write to a test patient.
- **What we do:** read only. Epic mode makes no write. If a later step writes (a booking, a
  refill request), it writes only made-up values, never a real person's details, and it
  expects the Sunday reset.

## 4. A real hospital: what it must do
> **Part:** designed · 2026-11-13 — from fhir.epic.com's OAuth 2.0 and app-request sections.
> Not yet done with any hospital.

**What we must do first:**
1. **Mark the app "Ready for Production"** on fhir.epic.com. After that, "neither you nor Epic
   can make updates to an app", except its redirect URIs and JWK Set URLs. A new API then
   needs a new app. So the API list (§2.1) must be final first. The reads for R1–R4 and
   booking (STU3 `$find`, `$book`) are in the list now; the list is checked again then.
2. **One key and one JWK Set URL per hospital and per environment** (non-production and
   production), as Epic recommends. Epic requires a JWK Set URL for every backend app from
   its May 2026 version. Our design already has one key per deploy (§2.3).
3. Give the hospital our client ID (non-production first) and the JWK Set URL for it.

**What the hospital does, in its own Epic:**
1. Signs the open.epic API Subscription Agreement with Epic.
2. A person with the "Purchase Apps" security point downloads our app by its client ID.
   A backend app's download waits until we set its client credential (the JWK Set URL) for
   that hospital. **"Download" is Epic's word for copying our app's record** (its client IDs,
   its API list, our JWK Set URL) from Epic's app registry into the hospital's own Epic. It
   installs no software. The person does it on Epic's website (fhir.epic.com or open.epic,
   with their organization's sign-in), and Epic syncs the record to their system. Our part is
   on our app's page there: give them the client ID, set the credential for that hospital, and
   approve if asked.
3. Activates it, **non-production before production**: Build Apps › Review and Manage
   Downloads › the environment › Activate.
4. Its IT team **maps the client ID to a background user** ("for the purposes of auditing and
   providing security") and gives that user the security points our APIs need. Epic's
   Interconnect topic "Backend OAuth2 (Client Credentials)" covers it.
5. Gives us the FHIR base URL of each environment, and opens its network (an allowlist or a
   VPN) if it requires one. **An allowlist can face two ways:**
   - **In, to its FHIR server:** our deploy's public **egress IP addresses**, the fixed
     addresses our `ehr` sends from. So each deploy needs fixed outbound addresses (in AWS, a
     NAT gateway with Elastic IPs, CLOUD.md), one set for non-production and one for
     production. Many hospitals publish the FHIR server openly and rely on OAuth instead.
   - **Out, from its Epic to us:** Epic fetches our **JWK Set URL** to check the signature on
     every token request. If the hospital limits outbound traffic, it allows that one HTTPS
     URL (host and path, port 443). Epic says to give the JKU to customers "so it can be
     mapped to the app or added to an outgoing-traffic allowlist". The non-production URL is
     `https://health-keys.deets.solutions/nonprod/jwks.json` (§2.3); production moves to AWS.
6. Builds the Cadence rules for each visit type the agent may book.

**A demo in a hospital's own system** runs in its non-production environment with our
non-production client ID: steps 1 to 5 for non-production only. Two cautions:
- **A hospital's test environment often holds copies of real records** (refreshed from
  production). Treat it as protected data unless the hospital says in writing that it holds
  synthetic patients only. With real copies, the BAA (SECURITY.md §5) comes before the demo,
  and every vendor in the call path needs one.
- The demo uses the hospital's own test patients, so the name and date search, the mobile
  number and the visits depend on its data. A probe run first (§5.2's checks) shows what works.

**Before the first real call:** production activation (steps 2 to 4 for production), the BAA
signed (SECURITY.md §5), and each hospital's deploy with its own key (§2.3).

## 5. The plan
1. **E-1.** The owner registers the app (§2). We make the key pair and publish the JWK Set URL.
   **Done 2026-11-11** (§2.1, §2.3), once the owner saves the form with the URL.
2. **E-2.** Backend Services sign-in against the sandbox: the JWT, the token, one `metadata`
   read, one `Patient` read. This is conversion step 5 (CONVERSION.md §5) and replaces
   `core/src/auth.rs`. **Done 2026-11-12** (§5.1).
   > **Part:** built · 2026-11-12

### 5.1 E-2 as built
- `ehr/src/epic.rs`: `EpicConfig::sandbox` reads the non-production client ID from
  `epic-client.json` and the P-384 key from `dev-certs/epic-nonprod-es384.key`. `Epic::connect`
  reads `.well-known/smart-configuration` for the token endpoint. `Epic::token` signs an ES384
  assertion (`iss` = `sub` = the client ID, `aud` = the token endpoint, a 128-bit `jti`, 240 s
  life) on AWS-LC and asks for a `client_credentials` token. The token stays in `Zeroizing` and
  is never printed. `Epic::get` reads one FHIR path; `Epic::read` (§5.2) reuses the token.
- `ehr epic-check` runs the step: the key set, the token, `metadata`, one `Patient` search for
  Camila Lopez. It prints status codes and counts only.
- **First run, 2026-11-12:** the key on disk matches the published set; the token came back
  (3600 s, 28 scopes); `metadata` 200, FHIR 4.0.1; the `Patient` search by name and birth date
  found 1 match. So Backend Services works with our non-production client ID, and the form
  holds the JWK Set URL.
- Tests: the `kid` in the code matches `nonprod.jwks.json`; an assertion verifies and carries
  the claims Epic checks.
- `core/src/auth.rs` (the patient app's standalone launch) is not moved yet. It moves with the
  pass redesign (CONVERSION.md §5.1).
3. **E-3.** Find the caller (`Patient.Search`) and the chart reads of DESIGN.md › R1–R4, each
   recorded for replay. **Started 2026-11-12** (§5.2): the caller and the next visit, live in
   dev; the other chart reads and the recording for tests are open.
   > **Part:** built · 2026-11-12 (the caller and the next visit)

### 5.2 Epic mode as built (the caller and the next visit)
The owner's forks 1A and 2A (2026-11-12): `ehr` serves Epic's sandbox beside its own fakes, and
each test call picks one with the Chat page's Epic / Local button (CHAT.md §13).

- **The contract:** `OpenCallRequest.ehr` (`EhrSource`: unspecified = local, `LOCAL`,
  `EPIC_SANDBOX`). It names a server, never a patient. `ehr` refuses `EPIC_SANDBOX` with
  FAILED_PRECONDITION when it was not given Epic. The session keeps the source for the whole
  call (`session.rs` `Source`).
- **`ehr/src/epic_ehr.rs`:** `EpicPatients` searches `Patient?family&given&birthdate`;
  `EpicScheduler.visits` searches `Appointment?patient&service-category=appointment&status&date`:
  the visits to come in a 90-day window, then 400; a day or month the caller named; or the last
  400 days for a past visit (EPIC.md §7.10; until 2026-11-14 the next visit only, in windows of
  7, 90 and 400 days).
  Finding and booking slots use `$find` and `$book` since 2026-11-15 (§7.10); before, the
  fake's slots moved to the weekdays after today. The token is reused until a minute before it expires;
  a 401 gets one new token and one more try (`Epic::read`).
- **A new pass, `EhrPass`** (`core/src/ehr_request.rs`): the search must carry the caller's name
  and date of birth, and the chart read the patient's id, in plain form. Only `epic_ehr.rs`
  names it (a test in that file). The patient app's `SitePass` was not reused: it belongs to
  core's own client, not `ehr`'s AWS-LC one.
- **The number for the code:** the `mobile` phone only, not ended, the best `rank` first, as
  +E.164. A chart with no such number still counts in the match (so a near-twin is never
  skipped), but no code goes out
  (`verify.rs` `Candidate.phone` is now an `Option`). The hospital setting and the rules of
  this default: FUTURE-SETTINGS.md §8.
- **Times:** Epic sends instants in UTC; `ehr` shows them in the place's time zone
  (the call's hospital's `time_zone`, Config › Hospital, HOSPITAL-CONFIG.md §2.4; the Default
  is America/Chicago; the `jiff` crate, which carries its own time zone data).
- **What a visit is** (2026-11-13, FUTURE-SETTINGS.md §9): the type from `serviceType`, the
  reason from `reasonCode`, through the sensitive screen in `ehr/src/visit_about.rs`. **Never
  `comment`.** On Camila Lopez's 284 visits: `serviceType` on all (281 "MyChart Video Visit",
  and one each of "New Patient Appointment", "X-Ray Exam", "Transthoracic Echocardiogram
  Complete"); `reasonCode` on none; `comment` on 274, holding other developers' test notes
  ("Synthetic sandbox booking latency test"), which also shows they book through `$book` here;
  `patientInstruction` on 3 (not read yet).
- **The rail's list:** `ehr epic-sandbox` reads each test patient in `epic-sandbox.json` and
  their visits in the next 60 days (10 shown, the rest counted), about 2 reads per patient. The
  dev server runs it and keeps the answer for 5 minutes.
- **Turning it on:** the dev server sets `DEETS_EPIC_SANDBOX=1`, `DEETS_EPIC_CLIENT_FILE` and
  `DEETS_EPIC_KEY_FILE` for `ehr` when `epic-client.json` and the key are on the machine. A
  failed sign-in at start leaves `ehr` on the local fakes, with one line on stderr.
- **A call in Epic mode** uses the real date for "today" (`call.py`, `flow.py`); a local call
  keeps `FAKE_TODAY`, so the replay cache holds.

**What the sandbox showed (2026-11-12):**
- `Patient` search: nothing for a name alone or a date alone. With family, given and birth
  date, it forgives a slip ("Lopes" finds Lopez). Our own match then decides.
- **Search by phone: nothing**, in 5 forms (`telecom` and `phone`, with and without the country
  code and a name or date). So the code by text must follow a name and date search, as built.
- **`Appointment/$find` is an STU3 operation** (§2.1). On the R4 address it answers 405; on the
  STU3 address (`.../api/FHIR/STU3/Appointment/$find`) it answers 400, "Content invalid", to a
  request with only the patient and a time range. So the sandbox serves it to our client; the
  parameters it needs (likely a visit type) are open. Booking stays on the stand-in until
  E-4 (corrected 2026-11-13: the first note said 405 meant "not served").
- The test patients: Camila Lopez has a rolling set of booked visits (261 from today to
  2026-12-15, 30 minutes each, "Physician Family Medicine, MD" at "EMC Family Medicine"). Derrick
  Lin, Linda Ross and Olivia Roberts have a mobile number and no visits ahead. Elijah Davis,
  Desiree Powell and Warren McGinnis have no mobile number, so a call as them is handed off.
- `Patient.name` lists the middle name after the first given name; `ehr` uses the first.

**Not done in E-3:** the medicine, refill and lab reads (DESIGN.md R2–R4); recording Epic's
replies for the test suite (VOICE-AGENT.md §9). Epic mode is live only; the local fakes carry
every test.
4. **E-4.** Booking on the sandbox, if it serves `$find` and `$book` (§3); otherwise the fake
   scheduler carries the tests and booking waits for a hospital's test system.

**Epic's own spec for `$find` and `$book` (read 2026-11-14).** Read through the open.epic MCP
server (`https://mcp.open.epic.com`, set up in the repo's `.mcp.json`; documents `api:840`
`$find` and `api:839` `$book`). Where it disagrees with the research below, the spec wins.
- **`$find`'s only required input is the visit type** (`1..1`). Every other input is optional
  in the spec; the hospital's Cadence rules may require or ignore any of them. The inputs:
  `patient`, `startTime`, `endTime`, the visit type, `indications` (a SNOMED or the hospital's
  diagnosis code), `location-reference`, `referrals` (a referral id; it picks the visit type
  and providers), `specialties` (the department's specialty) and `time-of-day` (a `Timing`:
  days of the week, a time, a length). The last three are new to us.
- **The visit type's name is spelled two ways.** The parameter table says `service-type`;
  Epic's sample request says `serviceType`. The probe sends only `serviceType`.
- **`patient` is a whole Patient resource, used to match a record by demographics**: name,
  birth date, sex, identifiers (the sample sends the EPI and the MRN), phone, address, a time
  zone extension. The probe sends a resource that holds only the FHIR id. That may be why the
  sandbox answers "Content invalid".
- **Epic's sample request is built for its sandbox**: the visit type `95014` "Office Visit" in
  the system `urn:oid:1.2.840.114350.1.13.0.1.7.3.808267.11`, an `indications` with SNOMED and
  ICD-10 codes, and a `location-reference` to a Location on the sandbox. Its window is
  2024-05-01 to 2028-05-05.
- **`$find`'s reply:** proposed Appointments, 10 a page, with a `next` link (a session id;
  error 4113 when it expires). The slots are held only for a short time. One slot per time:
  no overlaps. The ids work only with `$book`; a read of one fails. `$find` never returns the
  patient's existing visits.
- **`$book`'s sample** sends `patient` and `appointment` as `valueIdentifier` (each a FHIR
  id), and `appointmentNote` as `valueString`. Its reply is the Appointment, status `booked`,
  with the CSN, visit type, times, length and participants.
- **Sign-in:** both allow Backend Systems; neither checks scopes
  (`performsScopeValidation: false`).
- **What we will send and keep (decided 2026-11-14; Local built, Epic at E-4).** The caller's
  reason, in their own words (the agent checks them), goes in `$book`'s `appointmentNote`; `ehr`
  already passes it to every adapter's `book` (`BookNote`, EHR-SERVICE.md §4), and
  `EpicScheduler.book` sends it once E-4 is built. A coded `indications` waits until a hospital
  maps reasons to codes. The caller's in person or online (`visit_mode`) picks among the
  hospital's visit types for `$find`. From `$book`'s reply, the call keeps the visit one call
  long: CSN, visit type (so online or not), time, length, clinician, place, and the reason sent.
  Open, for the probe or the hospital: the note's length limit; where the note shows on a later
  read (`comment` or `reasonCode`; `visit_about.rs` never reads `comment`); which visit types are
  online at each site.
- **The sandbox's own rules** (Epic's Developer Testing Guide): it is upgraded each quarter
  (February, May, August, November), and errors come and go during the upgrade. **The 503
  "OltpServerUnavailable" of 2026-11-13 may be the November upgrade.** Data written to the
  sandbox is erased each week, so a `$book` there does not last.
- **The probe has three more tries (built 2026-11-14):** the patient's demographics in the
  resource (read from the test patient's record), the `service-type` spelling, and Epic's own
  sample visit type and indications. **Not run yet:** on 2026-11-14 the whole sandbox still
  answered 503 "OltpServerUnavailable", even its public `metadata` with no sign-in. At the
  same time, fhir.epic.com's banner said the sandbox "has been upgraded to the November 2026
  version and is now available for testing". Epic's FAQ (`faq:87`): a few minutes of
  maintenance each Thursday, 2–3 PM Central, and a banner before an upgrade; any other outage
  goes to the contact form on fhir.epic.com.
- **Epic's API change log (`tutorial:40285`), November 2026 back to 2025:** no item touches
  `$find`, `$book`, Appointment or Backend Services sign-in. Two items matter later, for the
  lab reads (R4): "DiagnosticReport (Results) Code System Changed" (Q-8294158) and
  "Observation (Flowsheets) (R4) Now Required to Retrieve Non-Vital Signs Flowsheet Data"
  (Q-8427283).

**What the docs say about `$find` and `$book` (research, 2026-11-13).** Epic's own spec pages
for both operations load by script on fhir.epic.com and could not be read then; what follows is from
the Argonaut guide Epic's `$find` is built on, from Epic staff in the FHIR chat, and from one
integrator's write-up. Each line says how sure it is. A sandbox `$find` settles the rest.
- **Epic built `$find` for provider-to-provider scheduling, not for patient-facing apps.** Its
  developer (FHIR chat, 2021): "our use of $find is only for provider-to-provider scheduling";
  "there is build needed to determine which visit types, departments, specialties, pools".
  So each hospital chooses what `$find` may return. *(Epic staff, 2021: may have changed.)*
- **`$find`'s inputs at Epic:** `patient`, `startTime`, `endTime`, `serviceType` (the visit type,
  coded in the hospital's own system), `indications` (the reason), `location-reference` (the
  department). Which are required is each hospital's build: the write-up's example site needs
  `serviceType` and `location-reference`. No provider input. *(Integrator's write-up; matches
  the sandbox's 400 to a request with only a patient and a time range.)*
- **`$find` returns** a Bundle of `proposed` Appointments, each with a contained Slot
  (`busy-tentative`) and the provider and location as participants. Results are paged by a
  session that expires (error 4113). *(Write-up; a forum post shows the same contained Slot.)*
- **`$book`'s inputs:** `patient`, `appointment` (a proposed one's id from `$find`; `$find`
  must come first) and an optional `appointmentNote`. No time and no visit type: the proposed
  appointment holds them. *(Write-up; Argonaut's `$book` has the same shape, with `comment`.)*
- **`$book` returns** the Appointment with status `booked`, its CSN as an identifier, and the
  same fields as a read: `serviceType` (the visit type), `start`, `end`, `minutesDuration`,
  participants (patient, practitioner, location). Argonaut: a rejected booking comes back
  `cancelled`, or as an OperationOutcome. A success code alone does not prove a booking: read
  the body. *(Write-up and Argonaut.)*
- **Patient preferences:** no Epic FHIR API returns a patient's scheduling preferences (times,
  clinician, place). The only preference on `Patient` is `communication` (language). A caller's
  preference reaches `$find` as its inputs: the visit type (online or in person is a visit type
  at Epic), the department, the dates. `appointmentNote` carries free text to the scheduler.
  *(No source shows an API for preferences.)*
- **No hold, no cancel, no move** through Epic's public FHIR (§2.1, and the write-up): keep the
  gap between `$find` and `$book` short.
- **The probe:** `ehr epic-find` (run from the repo root; it needs `epic-client.json` and the
  key). It reads one of Camila Lopez's visits for a real `serviceType` and department, then
  sends `$find` six ways: the patient as a resource or a reference; with the times; then
  `serviceType`; then `location-reference`. It prints each HTTP status, Epic's issue words and
  the reply's shape, never an id. About thirteen requests (nine `$find`s); it books nothing. **First run,
  2026-11-13 evening: the sandbox answered 503 "OltpServerUnavailable" to everything, even its
  `smart-configuration`.** Run it again when the sandbox is up.
- **The probe answered, 2026-11-15** (`ehr epic-find`, rewritten: each try changes one input).
  `$find` returns proposed visits when it gets the patient as a resource with their
  `identifier`, `name` and `birthDate` (the rest of the demographics are not needed), a
  `startTime`, and **`service-type` spelled with a dash** (`serviceType`, as in Epic's sample,
  gets "not supported"). `location-reference` and `endTime` are optional. With no `endTime` the
  reply holds 10 visits, `total` 10, and a `next` link; with a 14-day window, 6 and no link.
  `time-of-day` works: `dayOfWeek` tue and thu gave 3; `timeOfDay` 08:00 for 4 hours gave none
  (true: the open times are in the afternoon). Each proposed visit carries the timezone
  extension, `contained` Slot (`busy-tentative`), and participants as **absolute** references
  (`https://…/Practitioner/…`) with a display. Three "information" issues on every reply
  (`name.use`, `identifier (oid)`, `service-type (oid)`) change nothing. The visit types on the
  test patient's visits (system `urn:oid:1.2.840.114350.1.13.0.1.7.2.808267`): MyChart Video
  Visit 1400000001, New Patient Appointment 1003, X-Ray Exam 1053001, Transthoracic
  Echocardiogram Complete 10023. `$find` for 1003 returns visits too, but **each one comes back
  as a MyChart Video Visit**: in the sandbox every slot is online, so an in-person booking cannot
  be tried there. **The next page:** a GET of the `next` link (STU3 `$find?sessionID=…`) answers
  403; a POST of an empty Parameters to it answers 200 with the next 10 and another `next`. The
  probe's last step runs the real code (`EpicEhr::probe_find`): online 10 slots and a link, the
  next page 10 more; either 20 (two `$find`s) and two links. `$book` is not tried: it writes to
  the shared sandbox (the owner's call).

## 6. Every request we send to Epic
> **Part:** built · 2026-11-13 — the ledger. Change it in the same commit as the request.

All go from `ehr` (Rust) only, over `ehr`'s AWS-LC client (`ehr/src/epic.rs` `Epic`), with no redirects and a 15 s limit. The agent never calls Epic (VOICE-AGENT.md §8.7). "Protected data sent" is what leaves `ehr` in the request; every value in a reply goes into `Shielded` at once, except in `ehr epic-sandbox` (public test patients, dev only).

| Request | Sent by (file › symbol) | When | Protected data sent | Cost |
|---|---|---|---|---|
| `GET .well-known/smart-configuration` | `epic.rs` › `Epic::connect` | once, when `ehr` starts with Epic | none | 1 |
| `POST` token endpoint, `client_credentials` with a signed ES384 assertion | `epic.rs` › `Epic::token` (through `bearer`) | at start of use, then about once an hour (the token lives 3600 s, renewed a minute early); once more after a 401 | none (the assertion names our client ID only) | 1 per hour |
| `GET Patient?family&given&birthdate` | `epic_ehr.rs` › `EpicPatients::candidates` | each `Identify` (at most 3 per call, `MAX_IDENTIFIES`) | the caller's claimed name and date of birth, in the URL (`EhrPass`) | 1 per try |
| `GET Appointment?patient&service-category=appointment&status=…&date=ge…&date=lt…` | `epic_ehr.rs` › `EpicScheduler::visits` (`appointments`) | each `FindVisit`, after both factors pass | the verified patient's FHIR id (`EhrPass`); the day or month the caller named, as a date range | 1 (a day, a month, the past) or 1 to 2 (the visits to come: 90 days, then 400) |
| `POST STU3 Appointment/$find` | `epic_ehr.rs` › `EpicScheduler::find_one` | each `FindSlots` in Epic mode, after both factors pass | the verified patient's `identifier`, `name` and `birthDate`, in the body (`EhrPass`); the days, time of day and weekdays asked for | 1 per visit type (in person or online: 1; either: 2) |
| `POST` Epic's `next` link of a `$find` page (empty Parameters) | `epic_ehr.rs` › `EpicScheduler::find_more` | `FindSlots` with `more`, when this page's slots are all offered | none new (the link is Epic's own) | 1 per page |
| `POST STU3 Appointment/$book` | `epic_ehr.rs` › `EpicScheduler::book` | each `Book` in Epic mode | the patient's FHIR id and the caller's reason as `appointmentNote` (`EhrPass`) | 1 per booking; a write |
| `GET Appointment/{id}` (R4) | `epic_ehr.rs` › `EpicScheduler::book` | after a booking, for its instructions | the new visit's id | 1 per booking |
| `GET Appointment?patient&…&date` for one minute | `epic_ehr.rs` › `EpicScheduler::booked_at` | only when a `$book` reply is lost (V2A) | the patient's FHIR id | 1, rarely |
| `GET Patient/{id}` and the `Appointment` search for 60 days | `epic_ehr.rs` › `EpicEhr::sandbox_list` | `ehr epic-sandbox`: the Chat page's Epic list, at most once per 5 minutes (dev server cache) | none: public test patient ids from `epic-sandbox.json` | about 2 per test patient (about 15) |
| `ehr epic-find` (the probe): one `Appointment` search, `Patient/{id}`, seven `$find`s, then the real `find` and `find_more` (about 4 more) | `main.rs` › `epic_find`; `epic_ehr.rs` › `EpicEhr::probe_find` | by hand | Camila Lopez's public test details | about 13 |
| `ehr epic-book`: `Patient/{id}`, one `$find`, one `$book`, one `Appointment` read, two visit searches | `main.rs` › `epic_book`; `epic_ehr.rs` › `EpicEhr::probe_book` | by hand, only with the owner's go-ahead (a write; run once, 2026-11-15) | Camila Lopez's public test details | about 6, one write |
| `GET metadata` and one `Patient` search | `main.rs` › `epic_check` | `ehr epic-check`, by hand | Camila Lopez's public test details | about 5 |

Not sent yet: `Patient.$match`, the medicine, refill and lab reads (R2–R4), `Practitioner` / `Location` reads, and booking (STU3 `$find`, `$book`). Each joins this table when it is built. The planned writes are booking and the held chart note (`DocumentReference.Create`, §2.1).

## 7. Booking: every field Epic has, and what we do with it
> **Part:** designed · 2026-11-14 — research only, from Epic's specs through the open.epic MCP
> server: `api:840` `$find`, `api:839` `$book`, `api:10469` `Appointment.Search (R4)`,
> `api:862` `Slot.Read`, `api:500` `Schedule.Read`. No code changed. Each "not used" row is a
> candidate; each becomes a fork for the owner when its work starts.

The sandbox is the backup (the owner, 2026-11-14): this section is built from the spec, and a
sandbox run only confirms it. Today `ehr` reads visits from `Appointment.Search` and books on
the stand-in (§5.2). The agent's FindSlots asks the caller for three things only: the reason,
the earliest day and in person or online (`ehr.proto` `FindSlotsRequest`).

### 7.1 `$find`: the inputs Epic accepts
The hospital's Cadence rules decide which inputs count; any may be required or ignored at a
site. Only the visit type is required in the spec.

| Input | What it is | What we have now | Gap |
|---|---|---|---|
| `service-type` (sample: `serviceType`) | The visit type, coded in the hospital's own system. **Required.** | `visit_mode` (in person, online, either) | A per-hospital map from mode (and later reason) to visit-type codes. Without it no `$find` works. |
| `startTime` | The first moment to search. | `earliest_day` | None: send the day's start in the place's time zone. |
| `endTime` | The last moment to search. | Nothing | The caller cannot say "before Friday". Without it we must pick a window ourselves. |
| `time-of-day` (`Timing.repeat`: `dayOfWeek`, `timeOfDay`, `duration` + `durationUnit`) | Days of the week and a time of day the caller prefers. | Nothing | "Mornings", "Tuesdays or Thursdays", "after 3" are not asked. The highest patient value of the gaps: fewer slots read aloud and rejected. |
| `location-reference` | A Location (department) to search in. | Nothing; one place per fixture | "At the north clinic" or "the same place as last time". The last visit's `Location` reference is already in each Appointment we read (§7.3). |
| `specialties` | The department's specialty code. | Nothing | "I need a skin doctor." The flow assumes one clinic and one kind of visit. |
| `indications` | The reason as a code (SNOMED or the hospital's own diagnosis code). | The reason in words only | Decided 2026-11-14 (§5.2): waits until a hospital maps reasons to codes. |
| `referrals` | A referral id; Epic picks the visit type and providers from it. | Nothing | A caller who was referred. Needs a referral read (`ServiceRequest`, held in §2.1) first. |
| `patient` (a whole Patient resource) | Demographics Epic uses to match the record: `name`, `birthDate`, `gender`, `identifier` (MRN or national id), `telecom`, home and temporary `address`, and a **time zone extension** | We plan to send the FHIR id only (the probe) | Send the verified patient's demographics from the `Patient` search we already make (no new read). The time zone is the caller's: it matters for an online visit from another zone. |

**No input names a clinician.** "With Dr. Lin" can only be met by filtering the reply's
participants, and by following the `next` page when the first 10 hold none.

### 7.2 `$find`: what each proposed Appointment carries
| Field | Use it? |
|---|---|
| `id` | Yes: the `$book` input only. A read of it fails. It stays in `ehr` behind the slot token. |
| `identifier` (the CSN) | Not needed until booked. |
| `start`, `end`, `minutesDuration` | Yes (as built for visits). |
| **`extension` timezone: the department's time zone** | **Yes, new.** Replaces the one fixed `time_zone` in `epic-sandbox.json`: a hospital with sites in two zones speaks each slot in its own zone. |
| `serviceType` | Yes: the visit type, and from it online or not (`visit_about.rs`). |
| `specialty` | Candidate: lets the agent say "a cardiology visit". |
| `appointmentType` | No: a broad class (ambulatory). |
| `participant` (practitioner, location, and scheduled resources such as an MRI machine) | Yes: clinician and place by `display`. **Keep the references** too, for a clinician or place filter (§7.1). A resource participant (a machine) must never be read as the clinician. |
| `contained` Slot (`start`, `end`, `status` busy-tentative, `schedule`) | No: the Appointment's own times are enough. |
| `slot.display` | No: the provider's name again. |
| `comment` (500 characters at most), `reason` | No: never `comment` (§5.2). The 500 is the likely length limit of our `appointmentNote` (open: confirm on a booking). |
| `created`, `meta`, the Netherlands fields | No. |
| The Bundle's `next` link | Yes, new: 10 a page; we offer the hospital's "slots offered at a time" (3 by default; HOSPITAL-CONFIG.md §2.9). When the caller turns all down, read the rest of the page, then `next`. Error 4113 means the page expired: run `$find` again. |

### 7.3 `Appointment.Search (R4)`: the fields we do not read yet
We read `id`, `status`, `start`, `minutesDuration` (else `end`), the Practitioner and Location
`display`, `serviceType` and `reasonCode` (`epic_ehr.rs` › `appointment`).

| Field | What it is | Candidate use |
|---|---|---|
| **`patientInstruction`** | Prep for the patient ("arrive 15 minutes early", "do not eat after midnight"). On 3 of Camila Lopez's visits. | **The strongest one.** NextVisit and a new booking could tell the caller how to prepare. Free text: it must pass the sensitive screen (`visit_about.rs`) and the hospital may want it off. |
| `description` | The visit type's short subject line ("Visit for routine care"). | A fallback for the visit's words when `serviceType` has no text. Free text: through the screen. |
| `identifier` (CSN) | The visit's number at the hospital. | The key for "which visit" when the agent hands a move or cancel to staff (S3, S4), so staff find it at once. Also the `identifier` search. |
| `specialty` | The department's specialty. | As in §7.2. |
| `participant.actor.reference` | The Practitioner and Location FHIR ids. | "Same doctor, same place as last time" → `$find`'s `location-reference`, and a filter on the reply. `Location.Read` (selected, §2.1) gives the address and phone, so the agent can say where to go. |
| `participant.status`, `participant.period` | Accepted or declined; each person's time. | No. |
| `reasonReference` | A link to a Condition. | No: a second read of a diagnosis, and sensitive. |
| `appointmentType`, `created`, `comment` | | No (`comment` never). |

**Search parameters we do not send:** `status=booked` would cut the reply to visits still on
(we filter `booked`, `pending`, `arrived`, `checked-in` ourselves; a `status` list must keep all
four). `identifier`, `actor`, `practitioner`, `location`, `service-type`, `specialty`,
`appointment-type` and `reason-code` are **post-filters** in R4: Epic runs the plain search and
then filters, so they save bytes, not load on the hospital's server.

### 7.4 `$book`: what we send and get back
- **Inputs (all three are planned):** `patient` and `appointment`, each a `valueIdentifier`
  holding a FHIR id, and `appointmentNote` (`valueString`), the caller's reason. Epic's
  parameter table also lists an Appointment resource with a `slot` reference; the sample does
  not use it. Send the sample's shape; try the other only if a site refuses it.
- **The reply:** `id`, `status` (always `booked`), `identifier` (the CSN), `serviceType`,
  `start`, `end`, `minutesDuration`, `participant` (patient, practitioner, location). **Not in
  the reply:** `patientInstruction`, the time zone, the note as Epic stored it. One
  `Appointment.Read (R4)` of the new id (selected, §2.1) gets them, if §7.3's instruction is
  wanted after a booking.
- **No idempotency key at Epic.** `BookRequest.idempotency_key` protects the agent-to-`ehr`
  retry only. If `$book` times out, `ehr` must not send it again blind: it searches the
  patient's visits at that time first (one `Appointment.Search`), so a lost reply never books
  twice.

### 7.5 `Slot.Read` and `Schedule.Read`
A Slot is reachable only from `$find`; it gives a time and a Schedule reference. A Schedule
gives the Practitioner and Location. `$find` already sends both as participants, so neither read
is needed: **0 extra FHIR calls**. They stay selected (§2.1) in case a site's `$find` sends Slots
without participants.

### 7.6 Outside Epic's FHIR
Cancel, move, hold, waitlist and the patient's preferences have no FHIR API (§2.2, §5.2).
Epic's interface list has two scheduling paths that are not FHIR, to read before S3 and S4:
the HL7v2 **Incoming Appointment Scheduling** interface (new, moved, cancelled visits from an
outside system) and **Appointment Notifications** (an IVR confirms a visit and sends it back to
Epic). Each needs the hospital's interface team; neither is in our registration.

### 7.7 The candidates, by value to the caller
A draft for the owner to order; each is a fork when it is built.
1. `patientInstruction`: how to prepare, on NextVisit and after Book (§7.3).
2. Time of day and days of the week (`time-of-day`), and a last day (`endTime`) (§7.1).
3. The department's time zone from `$find` (§7.2): correct times at a site in two zones.
4. A safe `$book` retry: search before sending again (§7.4).
5. The same place or clinician as the last visit (`location-reference`, the reply's
   participants), and the place's address from `Location.Read` (§7.1, §7.3).
6. The full demographics in `$find`'s `patient` (§7.1), and its time zone.
7. Paging past the first 3 offers, then `next` (§7.2).
8. The CSN kept with each visit, for the staff hand-off of a move or cancel (§7.3).
9. A specialty, a referral, a coded reason: each waits for a hospital's map (§7.1).

### 7.8 The owner's asks (2026-11-14)
> **Part:** designed · 2026-11-14 — not built. Each item below is the owner's choice, forks 1A
> and 2A included.

- **The hospital's lists** (items 2, 5 and 9): visit types, hours, time of day, specialties,
  reasons as codes, locations and the time zone go on a new page, Config › Hospital
  ([HOSPITAL-CONFIG.md](../architecture/HOSPITAL-CONFIG.md); built 2026-11-15).
- **`patientInstruction` in NextVisit's output** (item 1).
  - `ehr` reads `Appointment.patientInstruction` in `appointment()` and adds it to `Visit` in
    `ehr.proto` (`instructions`, tagged `phi`).
  - The sensitive screen covers it. A private visit drops its instructions too, because they
    can name the care ("bring your methadone dose card"). Instructions that hold a word on the
    sensitive list are dropped the same way (FUTURE-SETTINGS.md §9).
  - The model sees only whether there are instructions (`said`, `none on record`, `private`),
    as for the reason. The words go to the caller in a fixed line spoken by code, never
    written by the model.
  - **When the agent says them** (the owner, 2A, 2026-11-14): it first asks "There are
    instructions for this visit. Would you like to hear them?" When the caller asks about one
    visit in particular, the agent reads that visit's instructions with no question first.
  - After a booking: `$book`'s reply has no instructions (§7.4). One `Appointment.Read` per
    booking gets them. Not asked for yet.
- **The CSN in staff flags** (item 8). `ehr` keeps each visit's CSN (`identifier`) and sends it
  to the agent with the visit, tagged `phi`. When a flag is about a visit (the code's
  `booking_change` on `change_visit`, and later a cancel or "I will be late"), code adds that
  visit's CSN to the flag's notes. The model never sees the CSN and never chooses the visit.
  FLAGS.md §3.3.
- **A time zone variable, and the time in `{today}`** (item 3).
  - One time zone per hospital, and later per location (HOSPITAL-CONFIG.md §2.4). `ehr` and the
    agent both read it. Today they do not agree: `ehr` takes "today" in the fixture's
    `time_zone`; the agent takes `date.today()`, the PC's own zone (`call.py`, `flow.py`).
  - `{today}` now holds the date only, in `role`. The code fills `role` at the start of each
    call (`flow.py` › `role`), so a change shows on the next call. But `role` is the start of
    the prefix the prompt cache shares across calls (ANTHROPIC.md §4.2): the cache matches it
    byte by byte, so it changes once a day.
  - **Where the time goes** (the owner, 1A, 2026-11-14): in a line the code sends at the start
    of the call's own messages, in the hospital's time zone. `role` keeps the date only, so
    the shared prefix stays the same for every call that day.
- **Offering slots** (item 7). Keep Epic's whole page of 10 in `ehr` for the call. Filter it by
  what the caller asked (time of day, the same clinician or place). A hospital setting decides
  how many the agent offers at a time (now Config › Hospital's "slots offered at a time", 3)
  and how it presents them. When
  the caller turns down all of them, the agent offers the next ones on the page, then asks
  Epic for the `next` page. A draft of the presentation choices, for the owner to edit:
  - how many at a time (1 to 5);
  - in what order: the soonest first, or spread over different days;
  - whether the clinician is named for each slot, or once when all are the same;
  - whether slots on the same day are said together ("Tuesday at 9 or at 2:30").

### 7.9 Still not used after §7.8
> **Part:** designed · 2026-11-14 — a list, by value to the caller. Each is a fork when its
> work starts.

1. **Every visit still to come, not only the soonest.** NextVisit returns one visit; the
   search already returns them all in its window (`epic_ehr.rs` › `next_visit`). Fork 2A's
   "a specific visit" ("my visit with Dr. Lin", "the one in March") needs this list, and so
   does the CSN in a flag when the caller has more than one visit.
2. **A last day** (`$find`'s `endTime`): "before Friday". The time-of-day list (HOSPITAL-CONFIG.md
   §2.5) does not cover it.
3. **A safe `$book` retry** (§7.4): search the patient's visits before sending `$book` again.
   Without it a lost reply can book twice.
4. **Instructions after a booking:** one `Appointment.Read` of the new visit (§7.4).
5. **The verified patient's demographics in `$find`'s `patient`,** and the caller's time zone
   for an online visit from another zone (§7.1). No new read.
6. **Past visits** ("when was I last seen?"): the same search with `date=lt` today. It also
   gives "the same place or clinician as last time".
7. **`description`** as the visit's words when `serviceType` has no text (§7.3).
8. **A referral** (`$find`'s `referrals`): needs a `ServiceRequest` read, held (§2.1).
9. **Moves and cancels done by the agent:** no FHIR API. Only the HL7v2 paths (§7.6), through
   the hospital's interface team.
10. **The `status` search parameter** (§7.3): fewer bytes back, no change for the caller.
11. **The 500-character limit on `appointmentNote`:** to confirm on a real booking (§7.2).

### 7.10 Wiring §7.8 and §7.9: the design
> **Part:** designed · 2026-11-14 — on paper, tested against `epic_ehr.rs`, `scheduler.rs`,
> `service.rs`, `ehr.proto` and `flow.py`. The owner's forks V1A and V2A, 2026-11-14. Not built.

Two builds: **build 1** uses `Appointment.Search` only and works on the sandbox now; **build 2**
is E-4, the real `$find` and `$book`, and carries every item that needs them. The owner,
2026-11-14: the two may be in flight together ("dw about one load-bearing in flight here").

#### Build 1: the patient's visits (no E-4)
- **One read path for visits.** `appointment()` in `epic_ehr.rs` reads, in addition:
  `patientInstruction` (§7.8), `identifier` (the CSN, §7.8), and `description` as the visit's
  words when `serviceType` has none (§7.9 item 7). All three pass the sensitive screen; the
  CSN does not need it, as it says nothing about the care.
- **The `status` parameter** (item 10). Visits to come: `status=booked,pending,arrived,checked-in`.
  Past visits: `fulfilled,noshow,cancelled`. `appointment()` keeps its own status check as a
  guard.
- **The contract.** `Visit` in `ehr.proto` gains `instructions` (phi), `instructions_private`
  (bool), `csn` (phi) and `status`. A new `token` (secret) names each visit on this call, as a
  slot's token does: the agent and the model refer to a visit by its number on the call,
  never by its CSN or its FHIR id.
- **Which visit** (items 1 and 6; the owner's V1A). `NextVisit` becomes `Visits`: the visits that match
  what the caller asked. The agent's tool `next_visit` becomes `find_visit`, with optional
  arguments the model fills from the caller's words:
  - `when`: a day (`YYYY-MM-DD`) or a month (`YYYY-MM`), past or to come;
  - `clinician`: the caller's words for the clinician. `ehr` matches them to the
    Practitioner's `display`, by whole words, any case;
  - `past`: true for "when was I last seen?".
  - `group`: a visit group the caller named ("my surgery"), an enum of the hospital's group
    names (below).
  With no argument: the soonest visit to come, as now, and how many more there are in the
  next 90 days.
- **The narrowing question** (the owner, with V1A: "configurably agent can ask a follow up to
  narrow down (ie surgical type, or primary type, etc)"):
  - **Visit groups** are a new list on Config › Hospital (HOSPITAL-CONFIG.md §2.2): a name the
    caller hears ("surgery", "primary care", "imaging"), and its members by visit type,
    specialty or location.
  - When more visits match than the hospital's limit, and the caller named no group, `ehr`
    returns how many matched in each group. Code speaks one fixed line that names the groups
    ("Is this about your surgery visit or your primary care visit?",
    `line.which_visit_group`, a draft). The model does not word the question. The caller's
    answer comes back as `group`.
  - **A group's name can reveal care.** A group that holds a private visit (FUTURE-SETTINGS.md
    §9), or whose name holds a word on the sensitive list, is never named. Its visits are
    said as "another visit", and the caller can still pick it by day or clinician.
  - Only one group matches, or groups are off: the soonest visit and the count, as above.
  - **The settings** (FUTURE-SETTINGS.md index, Booking): on or off; how many matches start
    the question (a draft default: more than 2); how many groups one question names (a draft:
    3 at most; the rest as "or another visit"); their order (a draft: the group with the
    soonest visit first).
- **Reads:** with `when`, one search of that day or month. With no argument, the 7, 90 and 400
  day windows, as now (1 to 3 reads). `past` with no `when`: one search of the last 400 days.
  No read is ever more than one patient's visits.
- **What the caller hears.** The visit line, as now. Then, by fork 2A, "There are instructions
  for this visit. Would you like to hear them?" When the caller asked about that visit by its
  day or its clinician, the instructions follow at once. A past visit says its status: seen,
  missed, or cancelled (the words are drafts in AGENT-PROMPT.md, the owner's to edit).
- **Flags.** `change_visit` takes an optional visit number. Code adds that visit's CSN to the
  `booking_change` flag; with no number, the visit booked on this call, else the last visit
  read (FLAGS.md §3.3).
- **The fake** (`scheduler.rs`, `ehr/fixtures/sandbox.json`) gets the same fields: a synthetic
  CSN, instructions on some visits, two visits for one patient, a past visit of each status.
  The kept scripts are recorded again.

#### Build 2: E-4, `$find` and `$book`
- **The `$find` request** (`EpicScheduler.find`):
  - `service-type`: the visit types for the caller's mode, from the visit-type list
    (HOSPITAL-CONFIG.md §2.1): each row the agent may book that has a code. Built on Config ›
    Hospital (2026-11-15).
  - `startTime`: the start of `earliest_day`, in the hospital's zone.
  - `endTime` (item 2): the end of a new `latest_day`, a `find_slots` argument like
    `earliest_day`. Empty: no end.
  - `time-of-day` (§7.8): `find_slots` gains `times` and `days`. `times` is an enum of the
    names in the time-of-day list (HOSPITAL-CONFIG.md §2.5), so the model can only pick a name
    the hospital made; `ehr` turns each name into `timeOfDay` and `duration`. `days` is an enum
    of the weekdays.
  - `patient` (item 5): a Patient resource with the FHIR id, the MRN `identifier`, the name and
    the birth date, from the `Patient` search this call already made. `ehr` keeps them in the
    session, `Shielded`, for the call only. No new read.
  - The caller's own time zone (item 5) is not sent: the agent does not know it. For an online
    visit the agent names the zone with the time ("2 PM Central time").
- **The page of 10** (§7.8). The session keeps the whole page and Epic's `next` link. Filtered
  by the clinician the caller named. The agent offers N at a time (a setting, now 3);
  `find_slots` with `more: true` offers the next N, then reads the `next` page (one read).
  Error 4113 runs `$find` again (one read).
- **The department's time zone** (§7.2): each slot is spoken in its own zone.
- **`$book`** sends `patient`, `appointment` and `appointmentNote` (§7.4), then reads the new
  visit once (`Appointment.Read`, item 4) for its instructions and CSN. 1 extra read per
  booking.
- **A lost reply** (item 3; the owner's V2A). `ehr` never sends a second `$book` blind. It
  searches the patient's visits at the slot's start: found and booked, the booking stands. Not
  found: the caller hears that the booking did not go through and is asked whether to try
  again (`line.book_not_through`, a draft). A yes sends one new `$book` for the same slot.
- **Referrals** (item 8) wait: `ServiceRequest` is held in the registration (§2.1). Adding it is
  the owner's choice. Then a caller who says they were referred gets their open referrals read
  once, and its id goes to `$find`.
- **Moves and cancels** (item 9) wait for a hospital's interface team (§7.6). Until then
  `change_visit` and the flag with the CSN.

#### Build 1 as built (2026-11-14)
> **Part:** built · 2026-11-14 · desk test open

Where this differs from the design above, this is the code.
- **ehr:** `FindVisit` (`ehr.proto`) replaced `NextVisit`. `Scheduler::visits` returns the
  visits a `VisitAsk` covers (`scheduler.rs`); `visits.rs` › `narrow` narrows them for every
  EHR (clinician, group, the narrowing question). `VisitStatus` (upcoming, seen, missed,
  cancelled; `VisitStatus::from_epic`). `VisitAbout::with_instructions` screens the
  instructions (`visit_about.rs`). `Shielded::names_clinician` and `Shielded::first_named`
  (`core/src/shielded.rs`) match the caller's words without revealing them. `edge.rs` ›
  `find_visit` turns a day or a month into a range of days.
- **Not built as designed:** no visit token. The agent numbers the visits the caller heard
  (`CallFlow.visits_heard`), as it numbers slots; ehr is never asked about a visit by number.
- **The narrowing question asks only when the caller named something** (a day, a month, a
  clinician, or a kind of visit the hospital does not have). "My next visit" with nothing named
  is always the soonest, with the count of the others. A design gap found while building: the
  question would otherwise come before every next visit of a patient with three visits.
- **A day or a month** searches every status a caller may hear of; a cancelled visit counts
  only when no other visit matches.
- **The window for the visits to come** (the owner, 2026-11-15, after the Camila call): 7 days,
  then 30, then 90, then no end; the others are counted in the window that found the soonest,
  and the caller hears "in the next 7 days" (`FindVisitReply.more_within_days`). The search
  with no upper date returns every visit, not paged, as fast as 90 days (the probe: 269 visits,
  1.3 s). The first build's 90-day first window took 1.3 s for Camila on a good day and passed
  the agent's 3 s deadline in her call (`20261115-002907-test`); 7 days take 0.5 s. A named
  day or month searches only that range.
- **One quiet retry** (`flow.py` › `_once_more`): `find_visit` and `find_slots` are sent once
  more when ehr does not answer in time, before the caller hears the system is down. A booking
  is never retried this way (V2A).
- **Epic statuses:** a past `arrived` or `checked-in` visit reads as seen; a visit still
  `booked` after its time has no known outcome and is never said.
- **The CSN** is the `identifier` whose system or type names a CSN, else the first.
- **The agent:** `find_visit` (`when`, `clinician`, `past`, `group`), `read_instructions`, and
  `change_visit`'s `visit` (`flow.py`). Each result gives the model `visit_number`,
  `visit_status`, `reason_for_visit` and `instructions` (`said`, `offered`, `none on record`,
  `private`); never the CSN.
- **The time (fork 1A):** ehr sends the hospital's date and time with `OpenCallReply`
  (`local_now`, `zone`) from the fixture's `time_zone` (`sandbox.json`, `epic-sandbox.json`),
  so ehr and the agent agree. An Epic call takes "today" and the time from it; a typed call
  keeps `FAKE_TODAY` and `FAKE_CLOCK`. `note.call_start` goes to the model as the greet step's
  second message, after `role`. Python on Windows has no time zone data, so the agent does not
  compute the zone itself.
- **The fakes:** `sandbox.json` gives Jordan Ellis three visits to come (one with
  instructions, an X-ray with instructions), a past visit seen and one missed, each with a
  synthetic CSN, and two visit groups (primary care, imaging). The Chat page's rail shows a past
  visit's status.
- **Words:** the new sections in AGENT-PROMPT.md and GUIDE.md (`line.visit_found` to
  `note.after_which_visit`, `note.call_start`), drafts for the owner to edit.
- **Tests:** `visits.rs` (the narrowing), `visit_about.rs` (instructions), `shielded.rs` (the
  matches), `epic_ehr.rs` (instructions, CSN, status, `description`), `service.rs`
  (`FindVisit` end to end on the fake), `test_gate.py` (the offer, `read_instructions`, no CSN
  to the model).
- **Desk test:** a typed or voice call as Jordan Ellis (Local): "when is my next visit?" (the
  soonest, "2 more", the offer of instructions; say yes); "what about my x-ray?" (read at
  once); "when was I last seen?" (the missed visit of October 20); "my visit with Dr. Lin"
  (the X-ray, and one more); "I need to cancel my x-ray" (the flag in the record holds CSN
  90000102). In Epic mode as Camila Lopez: "when is my next visit?"; the record's
  `staff_flag` event and the model's tool results hold no CSN for the model.

#### Build 2 as built (2026-11-15)
> **Part:** built · 2026-11-15 · desk test open · one live `$book` sent on the owner's go-ahead

- **The first real booking** (2026-11-15, `ehr epic-book`, the call's own code, the owner's
  go-ahead for one): `$find` online for Camila Lopez gave 10 slots; `$book` of the first
  answered `booked` with a CSN, the visit type, the same start as the slot offered, and the
  note kept as the reason; her visits to come went from 268 to 269. The read after found no
  instructions (the sandbox's video visits hold none). The switch stays off by default.
- **Epic's own guidance** (`api:839`, `api:840`, read 2026-11-15): `$find` must come before
  `$book`; the end user reviews the options "in collaboration with the patient" before booking
  (our read-back and yes); a new patient needs `Patient.Create` first (not ours: the agent books
  for verified patients only); the visit types and rules are agreed with each hospital before
  go-live ("pre-coordination", Config › Hospital's visit-type list); `$find` may carry the
  patient's time zone; `$book`'s reply is the Appointment, its CSN in `identifier`.

Where this differs from the design above, this is the code.
- **No switch, no stand-in** (the owner, 2026-11-15, after the first real booking): Epic mode
  always finds slots with `$find` and books with `$book`. Every booking in Epic mode writes to
  Epic's shared sandbox (Epic wipes test writes each week). Local mode stays fully fake. (A
  switch, `DEETS_EPIC_BOOKING`, held it off for one day and is gone.)
- **`$find`** (`EpicScheduler::find_one`): the patient's `identifier`, `name` and `birthDate`
  (kept from the patient search in the session, `Candidate::facts`, `Session::facts`; no new
  read), `startTime` (the earliest day's start in the hospital's zone, never before now),
  `service-type` from the hospital's visit-type list (Config › Hospital since 2026-11-15; the
  Default: New Patient Appointment in person, MyChart Video Visit online), `endTime` from
  `latest_day`, and `time-of-day` (`timeOfDay`, `duration`, `dayOfWeek`) from `time_of_day`
  and `days`. "Either" sends one `$find` per type (2 reads) and merges them by time. No
  `location-reference`: the hospital's location list is not built.
- **The reply** (`page`, `proposed`): proposed visits only, each in its own department's time
  zone, clinician and place from absolute or relative references. A slot's id carries its
  start, for a lost reply.
- **The page of 10 and More:** the service keeps the search's slots in the session, offers
  `hospital.json` › `slots_offered` (3) at a time, and `FindSlots` with `more` offers the next
  ones, then Epic's `next` page (`find_more`: a POST of empty Parameters to Epic's link, 1
  request; a refusal or an expired page gives none). The tokens
  already offered still book. The clinician the caller named narrows the slots in the service,
  for every EHR (`Shielded::names_clinician`).
- **`$book`:** the patient and the proposed visit as `valueIdentifier`s and `appointmentNote`
  (the caller's words, 500 characters at most); the reply's Appointment (or a Bundle holding
  one) must be `booked`; then one `Appointment.Read` for its instructions. HTTP 4xx: the slot
  is taken. No answer, 5xx, or a reply that is not a booked Appointment: a search of the
  patient's visits in the minute of the slot's start; found, the booking stands; not found,
  `BOOK_OUTCOME_NOT_THROUGH`, never kept as the key's answer, so the caller's yes books
  (`line.book_not_through`, V2A).
- **The hospital's own setup:** `ehr/fixtures/hospital.json` holds the times of day (morning
  8–12, afternoon 12–5, evening 5–8, drafts) and the slots offered at once; ehr
  (`hospital.rs`) and the agent (`flow.py` › `TIMES_OF_DAY`) read the same file. The model can
  only pick a listed name (an enum); ehr refuses any other.
- **The agent:** `find_slots` gains `latest_day`, `time_of_day`, `days`, `clinician` and
  `more`; its result says `more_waiting`. An online time says the hospital's zone ("8:30 AM
  CST"). The fake honours the same wishes, so the kept scripts and tests cover them.
- **Tests:** `edge.rs` (the new fields, refusals), `service.rs` (More keeps old tokens; time of
  day, days, last day, clinician), `epic_ehr.rs` (a `$find` page in the sandbox's shape; the
  demographics hold only three fields; the visit types), `hospital.rs`, `test_gate.py`
  (numbering goes on; unlisted wishes dropped; the zone).
- **Desk test (Local):** "I need a check-up, afternoons only" (2 times); "anything else?"
  (more; numbering goes on from 4); "with Dr. Okafor on Tuesday" (one time). **Epic, once the
  switch is on (the owner's call):** as Camila Lopez, "a video visit next week" (real proposed
  times in the department's zone); book one; "when is my next visit?" (the new visit, with its
  CSN in a change flag).

#### The cost, in FHIR calls
Build 1: 0 to 1 more read per question about a visit, 0 more for instructions and the CSN.
Build 2: 1 `$find` per search, 1 per later page, 1 `$book` and 1 `Appointment.Read` per
booking, 1 search only when a reply is lost.

#### The forks (the owner, 2026-11-14)
- **V1A:** `find_visit` with `when`, `clinician`, `past` and `group`; the soonest by default,
  then a count of the others; and the narrowing question, a hospital setting. Not chosen: B,
  every visit read aloud.
- **V2A:** a lost `$book` reply with no booking found: the caller is asked whether to try
  again. Not chosen: B, `ehr` sends `$book` again without asking.

### 7.11 Epic's code systems: what an OID says (research, 2026-11-15)
> **Part:** research · 2026-11-15. From the open.epic MCP server: "ID Types for FHIR APIs"
> (`tutorial:2659`), `Appointment.$find (STU3)` (`api:840`), `Location.Read (Organizational
> Directory) (R4)` (`api:928`), and our probe (§5.2). Where a line is our reading and not
> Epic's words, it says "we read".

**The parts of an Epic OID** (`tutorial:2659`). A common pattern is
`1.2.840.114350.1.13.<CID>.<EnvType>.7.<Value>`. Epic says this is one of several patterns,
and that an app must treat an OID as opaque.

| Part | Meaning |
|---|---|
| `1.2.840.114350` | Epic's root OID. |
| `.1` | An application-type ID. |
| `.13` | The application: 13 is EDI, 72 is Care Everywhere. |
| `.<CID>` | The organization's Epic ID. Epic's text says the FHIR sandbox is 529; the systems our sandbox sends carry 0. |
| `.<EnvType>` | 1 Epic (the sandboxes), 2 production, 3 non-production. |
| `.7` | For use in HL7-type messages. |
| `.<Value>` | Depends on the concept. It may have more sub-nodes. |

**The Value part** (we read, from Epic's own examples). After `.7` comes a kind and then an
Epic master file. The master file is written as the ASCII codes of its three letters. Epic's
example `.7.2.657380` is "the Interface Specification (AIP) master file": A=65, I=73, P=80.
- `.2.<file>`: the IDs of the records in a master file.
  - `.2.808267` = PRC, Cadence's visit types. The sandbox sends this system on its visit
    types; 1003 is the record "New Patient Appointment" (§5.2).
  - `.2.696871` = EDG, the organization's diagnoses (Epic's `$find` sample, `indications`).
  - `.2.686980` = DEP, departments (Epic's `Location.Read` sample: a Location's identifier,
    value "1").
- `.3.<file>.<item>`: the values of one item of a master file. Epic's `$find` sample sends
  its visit type as `.3.808267.11` (PRC item 11), code 95014. So a hospital can name a visit
  type by its record ID or by an item's value. The two are different lists.
- `.5.737384.<n>`: IIT record n, an ID type, such as an MRN (Epic's example `.5.737384.0`).
- `.10.<file>.<item>`: a category list. Epic's `Location.Read` sample gives a department's
  specialty in `urn:oid:1.2.840.114350.1.72.1.7.7.10.688867.4150`, code "3", "Allergy".
  That system is under Care Everywhere (`.72`), not EDI.

**What follows for Config › Hospital** (HOSPITAL-CONFIG.md §2.10):
- **All the visit types of one hospital share one system.** The system names the list (a
  master file or an item), not the visit type.
- **The system differs between a hospital's test and production Epic** (EnvType 3 and 2), and
  between hospitals (CID). The exception is an ID type whose "HL7 Root" field the hospital
  fills with its own OID.
- **A bare ID says nothing about its kind.** 1003 is a visit type only because its system is
  PRC. A department or a facility also has a short record ID ("1" in Epic's sample).
- **The `indications` systems:** "a community diagnosis code and/or a SNOMED code. If
  multiple codes are received, the community diagnosis code takes precedence" (`api:840`).
  The community code is the hospital's EDG list. Epic's sample also sends ICD-10-CM
  (`2.16.840.1.113883.6.90`); the text does not name it.
- **`specialties`** is "Department specialty code" (`api:840`): a category list value.
- **`location-reference`** is a reference to a Location, not an ID with a system.
  `Location.Read` gives the name, address, phone, hours of operation and the specialty, so one
  read per row can fill the Places tab.
- **No API lists a hospital's visit types or specialties.** `ValueSet.$expand` supports only
  DocumentReference today (`tutorial:2659`). Epic tells vendors to ask the hospital: the
  hospital runs Epic's "OID Generator" utility (a Galaxy document, for Epic's customers only)
  and sends the OIDs.
