---
status: built
desk_test: passed 2026-11-11
sources: [console/src/pages/cartesia.ts, docs/architecture/images/config-cartesia-2026-11-13.png, docs/architecture/images/config-advanced-2026-11-13.png, docs/architecture/images/config-credits-size-2026-11-13.png, docs/architecture/images/config-glow-2026-11-13.png, docs/architecture/images/config-info-align-2026-11-13.png, docs/architecture/images/config-info-pin-2026-11-13.png, docs/architecture/images/config-list-ask-2026-11-13.png, docs/architecture/images/config-offset-ask-2026-11-13.png, docs/architecture/images/config-rates-2026-11-13.png, docs/architecture/images/config-scrollbar-2026-11-13.png, docs/architecture/images/config-slider-ask-2026-11-13.png, docs/architecture/images/config-slider-centre-2026-11-13.png, docs/architecture/images/config-slider-line-2026-11-13.png, docs/architecture/images/config-status-line-2026-11-13.png, docs/architecture/images/config-title-bar-2026-11-13.png, docs/architecture/images/config-wide-rows-2026-11-13.png, ehr/src/settings.rs, ehr/migrations/0002_settings.sql, console/src/styles/config.css, scripts/icons.py, console/index.html, console/src/main.ts, console/src/nav.ts, console/src/theme.ts, console/src/pages/chat.ts, console/src/pages/settings.ts, console/src/pages/teams.ts, console/src/pages/config.ts, console/src/pages/audit.ts, console/src/pages/security.ts, console/src/pages/subnav.ts, console/src/styles/app.css, console/src/styles/skin.css, console/src/styles/themes.css, console/public/manifest.webmanifest, console/vite.config.ts, console/src/tokens.ts, docs/architecture/images/console-settings-note-2026-11-11.png, docs/architecture/images/console-rail-sketch-2026-11-11.png]
updated: 2026-11-13
---
# DeetsHealth — The console

> The hospital's web console: the shell as built. The shell is DeetsMotion's (its
> `docs/ui.md`, 2026-10-29), brought into a browser page and into this project's tokens.
> What the console will show is DESIGN.md §2 C; the order of its work is BUILD-ORDER.md §3.1.
> The patient app's card engine and its other looks are in `archive/src/`
> ([archive/SURFACES-AND-CARDS.md](../archive/SURFACES-AND-CARDS.md)).

## 1. Run it
```
npm install
npm run dev           # the agent's dev server, the dev database, then Vite (1420 or the next free port)
npm run dev:web       # the console alone, with no agent
npm run build         # type check and bundle into console/dist/
```
What `npm run dev` starts, and in what order, is HANDOFF.md › Run it; the routes the page calls
are [DEV-API.md](DEV-API.md). Open the address that Vite prints in Edge, Chrome, Firefox or Safari. The front-end check is
`npm run check -- web` (the type check); `npm run check:all` also bundles.

## 2. Where it runs (fork 1A, the owner, 2026-11-11)
> **Part:** built · 2026-11-11

**Decided: in a browser only.** The console is a web page. The hospital installs nothing, and
every operating system works the same: Windows, macOS, ChromeOS, Linux.

**The browsers** (`vite.config.ts` › `build.target`): Chrome and Edge 109 or later (109 is the
last on Windows 7 and 8.1, which some hospital PCs still run), Firefox 115 or later (its
long-term release), Safari 16 or later.

**Installable as an app.** `public/manifest.webmanifest` makes the console an installable web
app (a PWA). In Edge or Chrome, the install button in the address bar puts the console in
its own window, with a taskbar or Dock icon and a Start-menu entry. Nothing else is installed:
the window runs on the hospital's own managed Edge or Chrome, under its policies.
- **The window-controls overlay.** An installed console asks for
  `window-controls-overlay`, so its top bar becomes the window's title bar, as in
  DeetsMotion. The bar drags the window; the app name and its menu stay clickable
  (`app.css` › window-controls overlay). The OS's own window buttons stay, drawn by the
  browser.
- **Hospital IT can push the install.** Edge and Chrome both have the
  `WebAppInstallForceList` policy (Group Policy or Intune): it installs the console on every
  staff PC, with no installer, no code signing and no update feed. A new deploy updates every
  window on its next load.
- **No service worker.** Nothing is cached for offline use, so the browser holds no copy of
  any page or answer. The console needs the network to work. The theme and the last page are
  the only things it keeps (`localStorage`), and neither is protected data.
- **The theme color.** `theme.ts` sets `<meta name="theme-color">` to the theme's base color,
  so the installed window's bar matches the page.

