---
status: archived
desk_test: none
sources: [src/layout.ts, src/cards.ts, src/record-cards.ts]
updated: 2026-10-13
---
# DeetsHealth — Home and hubs: the portal's layout

> The owner's direction (2026-10-13): "Home style for the patient's home with drilldowns for
> various things which lead to different cards. I like cards being different hubs and Home
> unites it for a patient." This is the paper layout of that idea. Nothing here is built.
> It replaces SURFACES-AND-CARDS.md §2's card list once the forks close. The surfaces
> (Midi, Max) and the card engine stay as built.

## 1. Terms
- **Home** — the card that shows one patient's whole record at a glance, in shelves.
- **Shelf** — one titled row on Home, as on DeetsMusic's Home (`../DeetsMusic/docs/features/HOME.md`):
  "Recent results", "Active medicines". A shelf can be moved by holding its label.
- **Hub** — a card that owns one kind of thing: Visits, Results, Medicines, Problems,
  Allergies. A hub has levels.
- **Level** — a page inside a hub, in DeetsMusic's style (`../DeetsMusic/docs/features/FULL-LIB.md`):
  a hero on top, rows under it, Back to the level before. A hub's top level is its list; a
  drill opens one thing.
- **Drill** — a press on a tile or a row that opens its owner: a result tile on Home opens
  the Results hub at that test's level.
- **Drill swap** — the built move in `layout.ts`: the target card takes the slot, and Back
  brings the first card back (SURFACES-AND-CARDS.md §3).

## 2. The shape
```
 Midi (one column)                      Max
 ┌─────────────────────────┐            ┌──────────────┬──────────────────────────────┐
 │ title bar  (who ▾)  ⚙   │            │ title bar    (who ▾)                       ⚙ │
 ├─────────────────────────┤            ├──────────────┼──────────────────────────────┤
 │ HOME                    │            │ HOME         │ the hub stage                │
 │  hero: name, age, sites │            │  hero        │  Results › Hemoglobin A1c    │
 │  New since last look ▸  │  drill ──▶ │  shelves ▸   │  hero: latest, site's range  │
 │  Recent results ▸       │  (swap)    │  …           │  trend                       │
 │  Upcoming visits ▸      │            │              │  every value, with its visit │
 │  Active medicines ▸     │            │              │                              │
 │  Problems ▸             │            │              │  ◀ Back                      │
 └─────────────────────────┘            └──────────────┴──────────────────────────────┘
```
- **Midi:** Home fills the window. A drill swaps the hub into Home's slot. Back, or Esc, brings
  Home back where the user left it (card memory).
- **Max:** Home stays in the left column. The hub opens in the stage on the right (fork H3).
- **The patient switcher** ("who ▾") sits in the title bar: one Home per person (fork H5). A
  parent with two children sees three names.

## 3. Home
- **Hero:** the name, the age (computed at run time, never stored: HANDOFF.md › Decided 8.1b),
  and a chip for each site the record comes from, with its last fetch. The name reaches the
  page through `textContent` only.
- **Shelves** (a draft for the owner to edit, §7), each one row that scrolls sideways, each tile a drill:
  1. **New since last look:** any kind, newest first. Needs a sealed "seen" mark per site,
     which is an additive schema change.
  2. **Recent results:** the latest value per test, with the site's own range when it gives one.
  3. **Visits:** upcoming (from `Appointment`) first, then the recent ones.
  4. **Active medicines.**
  5. **Problems:** active.
  6. **Allergies:** all. No allergy recorded and "no known allergies" are two different
     states, and the tile says which (DESIGN.md §1: a missing value is a designed state).
- An empty shelf is left out, as on DeetsMusic's Home. The order is the user's (movable
  shelves).

## 4. The hubs and their levels
| Hub | Top level (the list) | Drill → level | From that level |
|---|---|---|---|
| **Visits** | every visit, upcoming first, then by date | **a visit**: hero = date, type, place, clinician; rows grouped by kind = everything the record links to it (results, orders, diagnoses, procedures, notes) | each row drills to its own hub |
| **Results** | tests, the latest value of each (labs, vitals, reports; split pill by kind) | **a test**: hero = the latest value and range; a trend; every value with its own range and its visit | a value → its visit |
| **Medicines** | Active \| Past (split pill) | **a medicine**: the dose text built from its parts (Decided 8.3a), the prescriber, every order | an order → its visit |
| **Problems** | Active \| Resolved | **a problem**: onset, status, the visits the record links to it | a visit |
| **Allergies** | every allergy | **an allergy**: substance, reactions, criticality, when it was recorded | its visit, when linked |

- **Later hubs:** Immunizations, Documents (notes), Care team and plans. They get their own
  hub when their data is in, one at a time (§7).
- **Links come from the record only.** A level shows a link the site sent (the link table,
  INDEX-AND-LINKS.md). It never guesses one: a result is never tied to a problem by our own
  inference (DESIGN.md §5).
- **Each hub is a collection card**, so Sort, View, Search, the grown card's columns, the
  keyboard and card memory come with it. This brings the collection-card engine and
  `media-levels`' level pattern across from DeetsMusic. Both were left out of the scaffold.

## 5. Designed for writes (not built)
The scope is read-only now, with writes designed for later (HANDOFF.md › Decided 2026-10-13).
- **The model:** each normalized item keeps its site, its FHIR id and its `meta.versionId`. A
  later write (a message about a result, an appointment request) can then name its target
  and refuse a stale one.
- **The hub head:** a later action ("Message about this") belongs in the hub head, in the icon
  square family. Nothing is shown before it works (LESSONS.md §3.5: no control without an
  action).
- **Who may write:** always the person signed in to the site, with their own token. A
  delegate never writes with another person's token (ideas/IDENTITY-WORKER.md).

## 6. Every platform
- **Web:** the same cards. A level fits one browser tab as it fits Midi.
- **iPhone:** a phone is Midi's shape: Home, a drill swaps in the hub, Back returns. The
  level pattern suits it best of the shells we looked at.
- **macOS:** as Windows. Esc and Back as here; Cmd in place of Ctrl.

## 7. Forks (for the owner)
The owner asked how many of the first six forks were real (2026-10-13). Four were not; they
follow from his words or a decided rule:
- **The Patient card becomes Home's hero.** His words: "Home style for the patient's home".
- **The tile is DeetsMusic's shelf tile, re-drawn for facts.** "Home style" means DeetsMusic's
  Home, and its shelves are tiles (LESSONS.md §4: take the literal reading).
- **The first hubs are Visits, Results, Medicines, Problems, Allergies.** All eight at once
  breaks the one-feature rule (CLAUDE.md › Working style). The others follow, one at a time.
- **Home's shelves** (§3) are a draft for him to edit, not a fork.

The real forks:

**H3. Where a drill opens in Max.**
- **A (recommended).** One hub stage on the right. A drill replaces the hub that is there, and
  Back walks the levels. Home stays on the left.
- **B.** Each hub keeps its own slot in the 2×2 grid. A drill brings that hub forward in its
  slot. More on screen at once, but four hubs at a quarter size each.

**H5. One Home per what?**
- **A (recommended).** **Per person.** At sign-in, the app asks "Whose record is this?": an
  existing person or a new one. Two health systems for one person go under one Home, each
  shelf showing both. Merging duplicates (one medicine from two sites) is later work. A new
  sealed list of people, outside the site stores.
- **B.** **Per site.** One Home per sign-in. Simple. A person with two health systems has two
  Homes.
