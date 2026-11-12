---
status: built
desk_test: open
sources: [console/src/pages/analytics.ts, console/src/styles/analytics.css, docs/architecture/images/analytics-sketch-2026-11-12.png]
updated: 2026-11-12
---
# DeetsHealth — The Analytics page

> The console's Analytics page: one panel with its own nav inside it. Each choice in that nav
> fills the rest of the panel with its own panels. The owner's design, narrated 2026-11-12
> over his sketch; he designs each view next. The shell it sits in is
> [CONSOLE.md](CONSOLE.md).

![The owner's sketch of the Analytics page, 2026-11-12](images/analytics-sketch-2026-11-12.png)

## 1. His words (2026-11-12)
The box at the left (blue in the sketch) is a nav "that we'll stack into the panel". It "can
operate like the big nav bar but specifically for the analytics page". "Each selection will
determine what panels and what is displayed on the screen." Add **Home** above Outcome, "a
summary overall". Add **Flags** at the end. "I'll design each subpage manually."

## 2. As built
> **Part:** built · 2026-11-12

- **The panel:** one glass panel fills the page. Its head is the title *Analytics* and the line
  "Nothing here yet.". Under the head: the Analytics nav at the left, the view at the right.
- **The Analytics nav:** a panel stacked on the page panel, with the chat canvas's look
  (`--canvas`). As wide as the big nav (`--subnav-width`, which is `--nav-width`). Its items,
  in order: **Home**, **Outcome**, **Cost**, **Logs**, **Speed**, **Flags**.
- **It works like the big nav** (CONSOLE.md §3): the same items and accent pill, which slides
  to the chosen item. ↑ and ↓ move between views. The old view fades out, then the new one
  fades in; with reduced motion, both are instant. The last view is reopened on the next visit
  (`localStorage`, `deets.console.analytics`).
- **The views:** each is built once, on its first visit, and kept (`View` in
  `pages/analytics.ts`). Each is empty now: "Empty for now", in the middle of the view.

**Decided inside his choice (for him to see):** the item names are his words as written
("Outcome", not "Outcomes"); the nav is as wide as the big nav; the view swap fades like the
page swap; the head keeps "Nothing here yet." until the views have a design.

## 3. The views
Each gets its own section here when the owner designs it.

| View | Intent | State |
|---|---|---|
| Home | A summary of all the others | Empty |
| Outcome | — | Empty |
| Cost | — | Empty |
| Logs | — | Empty |
| Speed | — | Empty |
| Flags | — (see [FLAGS.md](FLAGS.md)) | Empty |

## 4. Desk test
**Open.**
1. `npm run dev`, open the console, click **Analytics** in the big nav. One panel fills the
   page: the title, then the Analytics nav at the left with Home, Outcome, Cost, Logs, Speed
   and Flags. Home is chosen. The view says "Empty for now".
2. Click **Cost**. The pill slides from Home to Cost, and the view fades.
3. Press ↓ and ↑ with an item focused. The choice moves, and wraps at the ends.
4. Click **Chat**, then **Analytics** again: Cost is still chosen. Reload: Cost is still chosen.
5. Change the theme from the app-name menu. The Analytics nav keeps the chat canvas's look in
   each theme.
