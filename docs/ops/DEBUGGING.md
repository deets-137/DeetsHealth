---
status: built
desk_test: none
sources: [src-tauri/src/log.rs, src-tauri/src/fetch.rs, core/src/sync.rs, core/src/fhir.rs, core/src/dev.rs, src-tauri/src/dev_tools.rs, src-tauri/examples/inspect.rs, scripts/epic-redirect-check.mjs]
updated: 2026-10-13
---
# DeetsHealth — Debugging tools

> What the app writes down, and the tools that read it, so a fault can be explained without
> guessing. Modelled on DeetsMusic's set (`../DeetsMusic/docs/ops/DEBUGGING.md`,
> `LOGGING.md`), with one difference that shapes every tool: **the PHI wall** (SECURITY.md
> §4). The owner (2026-10-13): "Ensure we build good telemetry and debugging tools as we go".

## 1. Terms
- **The log** — `%APPDATA%\com.deetshealth.dev\deetshealth.log` (dev) or
  `…\com.deetshealth.app\…` (installed). Rolling, 512 KB, one old generation. Its wall
  (`log.rs` `scrub`) cuts every URL's query, every token and the user's folder.
- **Words** — what a tool may print for any site: FHIR type names, our own codes, FHIR's own
  issue words (from closed lists), numbers, sizes, times and JSON field paths. Never a value
  from a record, an id, a URL with a query, a name or a site's own text.
- **Sandbox site** — a site whose base URL is one of `auth::sandbox_sites()` (the Epic
  sandbox, the SMART launcher), compared shielded. Its data is public fake data, so a dev tool
  may show it in full (SECURITY.md §4.8; the owner confirmed 2026-10-13).
- **Dev feature** — the Cargo feature `dev` (core and host). Only the dev tools turn it on; a
  release never builds it.

## 2. The tools
| Tool | Where | Any site | Sandbox site | Built |
|---|---|---|---|---|
| The log | `log.rs` | words | words | scaffold |
| The call ledger (§3) | `sync::CallNote` → `fetch.rs` `call_line` | one line per FHIR call | the same | 2026-10-13 |
| The fetch summary (§4) | `sync::Summary` → `fetch.rs` | kinds, counts, link gaps, warning words | the same | 2026-10-13 |
| The inspector (§5) | `npm run inspect` | counts, links, warning words, shapes | + warning texts, the patient row, sample resources | 2026-10-13 |
| Epic redirect check | `node scripts/epic-redirect-check.mjs` | — | does Epic know our redirect URIs | 2026-10-13 |
| `__diag`, frames, the freeze watchdog | `diag.ts`, `frames.ts`, `watchdog.rs` | DeetsMusic's, cut to fit (diag logs an error's kind, never its message) | | scaffold |

Next (not built): Settings › Diagnostics for users (the last fetch in words, Copy a report),
after the layout work; a "Report a problem" form with the identity worker; an agent route only
once the wall is designed for it (DESIGN.md §3).

## 3. The call ledger
One `INFO` line per HTTP call of a fetch, as it ends (the numbers below are illustrative):
```
fetch: call metadata p1 · 200 · 211.4 KB · 640 ms · last-page
fetch: call Observation?category=laboratory p1 · 200 · 100 + 4 included · 180.2 KB · 1210 ms · issues warning:processing · next-page
fetch: call CarePlan p1 · 400 · 1.1 KB · 95 ms · issues fatal:business-rule · refused
```
| Part | Meaning |
|---|---|
| `what` | `metadata`, `Patient`, or the job: its FHIR type and our own extra parameters. Never the patient id. |
| `p<n>` | the page within the job |
| status | the HTTP status, or `no reply` after every try |
| `N + M included` | resources the search matched, and resources an `_include` brought |
| `KB`, `ms` | the reply's size, and the time for all tries |
| `tries` | shown when a retry happened |
| `issues` | the site's issues on that page as `severity:code#detail`: each part only when it is in FHIR's closed lists; `#n` only when the site's message code is all digits (Epic numbers its messages). Never the text. `fhir::issue_word` |
| outcome | `next-page`, `last-page`, `fallback` (the plain search was refused; categories follow), `category-not-served` (one fallback category refused: expected, no warning), `refused`, `token-renew`, `network`, `unreadable`, `store-failed`, `next-link-off-site` |

## 4. The fetch summary
After `fetch: done: …` (FETCH.md §9), three more lines:
```
fetch: types: AllergyIntolerance 1, CareTeam 1, Condition 7, …
fetch: waiting links point at: Encounter 250, Practitioner 291, …
fetch: warnings: CarePlan http-400 1, Observation warning:suppressed 18, …
```
A waiting link's target type is shown only when it is a FHIR type with a table; anything else
reads `other`.

