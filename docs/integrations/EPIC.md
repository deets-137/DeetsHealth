---
status: project
desk_test: none
sources: [ehr/src/epic.rs, ehr/src/epic_ehr.rs, ehr/fixtures/epic-sandbox.json]
updated: 2026-11-13
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
  `EpicScheduler.next_visit` searches `Appointment?patient&service-category=appointment&date`
  in windows of 7, 90 and 400 days, so a patient with hundreds of visits costs one small read.
  Finding and booking slots use the fake's slots moved to the weekdays after today
  (`FakeScheduler::from_weekdays_after`). The token is reused until a minute before it expires;
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
  (`ehr/fixtures/epic-sandbox.json` › `time_zone`, America/Chicago; the `jiff` crate, which
  carries its own time zone data).
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

## 6. Every request we send to Epic
> **Part:** built · 2026-11-13 — the ledger. Change it in the same commit as the request.

All go from `ehr` (Rust) only, over `ehr`'s AWS-LC client (`ehr/src/epic.rs` `Epic`), with no redirects and a 15 s limit. The agent never calls Epic (VOICE-AGENT.md §8.7). "Protected data sent" is what leaves `ehr` in the request; every value in a reply goes into `Shielded` at once, except in `ehr epic-sandbox` (public test patients, dev only).

| Request | Sent by (file › symbol) | When | Protected data sent | Cost |
|---|---|---|---|---|
| `GET .well-known/smart-configuration` | `epic.rs` › `Epic::connect` | once, when `ehr` starts with Epic | none | 1 |
| `POST` token endpoint, `client_credentials` with a signed ES384 assertion | `epic.rs` › `Epic::token` (through `bearer`) | at start of use, then about once an hour (the token lives 3600 s, renewed a minute early); once more after a 401 | none (the assertion names our client ID only) | 1 per hour |
| `GET Patient?family&given&birthdate` | `epic_ehr.rs` › `EpicPatients::candidates` | each `Identify` (at most 3 per call, `MAX_IDENTIFIES`) | the caller's claimed name and date of birth, in the URL (`EhrPass`) | 1 per try |
| `GET Appointment?patient&service-category=appointment&date=ge…&date=lt…` | `epic_ehr.rs` › `EpicScheduler::next_visit` (`appointments`) | each `NextVisit`, after both factors pass | the verified patient's FHIR id (`EhrPass`) | 1 to 3 (windows of 7, 90, 400 days) |
| `GET Patient/{id}` and the `Appointment` search for 60 days | `epic_ehr.rs` › `EpicEhr::sandbox_list` | `ehr epic-sandbox`: the Chat page's Epic list, at most once per 5 minutes (dev server cache) | none: public test patient ids from `epic-sandbox.json` | about 2 per test patient (about 15) |
| `GET metadata` and one `Patient` search | `main.rs` › `epic_check` | `ehr epic-check`, by hand | Camila Lopez's public test details | about 5 |

Not sent yet: `Patient.$match`, the medicine, refill and lab reads (R2–R4), `Practitioner` / `Location` reads, and booking (STU3 `$find`, `$book`). Each joins this table when it is built. The planned writes are booking and the held chart note (`DocumentReference.Create`, §2.1).
