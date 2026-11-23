---
status: project
desk_test: passed 2026-11-21
sources: [console/src/picker.ts, console/src/pages/panel-grow.ts, console/src/tokens.ts, console/src/nav.ts, console/src/pages/subnav.ts, console/src/pages/chat.ts, console/src/pages/page.ts, console/index.html, console/src/styles/app.css, console/src/styles/chat.css, console/src/styles/subnav.css, console/src/styles/skin.css, docs/architecture/images/nav-compact-layout-2026-11-14.png, docs/architecture/images/chat-compact-toggles-2026-11-14.png, docs/architecture/images/chat-panel-handle-2026-11-14.png, docs/architecture/images/chat-panel-overlap-2026-11-14.png, docs/architecture/images/chat-panel-no-collapse-2026-11-14.png]
updated: 2026-11-21
---
# DeetsHealth — The console on a phone

> The tracker for the console's narrow and touch layouts: how a page knows it is narrow, what
> is built, page by page, and what is open. Each piece's design, as-built record and desk test
> stay in the doc of its page (CONSOLE.md, CHAT.md); this file points to them. Add a row to §4
> and a line to §6 with each piece built.

## 1. Why
The owner and the analysts open the console on a phone too: the demo over the tunnel
(docs/ops/DEMO.md) reaches any device. The console was built for a desktop browser. The work
started on 2026-11-14, nav first (the owner: the shell before the pages).

## 2. How a page knows (decided 2026-11-14)
We never ask "is this a phone?". We ask three things the browser answers well.

| Question | What it decides | How |
|---|---|---|
| How wide is the window? | Layout | CSS `@media (max-width: …)` from the width tokens (§3); script: `tokens.ts › compactQuery()`, `narrowQuery()` |
| How does the user point? | Tap sizes, hover | `@media (pointer: coarse)` for a finger; `@media (hover: hover)` around every hover wash |
| What does the phone cover? | Insets, the keyboard | `env(safe-area-inset-*)` with `viewport-fit=cover`; `window.visualViewport` (not used yet, §5) |

- **No user-agent checks.** `navigator.userAgent` breaks easily (iPadOS Safari reports itself as a
  Mac); `navigator.userAgentData.mobile` and the `Sec-CH-UA-Mobile` header exist in Chromium
  only.
- **Width only for layout:** a tablet with a keyboard keeps the desktop layout; a narrow desktop
  window gets the phone layout. Both are right.
- **A media query cannot read a token.** Each `@media` rule repeats the width in a comment that
  names its token; script reads the token itself.

## 3. The widths
| Token (skin.css) | Below it | Where |
|---|---|---|
| `--narrow` | Chat's rail drops under the stage; the rail panels' grow handle; the call panel above the stage in Demo and PHI | app.css › split-page, chat.css |
| `--compact` | The nav and each page's own nav become dropdowns; Chat's toggles move under the text box; Start / End call in halves | app.css › compact bar, subnav.css, chat.css |
| `--tap-min` | (not a width) the least height of a tap target with `pointer: coarse` | the dropdowns, the grow handle |

## 4. Built, page by page
| Part | State | Doc |
|---|---|---|
| The shell: the page dropdown under DeetsHealth, a page's own nav as a dropdown at the right of the same row, in halves (`picker.ts`, the compact bar) | built 2026-11-14, desk test passed 2026-11-21 | CONSOLE.md §11 |
| Hover washes on the nav, the menus and the app name only with `hover: hover` | built 2026-11-14 | CONSOLE.md §11.1 |
| Chat: the seat, EHR and speed bars under the text box; the title box alone, full width | built 2026-11-14, desk test passed 2026-11-21 | CONSOLE.md §11.4 |
| Chat: Start / End call and Play / Stop fill the canvas's width in halves | built 2026-11-14, desk test passed 2026-11-21 | CONSOLE.md §11.4 |
| Chat: the call panel above the stage in Demo and PHI until a pick; it closes in place and opens under the stage (no crossing) | built 2026-11-14, desk test passed 2026-11-21 | CHAT.md §15.1 |
| Chat: the grow handle at each rail panel's foot (shown only when useful; nothing saved) | built 2026-11-14, desk test passed 2026-11-21 | CHAT.md §15.2 |
| Chat: the page grows with its rows, so a grown panel pushes the panels after it down | built 2026-11-14 | CHAT.md §15.2 |
| Config › Hospital: System's tables, each one scrolling on its own (`.sys-table-wrap`); no narrow layout of its own | built 2026-11-15, not looked at on a phone | HOSPITAL-CONFIG.md §4 |
| Every table (the one table): sideways scroll inside its own box below the sum of its column widths (`--col-min` per text column); a title line's buttons wrap under the title | built 2026-11-15, not looked at on a phone | TABLES.md §3.1 |
| Config › Hospital › Places: the facility cards wrap, one full-width column when the panel is narrower than a card; the drill-in and the Table view use the tables above | built 2026-11-15, not looked at on a phone | HOSPITAL-CONFIG.md §8.2 |
| Chat › Timeline: each row's label and span on one line, its track under them at the full width, so the track keeps its room in a narrow rail | built 2026-11-22, not looked at on a phone | CHAT.md §18 |
| Rules › Booking: the Booking rules table (the one table, as on Config › Hospital before) | built 2026-11-22, not looked at on a phone | BOOKING-RULES.md §5.5 |
| Analytics › Logs: the filter bar wraps line by line; the table (16 columns) scrolls sideways in its box | built 2026-11-22, not looked at on a phone | ANALYTICS.md §3.1.1 |
| Config, Rules, Teams, Analytics, Security, Audit Logs, Settings: their own narrow layouts | not looked at (Config's rows and Security › Auth have container queries at their own widths) | — |

## 5. Open
- **The other pages** (§4's last row): walk each one at `--compact` with the owner and bring the
  forks, page by page.
- **Hover on the other controls:** Chat's buttons, the tab bars and the Config rows still show a
  hover wash after a tap on a touch screen. Wrap each in `@media (hover: hover)`.
- **The safe areas:** `viewport-fit=cover` in `index.html` and `env(safe-area-inset-*)` on the top
  bar and the page's foot, for a notch and a home bar.
- **The on-screen keyboard:** the Chat text box under the keyboard (`visualViewport`,
  `interactive-widget=resizes-content`).
- **A real phone:** every desk test above was planned for a narrow desktop window or the
  browser's device mode. Run them once on a phone over the demo tunnel (docs/ops/DEMO.md).

## 6. Log
One line per piece built; the full record of each sitting is WORKLOG.md.
- 2026-11-14 — the checks a browser gives (§2); the compact nav (his sketch); the dropdowns in
  halves; Chat's toggles under the text box; Start / End call in halves; the call panel first in
  Demo and PHI (his fork A); the grow handle (his fork B, hidden when the list fits); the narrow
  page grows with its rows; the pick move closes and opens instead of crossing.
- 2026-11-21 — the owner's desk test of the 2026-11-14 pieces passed ("console on a phone also
  good to go"). §5 stays open.
- 2026-11-15 — Config › Hospital: System's tables, each scrolling on its own; not looked at on a
  phone yet. Later the same day: Places' facility cards, one column on a phone. Late: the one
  table (TABLES.md) for every table; not looked at on a phone yet.
- 2026-11-22 — Chat › Timeline, its track under the label at full width; Rules › Booking, the
  one table moved from Config › Hospital. Neither looked at on a phone yet.