## 5. The inspector — `npm run inspect`
Opens each site's sealed store with this PC's keys and prints the core's report
(`core/src/dev.rs`):
```
npm run inspect -- [--dir <app data>] [--site <key>] [--shapes] [--sample N] [--type T]... [--out <file>]
```
- Every site: the index version, the granted scopes (the site's own scope words), rows by
  table and FHIR type, links by table pair with the waiting ones split out, **the waiting links
  by target type with the number of distinct records they name** (the cost of fetching them),
  the warnings as words, and with `--shapes` each type's field paths with how many resources
  have each (`code.coding[].system: text  254/254`).
- A sandbox site also: the patient row, every warning's text (grouped, with a count), and with
  `--sample N [--type Observation]` whole resources, pretty-printed.
- **`--csv <folder>`, a sandbox site only** (the core refuses a real site): the whole record as
  CSV files in `<folder>\<site>\`, to model cards on (the owner, 2026-10-13). `records.csv` has
  one row per resource with its index columns (kind, code, display, status, dates);
  `<Type>.csv` has one column per JSON field path, several values joined with ` | `;
  `links.csv`, `warnings.csv`, `patient.csv`. The `row` column joins them. Write it under
  `dev-dumps\` in the repo (git-ignored):
  `npm run inspect -- --csv dev-dumps\epic-sandbox`.
- **Run it outside the Claude desktop app.** A process started inside it sees a private copy of
  AppData, with no sites in it (CLAUDE.md › How to verify). From a Claude session, start it
  through WMI with `--out`:
  ```powershell
  $exe = "<repo>\src-tauri\target\debug\examples\inspect.exe"
  Invoke-CimMethod Win32_Process -MethodName Create -Arguments @{ CommandLine = "`"$exe`" --shapes --out C:\Users\Public\deetshealth-inspect.txt" }
  ```
  Build it first: `cargo build -p deetshealth --features dev --example inspect`. Delete the
  output file when done; for a real site it holds words only, but it is still a file about a
  person's record.
- The pass behind it, `dev::DevPass`, is the fourth pass (shielded.rs). It exists only in a
  `dev` build. It is made for a sandbox site, and inside three helpers that let out FHIR's own
  words whatever the site: `shape_of`, `plain_type`, `warning_word`.

## 6. Recipes
**"How did the fetch go?"** Read the log from `fetch: started` to the summary lines. A
`refused` call names the job; its issue words say why. Then `npm run inspect` (sandbox: the
warning texts say it in the site's words).

**"Why is a card missing records?"** In the inspector: the type's row count; its warnings
(`suppressed` = the site withheld a category from this client); its ledger lines (`fallback`,
`refused`).

**"Why do the links not join?"** The inspector's waiting links by target type, and the distinct
records behind them. A type the site cannot search by patient (Practitioner, Location,
Organization, Medication) arrives only through an `_include` or a read (FETCH.md §5.3).

## 7. What the 2026-10-13 Epic fetch showed (Camila Lopez, the first run)
296 rows of 12 types; 32 calls; 8 s. From the inspector:
1. **CarePlan refused** (`http-400`, `fatal:business-rule`): Epic requires a `category` and
   names ten: longitudinal `38717003`, encounter `734163000`, outpatient `736271009`, inpatient
   `736353004`, dental `738906000`, oncology `736378000`, questionnaires due
   `719091000000102`, `inpatient-pathway`, education `409073007`, `care-path`. **Fixed the
   same day:** CarePlan falls back to one search per category, as Observation does
   (FETCH.md §5.2). The Refresh at 18:09 (the first run of the call ledger: 42 calls, 6 s)
   showed Epic's sandbox accepts only longitudinal (1 CarePlan) and encounter (none); the
   other 8 answer `fatal:invalid#59022`. Those refusals then raised a false "incomplete"
   toast, so a refused fallback category is now "not served" (FETCH.md §5.2), not yet run
   live. Epic's message numbers seen: `#4101` no results, `#4119` the standard
   "information available to the authorized user" note, `#59204` Outside Record not
   authorized, `#59205` SmartData Elements not authorized, `#59108` a required parameter is
   missing, `#59159` a category is required, `#59022` an invalid category, `#59134` the
   document list could not be built.
2. **"Client not authorized for … - Outside Record"** on 17 types: Epic withholds records from
   other health systems (Care Everywhere) from this client. **The Outside Record APIs are in the
   registration's Selected list** (the owner, 2026-10-13), so the cause is elsewhere: open.
   Candidates: Epic's sandbox may hold no outside records for its test patients, or the USCDI
   v3 distribution may not carry them.
3. **"User not authorized for Observation - SmartData Elements"** ×9: an Epic-specific
   Observation API this client does not have.
4. **"This response includes information available to the authorized user …"** on every type
   (`warning:processing`): Epic's standard note. Noise for a card; keep it as a warning.
5. **592 waiting links name 54 distinct records:** Practitioner 291 → 10, Encounter 250 → 2,
   Observation 25 → 25, Location 9 → 5, Condition 10 → 6, Organization 3 → 2, Specimen 2,
   Medication 1, ServiceRequest 1. The two Encounters behind 250 lab links are encounters the
   Encounter search does not return. Reading the 54 would cost at most 54 calls, fewer by `_id`
   search per type.
6. `Coverage`, `Device`, `ExplanationOfBenefit`, `MedicationDispense`,
   `QuestionnaireResponse`, `RelatedPerson`: "Resource request returns no results."
