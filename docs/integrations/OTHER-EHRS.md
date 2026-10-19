---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# The EHRs after Epic

> Not started. The wedge is hospitals not on Epic (VOICE-AGENT.md §7), so each of these gets an
> adapter behind the one EHR interface. Epic is [EPIC.md](EPIC.md). Facts from vendors' docs and
> integrators' write-ups, read 2026-10-19; developer-program fees were not found for any.

## 1. Oracle Health (Cerner Millennium)
- **Registration:** the Oracle Health **Code Console**. Plan the use cases and the FHIR resources
  first; the app uses OAuth 2.0 with a **JWKS** for backend access (the same key setup as Epic,
  EPIC.md §2).
- **Booking:** FHIR R4 `POST /Appointment` and status updates are documented, with an
  is-reschedulable flag. Each hospital chooses which providers and locations are open to booking.
  Write scopes may need the hospital's extra approval.
- **The marketplace:** a listing in the Oracle Healthcare Marketplace needs validated-integration
  reviews (security, compliance, interoperability).
- **To find out:** a sandbox with booking; the backend-access flow for a system with no user.

## 2. athenahealth (athenaOne)
- **Booking is its own REST API, not FHIR:** find open slots, book, cancel, reschedule. Its FHIR
  `Appointment` is mainly for reading. So its adapter speaks two APIs.
- **The marketplace:** athenaOne Marketplace has a certification process; steps and fees not
  found.
- **To find out:** the partner program's terms and a developer sandbox.

## 3. MEDITECH (Expanse)
- Scheduling APIs are listed for Expanse customers, connected customer by customer. Write support
  is not confirmed.
- **To find out:** everything; the source found is old.

## 4. Testing
Their sandboxes go through record and replay like Epic's (VOICE-AGENT.md §9). The first non-Epic
adapter that needs real FHIR writes brings back a local FHIR server (HAPI FHIR with Synthea).

## 5. Sources
- [Oracle — FHIR R4 Appointment](https://docs.oracle.com/en/industries/health/millennium-platform-apis/mfrap/api-appointment.html)
- [Oracle Health — Developer, provider](https://www.oracle.com/health/developer/provider/)
- [Thinkitive — Cerner EHR integration](https://www.thinkitive.com/blog/cerner-ehr-integration-guide/)
- [6B — Oracle Health API integration](https://6b.health/insight/oracle-health-cerner-api-integration/)
- [athenahealth — Appointment](https://mydata.athenahealth.com/static/apidoc/ig-ge-appointment.html)
- [Taction — athenahealth API guide](https://www.tactionsoft.com/blog/athenahealth-api-integration-guide/)
- [Nirmitee — FHIR scheduling APIs per EHR](https://nirmitee.io/blog/fhir-scheduling-api-appointment-slot-epic-oracle-athenahealth/)