**Low load on weak PCs.** A remote desktop (Citrix, VDI) often has no GPU, so blur and
animation cost CPU.
- The backdrop's shapes move by `transform` only. They stop when the OS asks for reduced
  motion, and they pause while the tab is hidden (`main.ts` sets `data-idle`).
- When the OS asks for reduced transparency (Windows: Settings › Personalization › Colors ›
  Transparency effects off), the panels lose their blur and take the denser fill.
- Not measured yet in a remote desktop: a desk test there is open (§6).

**Later, not now.** A native app for Windows, macOS or iPhone is not built. If one is needed,
the same page can go in a thin native shell (Tauri: WebView2 on Windows, which is Edge's own
engine; WKWebView on macOS and iOS). Raise it when a hospital asks for something an installed
web app cannot do.

## 3. The shell as built
> **Part:** built · 2026-11-11

- **The top bar** (40 px, `--topbar`): the app name, bold, is the menu button, as in
  DeetsMusic and DeetsMotion. The menu has one row, *Theme ›*, which flies out the three
  themes with a swatch each and a dot on the current one. Keys: ↓ opens it, → enters the
  themes, ↑ and ↓ move, ← and Esc go back. There are no window buttons: the browser owns the
  window.
- **The nav** (168 px, `--nav-width`): text items. **Chat**, **Analytics**, **Teams** and
  **Config**, in that order, at the top; **Security**, **Audit Logs** and **Settings**, in that
  order, at the foot (the owner, 2026-11-12 and 2026-11-13). One accent pill slides to the selected item (280 ms). ↑ and ↓ move between pages.
  The old page fades out (120 ms), then the new one fades in (220 ms). With reduced motion,
  both are instant. The last page is reopened on the next visit (`localStorage`,
  `deets.console.page`).
