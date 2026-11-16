---
status: project
desk_test: none
sources: [ehr/src/session.rs, ehr/src/verify.rs, ehr/src/scheduler.rs, ehr/src/service.rs, ehr/src/epic_ehr.rs, ehr/src/edge.rs]
updated: 2026-11-15
---
# The EHRs other than Epic

> **Part:** designed · 2026-11-15. Nothing is built. Was OTHER-EHRS.md (renamed 2026-11-15).

The wedge is hospitals not on Epic (VOICE-AGENT.md §7). Each EHR gets an adapter behind the
EHR traits in `ehr`. Epic is [EPIC.md](EPIC.md). §1–§5 are the design (the talk with the owner,
2026-11-15). §6–§9 are the facts about each vendor.

## 1. The decisions so far
- **Architecture first (the owner's 1B, 2026-11-15).** The next step builds the vendor-neutral
  shape (§2), not a second vendor's full adapter. Reason: less risk, and each vendor after it
  is a smaller step.
- **Oracle Health is the first vendor to consider after that** (the owner, 2026-11-15: "we can
  consider Oracle first then"). Oracle books with standard FHIR, so the shared core does most of
  its work (§2.1). Its sandbox also runs real scheduling rules, which a plain FHIR server does
  not (§4.2). Open: whether Oracle's sandbox comes in with the core as the booking test bed.
- **One hospital uses one EHR** (the owner, 2026-11-15). A hospital's lists hold codes from its
  own EHR only. How the console handles many hospitals is a separate talk; the old Fork 2 (codes
  for several EHRs on one row) is dropped.

## 2. The shape
The seams exist today: `Patients` (verify.rs) finds the candidates, `Scheduler` (scheduler.rs)
finds, books and lists visits, and `Codes` (the text service) does not depend on the EHR. Each
call picks its `Source` once, at OpenCall (session.rs). What changes:

### 2.1 One standard FHIR R4 core, a variant per EHR
Most of the work is the same at every EHR. It goes in one shared module:
- `Patient` search by name and date of birth; the number for the code from `telecom`.
- `Appointment` search for the patient's visits.
- `Slot` search, then `POST /Appointment` that names the slot, then a read back.

Each EHR then changes only what it does differently:

| EHR | Patients | Find and book | Sign-in |
|---|---|---|---|
| Epic | the core | its own `$find` and `$book` (today's `epic_ehr.rs`) | signed JWT (backend services) |
| Oracle Health | the core | the core, with Oracle's quirks | system account, JWKS |
| SMART sandbox | the core | the core (reads; see §4.1) | none (open server) |
| athenahealth | the core or its REST | its own REST adapter | client ID and secret |
| Local | the fake | the fake | none |

For athenahealth the core helps only in shape (the traits, `Source`, the sign-in interface, the
tests). Its booking is its own REST API, so it needs its own scheduler adapter.

### 2.2 `Source` has two parts
`Source` becomes the EHR (Epic, Oracle, athena, MEDITECH, SMART, Local) and the environment
(sandbox or production). `is_sandbox()` reads the environment. The rule "a synthetic call reaches
only a sandbox, a real call only a production EHR" (DATABASE.md §7.5) then holds for each new
EHR without a new arm. The proto's `EhrSource` (edge.rs) grows the same way.

### 2.3 Dispatch stays an enum
The traits return `impl Future`, so they are not object-safe. Dispatch stays an enum, as `Sched`
does today (service.rs). Each EHR adds one variant.

### 2.4 Sign-in is one interface
One token interface, a variant per EHR (the table in §2.1). Each hospital's keys stay sealed
(SECURITY.md). Epic's code in `epic.rs` becomes the first variant.

### 2.5 Normalization stays in Rust, per site
Each new source gets a probe (`docs/architecture/probes/<site>.md`, by
`node scripts/fhir-probe.mjs`) and its rules in FHIR-SHAPES.md. The console still sees only the
normalized model.

### 2.6 The console
- Chat's source toggle gets one entry per sandbox.
- **Import from Epic** on Config › Hospital › Places becomes **Import from <EHR>**, behind each
  adapter (HOSPITAL-CONFIG.md §8).
- The hospital's lists (visit types, places, reasons) hold the codes of that hospital's EHR
  (§1). In Oracle a visit type is a Millennium code value and a `Schedule`; in athenahealth an
  `appointmenttypeid` and a department.

## 3. The order of work
1. Pull the standard FHIR core out of `epic_ehr.rs`; Epic becomes the first variant. The kept
   scripts stay green in Epic mode.
2. `Source` in two parts (§2.2), the token interface (§2.4).
3. The SMART sandbox as a source: patient match and the visit list (§4.1).
4. Booking on the standard path against a server we can book on (§4.2).
5. Then Oracle Health (§6), on the core.

## 4. Testing
Every source goes through record and replay like Epic's (VOICE-AGENT.md §9), and the kept
scripts run once per source.

### 4.1 The SMART sandbox (checked 2026-11-15, read only)
`https://r4.smarthealthit.org` runs Smile CDR 2019.08 (built on HAPI FHIR), open with no sign-in.
It held 795 patients, 23,563 appointments, 186 schedules and 598 slots. Of the slots, 491 were
free, and all of them were in the past: the latest started 2026-10-29. A search from today finds
none. It is good for patient match and visit reads on a non-Epic data shape.

### 4.2 Booking on the standard path
Booking an old slot works for a test, with two limits:
- **Our code searches from today forward.** A test needs a clock that the source can set back
  (a test-only "today" for the call). It must never exist in production.
- **A plain FHIR server only stores what we send.** The SMART server and a local HAPI server do
  not run scheduling rules: no double-booking check, and the slot is not marked busy. So a test
  there proves our requests, not the EHR's answer. Oracle's sandbox runs real Millennium rules
  (to confirm), which is a reason to bring it in early (§1).

Where to write:
- **The SMART server** is public and shared. Other developers read the same data, and it can be
  reset at any time. A write there is the owner's call each time; Claude made no writes.
- **A local HAPI FHIR server** (Synthea patients, our own future slots), started by
  `npm run dev` like Postgres, is ours to write and reset. LOCAL-EHR.md has a related idea in
  Epic's shapes.

## 5. To find out
- Oracle's sandbox: whether it books, and whether it rejects a taken slot (§4.2).
- Oracle's backend sign-in for a system with no user (§6).
- Whether a test clock (§4.2) is enough, or the standard path needs our own slots.

## 6. Oracle Health (Cerner Millennium)
Facts read 2026-10-19 from vendors' docs and integrators' write-ups; developer-program fees were
not found for any vendor.
- **Registration:** the Oracle Health **Code Console**. Plan the use cases and the FHIR resources
  first; the app uses OAuth 2.0 with a **JWKS** for backend access (the same key setup as Epic,
  EPIC.md §2).
- **Booking:** FHIR R4 `POST /Appointment` and status updates are documented, with an
  is-reschedulable flag. Each hospital chooses which providers and locations are open to booking.
  Write scopes may need the hospital's extra approval.
- **The marketplace:** a listing in the Oracle Healthcare Marketplace needs validated-integration
  reviews (security, compliance, interoperability).
- **To find out:** a sandbox with booking; the backend-access flow for a system with no user.

## 7. athenahealth (athenaOne)
- **Its market is outpatient practices, not hospitals.** It fits if clinic groups become
  customers.
- **Booking is its own REST API, not FHIR:** find open slots, book, cancel, reschedule. Its FHIR
  `Appointment` is mainly for reading. So its adapter speaks two APIs.
- **The marketplace:** athenaOne Marketplace has a certification process; steps and fees not
  found.
- **To find out:** the partner program's terms and a developer sandbox.

## 8. MEDITECH (Expanse)
- Scheduling APIs are listed for Expanse customers, connected customer by customer. Write support
  is not confirmed. A developer workspace (Greenfield) exists for FHIR reads.
- **To find out:** everything; the source found is old.

## 9. Sources
- [Oracle — FHIR R4 Appointment](https://docs.oracle.com/en/industries/health/millennium-platform-apis/mfrap/api-appointment.html)
- [Oracle Health — Developer, provider](https://www.oracle.com/health/developer/provider/)
- [Thinkitive — Cerner EHR integration](https://www.thinkitive.com/blog/cerner-ehr-integration-guide/)
- [6B — Oracle Health API integration](https://6b.health/insight/oracle-health-cerner-api-integration/)
- [athenahealth — Appointment](https://mydata.athenahealth.com/static/apidoc/ig-ge-appointment.html)
- [Taction — athenahealth API guide](https://www.tactionsoft.com/blog/athenahealth-api-integration-guide/)
- [Nirmitee — FHIR scheduling APIs per EHR](https://nirmitee.io/blog/fhir-scheduling-api-appointment-slot-epic-oracle-athenahealth/)
- [SMART Health IT — the R4 sandbox](https://r4.smarthealthit.org/metadata) (read 2026-11-15)
