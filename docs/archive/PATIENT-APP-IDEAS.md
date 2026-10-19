---
status: archived
desk_test: none
sources: []
updated: 2026-10-19
---
# Ideas for the patient app — not built

> **Archived 2026-10-19.** The patient app's idea list, kept as a record. The live list is
> [ideas/README.md](../ideas/README.md).

The docs in this folder describe features that do not exist in DeetsHealth. Do not tell a
user that the app can do any of these things.

- Proxy access (a parent or caregiver reads another person's record). Epic's own proxy sign-in
  already works with one site per patient; sharing inside DeetsHealth, with tiers, is
  [IDENTITY-WORKER.md](IDENTITY-WORKER.md).
- Several sites in one view (two health systems, one timeline).
- A MyChart file import (C-CDA XML) for people whose site has no working API.
- An on-device small model — Cactus Needle 3 (the owner, 2026-09-21). 8–29 MB, Apache-2.0,
  tool calling, structured extraction and embeddings, engines for Windows and the browser.
  Runs in-process, so the record stays on the PC. Candidate uses, lowest risk first: Compass
  words to a card and filter; search by meaning ("kidney" finds creatinine); suggesting
  groups for lab names with no code. Never for dose text or any value the user reads.
  Planned shape (the owner, 2026-09-21): a layer on Compass that handles ambiguous asks;
  typed Compass works without it.
- Lab companies as sites: Quest and Labcorp (2026-09-21). Neither has a documented open
  patient SMART API the way Epic does. Quest has a login-gated developer portal and a FHIR
  API for its Quanum EHR product; Labcorp has a patient registration page on
  `fhir.labcorp.com` (the host returned 503 when checked) and sends results to Apple Health.
  Most results already reach the ordering clinic's EHR, so the same result can arrive twice:
  de-duplication would be needed.
- Filling a missing reference range (the owner, 2026-09-21). Sites give a range for only
  about half of lab results, and ranges differ between hospitals. Any filled range must
  say where it came from and never pass as the site's own. Ranges depend on age and sex.