- **Pages** are built once, on the first visit, and kept (`pages/page.ts`).
- **Chat** (`pages/chat.ts`): DeetsMotion's Visualizer layout. A large stage panel and a rail
  of four panels. The rail takes a third of the page's width (`--stage-share` 2fr,
  `--rail-share` 1fr), and never less than 300 px (`--rail-min`) (the owner, 2026-11-11: a
  wider rail for the Timeline). The last rail panel fills to the stage's bottom edge. Below
  1100 px the rail moves under the stage. There the stage keeps 70 % of the window's height
  (`--stage-narrow`), and the rail's panels sit side by side, as many in a row as fit at
  `--rail-min` each; the Review panel takes a row of its own (the owner's sketch, 2026-11-12).
  The panels are empty. The stage is *Chat*; the rail
  is *Timeline* (the owner's, at the top), then *Caller*, *Step* and *Tools* (a draft for the
  owner).
- **Teams**, **Config**, **Security**, **Audit Logs** and **Settings** (`pages/teams.ts`,
  `pages/config.ts`, `pages/security.ts`, `pages/audit.ts`, `pages/settings.ts`): each is one panel with its own nav, the same as Analytics
  (`pages/subnav.ts`; [ANALYTICS.md](ANALYTICS.md) §2). The head is the title alone. Each view
  says "Empty for now". The last view is reopened on the next visit (`localStorage`,
  `deets.console.<page>`). The nav items, in order (the owner, 2026-11-13):
  - Teams: **Org Chart**, **People**.
  - Config: **Agent**, **System**, **Cartesia** (§7).
  - Security: **Auth**, **Roles**.
  - Audit Logs: **Config**, **Calls**, **Security**.
  - Settings: **Personalization** (its first item). Settings had one panel with "No settings
    yet." until 2026-11-13; with it went `pageHead` and the `.page-head` styles.
- **Analytics** (`pages/analytics.ts`): one panel with its own nav inside it
  ([ANALYTICS.md](ANALYTICS.md)).
- **A page with its own nav** (`pages/subnav.ts`, `styles/subnav.css`; 2026-11-13): the one
  module behind Analytics, Teams, Config, Security, Audit Logs and Settings. A page gives only its title,
  its `localStorage` key and its views (`subnavPage`); an empty view is `blankView`. The module
  draws one glass panel: the title alone in its head, then the page's own nav at the left and
  the chosen view beside it. The page's nav is a panel stacked on the page panel, in the chat
  canvas's look (`--canvas`), as wide as the big nav (`--subnav-width`). Its items, pill, keys
  and view swap work like the big nav's. Each view is built once, on its first visit, and kept.
  A new page with its own nav is one file and one line in `main.ts`.
- **The page title** is the fixed text "DeetsHealth" (SECURITY.md §4.3).

### The look (fork 2A, the owner, 2026-11-11)
- **Three themes, DeetsMotion's:** Dark, Light (Solarized), Moonlight. With no saved choice,
  the OS's dark mode gives Dark and its light mode gives Light; nothing is saved until the user
  picks a theme. `public/prepaint.js` sets the theme before the first paint.
- **One skin, Glass.** `skin.css`'s base block is Glass. The `data-skin` attribute and the
  mechanism stay, so a second skin is one more block.
- **The tiers.** `palette.css` holds the solid paints; `themes.css` maps them to color roles
  (the Chat page's call colors, `--go` and `--stop`, and its canvas and bubbles joined on
  2026-11-12: CHAT.md); `skin.css` holds the tokens for type, layout, shape, spacing, blur and
  motion; `app.css` reads only tokens. The list and the counts are [TOKENS.md](TOKENS.md)
  (`npm run tokens`).
- **Shared helpers:** `tokens.ts` reads a duration token (`tokenMs`) and the reduced-motion
  setting (`reducedMotion`) for the nav and the Chat page (2026-11-12).
- **Shape and spacing tokens, as of 2026-11-13** (the full list is [TOKENS.md](TOKENS.md)):
  - **One radius, a trial** (the owner, 2026-11-13: "the same 16 px radius for everything").
    `--radius-small` (a menu, the help card), `--radius-item` (a nav item and its pill, every
    control) and `--radius-bubble` (a chat bubble) all point at `--radius`, 16 px, the panels'
    radius. Before the trial: 10, 11 and 14 px. To end the trial, set those three back.
  - `--radius-control` stays 6 px: the short rows (a menu row, the app-name button, the Step
    panel's rows and marks), where 16 px would turn a row into a pill.
  - `--pad-panel` is now two tokens, `--pad-panel-y` (16 px) and `--pad-panel-x` (20 px), and
    `--pad-panel` is made of them. Its value is the same. The Chat page uses the halves to put
    its title, canvas and bar on the same edges as every other page's title (CHAT.md §3).
- **Scrollbars: our own, not the system's** (the owner, 2026-11-13, "so we can pull for
  future pages"; desk test passed 2026-11-13 on the Chat page). A page opts in with one class, `own-scrollbars`, on its root
  (`<section class="page own-scrollbars">`); every scroll box inside it then gets them. The
  Chat page is the first; to give another page them, add the class there. The parts:
  - **The rule:** `app.css` › `.own-scrollbars`. No track, no arrows, a rounded thumb that
    darkens under the pointer.
  - **The tokens:** skin `--scrollbar-size` (10 px, the gutter) and `--scrollbar-inset` (3 px
    of clear space around the thumb, so the thumb is 4 px wide); color roles
    `--scrollbar-thumb` (`--press`) and `--scrollbar-thumb-hover` (`--text-3`), made of other
    roles, so each theme gets its own.
  - **The browsers:** Chromium (Chrome, Edge) and Safari draw the `::-webkit-scrollbar` parts.
    Firefox has none, so it gets the standard thin bar in the same colors
    (`scrollbar-width`, `scrollbar-color`).
  - **A strip that must never show a bar** (the Chat rail's tab bar) sets
    `scrollbar-width: none` on itself; that wins over the class.
- **Glass, as in DeetsMotion:** a panel's fill, edge, highlight, shadow and blur are on its
  `::before` layer, so its text is never blurred. Panels are never transformed. Liberation
  Serif, 400 and 700, with `font-synthesis: none`; 16 px body, 14 px floor.

### Decided while building, inside the owner's choices
- The theme is saved in `localStorage` (DeetsMotion saved it through Rust, which a web page
  does not have). Key `deets.console.theme`.
- The OS light or dark mode picks the first theme (DeetsMotion always started on Dark).
- The blur radius is one skin token, 28 px, for all three themes (DeetsMotion's Light used
  24 px): a blur is geometry, so it is a skin token, not a theme role.
- The panels go solid with no blur under reduced transparency (new; DeetsMotion has no such
  rule).
- The nav has no status lines at its foot (DeetsMotion showed the hand tracker's state there;
  the console has nothing to show yet).
- The empty panels say "Empty for now".
- **The icon** (the owner's pick, 2026-11-11, after two rounds of trials): DeetsMusic's
  app icon, his head on the Skyline1 city, with the head from his standing sprite (no
  headphones), and a white H in a smooth white ring over the whole icon; the art inside the
  ring is darkened so the H reads at 16 px. `python scripts/icons.py` (Pillow) builds
  `public/icons/icon-{512,256,192,32}.png` from `console/icon-template/` (copied from
  DeetsMusic). The manifest uses 192, 256 and 512; the browser tab uses 32.
- The styles are one stylesheet linked in `<head>` (`styles/index.css`), so the first paint is
  styled; DeetsMotion imported them from its script behind a hidden window.

## 4. What moved to the archive (fork 2A)
Into `archive/src/` and `archive/` by `git mv`, so each file keeps its history:
- the six old themes and the Vanilla, Press, Ocean, Glass and Cyber skins (`styles/`, the
  ambient layers `ambient.ts` and `storm.ts`, the other fonts);
- the card engine (`cards`, `layout`, `layout-bus`, `card-grow`, `card-swap`, `card-memory`,
  `surface`) and its doc, now [archive/SURFACES-AND-CARDS.md](../archive/SURFACES-AND-CARDS.md);
- the patient-only files (`sites`, `record-cards`, `frames`, `boot-cover`, `main`, `model/`);
- **the primitives too** (`toast`, `hint`, `dropdown`, `slider`, `context-menu`, `pop`), and
  the settings card and quick panel. They import the patient-only `frames.ts` and
  `boot-cover.ts`, and their styles use the old skin's tokens. Each one comes back, in the new
  tokens, with the first console feature that needs it.
- the old `index.html` and `public/prepaint.js`.

`@tauri-apps/api` is gone from `package.json`: nothing in the build calls Tauri.
`npm run types` now writes into `archive/src/model/`, because the payloads it writes are the
patient app's cards (CONVERSION.md §5.1).

## 5. Security notes
- The page carries no protected data yet: no sign-in, no calls, no settings.
- **The Content-Security-Policy comes from the deploy's response headers**, not a `<meta>`
  tag: Vite's dev server adds inline styles that a strict meta policy would block. The
  policy (no inline script, which is why `prepaint.js` is a file) is set with the deploy
  (VOICE-AGENT.md §7.2). Not built.
- No service worker and no offline cache (§2).

## 6. Desk test
**Passed, 2026-11-11** (the owner, in Edge). His notes, built the same day: no eyebrow over
Settings, its panel fills the page; the icon; a Timeline panel at the top of the rail and a
wider rail. Steps 5–7 (install, transparency, other browsers) are still open.

![His note, 2026-11-11: remove the eyebrow over Settings](images/console-settings-note-2026-11-11.png)
![His sketch, 2026-11-11: a wider rail (the red line marks its new left edge)](images/console-rail-sketch-2026-11-11.png)
1. `npm run dev`, then open the address in Edge. The page shows the top bar, the nav (Chat,
   Analytics, Teams and Config at the top; Security, Audit Logs and Settings at the foot), and
   Chat's stage with its panels on its right.
2. Click **DeetsHealth** in the top bar, point at **Theme**, and pick each theme. The colors
   change at once, the dot moves, and a reload keeps the theme.
3. Click **Settings**, then **Chat**. The pill slides, and the pages fade. Reload: the last page
   opens.
4. Make the window narrower than 1100 px. The panels move under the stage. The stage fills
   70 % of the window's height. The panels sit two to a row (Timeline beside Sandbox patients,
   Step beside Tools); narrower still, one to a row. When a call ends, Review takes its own row.
5. In Edge, use the install button in the address bar (or … › Apps › Install). The console
   opens in its own window, with the top bar as the title bar: drag it, and the window moves.
6. Turn off Windows' Transparency effects. The panels go solid.
7. Later: the same page in Chrome, Firefox and Safari, and in a remote desktop with no GPU.

### The 2026-11-13 additions
> **Part:** built · 2026-11-13 — desk test open.

1. The nav reads Chat, Analytics, Teams, Config at the top, and Security, Audit Logs, Settings
   at the foot.
2. Click **Teams**, **Config**, **Security**, **Audit Logs** and **Settings** in turn. Each shows one panel:
   the title alone, then its own nav at the left (Org Chart, People · Agent, System · Auth,
   Roles · Config, Calls, Security · Personalization). The first item is chosen; the view says "Empty for now".
3. On one of them, click the second item. The pill slides and the view fades. Press ↑ and ↓:
   the choice moves. Reload: the same item is chosen.
4. Analytics looks and works as before, with no line under its title.
5. The radius trial: the toggles, Play / Stop, the picker, the text box, the bubbles, the nav
   pill, the theme menu and the help card all have 16 px corners, as the panels do. Decide if
   it stays.

## 7. Config › Cartesia
> **Part:** built · 2026-11-13 · desk test open — the owner, 2026-11-13: "a subpage under config
> dedicated to Cartesia settings and items … At the top of the subpage, let's do a tabbar of
> 'Read' and 'Edit' … On read mode, everything can be grayed out." Then the versions and the
> settings store: "Set it up please, I'll desk test in the morning." Every lever: CARTESIA.md §9.

### 7.1 The view as built (`pages/cartesia.ts`, `styles/config.css`)
![The owner's screenshot, 2026-11-13: Config › Cartesia in Read, two columns, the first saved
version Active](images/config-cartesia-2026-11-13.png)

- **Config's third item, Cartesia**, after Agent and System.
- **The top row** (the owner, 2026-11-13: "align the buttons in the title bar"): a **Read | Edit**
  tab bar (the Chat page's pair bars, `--pair-part`); the **versions** dropdown right of it; a
  status line that is empty unless a save says something; **Discard | Save** as one tab bar,
  always shown (each greys when it has nothing to do; Save, when it can save, wears the chosen
  part's colors). All three are `--control-height` tall with the tab bar's frame. The view opens
  on Read each time. No line of help (the owner: "it's clear to a healthcare analyst").
- **The versions** (the owner, 2026-11-13: "a version as determined by date + version of the day
  … 11-12 | 1 for the first today, 11-12 | 2 for the second"): newest first; the one the agent
  runs with reads **Active | [MM-DD | n]** ("for clarity's sake"), every other **MM-DD | n** (the
  day's date on the database's clock, and its number that day). Its hint gives when it was saved.
  The dropdown is `--cfg-version` wide: room for "Active | [MM-DD | VVV]".
  - **Default** (the owner: "The 'Default' can be a version, the first ever in the dropdown for
    every new implementation, but it's still a version"): number 0, the code's defaults, always
    listed last. The dev server makes it the first time the page asks (`ensure_default`), and
    it is active until the first save ("Active | [Default]"). There is never a state with no
    Active version. A store that had saves before its Default (the owner's dev database) gets one
    too, listed last and not active.
  - Without Postgres: "No settings store", disabled, and Save is off with a hint to start
    Postgres.
  - **An older version** shows read only (Edit is off, with the hint "Choose Active to edit"),
    and each row whose value differs from the active one glows (the owner: "a gentle glow"): a
    soft wash and glow of the accent that fades in (`--glow-fill`, `--glow-strength`,
    `--glow-blur`), on a layer centred on the row's content (`--glow-pad` past it on every side;
    the owner: it "looks too offset vertically"). The turn profile glows with any turn value;
    Plan credits with the offset.
  - **Save** (Edit, on Active, with a change): the whole set becomes a new version, the active
    one, and the page goes back to Read on it, with "Saved as MM-DD | n. Active from the next
    call". A refusal shows "Not saved: …" with the setting it names.
- **Read:** every control greyed (`--disabled-opacity`) and disabled. The "i" buttons still open.
- **Each row** (the owner, 2026-11-13: "remove all the subtitle text … add an info icon 'i' in a
  rounded square in the left most column before every setting"): the "i" (`--info-size`), the
  setting's name, its control. A press on the "i" opens the setting's help under the name (its
  edge takes the accent); a second press closes it. It rolls down and back up with a fade, over
  `--dur-collapse` (the Chat page's help card's collapse; the owner, 2026-11-13), instant with
  reduced motion. The help words are `HELP` in
  `cartesia.ts`. The "i" is the letter itself, lower case, in the console's serif face
  (Liberation Serif) at body size (the owner, 2026-11-13: "after a lower case classical serif
  font"); its left edge sits on the card's text edge, under the card's title.
- **Edit:** the controls open. A change stays on the page until Save. Discard puts every value
  back. Choosing another version drops changes not saved. Edits made
  in Edit stay when you go back to Read, greyed.
- **The cards** (in a canvas: `.cfg-frame` draws the edge and clips; `.cfg-scroll` inside it
  scrolls, and its scrollbar's track stops a radius short of each end, so the thumb stays inside
  the rounded corners; the owner, 2026-11-13: "the scrollbar is leaving the panel"):
  - **Model first in each card** (the owner, 2026-11-13).
  - **Voice (Sonic):** Model (`sonic-3.6`, shown only); Voice (Cartesia's 10 stable voices for
    agents; Listen shows, disabled: not built); Speed and Volume (sliders); Emotion (Cartesia's
    default, or one of five); Number reading (Auto, Off); Pronunciations (a table).
  - **Hearing (Ink)**, every row open (the owner, 2026-11-13: no "Advanced" fold), in the order a
    turn happens: Model (`ink-2`, shown only); Turn profile; Start of a turn; End of a turn;
    Longest wait (ms); Early end of turn (Off | On) with Early end guess under it, the value it
    uses; Barge-in (Off | On); Keyterms (a table).
  - **The turn profile moves the sliders.** Cartesia's docs: a profile is not an API parameter,
    only a named set of the four turn values (CARTESIA.md §9). So choosing Balanced (Cartesia's
    default), Responsive or Patient sets the four sliders to its values; moving any slider makes
    the profile read the one the four now match, or Custom. Custom is shown, never chosen. The
    draft holds only the four values; the profile is worked out from them.
  - **Rows are one height:** each control is at least `--field-height`, and the Off | On bars
    are a field's height.
  - **Test-call guard:** Silence (s) and Longest call (s).
  - **Usage** (the owner, 2026-11-13: "a box … where I can set a manual offset", and "Credits
    Left" as plan credits − offset − usage): Plan credits, then "−" and an Offset box on the same
    row; Cycle day; **Credits left**, in the page's normal type (the owner, 2026-11-13: a larger
    figure belongs on Analytics, not Settings): the plan's credits, less the offset, less what
    this cycle's voice calls used (`/agent/resources`). It updates as you type.
  - **Key and facts:** the API key ("Set", or "Not set: add CARTESIA_API_KEY to .env"; never the
    key), the API version, the rates.
- **The sliders** (the owner, 2026-11-13: the bar changed size, and in place of the Default
  button "a vertical line where 1.0 / the default is"): our own track and thumb
  (`--track-height`, `--thumb-size`); a line at Cartesia's default, under the thumb's centre
  there and taller than the thumb (`--mark-height`), so it still shows above and below the
  thumb when the thumb sits on it; the value beside it as a number only, in a fixed width
  (`--cfg-value`). Sliding back onto the line returns the setting to Cartesia's default. On the
  default, the thumb is centred on the line both ways: the track's edge is an inset shadow, not
  a border, because a border moves the thumb's travel in by its width (the owner, 2026-11-13).
- **The tables** (the owner, 2026-11-13: "move the active list into a structured table below the
  text boxes"): the add row first (one box per column, then Add; Enter adds too), the entries
  under it as a table with a header row and Remove at each row's end. No table while it is
  empty.
- **The data:** `GET /agent/cartesia` (DEV-API.md §2): every value the next call runs with, and
  each lever's range and Cartesia's default, from the constants `voice.py` uses
  (`config.py`). A null value is "Cartesia's default": the agent sends nothing for it.
- **The layout** (the owner, 2026-11-13: rows "wider than they need to be"; then option A, "hearing
  and voice can be the two columns and their relevant settings below them"): each card is as
  wide as its rows, which are fixed columns: the "i", the name (`--cfg-label`, 168 px) and the
  control (`--cfg-control`, 320 px), so no field or slider stretches. One card is 582 px.
  - **Two columns** when the view has room for two cards (1,176 px, a container query on the
    view's own width): Voice | Hearing; the test-call guard under Hearing (silence is what Ink
    hears); then Usage | Key and facts.
  - **One column** below that, in the order Voice, Hearing, guard, Usage, Key and facts.
  - **Narrower than one card:** the cards fill the width, and each row puts its control under
    its name.
  - The container query cannot read tokens, so `config.css` writes the sum out and names the
    tokens it comes from.
- **Tokens:** `--field-height`, `--cfg-label`, `--cfg-control`, `--cfg-number`, `--cfg-value`,
  `--info-size`, `--help-max`, `--thumb-size`, `--track-height`, `--mark-height` (new), with `--pair-part`,
  `--disabled-opacity` and the color roles.

### 7.2 Decided inside his choices (for his review)
- The cards' order and titles; every lever's name and its help text (`HELP` in `cartesia.ts`).
- Read each time the view opens; edits kept on the page when you switch back to Read.
- Booleans as Off | On bars, the same as Read | Edit; numbers with a range as sliders.
- A slider within half a step of the default counts as the default.
- One help open per press, several may be open at once; help opens in Read too.
- The Hearing order: start, end, longest wait, then the early switch with its guess, then
  barge-in and keyterms.
- The profile select's words: "Balanced (Cartesia's default)", "Responsive", "Patient",
  "Custom".
- A voice outside the 10 with ids in Cartesia's docs gets its id from Cartesia's voice list by
  name when saved (no credits; it needs the key).
- The offset is one number for the cycle, with no note; Credits left may go below zero.
- The versions' words: "Active | [MM-DD | n]", "MM-DD | n", "Default", "No settings store". The Default is made by the dev server on first view, by "default".
- Save returns the page to Read on the new Active version.
- An older version cannot be made Active again from the page yet: edit Active to match it and
  save (a new version). A "Make active" button is a small follow-up.

### 7.3 The settings store (as built, 2026-11-13)
> **Part:** built · 2026-11-13 · desk test open. The design before it was a change table per
> setting; the build keeps whole versions instead, which answers the same question exactly.

- **One key–value table per service** (the owner, 2026-11-13: "A per service as we continue
  (Core model provider, Voice model provider, etc)"): `settings_voice` today; `settings_llm` and
  others join the same way. Each row: a version, a setting's name, its value (JSON).
- **Whole versions:** a save sends every setting of the service, once each, and they become one
  new version (`setting_versions`: the service, the day, its number that day, when, who). So a
  version is the exact set the agent ran with.
- **Exactly which value was live, and when** (the owner's question): `setting_activations` gets one
  row each time a version becomes the active one, with when and who. The active version is the
  newest row; the value live at any moment is in the version of the last activation before it.
- **Insert-only:** a trigger on all three tables refuses every UPDATE, DELETE and TRUNCATE
  (`ehr/migrations/0002_settings.sql`). The history can only grow.
- **Checked in Rust:** `ehr/src/settings.rs › VOICE` lists every setting and what it may hold
  (each range is Cartesia's; the same as `config.py`'s for the page). `ehr` refuses a set with a
  setting missing, given twice, unknown, out of range or the wrong kind, and names the setting,
  never its value. A test (`test_the_voice_defaults_name_every_setting_ehr_checks`) fails if
  the agent's defaults and `VOICE` list different settings.
- **The path:** the page → the dev server (`GET`/`POST /agent/cartesia`, DEV-API.md §2) →
  `ehr`'s `ListSettingVersions`, `GetSettings`, `SaveSettings` (EHR-SERVICE.md §3) → Postgres. The
  console never opens the database (DATABASE.md §1).
- **The agent reads the active version** when a voice call starts (`dev_server.py ›
  active_voice_settings`), so a save applies to the next call, never the one in progress. Every
  lever is applied (`voice.py › build_stt`, `build_tts`, `VoiceMeter`) except the
  pronunciations: they are stored, and need a Cartesia pronunciation dictionary before Sonic can
  use them (§7.6). The Resources tab and Credits left take the plan's credits, the offset and the
  cycle day from it. Without Postgres, or before a save, the code's defaults apply.
- **Not yet:** the usage columns on `calls`; the `audit` row for each save (the activation row
  records who and when meanwhile); staff ids in place of "dev".

### 7.6 Open
- **Pronunciations:** make or update a Cartesia pronunciation dictionary from the saved table,
  and send its id with Sonic.
- **Make active:** restore an older version as a new activation.
- **Listen:** Cartesia's voice list can give each voice's preview file, which plays with no
  credits; the dev server would fetch it, so the page loads no remote file.

### 7.4 The API key, for a hospital
The key never goes into the console or the database (SECURITY.md §4.6). Who sets it up depends on
who holds the Cartesia contract:
- **We hold it** (likely in A2, our cloud): we make one key for each hospital, so each one's use
  shows apart, and put it in that hospital's secret manager, in its own cloud account. Only the
  agent's role may read it. The hospital does nothing.
- **The hospital holds its own contract and BAA** (likely in B, its own cloud account): its admin
  makes the key in Cartesia and stores it in its own secret manager under the name our deploy
  reads. We give them a short runbook (CLOUD.md, when the deploy is built) and a deploy step that
  makes the empty secret and the read permission.
- **On the page, in both:** "Key set", when the agent last read it, when it was last rotated, and
  later a "Test the key" button that makes a call costing no credits. Never the key.
- A write-only field that sends a key straight to the secret manager is possible, but it lets
  the console's server write secrets; it stays an idea.
- To find out: whether Cartesia offers one organization with a key and usage per hospital.

### 7.5 Desk test
**First:** stop the dev server (Ctrl+C) and start it again with `npm run dev`. It builds the new
`ehr` and runs the settings migration in the dev database.

**The versions and the store:**
- A. Config, Cartesia. The dropdown reads "Active | [11-13 | 1]" (your save of 2026-11-13), and
  "Default" sits last in its list. Read | Edit, the dropdown and Discard | Save are one height,
  their tops in line; Discard and Save show, both grey. No help line.
- B. Press Edit, set Offset to 270: Discard and Save light. Press Save. The page goes to Read;
  the status says "Saved as MM-DD | n. Active from the next call"; the dropdown reads
  "Active | [MM-DD | n]".
- C. Choose the version before it: it shows read only, Edit is off, and the Plan credits row
  glows, the glow centred on the row. Choose "Default": the Voice row glows too (Gemma against
  Skylar). Choose Active again: no glow, and Edit is back.
- D. On a new database (or another machine), the first view shows "Active | [Default]".
- E. Chat, Resources: the credits left are 270 lower.
- F. Optional, with Postgres stopped (`npm run db:stop`, then restart `npm run dev`): the
  dropdown says "No settings store" and Save is off.

**The page:**
1. Config, Cartesia. The top shows Read | Edit on Read, and every control is
   grey. No line sits under any setting's name.
2. Press an "i": its help opens under the name, and its edge takes the accent. Press it again:
   it closes. This works in Read.
3. Press Edit. Move Speed: the value beside it changes, and the track keeps its size. Slide back
   onto the line at 1.00: the line shows above and below the thumb. Discard showed after the
   move and went when you slid back. Each "i" is a serif lower-case letter, its left edge under
   the card title's.
4. Add a keyterm (type it, press Add or Enter): a table appears under the box with a header and
   the entry, with Remove. Remove it: the table goes.
5. Usage: type 20000 in Plan credits and 270 in Offset. Credits left, in normal type, shows
   19,730 and updates as you type.
6. Discard: every value goes back. Press Read: all grey again; Save is off in Read.
7. Widen the window: with room for two cards, Voice and Hearing sit side
   by side, the test-call guard under Hearing, then Usage and Key and facts. Narrow it: one
   column. No field or slider stretches past its column.

### 7.7 The owner's screenshots (2026-11-13), in the order of his notes
Kept so the reason for each change stays with it. Each one is what he saw before the change.

![The owner's screenshot, 2026-11-13: the slider's track changes size, and a Default link; he
asks for "a vertical line where 1.0 / the default is"](images/config-slider-ask-2026-11-13.png)

![The owner's screenshot, 2026-11-13: a list above its add boxes; he asks to "move the active list
into a structured table below the text boxes"](images/config-list-ask-2026-11-13.png)

![The owner's screenshot, 2026-11-13: Usage with Plan credits; he draws the box for "a manual
offset" beside it](images/config-offset-ask-2026-11-13.png)

![The owner's screenshot, 2026-11-13: the "i" icon reads as a pin; he asks for a classic serif
lower-case i](images/config-info-pin-2026-11-13.png)

![The owner's screenshot, 2026-11-13: the "i" button's left edge against the card title's; he
asks to align them](images/config-info-align-2026-11-13.png)

![The owner's screenshot, 2026-11-13: "default" beside each slider's value; he asks to remove the
word and lengthen the line](images/config-slider-line-2026-11-13.png)

![The owner's screenshot, 2026-11-13: Credits left in large type; he asks for normal type,
larger figures belong on Analytics](images/config-credits-size-2026-11-13.png)

![The owner's screenshot, 2026-11-13: the default line through each thumb; he asks that the thumb
centre on the line at 1.00](images/config-slider-centre-2026-11-13.png)

![The owner's screenshot, 2026-11-13: the scrollbar in the panel's rounded corner; "the
scrollbar is leaving the panel"](images/config-scrollbar-2026-11-13.png)

![The owner's screenshot, 2026-11-13: one column of cards whose rows are "wider than they need to
be"; he asks for two columns, Voice and Hearing](images/config-wide-rows-2026-11-13.png)

![The owner's screenshot, 2026-11-13: the four turn rows inside an Advanced fold; he asks for
them "permanently opened", Model first](images/config-advanced-2026-11-13.png)

![The owner's screenshot, 2026-11-13: a help line beside Read | Edit; he asks to remove it and
put the versions dropdown there](images/config-status-line-2026-11-13.png)

![The owner's screenshot, 2026-11-13: the whole page after the settings store, his first save
Active](images/config-cartesia-2026-11-13.png)

![The owner's screenshot, 2026-11-13: the title bar's controls at different heights; he asks to
align them, with Discard | Save styled like Read | Edit](images/config-title-bar-2026-11-13.png)

![The owner's screenshot, 2026-11-13: an older version's changed row, its glow too low; he asks
to centre it](images/config-glow-2026-11-13.png)

![The owner's screenshot, 2026-11-13: the Rates row; he asks for "Ink: 3 credits per second |
Sonic: 1 credit per character"](images/config-rates-2026-11-13.png)
