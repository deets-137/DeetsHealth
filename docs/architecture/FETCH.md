---
status: built
desk_test: open
sources: [core/src/sync.rs, core/src/fhir.rs, core/src/net.rs, archive/src-tauri/src/fetch.rs]
updated: 2026-09-24
---
# DeetsHealth — The fetch

> How the app downloads a patient's record from a site, step by step, with every value that
> can be changed and where. The twin of [SIGN-IN.md](SIGN-IN.md). The design history is
> CORE-CRATE.md §7 and §12 (stage 5); where the rows go is [STORE.md](STORE.md); what the
> fetch reads from each resource is [INDEX-AND-LINKS.md](INDEX-AND-LINKS.md). Settings ›
> Sites shows the fetch and starts it again (stage 7, CORE-CRATE.md §12).

## 1. Terms
- **Site** — one health system's FHIR server, with its own base URL.
- **Capability** — the site's `CapabilityStatement`, read from `<base>/metadata`. It lists
  the types the site serves and the search parameters of each type.
- **Job** — one search: one FHIR type (or one Observation category) for this patient, with
  all of its pages.
- **Page** — one reply to a search: a `searchset` Bundle with up to `PAGE_SIZE` matches, the
  `_include`d resources, and a `next` link when there are more pages.
- **Worker** — one of the `PACE` loops that take jobs from the list, one at a time.
- **Warning** — one row in `site_warning`: an `OperationOutcome` issue from the site, or a
  fixed code of ours when a job stops (§7).

## 2. What the user sees
1. After a sign-in (or a press of **Refresh** in Settings › Sites), the Sites row counts
   up: "Fetching… 640 results · 12 medicines".
2. When the fetch ends, the row shows the counts and "Updated 2 min ago" (CORE-CRATE.md §2, S6).
   A kind with no records is left out of the line; with none at all it reads "No records yet".
3. If the site refuses the refresh token, the row says "Sign in again", and its left half
   becomes **Sign in**. The fetch drops both dead tokens, so this lasts across a restart.
4. If the fetch fails, the row says "Could not reach this site. Refresh to try again."

The page side is `src/sites.ts` (state and events) and `src/settings-card.ts` › Sites.

## 3. The flow in the code
| Step | Where | What happens |
|---|---|---|
| 1 | `signin::on_link` (after a sign-in) or `fetch::site_refresh(site)` (command) | Calls `fetch::start(app, site)`. |
| 2 | `fetch::start` | Refuses a second fetch for the same site (`Running`). Runs the fetch on a blocking thread (`spawn_blocking` + `block_on`), because SQLite and sealing block. Returns at once. |
| 3 | `fetch::run` → `open_current` | Opens the site's store (`SiteFiles::open`; an old schema is migrated here, STORE.md §6). Re-indexes it when its index is older than `INDEX_VERSION` (INDEX-AND-LINKS.md §10). Logs `fetch: started`. Calls `sync::full_fetch`. |
| 4 | `sync::full_fetch_paged` | Reads the site row. If the token runs out within 2 minutes, `auth::refresh`. `SignInAgain` ends the fetch. |
| 5 | same | GET `<base>/metadata` with no token. Not 2xx → the fetch fails. `fhir::read_capability` picks the searchable types (§5.1). `read_includes` picks the `_include`s (§5.3). The capability and the software version go into the site row. |
| 6 | same | GET `Patient?_id=<id>&_elements=name,birthDate,gender`. `fhir::read_patient` keeps name, birth date and sex and drops the rest. A reply that is not 2xx is skipped; the fetch goes on. |
| 7 | same | Makes the job list (§5). Clears the old warnings (`clear_warnings`). |
| 8 | `worker` × `PACE` | Each worker takes the next job and runs it (`run_job`). All workers run on one task (`futures_util::join_all`); there are no extra threads. |
| 9 | `run_job` | GETs each page with retries (§6). Reads it (`fhir::read_page`), writes it in one transaction (`write_page`), stores its issues, sends progress. Follows `next` while it stays on the site. |
| 10 | `write_page` | Each resource replaces its earlier row (same FHIR type and id) or gets a new row. Its index columns and links are written in the same transaction (INDEX-AND-LINKS.md §7). |
| 11 | `full_fetch_paged` | When all jobs end: `resolve_links` points each waiting link at its target, in one transaction. Writes `last_fetch` and `last_full_fetch` (seconds since 1970, sealed). |
| 12 | `fetch::run` | Logs the summary (§9). Sends `app-error` for a failure or a partial fetch (ERRORS.md), then `fetch-done`. |

