---
status: built
desk_test: passed 2026-09-21
sources: [src/surface.ts, src/layout.ts, src/cards.ts, src/record-cards.ts, src/card-grow.ts, index.html]
updated: 2026-09-21
---
# DeetsHealth — Surfaces and cards

## 1. Surfaces
Two, from DeetsMusic's three. The shapes are DeetsMusic's, with the **Patient card** where
DeetsMusic has Now Playing and the **Timeline card** where it has the Queue (the owner,
2026-09-21):
- **Midi** — the default window. The Patient card is anchored on top, full width. Under it,
  two content slots side by side (`left`, `right`). Defaults: Labs | Medications.
- **Max** — the stage column on the left: the Patient card over the anchored Timeline card.
  On the right, a 2×2 grid of content slots (`left`, `right`, `c`, `d`). Defaults: Labs,
  Medications, Allergies, Conditions.

There is no Mini. The surface switch, card slots, card memory and card grow come from
DeetsMusic's engine (`../DeetsMusic/docs/architecture/SURFACES-AND-CARDS.md`,
`../DeetsMusic/docs/cards/`).

## 2. Cards
| Card | Role | Midi | Max |
|---|---|---|---|
| Patient | the NP twin: who the record is for | anchored top | anchored, top of the stage |
| Timeline | the Queue twin: visits, past and upcoming (the owner designs it) | pickable | anchored under Patient |
| Labs | results with the site's own range | pickable | pickable |
| Medications | the medicine list | pickable | pickable |
| Allergies | substance and criticality | pickable | pickable |
| Conditions | the problem list (added 2026-09-21 as Max's fourth card) | pickable | pickable |
| Settings | the app's settings | pickable | pickable |

Each record card is a collection card over one normalized model. The first slice is Labs,
Medications and Allergies (HANDOFF.md › Decided 3).

## 3. As built (the empty shell, 2026-09-21)
> **Part:** built · 2026-09-21

Where this part and §1–§2 disagree, this part is the code.
- **Cards are empty.** Each record card mounts its header title and an empty body
  (`src/record-cards.ts`). No text in a body: each card's empty state is designed with the card.
  The Patient card has no header, so it gets no picker and no grow button.
- **Heights.** The empty Patient card holds its row open at two skin tokens:
  `--patient-card-h` 120 px (Midi) and `--patient-stage-h` 320 px (Max). They go when the
  card's design sets its own height.
- **Band numbers.** `surface.ts` keeps DeetsMusic's: Midi ↔ Max at 820 px wide with 40 px of
  dead-band; a Max window under 745 px tall becomes Midi. Floors: Midi 495 × 670, Max 495 × 750.
  Open sizes: Midi 495 × 670, Max 1100 × 950. They are DeetsMusic's measured numbers for its
  stage; the Patient card will set its own.
- **Engine.** Swap and replace motion (`card-swap.ts`), card memory (`card-memory.ts`, view
  state only — never a record key), card grow with zones, button, Pin and Fill
  (`card-grow.ts`), the drill swap (`layout.ts`; no card drills yet), the title picker, the
  launch cover and the look-change cover.
- **Not yet:** the collection-card engine (it comes with the first card that lists records),
  Compass (step 4), the card keyboard shortcuts (they ride Compass).

## 4. Desk test
`npm run dev:app`, then:
1. **Launch.** The window opens in Midi at 495 × 670. The cover fades and the cards rise:
   the Patient card on top, Labs and Medications under it. Light mode: Lilac × Press.
   Dark mode (Windows): Moonlight × Glass.
2. **Pick.** Click the title *Labs*. The menu lists Medications, Allergies, Conditions,
   Timeline, Settings. Pick *Medications*: the two cards swap places with the skin's motion.
   Pick *Allergies*: it replaces the card and rises in.
3. **Grow.** Hover a card's title: the Grow button enters. Click it: the card opens over its
   neighbor. Click outside: it collapses. Grow again, press Pin, click outside: it stays.
   Esc collapses. Hover the gap between the two cards: a bar shows; click it to grow.
4. **Max.** Title menu › Surface › Max. The window becomes 1100 × 950: the Patient card over
   the Timeline on the left, four cards on the right. Hover the gap above the Timeline and
   click: the Timeline grows up over the Patient card. On a content card, the button's second
   press fills the four slots.
5. **Resize.** Drag the Max window narrower than about 780 px: it becomes Midi. Drag a Midi
   window wider than about 860 px: it becomes Max.
6. **Settings.** Title menu › Settings…: the Settings card comes into a slot. Open each
   section; change a toggle; search "grow". The cog › Window square shows the same rows.
   Cog › All settings opens Settings as wide as the window allows; press it again to collapse.
7. **Look.** Every theme and every skin from the title menu, each with the look-change cover.
   Under Ocean and Glass, their Skin settings rows appear. Reset › Theme and skin asks, then
   offers Undo.
8. **Memory.** Restart: the same surface, the same cards in the same slots.
