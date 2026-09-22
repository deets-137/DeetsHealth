---
status: built
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — Data architecture (the map)

> The path of a patient's data, from sign-in to the card, one step per line. Each step
> points to the doc that owns it. This file holds no rule of its own. Built: sign-in, the
> fetch, the index and links, the sealed store (CORE-CRATE.md §12, stages 1–6). Not built:
> the card models, the payloads, the Sites section, refresh and erase (stages 7–8).

## The path
| # | Step | State | Owner doc |
|---|---|---|---|
| 1 | The user picks a site (Settings › Sites). | stage 7 | CORE-CRATE.md §2 |
| 2 | Sign-in: SMART standalone launch in the system browser, PKCE and `state`, the `deetshealth://callback` link, the token trade. | built | [SIGN-IN.md](SIGN-IN.md) |
| 3 | A new site key, data key and SQLite file; the tokens sealed in the site row. | built | [STORE.md](STORE.md) §2, §5 |
| 4 | The fetch: `/metadata`, the patient, one job per type, 3 at once, every page. | built | [FETCH.md](FETCH.md) |
| 5 | Each resource: its table, its index columns and its links, in the same parse. | built | [INDEX-AND-LINKS.md](INDEX-AND-LINKS.md) |
| 6 | Each value sealed on its own, one transaction per page. | built | [STORE.md](STORE.md) §5–§6 |
| 7 | The rows as a woven record: the encounter at the center. | designed | [DATA-MODEL.md](DATA-MODEL.md) |
| 8 | Read time: the normalizer turns raw rows into the model a card needs. | card by card | CORE-CRATE.md §8 |
| 9 | One payload struct per card view, plain values, TypeScript types generated. | stage 7 on | CORE-CRATE.md §9 |
| 10 | The card DOM, through `textContent` only. | card by card | SECURITY.md §4.3 |
| 11 | Refresh (on open, weekly full) and the sign-out erase. | stage 8 | FETCH.md §12, STORE.md §8 |

The security path of one value, end to end, is SECURITY.md §8.2.

## 1. Terms
- **Site** — one health system's FHIR server, with its own base URL.
- **Client ID** — the ID a site gives the app on registration. The SMART sandbox accepts any.
- **PKCE** — the proof a public client (no secret) sends so a stolen code is useless.
- More terms: SIGN-IN.md §1, FETCH.md §1, INDEX-AND-LINKS.md §1, STORE.md §1.

## 2. Sign-in
Moved to [SIGN-IN.md](SIGN-IN.md) (the flow, the constants, the SMART launcher, the link on
Windows). The sandbox sites today: SMART Health IT (live) and Epic (on hold). Real sites
come later from Epic's endpoint directory
([ENDPOINT-DIRECTORY.md](../integrations/ENDPOINT-DIRECTORY.md)); every site-specific fact
comes from the site's `smart-configuration`, never from our code.

## 3. Tokens
Refresh, "sign in again" and where the tokens live: SIGN-IN.md §3 and §9. The tokens are
sealed in the site row with the site's own key (STORE.md §3), not in a separate file.

## 4. Fetch, normalize, cache
- The fetch: [FETCH.md](FETCH.md).
- What each row holds, and the links: [INDEX-AND-LINKS.md](INDEX-AND-LINKS.md).
- The cache on disk, sealing, keys and erase: [STORE.md](STORE.md). Sign-out deletes the
  site's data by design (SECURITY.md §4.7).
- The front end never sees raw FHIR, only payload structs (CORE-CRATE.md §9).

## 5. The PHI wall
The rules are SECURITY.md §4; the summary is SECURITY.md §8. In short: protected data (PHI
and PII) may exist only in `Shielded<T>` in memory, the sealed store, and the card DOM.