**Where each resource goes.** A match goes to its job's table. A resource of another type
(an `_include`, or a type the site adds on its own) goes to the table of its own type
(`fhir::table_for`). A type with no table is skipped. A resource with no `id` is skipped.

## 4. What to change, and where
| To change | Where | Today | Notes |
|---|---|---|---|
| Jobs at once, per site | `sync::PACE` | 3 | A stewardship choice (fork 5a). Pages inside one job run in order, so the longest page chain sets the floor. |
| Resources per page | `sync::PAGE_SIZE` | `"100"` | The `_count` value. A site may send fewer. The live test uses 20 (`full_fetch_paged`). |
| Tries per request | `sync::TRIES` | 4 | For a network error, 429, 500, 502, 503, 504. |
| Longest `Retry-After` we wait | `sync::MAX_WAIT_S` | 60 s | A longer value from the site is cut to this. |
| Back-off with no `Retry-After` | `sync::get` | 1, 2, 4 s | `1 << (attempt - 1)`. |
| `_include`s we ask for | `sync::WANTED_INCLUDES` | 8 (§5.3) | Sent only when the site lists them in `searchInclude`. |
| Observation categories | `fhir::OBSERVATION_CATEGORIES` | FHIR's 9 | `laboratory`, `vital-signs`, `social-history`, `survey`, `exam`, `imaging`, `procedure`, `therapy`, `activity`. |
| How a type is tied to the patient | `fhir::PATIENT_PARAMS` | `patient`, `subject`, `beneficiary`, `member` | The first one the site lists wins. |
| Which types we fetch | `fhir::table_for` | 93 types | A type with no table is never fetched (INDEX-AND-LINKS.md §3). |
| What the scopes allow | `fhir::scope_allows` | SMART v1 and v2 | `patient/<type or *>.read`, `.*`, or a v2 access with `r` and `s`. No scope list: all. |
| Patient fields kept | `sync::full_fetch_paged` (`_elements`) + `fhir::read_patient` | name, birth date, sex | FHIR-SHAPES.md §5, fork 1. |
| Refresh margin | `auth::REFRESH_MARGIN_S` | 2 min | SIGN-IN.md §4. |
| HTTP rules | `net::Net::new` | HTTPS only, TLS 1.2+, no redirects, gzip, 30 s | `Accept: application/fhir+json, application/json`. |
| Event payloads | `fetch::progress_event`, `fetch::start` | §8 | Counts and the site key only. |

## 5. The plan
### 5.1 Which types
A type becomes a job when all four are true:
1. The capability lists it, with `search-type` among its interactions (or no interaction list).
2. It has a table (`fhir::table_for`).
3. Its search parameters include one of `PATIENT_PARAMS`.
4. The granted scopes allow it (`fhir::scope_allows`).

