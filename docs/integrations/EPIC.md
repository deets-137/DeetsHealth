---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# DeetsHealth — Epic: the backend app registration and the sandbox

> Not registered yet. The phone agent reaches Epic as a system, not as a patient: SMART Backend
> Services. This doc is the sheet the owner fills in on fhir.epic.com, what the sandbox can
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
| 3 | Public key | **A JWKS URL** (recommended; follows: a key per deploy, rotation with no form change). A pasted public key works for the first sandbox test if the URL is not up yet. | Epic checks our JWT against it. |
| 4 | SMART on FHIR version | **R4** | |
| 5 | SMART scope version | Tell me **v1 or v2**. | Sets the scope words (`system/*.read` or `system/*.rs`). |
| 6 | Non-production client ID | Copy it to `epic-client.json`. | The sandbox. |
| 7 | Production client ID | Copy it to `epic-client.json`, unused until the first real hospital. | §4. |
| 8 | Public documentation, privacy policy, terms | Blank if the form allows. Needed before production. | |

### 2.1 The APIs to pick
| What the call needs | API (names as Epic lists them; check the form) |
|---|---|
| Find the caller | `Patient.Search (R4)` (birth date with phone, name), `Patient.Read (R4)`, and `Patient.$match` if offered |
| Visits | `Appointment.Read`, `Appointment.Search (R4)`, `Encounter.Read`, `Encounter.Search (R4)` |
| Booking | `Appointment.$find (STU3)`, `Appointment.$book (STU3)`, and the cancel and reschedule operations Epic offers |
| Medicines and refills | `MedicationRequest.Read`, `MedicationRequest.Search (R4)`, `Medication.Read` |
| Results | `Observation.Read`, `Observation.Search (R4)` (labs), `DiagnosticReport.Read`, `DiagnosticReport.Search (R4)` |
| Who and where | `Practitioner.Read`, `Location.Read`, `Organization.Read` |
| The verification text, second factor | None: the text and the call back go through Twilio to the number on file (SECURITY.md §4.4) |

Skip every other write, DSTU2, and **(Premium Billing)** APIs (Epic charges for them).

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
1. **E-1.** The owner registers the app (§2). We make the key pair in the secret manager and
   publish the JWKS URL.
2. **E-2.** Backend Services sign-in against the sandbox: the JWT, the token, one `metadata`
   read, one `Patient` read. This is conversion step 5 (CONVERSION.md §5) and replaces
   `core/src/auth.rs`.
3. **E-3.** Find the caller (`Patient.Search`) and the chart reads of DESIGN.md › R1–R4, each
   recorded for replay.
4. **E-4.** Booking on the sandbox, if it serves `$find` and `$book` (§3); otherwise the fake
   scheduler carries the tests and booking waits for a hospital's test system.
