---
status: project
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — The endpoint directory

> The list of real sites a user can connect to, where it comes from, what it looks like, and
> how it reaches the app. Nothing here is built. Open forks: §6.

## 1. Terms
- **Brand** — a health system as a patient knows it ("primary brand" in Epic's file). One
  brand has one FHIR endpoint.
- **Location** — a clinic or hospital under a brand (an `Organization` with `partOf`).
- **Endpoint** — a FHIR base URL. Several brands can share one.
- **Worker** — a small static host that we run; it serves one public file and holds no
  patient data.

## 2. Why the app needs it
Epic has no central sign-in. Each health system runs its own FHIR server and its own MyChart,
and Epic never tells an app which one a user belongs to. The user picks their health system
in the app, and sign-in goes to that system's MyChart (CORE-CRATE.md §6). A patient seen at
three systems connects three sites (DATA-MODEL.md §7.5 `directory`).

## 3. The source
- `https://open.epic.com/Endpoints/Brands` — a FHIR R4 `Bundle` (type `collection`), the SMART
  "User-access Brands" format. No sign-in. The R4 and DSTU2 endpoint bundles and the JSON
  file are deprecated.
- **Epic's terms:** apps should not query it at runtime; download it regularly (for example
  weekly) and re-host it.
- Other certified EHR vendors must publish the same format under the Cures Act, so the same
  reader serves them later.

## 4. Its shape (read 2026-09-21)
One file, 94.9 MB, 97,743 entries. Each entry is `fullUrl` + `resource`. The `fullUrl` is a
bare id, and every reference in the file is that bare id. The bundle's `identifier` carries
the totals as extensions (`https://open.epic.com/brands-bundle-total-*`).

| Kind | Count | Fields (share) |
|---|---|---|
| `Organization`, primary brand | 1,278 | `name`, `active`, `type` = `prov`, `endpoint[]` (always exactly 1), `identifier[]` (100%: Epic's `brand-identifier`; NPIs, 27,251 in all), `address[].state` and `.country` (79%; the rest carry `data-absent-reason = unknown`) |
| `Organization`, location | 95,645 | `name`, `active`, `partOf` (100%, always a primary brand), `address[]` with `line`, `city`, `state`, `postalCode`, `country`, `text`, `type` = physical (100%), `meta.lastUpdated` |
| `Endpoint` | 820 | `address` (always ends `/R4/`), `name`, `status` = active, `connectionType` = `hl7-fhir-rest`, `payloadType` = none, 4 mime types, the `endpoint-fhir-version` extension = 4.0.1, `managingOrganization` (Epic's organization id), `contact[]` (a url), `period.start` |

**How they link:** location → `partOf` → brand → `endpoint[0]` → Endpoint.
- 1,018 brands have locations: 349 have one, 444 have more than 20.
- 721 endpoints serve one brand. The rest are shared, up to 42 brands on one endpoint.
- Hosts: 250 endpoints on `epichosted.com`, 216 on `ochin.org`, the rest on 275 other domains.

**What is not in it:** no logos, no patient-portal links, no brand aliases. The SMART format
allows these; Epic's file does not fill them.

## 5. How it reaches the app (the owner, 2026-09-21)
- **A worker hosts it statically**, refreshed weekly. It downloads Epic's file, trims it to
  what the picker needs, and serves one compressed file.
- **Size:** trimmed to brand name, state, endpoint URL, and each location's name, city, state
  and ZIP, it is 6.8 MB raw and 1.06 MB brotli-compressed. Brands only: 0.02 MB.
- **Privacy:** every install downloads the same whole file, so the download tells the worker
  nothing about the user. Search runs inside the app. A search term never goes to the worker
  (SECURITY.md §4). The user's chosen site is protected data and is sealed (§4.1).
- **Deploying the worker is a stop rule** (LESSONS.md §2, rule 2). The release path stops
  before any worker (HANDOFF.md › Decided); this worker is built on paper first and deployed
  only with the owner's word.

## 6. Open forks
1. **What the picker searches** — brands only, or brands and their locations ("the clinic on
   Main Street"). Sets the file size (0.02 MB or 1.06 MB). Brought with the picker's design.
2. **The worker's host and the app's fallback** when the worker cannot be reached (for
   example, a copy shipped with each release). Brought when real sites come.