### 5.2 One job per type, and the category fallback
> **Part:** built · 2026-09-21 (Observation), 2026-10-13 (CarePlan) · plain search first, then fall back (the owner's choice)

- Every type: one job, `<Type>?<patient param>=<id>&_count=100`.
- `Observation` is searched plain too, so an Observation with no FHIR category is not
  missed. When the site lists `category` as a search parameter, the job carries 9
  **fallback** jobs, one per FHIR category (`&category=<code>`).
- **The fallback runs** when the site refuses the plain search's first page: any 4xx but
  401 and 429, or a page with no resources and an `error` or `fatal` issue. The fallback jobs
  go to the end of the list and `jobsTotal` grows by 9. No warning is stored: the refusal is
  expected on such a site (Epic).
- **`CarePlan` too** (2026-10-13): Epic refuses a plain CarePlan search ("This resource
  requires a category for searching") and names ten categories, so CarePlan carries 10
  fallback jobs (`fhir::CAREPLAN_CATEGORIES`; ops/DEBUGGING.md §7). The table of types and
  their categories is `fhir::category_fallback`.
- **A refused category is "not served", not a failure** (2026-10-13). Epic's sandbox accepted
  only 2 of the 10 CarePlan categories (longitudinal, encounter) and refused 8 with
  `fatal:invalid`. A fallback job whose first page gets a 4xx (not 401 or 429) ends quietly:
  ledger outcome `category-not-served`, no warning, the type not marked incomplete, no toast.
- The SMART sandbox accepts the plain search: 1 job instead of 9.
- **On a site that refuses the plain search** (Epic), an Observation in none of the 9
  categories is still missed. Epic has categories of its own; check the list when Epic comes
  back (§12).

### 5.3 The `_include`s
`WANTED_INCLUDES` names the references the cards show, so they come in the same call, never
one read per row (LESSONS.md §3.3):

| Include | Brings |
|---|---|
| `MedicationRequest:medication` | the `Medication` (the drug) |
| `MedicationRequest:requester` | who ordered it |
| `MedicationDispense:medication`, `MedicationAdministration:medication`, `MedicationStatement:medication` | the drug |
| `Encounter:participant` | the visit's care team |
| `Encounter:service-provider` | the organization |
| `Encounter:location` | the place |

A job asks only for the includes that start with its own type, and only those the site lists
in that type's `searchInclude`.

## 6. Retries and stops
| Reply | What the job does |
|---|---|
| 2xx | Reads the page. Goes on. |
| Network error, 429, 500, 502, 503, 504 | Waits and tries again, up to `TRIES` in all. The wait is `Retry-After` (at most 60 s), or 1, 2, 4 s. |
| 401, the first time in this job | Renews the token once (`renew`) and tries the same page again. If another worker renewed it already, it uses the new token and does not renew again. |
| Refresh answers `SignInAgain` | Every worker stops taking jobs. The fetch ends with "sign in again". |
| Refresh fails for another reason | The job stops. Warning `token-refresh`. |
| The first page of a job with fallback jobs is refused (§5.2) | The job ends, and its fallback jobs are queued. No warning. |
| Any other status, or the same status after the last try | The job stops. Warning `http-<status>`, plus the site's own `OperationOutcome` issues. The type goes into the "refused" list. |
| Network error after the last try | The job stops. Warning `network`. |
| A page that is not a Bundle | The job stops. Warning `unreadable-page`. |
| A page that will not seal or store | The job stops. Warning `seal` or `store`. The page's transaction rolls back. |
| A `next` link that does not start with the base URL | The job stops. Warning `next-link-off-site`. The token never goes to another host. |

The whole fetch fails only when the site row is missing, `/metadata` fails, the capability
or the patient cannot be read, or the final link pass cannot be written. Every other failure
stops one job, and its type goes into `Summary.stopped`. A fetch that ends with any stopped
type sends `app-error` `fetch-partial` with the number of types (ERRORS.md §3).

## 7. Warnings
Every warning is a row in `site_warning`: the FHIR type of the job, a severity, a code, a
text, and when (STORE.md §3). All are sealed. A full fetch clears the old warnings first.

| Code | Severity | From | Meaning |
|---|---|---|---|
| any FHIR issue code (e.g. `suppressed`, `processing`) | the site's | an `OperationOutcome` in a page, or in an error reply | The site says something about this search. `text` holds its `diagnostics` or `details.text`. |
| `http-<status>` | `error` | ours | The site refused the search (e.g. `http-403`). |
| `network` | `error` | ours | No reply after every try. |
| `unreadable-page` | `error` | ours | The reply was not a Bundle. |
| `next-link-off-site` | `error` | ours | The site's `next` link left the base URL (§13). |
| `token-refresh` | `error` | ours | A 401 came, and the token could not be renewed or saved. |
| `seal` | `error` | ours | The cipher failed on a value. |
| `store` | `error` | ours | SQLite failed on a write. |

A card can use the site's warnings to say "the site withheld some results", so a gap looks
like a gap (DATA-MODEL.md §7.3).

## 8. Events
Both carry counts and the app's own random site key only (PATIENT-APP-SECURITY.md §4.3).

| Event | Payload | When |
|---|---|---|
| `fetch-progress` | `{ site, counts: { results, medicines, allergies, conditions, visits }, jobsDone, jobsTotal, calls }` | After every page, and after every job. |
| `fetch-done` | `{ site, end }`, `end` = `done`, `sign-in-again` or `failed` | Once, at the end. |
| `app-error` | `{ code, kind?, site?, count? }` | On every failure, and on a partial fetch (ERRORS.md). |

The types are `FetchProgress` and `FetchDone` in `core/src/payload.rs`; their TypeScript is
generated into `src/model/`. `counts` are the five words of the Sites row, made from the
counts by FHIR type (`SiteCounts::from_types`, CORE-CRATE.md §12 stage 7). They count this
fetch only, so a Refresh starts again at 0. The log line keeps the counts by FHIR type.

## 9. Log lines
The log is `%APPDATA%\com.deetshealth.dev\deetshealth.log` (debug) or
`%APPDATA%\com.deetshealth.app\deetshealth.log` (installed). Every fetch line starts `fetch:`
and carries counts, FHIR type names and error kinds only.

| Line | Meaning | Next step |
|---|---|---|
| `started` | The store opened; the fetch runs. | — |
| `already running for this site; not started again` | A second start while one runs. | Wait for `done`. |
| `call <what> p<n> · <status> · …` | One per HTTP call: the call ledger (ops/DEBUGGING.md §3). | A `refused` line's issue words say why. |
| `done: N resources, T types, L links (W waiting), C calls, S s, X warnings, key reads K ms` | The fetch ended. `W waiting` = links to a resource the site never sent. | — |
| `types: …` · `waiting links point at: …` · `warnings: …` | The fetch summary: kinds and counts (ops/DEBUGGING.md §4). | `npm run inspect` for more. |
| `the site refused: <types>` | These jobs stopped on an HTTP status. | Read the warnings; check the scopes. |
| `incomplete: <types>` | These jobs stopped early, for any reason. | Refresh; read the warnings. |
| `the site refused the refresh token; sign in again` | The refresh answered 400 or 401, or there is no refresh token. | Sign in again. |
| `failed: <kind>` (warning) | The whole fetch stopped. `network error`, `unexpected reply`, `storage error`, `crypto error`, `written by a newer version`. | §11. |

Lines from opening a site (`fetch::open_current`, also at app start):

| Line | Meaning |
|---|---|
| `site: the key is gone; the store cannot open` (error) | No key file. STORE.md §9. |
| `site: the store would not open: <kind>` (error) | DPAPI, the file, the migration, or a newer schema. STORE.md §9. |
| `reindex: N rows, L links (W waiting)` | The index was stale and is rebuilt. |
| `reindex: failed: <kind>` (warning) | The old index stays; the next open tries again. |
| `sites: N checked at start` | The start-up check is done. |

## 10. Measured cost
On the SMART sandbox, 2026-09-21 (CORE-CRATE.md §12, stages 5–6):
- The site serves 74 patient-searchable types; 66 of them have a table.
- Stage 5, with 9 Observation jobs: **76 jobs, 78 calls** at 100 a page; 5–7 s for 94–397
  resources.
- **With the plain Observation search** (after the owner's fix, 2026-09-21): **68 jobs, 70
  calls** at 100 a page (1 `/metadata`, 1 patient, 1 call per job); 10–11 s for one patient
  of 163 resources (90 Observations), 223 links, 0 waiting. The time is the site's; the call
  count is ours.
- Row-key reads (matching a resource to its row): under 1 ms.

**The cost formula:** 2 calls, plus 1 call per page of each job, plus retries, plus at most
one token refresh per 401. A type with 0 resources still costs 1 call. A site that refuses
the plain Observation search costs 1 more call, then 9 category jobs. Epic's record will
have more pages; measure it when Epic comes back.

## 11. When it goes wrong
| Symptom | Likely cause | Fix |
|---|---|---|
| `fetch: failed: network error` at once | `/metadata` did not answer 2xx. | Check the base URL and the network. Refresh. |
| `fetch: failed: unexpected reply` | `/metadata` is not a `CapabilityStatement`, or the patient reply is not a `Patient`. | Look at the site's reply shape with `scripts/fhir-probe.mjs`. |
| `fetch: failed: crypto error` or `storage error` | The key does not open the file, or SQLite failed. | STORE.md §9. |
| A toast after every fetch: "Some records could not be updated" | `fetch-partial`: some jobs stop each time. | Read `fetch: incomplete:` and the warnings. |
| `the site refused: X` | The scopes do not include X, or the site withholds X. | Compare the granted scopes (`site.scopes`) with the type. |
| A type is missing, with no warning | The capability does not list it, it has no table, it has no patient parameter, or the scopes do not allow it (§5.1). | Read the capability; check `table_for` and `PATIENT_PARAMS`. |
| Some Observations are missing | The site refused the plain search, and they have none of the 9 FHIR category codes (§5.2). | Known gap on such a site. |
| A `token-refresh` warning | A 401 came, and the token could not be renewed or saved. | Refresh again. |
| The row counts stay the same after a fetch | Correct: a resource replaces its row. | — |
| Rows the site deleted are still here | A full fetch does not remove rows yet. | Stage 8. |

## 12. Changing it later
- **Stage 8:** refresh on app open when the last fetch is more than 1 hour old; the small
  update (`_lastUpdated` after the last fetch); a weekly full refresh that removes rows the
  site deleted. (The sign-out erase came early, in stage 7: STORE.md §8.)
- **Epic:** measure the pages and the time. Check that Epic refuses the plain Observation
  search in a way §5.2 sees, and add Epic's own Observation categories to the fallback.
- **`Binary`, `$summary`, `$docref`:** not fetched yet (DATA-MODEL.md §7.4).
- **The web:** the core runs as it is. `Timer` and the HTTP client come from the browser.
  The browser sets its own TLS rules; CORS on each site is the open question.

## 13. Security, in one place
- The token, the `Patient` id and the base URL go only to the site they came from, through
  the network module's pass (PATIENT-APP-SECURITY.md §4.2).
- No redirects are followed, and a `next` link must stay on the base URL, so the token never
  reaches another host. `net::on_site` parses both URLs: the same scheme, host and port, no
  user name, and a path at or under the base path, split at a `/` (so `/fhir2` is not under
  `/fhir`). Fixed 2026-09-21: the old text-prefix check let
  `https://fhir.example.org.other.net` pass for a base of `https://fhir.example.org`.
- Each reply is read inside `net::parse_reply`, and every kept value is shielded there.
- Each page is written in one sealed transaction (STORE.md §5).
- Events and log lines carry counts, FHIR type names and error kinds only.

## 14. Tests
- `cargo test -p deetshealth-core --features sqlite`: the table map, the capability (the
  patient parameter, `search-type`, `category`), the scopes (v1 and v2), a page with
  matches, includes and an outcome, the trimmed patient, the re-index, and the `next`-link
  rule (`net::tests`: the base itself, a path boundary, scheme, port, user name, a base with
  no path and its look-alike host).
- `cargo test -p deetshealth --lib fetch -- --ignored --nocapture`: **the live fetch.** It
  signs in with no browser, fetches at 20 a page (so the `next` links are followed), checks
  that no warning was stored (every `next` link stayed on the site), the patient row, the
  capability, the links and the fetch times, then fetches again at 100 a page and checks
  that every table's row count did not change. About 165 calls with the sign-in.

## 15. Desk test
> Open. Built 2026-09-24; the whole Sites test is CORE-CRATE.md §12, "Stage 7 desk test".
1. Start `npm run dev:app` from your own terminal (SIGN-IN.md §6).
2. Settings › Sites › Connect a site › SMART Health IT sandbox. Pick a patient.
3. Expect the row to count up, then the counts and "Updated just now".
4. Press **Refresh**. Expect the same counts.
5. Read the log: the `fetch:` lines of §9, with counts and type names only.
