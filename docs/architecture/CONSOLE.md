---
status: built
desk_test: passed 2026-11-11
sources: [docs/architecture/images/hospital-places-cards-dark-2026-11-15.png, docs/architecture/images/hospital-places-cards-charcoal-2026-11-15.png, console/src/styles/people.css, console/src/styles/palette.css, console/security-headers.json, agent/deets_agent/security_headers.py, scripts/html-sinks.mjs, docs/architecture/images/chat-compact-toggles-2026-11-14.png, docs/architecture/images/nav-compact-layout-2026-11-14.png, console/src/picker.ts, console/src/pages/page.ts, console/src/styles/subnav.css, console/src/styles/chat.css, docs/architecture/images/config-agent-test-button-2026-11-14.png, console/src/pages/roles.ts, console/src/styles/roles.css, console/src/pages/rules.ts, docs/architecture/images/security-roles-split-2026-11-14.png, docs/architecture/images/security-roles-panel-name-2026-11-14.png, console/src/pages/agent.ts, console/src/pages/system.ts, console/src/pages/word-picker.ts, console/src/pages/tab-bar.ts, ehr/migrations/0006_settings_system.sql, ehr/migrations/0005_settings_prompt.sql, console/src/pages/voice.ts, console/src/pages/anthropic.ts, console/src/pages/config-kit.ts, ehr/migrations/0004_settings_llm.sql, docs/architecture/images/config-cartesia-2026-11-13.png, docs/architecture/images/config-advanced-2026-11-13.png, docs/architecture/images/config-credits-size-2026-11-13.png, docs/architecture/images/config-glow-2026-11-13.png, docs/architecture/images/config-info-align-2026-11-13.png, docs/architecture/images/config-info-pin-2026-11-13.png, docs/architecture/images/config-list-ask-2026-11-13.png, docs/architecture/images/config-offset-ask-2026-11-13.png, docs/architecture/images/config-rates-2026-11-13.png, docs/architecture/images/config-scrollbar-2026-11-13.png, docs/architecture/images/config-slider-ask-2026-11-13.png, docs/architecture/images/config-slider-centre-2026-11-13.png, docs/architecture/images/config-slider-line-2026-11-13.png, docs/architecture/images/config-status-line-2026-11-13.png, docs/architecture/images/config-title-bar-2026-11-13.png, docs/architecture/images/config-wide-rows-2026-11-13.png, ehr/src/settings.rs, ehr/migrations/0002_settings.sql, console/src/styles/config.css, scripts/icons.py, console/index.html, console/src/main.ts, console/src/nav.ts, console/src/theme.ts, console/src/pages/chat.ts, console/src/pages/settings.ts, console/src/pages/teams.ts, console/src/pages/config.ts, console/src/pages/audit.ts, console/src/pages/security.ts, console/src/pages/subnav.ts, console/src/styles/app.css, console/src/styles/skin.css, console/src/styles/themes.css, console/public/manifest.webmanifest, console/vite.config.ts, console/src/tokens.ts, docs/architecture/images/console-settings-note-2026-11-11.png, docs/architecture/images/console-rail-sketch-2026-11-11.png]
updated: 2026-11-22
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
- **The nav** (168 px, `--nav-width`): text items. **Chat**, **Config**, **Rules**,
  **Teams** and **Analytics**, in that order, at the top (the owner, 2026-11-14; before, Chat,
  Analytics, Teams, Config); **Security**, **Audit Logs** and **Settings**, in that
  order, at the foot (the owner, 2026-11-12 and 2026-11-13). One accent pill slides to the selected item (280 ms). ↑ and ↓ move between pages.
  The old page fades out (120 ms), then the new one fades in (220 ms). With reduced motion,
  both are instant. The last page is reopened on the next visit (`localStorage`,
  `deets.console.page`).
- **Pages** are built once, on the first visit, and kept (`pages/page.ts`).
- **Chat** (`pages/chat.ts`): DeetsMotion's Visualizer layout. A large stage panel and a rail.
  *Since 2026-11-13 the rail is two panels with tabs, all built ([CHAT.md](CHAT.md) §15; the
  Timeline tab, §18); the rest of this item is the first layout of 2026-11-11.* Then: a rail
  of four panels. The rail takes a third of the page's width (`--stage-share` 2fr,
  `--rail-share` 1fr), and never less than 300 px (`--rail-min`) (the owner, 2026-11-11: a
  wider rail for the Timeline). The last rail panel fills to the stage's bottom edge. Below
  1100 px the rail moves under the stage. There the stage keeps 70 % of the window's height
  (`--stage-narrow`), and the rail's panels sit side by side, as many in a row as fit at
  `--rail-min` each; the Review panel takes a row of its own (the owner's sketch, 2026-11-12).
  The panels are empty. The stage is *Chat*; the rail
  is *Timeline* (the owner's, at the top), then *Caller*, *Step* and *Tools* (a draft for the
  owner).
- **Teams**, **Config**, **Rules**, **Security**, **Audit Logs** and **Settings** (`pages/teams.ts`,
  `pages/config.ts`, `pages/rules.ts`, `pages/security.ts`, `pages/audit.ts`, `pages/settings.ts`): each is one panel with its own nav, the same as Analytics
  (`pages/subnav.ts`; [ANALYTICS.md](ANALYTICS.md) §2). The head is the title alone. Each view
  says "Empty for now". The last view is reopened on the next visit (`localStorage`,
  `deets.console.<page>`). The nav items, in order (the owner, 2026-11-13):
  - Teams: **People** (built 2026-11-14, `pages/people.ts`): who may
    sign in, ops/DEMO.md §3.5 and §7; since the owner's sketch of 2026-11-14, one card per
    person, read only. Org Chart was removed on 2026-11-22 (the owner: "for now").
  - Config: **Agent**, **System**, **Cartesia** (§7).
  - Rules (2026-11-14): **Booking**, **Agents**, **Flags**, **Tools**. One place for every rule
    (the owner's 2A, 2026-11-22). Booking is the Booking rules table, moved from Config ›
    Hospital ([BOOKING-RULES.md](BOOKING-RULES.md) §5.5): the same Config frame and the same
    hospital versions (`hospital.ts › bookingRulesView`). Agents, Flags and Tools are empty.
  - Security: **Auth**, **Roles**. Auth is built (2026-11-14, `pages/auth.ts`): who may sign
    in as a table that can be edited, a panel under each row (ops/DEMO.md §7). Roles (2026-11-14, `pages/roles.ts`,
    `styles/roles.css`) is the role templates as a table in Auth's look: one row per role, one
    column per lever, each cell None, View or Edit ([ACCESS-ROLES.md](ACCESS-ROLES.md)
    §4). A click on a cell moves it to the next level. *(Since 2026-11-18 the tables read the
    roles store and Save makes a new version: §13.)* Two tables, each with its title above it: **Customer** (the hospital's six
    roles), then **DeetsHealth Team** (our two); no page title and no status line (the owner's
    sketch, 2026-11-14). **Add** at the right of each title (the owner, 2026-11-14) puts a new
    role at that table's foot: its name the placeholder "Role name", every lever None, its
    panel open. **Clicking a role's name** opens a panel under its row, as on Auth: the name in
    a text box with no label, as tall as Discard | Save and on one row with them (the owner's
    sketch below), a result line under them only when there is one, and "Users with this role", a read-only table (picture,
    first, last, email, last seen). It is empty for now ("No users yet: roles are not
    stored"). One panel is open at a time, across both tables. Discard on a new role takes it
    away, and so does opening another role. Save refuses an empty name or a name already in
    that table. Desk test: Security, Roles. Two tables, Customer then DeetsHealth Team. Click
    a cell three times: None, View, Edit, then back. Click Add on Customer: a row with "Role
    name" and every cell None, its panel open. Type a name, Save: the row shows it. Click
    Director's name: the new role's panel closes, Director's opens. Reload: the draft is back.

    ![The owner's screenshot, 2026-11-14: Security › Roles; the title Roles struck out, the status line boxed, a red line under Compliance to split the table in two](images/security-roles-split-2026-11-14.png)
    ![The owner's screenshot, 2026-11-14: a role's panel; the empty row beside Discard and Save boxed in yellow, for the name box to fill; the Role name label to go](images/security-roles-panel-name-2026-11-14.png)
  - Audit Logs: **Config**, **Calls**, **Security**.
  - Settings: **Personalization** (its first item). Settings had one panel with "No settings
    yet." until 2026-11-13; with it went `pageHead` and the `.page-head` styles.
- **Analytics** (`pages/analytics.ts`): one panel with its own nav inside it
  ([ANALYTICS.md](ANALYTICS.md)).
- **A page with its own nav** (`pages/subnav.ts`, `styles/subnav.css`; 2026-11-13): the one
  module behind Analytics, Teams, Config, Rules, Security, Audit Logs and Settings. A page gives only its title,
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
- **Dark's base is charcoal `#121212`, not black** (the owner, 2026-11-15, option B of
  three: black, `#181818`, `#121212`, warm `#16140f`). Pure black stays for what must read
  darker than the page: a card's band (`--theme-color`), the chat canvas, text on the accent.
  The browser's bar and the installed app's splash take the same charcoal.
  ![The owner's screenshot, 2026-11-15: Places › Cards in Dark on pure black; the cards' black band is lost on the page](images/hospital-places-cards-dark-2026-11-15.png)
- **A card has its own fill, `--card`** (the owner, 2026-11-15: a tint per theme, not a solid
  paint and not a lighter `--canvas`). Cards sit on `--canvas` in Config's frame, so with no
  fill a card's body was as dark as its band. The tint lifts a card one step above any
  surface under it: white in Dark, cream in Light, moon blue in Moonlight. Teams › People and
  Config › Hospital › Places use it (`.people-card`, `.place-card`).
  ![The owner's screenshot, 2026-11-15: the charcoal base is in, but the cards on the canvas are still near black](images/hospital-places-cards-charcoal-2026-11-15.png)
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
  (VOICE-AGENT.md §7.2). Built 2026-11-14 for this PC and the demo: §5.1.
- No service worker and no offline cache (§2).

### 5.1 The Content-Security-Policy and its partner headers
> **Part:** built · 2026-11-14 · for this PC and the demo; the deploy's server is not built.
> Fork test: 0 real forks (each choice below had one option left). As built: the end of this
> section.

**What the code allows (read 2026-11-14).** The built page (`console/dist-demo/index.html`) has
no inline script and no inline style. No template holds a `style="…"` attribute. Styles set
from code use `element.style`, which a policy does not block. The fonts are local files. A
person's photo is a `blob:` address (`api.ts` `personPhoto`). Voice audio plays from a
`MediaStream` (`srcObject`), which a policy does not govern. Every request (`fetch`, the
`/agent/watch` WebSocket, the `/agent/build` EventSource) goes to the page's own host.
`innerHTML` takes fixed app HTML only: `el()` in `page.ts`, the theme menu (`theme.ts`) and the
icons in `chat.ts`. No template holds a value from a call or a record.

**The page's policy** (the demo at `/demo/` and the deploy):

| Directive | Value | Why |
|---|---|---|
| `default-src` | `'none'` | Everything not named below is refused. |
| `script-src` | `'self'` | No inline script, no remote script, no `eval`. |
| `style-src` | `'self'` | The built CSS file only. |
| `img-src` | `'self' blob:` | The icons; a person's photo. No remote image. |
| `font-src` | `'self'` | The Liberation fonts. |
| `connect-src` | `'self'` | `fetch`, the WebSocket, the EventSource. |
| `manifest-src` | `'self'` | The installed app. |
| `base-uri`, `form-action`, `frame-ancestors`, `object-src` | `'none'` | No base change, no form post, no frame of the page, no plugin. |
| `require-trusted-types-for` | `'script'` | A string given to `innerHTML` and its partners throws. |
| `trusted-types` | `deets-html` | The one policy that may make HTML (below). |

- **Trusted Types.** `page.ts` makes the one policy, `deets-html`. Every place that sets
  `innerHTML` takes its HTML from that policy. A check in `npm run check` fails when
  `innerHTML`, `outerHTML`, `insertAdjacentHTML` or `DOMParser` appears outside `page.ts`.
  Chromium browsers (Edge, Chrome) enforce Trusted Types. A browser that does not know the
  directive ignores it, so nothing breaks there.
- **The partner headers:** `X-Content-Type-Options: nosniff`; `Referrer-Policy: no-referrer`;
  `Cross-Origin-Opener-Policy: same-origin`; `Permissions-Policy` with the microphone for the
  page itself only (the caller's seat) and every other feature off.
  `Strict-Transport-Security` (one year, this host only) on the demo; the deploy's edge sends
  it there.
- **The `/agent/*` replies** (JSON, the stream, the photo): `Content-Security-Policy:
  default-src 'none'; frame-ancestors 'none'` and `nosniff`. A reply opened as a page then
  cannot run anything.
- **Violation reports.** The policy names a report route on the agent (`POST
  /agent/csp-report`). Its log line names the directive and the blocked address's scheme and
  host, never the page's path or a sample. Without `'report-sample'`, a browser sends no
  sample. The route needs its row in DEV-API.md when it is built.
- **One source for the text.** The headers are one file in `console/`. The agent's dev server
  reads it at start; the deploy reads the same file. No second copy.

**On this PC (`npm run dev`).** Vite's dev server sends the same policy with two changes. Vite
adds each CSS file as an inline `<style>`, so `style-src` adds `'unsafe-inline'`. Vite's error
overlay sets `innerHTML`, so Trusted Types is not sent. Every other directive is the same, so
an inline script added by mistake fails on this PC first, not on the demo.

**Each place it runs.**
- **The call:** not touched. The policy is the console's only.
- **The console:** Edge, Chrome, Firefox 115 and Safari 16 (§2). Check on Safari that
  `'self'` matches the `wss://` WebSocket. If it does not, add the host's `wss://` origin from
  the server's own setting, never from the request's `Host`.
- **The deploy (A2 and B):** the same file, sent by whatever serves the built console
  (CLOUD.md).
- **Each EHR:** not touched. The console never talks to an EHR.

**Desk test (when built).**
1. Run `npm run demo`. Open the demo in Edge. Open DevTools › Console. Use every page: Chat
   (a typed call and a voice call), each Config page, Teams › People with a photo.
   Expect: no line that names Content-Security-Policy or Trusted Types.
2. In DevTools › Network, select the page's first request. Expect: the policy and the partner
   headers in Response Headers.
3. Open `/demo/` on a phone in Safari. Use Chat. Expect: the call connects (the WebSocket
   passes the policy).
4. Run `npm run dev`. Use the same pages on this PC. Expect: no policy line in DevTools ›
   Console, and no `csp:` line in the terminal.

**As built (2026-11-14).**
- **The file:** `console/security-headers.json`: the page's directives, the `dev` changes (a
  null drops a directive), the report path, the page's partner headers, the demo's HSTS, the
  API's headers.
- **The demo:** `agent/deets_agent/security_headers.py` reads the file at start
  (`SecurityHeaders`; a missing file stops the start). A middleware in `dev_server.py`
  (`response_headers`) runs outside the wall, so a refusal carries headers too. A path under
  `/demo/` gets the page's headers and HSTS; every other reply gets the API's.
- **This PC:** `console/vite.config.ts` builds the dev policy from the same file and sends it
  with the partner headers (`server.headers`). A reply from the agent through the proxy keeps
  its own.
- **Trusted Types:** `setHtml` and `el` in `pages/page.ts` hold the `deets-html` policy. The
  theme menu and Chat's four icons call `setHtml`. `scripts/html-sinks.mjs` is a step of the
  web scope in `npm run check`: it fails on `innerHTML`, `outerHTML`, `insertAdjacentHTML`,
  `DOMParser`, `createContextualFragment`, `document.write`, `srcdoc`, `eval` or
  `new Function` outside `page.ts`. The built bundle holds one `innerHTML`, in `setHtml`.
- **The report route:** `POST /agent/csp-report` (DEV-API.md §2). Tests:
  `agent/tests/test_demo_door.py` (the page's and the API's headers, a refusal's headers, a
  report's log line with no path or query, a bad report).

**Decided while building, inside the design (for his review).**
- **`report-uri`, not `report-to`.** A `report-to` report goes without cookies, so Access
  would refuse it at Cloudflare. A `report-uri` report from the page goes with them and passes
  both doors.
- **A report prints the first time** a directive and blocked pair is seen, then every
  hundredth time, so a page in a loop cannot flood the terminal.
- **The Permissions-Policy list** names only features that Chromium knows (microphone for the
  page; camera, geolocation, payment, USB, serial, HID, MIDI, screen capture off), so the list
  adds no warning lines to DevTools.
- **HSTS goes on every demo file,** with no `includeSubDomains`, so it binds the demo's host
  only.

### 5.2 Protected data on screen
> **Part:** built · 2026-11-14 · the owner: "add all 4". For this PC and the demo.

The policy in §5.1 controls what the page loads and runs. It does not stop the browser from
storing what the page shows, or from sending it to a vendor. These four rules close that gap.
Every view that shows protected data follows them.

1. **Nothing with data is stored.** Every `/agent/*` reply carries `Cache-Control: no-store`
   (the `api` headers). The page and its other files carry `no-store` too (the `cache` entry),
   so no disk cache keeps a reply on a shared PC, and Back after leaving the page does not
   show the old view again (no back/forward cache). Only the hashed files under `assets/`,
   which hold no data, keep their one-year cache.
2. **No other site may load a reply.** Every reply carries `Cross-Origin-Resource-Policy:
   same-origin`.
3. **No browser spellcheck in a field.** Edge's Microsoft Editor and Chrome's enhanced
   spellcheck send the typed text to the vendor, which has no BAA. `<body spellcheck="false">`
   in `index.html` is inherited by every field. A field for the agent's own words opts back in
   with `spellcheck="true"` (Config › Agent's and Config › System's word boxes). A new field
   that can hold protected data never opts in.
4. **No page translation of protected data.** The browser's "Translate this page" sends the
   page's text to Google or Microsoft. Each region that shows a call or a person carries
   `translate="no"`: Chat's lines, the code card, the sandbox list, the recorded and live call
   lists, the steps and tools logs, the timeline, the review's call and rows, and Teams ›
   People's cards. Menus, labels and buttons stay translatable. A new view that shows protected
   data puts `translate="no"` on its region.

**On this PC (`npm run dev`)** Vite sets its own cache headers on the files it serves; rules 2
to 4 apply as on the demo. Rule 1 holds for every `/agent/*` reply.

**Later, with their own work:**
- **Idle logoff and `Clear-Site-Data` at sign-out:** with staff sign-in (ideas/STAFF-SIGN-IN.md,
  M2).
- **Identifiers hidden until a click, each reveal audited:** a fork for the audit trail's
  design (SECURITY.md §8).
- **The hospital's browser policy:** a page cannot turn off extensions that read it, Edge's
  Copilot sidebar, browser sync, or enhanced spellcheck and translation for a whole hospital.
  Managed Edge and Chrome policies can. This goes on the list of what a hospital must do,
  with the deploy.
- **What no page can stop:** a screenshot, a phone camera, a person at the screen. The audit
  trail and the idle logoff are the answer there.

**Desk test.**
1. Run `npm run demo`. Open the demo in Edge. In DevTools › Network, select a request to
   `/demo/agent/catalog`. Expect: `cache-control: no-store` and
   `cross-origin-resource-policy: same-origin`.
2. Start a typed call in Chat and type a word with a spelling error. Expect: no red underline.
   On Config › Agent, type the same word. Expect: a red underline.
3. Right-click on Chat's lines › Translate to another language (Edge) or Translate (Chrome).
   Expect: the menus and labels change; the call's lines and the rail's lists do not.
4. Open a recorded call, go to another site, then press Back. Expect: the page loads again
   from the start, not the old view.

## 6. Desk test
**Passed, 2026-11-11** (the owner, in Edge). His notes, built the same day: no eyebrow over
Settings, its panel fills the page; the icon; a Timeline panel at the top of the rail and a
wider rail. Steps 5–7 (install, transparency, other browsers) are still open.

![His note, 2026-11-11: remove the eyebrow over Settings](images/console-settings-note-2026-11-11.png)
![His sketch, 2026-11-11: a wider rail (the red line marks its new left edge)](images/console-rail-sketch-2026-11-11.png)
1. `npm run dev`, then open the address in Edge. The page shows the top bar, the nav (Chat,
   Config, Rules, Teams and Analytics at the top; Security, Audit Logs and Settings at the foot), and
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
> **Part:** built · 2026-11-13 — desk test closed 2026-11-22.

1. The nav reads Chat, Config, Rules, Teams, Analytics at the top (since 2026-11-14), and
   Security, Audit Logs, Settings at the foot.
   Click **Rules**: its own nav reads Booking, Agents, Flags, Tools (since 2026-11-22); Agents,
   Flags and Tools say "Empty for now".
2. Click **Teams**, **Config**, **Security**, **Audit Logs** and **Settings** in turn. Each shows one panel:
   the title alone, then its own nav at the left (People · Agent, System · Auth,
   Roles · Config, Calls, Security · Personalization). The first item is chosen; the view says "Empty for now".
3. On one of them, click the second item. The pill slides and the view fades. Press ↑ and ↓:
   the choice moves. Reload: the same item is chosen.
4. Analytics looks and works as before, with no line under its title.
5. The radius trial: the toggles, Play / Stop, the picker, the text box, the bubbles, the nav
   pill, the theme menu and the help card all have 16 px corners, as the panels do. Decide if
   it stays.

## 7. Config › Voice (was Config › Cartesia)
> **Part:** built · 2026-11-23 · desk test open — **Config › Cartesia is now Config › Voice**
> (VOICE-VENDORS.md, the owner's V-3 A: one page for the voice). The page is `pages/voice.ts`;
> its data is `GET`/`POST /agent/voice/settings` and `GET /agent/voice/voices` (DEV-API.md §2).
> What changed: a **Vendor** row first in each of the two cards, now titled **Speaking (Sonic)**
> and **Hearing (Ink)**; every model, range, default, rate, turn profile and recommended voice
> comes from the voice catalog (ehr's `GetVoiceCatalog`), not from `config.py`; **Keys and
> facts** has one key row per vendor in use; Usage is titled **Usage (Cartesia)**. The settings'
> keys carry the vendor (`cartesia.speed`; VOICE-VENDORS.md §2.3). Everything else below is the
> same. Where the text below says `cartesia.ts`, `/agent/cartesia` or `config.py`'s constants, read
> `voice.ts`, `/agent/voice/settings` and the catalog. Desk test: §7.5a.
>
> **Part:** built · 2026-11-13 · desk test closed 2026-11-22 — the owner, 2026-11-13: "a subpage under config
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
  - **Voice (Sonic):** Model (`sonic-3.6`, shown only); Voice (Cartesia's stable voices for
    agents, checked live, then More voices; Listen shows, disabled: not built; §7.8); Voice list
    (when the list was checked, and Refresh); Speed and Volume (sliders); Emotion (Cartesia's
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
    this cycle's voice calls used (`/agent/resources`). It updates as you type. Since
    2026-11-22, **Plan price ($ a month)** (`plan_price_usd`; 0 is not set): Analytics › Cost
    turns credits into dollars with it (ANALYTICS.md §3.2).
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
  text boxes"; since 2026-11-15 the console's one table, TABLES.md §3.16): Add at the top right
  of the row's control, level with its name; each entry's cells edited in place, and its ✕. A
  new entry is blank; Save waits until each cell has words. "None yet" when it is empty.
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
  - **Selects** (the owner, 2026-11-13: the dropdowns ran past the card when narrow; "they only
    need to be wide enough to contain the widest option"): each select is as wide as its
    longest option, not the control column. In a card narrower than that, it shrinks to the
    card and ends its words with "…". No field, number or fixed value is wider than its card.
    Both Config pages. The cards keep their width: the sliders and the tables set it.
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
  name when saved (no credits; it needs the key). Since 2026-11-16 the page sends the id itself
  (§7.8); the name search is the fallback when no list is held.
- The offset is one number for the cycle, with no note; Credits left may go below zero.
- The versions' words: "Active | [MM-DD | n]", "MM-DD | n", "Default", "No settings store". The Default is made by the dev server on first view, by "default".
- Save returns the page to Read on the new Active version.
- An older version cannot be made Active again from the page yet: edit Active to match it and
  save (a new version). A "Make active" button is a small follow-up.

### 7.3 The settings store (as built, 2026-11-13)
> **Part:** built · 2026-11-13 · desk test closed 2026-11-22. The design before it was a change table per
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
- **Checked in Rust:** `ehr/src/settings.rs › voice_list()` (since 2026-11-23, built from the voice catalog, VOICE-VENDORS.md §8; before, the `VOICE` list) lists every setting and what it may hold
  (each range is Cartesia's; the same as `config.py`'s for the page). `ehr` refuses a set with a
  setting missing, given twice, unknown, out of range or the wrong kind, and names the setting,
  never its value. Since 2026-11-23 there is one list, so that test is gone; `test_ehr_serves_the_catalog_the_binary_prints` checks the adapters against the catalog. Before, a test failed if
  the agent's defaults and `VOICE` list different settings.
- **The path:** the page → the dev server (`GET`/`POST /agent/cartesia`, DEV-API.md §2) →
  `ehr`'s `ListSettingVersions`, `GetSettings`, `SaveSettings` (EHR-SERVICE.md §3) → Postgres. The
  console never opens the database (DATABASE.md §1).
- **The agent reads the active version** when a voice call starts (`dev_server.py ›
  active_voice_settings`), so a save applies to the next call, never the one in progress. Every
  lever is applied (`voice/cartesia.py › build_stt`, `build_tts`; `voice/__init__.py › VoiceMeter`) except the
  pronunciations: they are stored, and need a Cartesia pronunciation dictionary before Sonic can
  use them (§7.6). The Resources tab and Credits left take the plan's credits, the offset and the
  cycle day from it. Without Postgres, or before a save, the code's defaults apply.
- **Not yet:** the usage columns on `calls`; the `audit` row for each save (the activation row
  records who and when meanwhile); staff ids in place of "dev".

### 7.6 Open
- **Pronunciations:** make or update a Cartesia pronunciation dictionary from the saved table,
  and send its id with Sonic.
- **Make active:** restore an older version as a new activation.
- **Listen:** Cartesia's voice list can give each voice's preview file
  (`expand[]=preview_file_url`; it needs the key), which plays with no credits; the dev server
  would fetch it, so the page loads no remote file. A fork for the owner: Cartesia's sample, our
  fixed sentence (about 150 credits), or both. The list route (§7.8) is where it would go.

**Levers Pipecat or Cartesia take that the page does not show** (a review, 2026-11-13; each
needs the owner's fork before it is built; CARTESIA.md §9 has the full lever list):
- **Text aggregation, Sentence | Token** (Pipecat's `text_aggregation_mode`). Token mode starts
  Sonic on each token, not each whole sentence: about 200–300 ms sooner a sentence (Pipecat's
  note). The largest latency lever not on the page.
- **Text transforms** (Pipecat's `text_transforms`, find and replace before the text reaches
  Sonic). They could carry the Pronunciations table now, with no Cartesia dictionary. That
  removes the open question of whether ZDR covers the dictionary (CARTESIA.md §7), and the same
  table works with any voice vendor (ElevenLabs, the fallback).
- **Emotion: five of more than 60.** Pipecat's docs say Sonic 3 takes over 60 emotions, best
  with the voices Leo, Jace, Kyle, Gavin, Maya, Tessa, Dana and Marian. None of them is on our
  list of 10 voices for agents, so an emotion may do little on Skylar. Test before more words go
  on the list.
- **Language and accent** (Sonic): fixed to English. Ink-2 detects the language, but Pipecat
  labels every line English. A row only after Spanish is tested on phone audio (DESIGN.md C8).
- **Number reading by locale:** the page has Auto and Off; Cartesia also takes a locale
  (`en-US`, `en-GB`), which reads numbers that way whatever language is spoken.
- **`watchdog_min_timeout`** (0.5 s): dev only; no row needed.

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

### 7.5a Desk test: Config › Voice (2026-11-23)
**First:** stop the dev server (Ctrl+C) and start it again with `npm run dev`. It builds the new
`ehr` (with the `voice` crate) and the agent's Python changed.

1. Config's third item reads **Voice**, not Cartesia. Open it: the version dropdown still lists
   your old saves, newest first, and "Default" last.
2. The two top cards are **Speaking (Sonic)** and **Hearing (Ink)**. The first row of each is
   **Vendor**, set to Cartesia (the only vendor in the catalog today; the menu has one entry).
3. Choose an old version (one saved before today): it shows your values as you saved them
   (your voice, Offset, Plan price). Nothing reads as a default that you had set.
4. Press Edit, change Speed, press Save. The page goes to Read on the new Active version, Speed
   as set. Make a short voice call on Chat: it speaks in that voice and speed.
5. Speed's slider has its line at 1.00; the turn profile still moves the four turn sliders;
   Credits left still counts down from Plan credits less Offset.
6. **Keys and facts** has one row, "Cartesia API key: Set" (with your key in `.env`), the API
   version, and "Ink: 3 credits per second | Sonic: 1 credit per character".
7. Audit Logs: the save's row names only `cartesia.speed`, not every setting.
8. Optional: rename `CARTESIA_API_KEY` in `.env` and restart. Chat's voice call refuses with "a
   voice call needs CARTESIA_API_KEY in .env". Put it back.

### 7.8 The voice list, checked live (as built, 2026-11-16)
> **Part:** built · 2026-11-16 · desk test closed 2026-11-22. The owner's fork A (2026-11-16): the shortlist
> checked live, plus every English voice in a table to search and choose from.

- **Why:** the menu was a list copied by hand from Cartesia's "Choosing a voice" guide. Emma left
  Cartesia on 2026-11-16 and the page could not tell. Cartesia's API has no "recommended for
  agents" mark, so the shortlist's names stay in the catalog (`voice/catalog/cartesia.json`, `speaking.voices`); the API checks them.
- **The list:** the dev server reads Cartesia's List Voices (`voice_catalog.py`;
  `GET /agent/cartesia/voices`, DEV-API.md §2) for English voices, every page, with no credits.
  It holds the list for `TTL_S` and reads one at a time. Left out: archived voices, voices our
  organization owns, and pro clones (no cloning, CARTESIA.md §1). A failed read keeps the list
  it held, and says why with Cartesia's status.
- **Its own store** (`stores.ts › cartesiaVoices`, `leaves: true`): the settings never wait on
  Cartesia. When the list lands, the cards draw again and the draft stays.
- **The Voice menu:** each shortlist name, matched by name in the list. A name Cartesia no
  longer has reads "Name: not in Cartesia's list" and cannot be chosen. A name with several
  voices shows each with its tagline. The voice in the draft shows first when it is not in the
  shortlist. Then **More voices…**, only when a list is held.
- **The voice is its id** (a name can belong to several voices). The draft holds `{name, id}`;
  Save sends the id. The dev server checks it against the held list, or with Cartesia's Get
  Voice when the list lacks it, and refuses a voice Cartesia no longer has
  (`dev_server.py › resolve_voice`).
- **Voice list row:** "Checked N min ago: N English voices", or why it is not checked (no key;
  the dev server did not answer; Cartesia's status). Refresh reads again now (at most every
  `FRESH_GAP_S`). Like every control, it works in Edit only.
- **The table of every voice** (More voices…; the one table, TABLES.md): a card under Voice,
  as wide as the cards' columns. Name, Gender, Accent (the voice's own locale), Tagline, then
  Use ("In use" on the voice in the draft). The "i" opens the description, the other accents
  and the id. The search box on the title line (TABLES.md §6.3: Ctrl+F, Esc clears) matches
  every word in the name, tagline, description, accent and gender; the count reads "n of N".
  Past 200 rows the list windows (`list-window.ts`). Close shuts the card.
- **Decided inside his choice (for his review):** the 10-minute hold and the 15-second Refresh
  gap; English only (`language=en`); archived, owned and cloned voices left out; the table as a
  card in the page, not a dialog (the console has no dialog yet); the columns and their order;
  the words "More voices…", "not in Cartesia's list", "Checked … ago", "Use", "In use"; the
  Voice list row's help (`HELP.voiceCheck`, a draft for his words).
- **Not verified:** no read of Cartesia's real list has run yet (Claude's shell could not use
  the key). The answer's shape is from Cartesia's docs.

**Desk test** (restart `npm run dev` first):
1. Config, Cartesia. The Voice list row reads "Checking with Cartesia", then "Checked just now:
   N English voices".
2. Press Edit. Open the Voice menu: the shortlist names, and "More voices…" last. A name
   Cartesia no longer has is greyed with "not in Cartesia's list".
3. Choose More voices…: the "Cartesia's English voices" card opens under Voice, with the cursor
   in its search box. Type "calm": the rows narrow and the count reads "n of N". Esc clears.
4. Press a row's "i": its description, accents and id open under it.
5. Press Use on a voice: the menu now shows that voice first, its row reads "In use", and
   Discard | Save light. Save: the page says "Saved as …". Start a voice call: it speaks in that
   voice.
6. Press Refresh: the row reads "Checked just now". Close shuts the table.
7. Without `CARTESIA_API_KEY` in `.env` (restart): the row reads "Not checked: no Cartesia key in
   .env", the menu is the shortlist as before, and there is no More voices.

Signs that Cartesia's answer is not the shape its docs give (no real read has run yet): the
count is exactly 100 (paging stopped after one page), or every shortlist name is greyed (the
English filter matched nothing). Skylar and Daniel match by the ids in Cartesia's docs; the
other seven match by name until their ids are pinned in `voice/catalog/cartesia.json`.

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

## 8. Config › Anthropic
> **Part:** built · 2026-11-13 · desk test passed 2026-11-13 (wide screens; narrow screens to redo, §8.4) — the owner, 2026-11-13: "Let's chat a new config
> subpage for 'Anthropic'", then option A: the page chooses the model. Every lever and how it
> reaches the request: ANTHROPIC.md §4.1.

### 8.1 The view as built (`pages/anthropic.ts`)
- **Config's fourth item, Anthropic**, after Cartesia.
- **The same frame as Cartesia** (§7.1): Read | Edit, the versions dropdown ("Active |
  [MM-DD | n]", Default last), Discard | Save, an "i" on each row, an older version read only
  with its changed rows glowing, two columns when there is room. The shared parts are one file,
  `pages/config-kit.ts`, moved out of `cartesia.ts` with no change of behavior; each view gives
  its values, help words and cards.
- **The cards:**
  - **Model:** Model (Haiku 5.5, Sonnet 5.5, Opus 5.5); Effort (Low, Medium, High, Extra high,
    Max; the model's own default is marked); Thinking (Off | On).
  - **Reply and cache:** Max tokens (256–16,000); Retry cap (256–32,000, at least Max tokens);
    Cache life (5 minutes, Anthropic's default; or 1 hour).
  - **Usage:** Budget ($), then "−" and an Offset ($, to the cent) on the same row; Cycle day;
    **Dollars left**: the budget, less the offset, less what this cycle's call records spent.
  - **Key and facts:** the API key ("Set", or "Not set: add ANTHROPIC_API_KEY to .env"; never
    the key); Region ("US only (1.1× price)"); Features ("Generally available only"); Rates for
    the chosen model ("In: $0.10 | Out: $0.50 | Cache read: $0.01 per 1M tokens", in the form of
    Cartesia's Rates row).
- **The model decides what else is allowed.** Thinking Off is greyed, with the reason on hover,
  when the model cannot turn it off (Opus 5.5) or the effort is above High. Choosing such a model
  or effort turns thinking back on. The cards are drawn again, so Effort marks the new model's
  default and Rates show its prices. `ehr` checks the same rules on save (`settings.rs ›
  coherent`), so a set the API would refuse is never stored.
- **The layout:** Model | Reply and cache, then Usage | Key and facts; one column when narrow
  (`config.css › .cfg-body[data-layout="anthropic"]`).
- **The data:** `GET`/`POST /agent/anthropic` (DEV-API.md §2). The spend is `resources.py ›
  anthropic_cycle`: the `cost_usd` of each call record this cycle, plus the live call's. Like
  Cartesia's count, a floor: it cannot see requests outside these records.

### 8.2 Decided inside his choice (for his review)
- The cards' names and order; every row's name and help (`HELP` in `anthropic.ts`).
- Thinking as Off | On. "On" sends no thinking field (adaptive, the model's default); "Off"
  sends `disabled` on Haiku and `between_tools` on Sonnet (ANTHROPIC.md §4.1).
- A model or effort that forbids thinking off turns it on, rather than refusing the choice.
- The budget's Default: $100 a month (his API budget), cycle day 1; `ANTHROPIC_BUDGET_USD` and
  `ANTHROPIC_CYCLE_DAY` in `.env` set others. The offset is in dollars and cents.
- Region and Features are facts, not settings: US-only inference is ANTHROPIC.md §2's rule
  (it costs 1.1×), and GA-only is `llm.py`'s rule (betas are outside the BAA).
- No "caching off" switch and no pre-warm row: caching off only adds cost and latency; pre-warm
  is not built (§8.4).
- **What the page changes:** test calls from the Chat page only, from the next call. The tests
  and the typed harness always run with the code's values, so the recorded replies stay valid.
  The code's values make the same requests as before: no thinking field, no cache life field.

### 8.3 The store and the agent (as built, 2026-11-13)
- **A second service, `llm`**, in the same store (§7.3): `ehr/migrations/0004_settings_llm.sql`
  adds `settings_llm` (insert-only) and lets the version and activation tables name `llm`.
  `ehr/src/settings.rs › LLM` lists the nine settings; `coherent` checks thinking against the
  model and effort, and the retry cap against the first cap.
- **The dev server** (`dev_server.py`): the settings helpers now take a service (`active_settings`,
  `ensure_default`, `settings_view`, `save_settings`), so the voice and llm views share them. A
  test call reads the active llm settings once, when it starts: `build_llm(values=…)` sets the
  model, effort, max tokens, the retry cap, the cache life and thinking; the call's record opens
  with that model.
- **US-only inference is now sent** on every live request (`llm.py`, `inference_geo: "us"`). It
  is added after the request's key is made, so the recorded replies still match.
- **The record (schema 8):** `call_start` carries `llm` (effort, thinking, the caps, the cache
  life) and `inference_geo`. `cost_usd` now uses each model's own prices and cache read price
  (`call_record.py › cost_usd`), 1.25× or 2× for a cache write by the cache's life, and 1.1× for
  US-only inference. Records before schema 8 keep the cost they were written with.

### 8.4 Open
- **The desk test on narrow screens** (the owner, 2026-11-13: "we'll have to redo it for
  narrower screens"): §8.5 step 9, on both Config pages.
- **Pre-warm the cache** before the caller's first turn (`max_tokens: 0`, ANTHROPIC.md §4): not
  built. A row when it is.
- **`strict: true` on each tool** (ANTHROPIC.md §4): not set yet. It changes every request, so
  every recorded reply must be recorded again; the cost is to be counted and quoted first. A
  separate change.
- **Make active**, as for Cartesia (§7.6).
- **Per-message effort** (a cheaper effort for some steps of the call) is a beta: off while betas
  are outside the BAA.

### 8.5 Desk test
**First:** stop the dev server (Ctrl+C) and start it again with `npm run dev`. It builds the new
`ehr` and runs migration 0004.

1. Config, Anthropic. The dropdown reads "Active | [Default]". Model reads Haiku 5.5, Effort
   Low, Thinking On, Max tokens 1200, Retry cap 2400, Cache life 5 minutes, Budget 100, Offset
   0.00, Cycle day 1. Rates read "In: $0.10 | Out: $0.50 | Cache read: $0.01 per 1M tokens".
2. Dollars left shows $100.00 less this cycle's test calls (a few cents).
3. Press Edit. Choose Opus 5.5: Rates change to Opus's prices, Effort marks "Medium (Opus 5.5's
   default)", and Thinking's Off is grey (hover: "Opus 5.5 cannot turn thinking off").
4. Choose Sonnet 5.5, Effort High, Thinking Off. Then choose Effort Max: Thinking goes back to
   On, and Off is grey ("Off needs effort High or below").
5. Choose Haiku 5.5, Effort Low, Thinking Off. Press Save: "Saved as 11-13 | 1. Active from the
   next call".
6. Chat, Test, one typed call: it works, and its first reply comes sooner. In the catalog, the
   call's model is Haiku 5.5. (If the call fails at once, US-only inference may be refused for
   this account: tell me the error line.)
7. Back in Config, Anthropic: choose "Default" in the dropdown. Thinking's row glows. Choose
   Active again, Edit, Thinking On, Save: the next call thinks again.
8. Config, Cartesia: everything works as before (the shared frame moved to `config-kit.ts`).
9. On both pages, each dropdown is only as wide as its longest option. Narrow the window to one
   thin column: no dropdown or number box runs past its card's edge.

## 9. Config › Agent
> **Part:** built · 2026-11-13 · desk test closed 2026-11-22 — the owner, 2026-11-13: "have the agent subpage
> contain our agent prompt segments", five tabs of sections, and a Semantic | Flow bar. The
> design before it: PROMPT-EDITOR.md.

### 9.1 The view as built (`pages/agent.ts`)
> The owner's first look (2026-11-13): "visually cluttered", Semantic "a lot better for
> reference but could be improved", the cost should be real. Then his forks: plain text in Read
> with an animated change to Edit; the name left of the words; a free dry run for the cost; and
> "minimize jitter and inconsistency … sticking to UI theming and prior precedence".

- **Config's first item, Agent.** The same top row as the other Config pages (§7.1): Read |
  Edit, the versions ("Active | [MM-DD | n]", Default last), Discard | Save. Under it, the
  sections sit straight on the view's frame: no card edge inside it.
- **The bars:** the five section groups at the left (General, Open and close, Verify, Visits,
  Safety); **Semantic | Flow** at the right; they wrap when narrow. Both are the Read | Edit
  bar's style. The tab and the order are kept in this browser (a way to read, not a change);
  Read | Edit still opens on Read.
- **Test | cost, in the title bar** just left of Discard | Save (the owner, 2026-11-14, with his
  screenshot; until then Record and test was a quiet line under the bars). Two parts like the
  Chat page's Start | End: "Test", then the cost. The frame's `top` slot (`config-kit.ts`) holds
  it; `agent.ts › showTest` keeps it current. It is the same size as Discard | Save (the owner,
  2026-11-14): every part of both is `--action-part` wide, and the two have the same height.
  ![The owner's screenshot, 2026-11-14: the Record and test line under the bars, with an arrow to the space left of Discard and Save](images/config-agent-test-button-2026-11-14.png)
  - **Red** (`--stop`, each theme's End call red) while the kept recordings do not match the
    saved words: "Test | $?.??" until the cost is known. **The first press** works out the cost
    (the free dry run below): "Test | $0.02", or "<$0.01", or "$0.00" when nothing is new.
    **The second press** records and runs the tests.
  - **Working out the cost** (2026-11-18, the owner: "no feedback or progress bar"): a pale
    wash (`--on-call-button` at `--progress-wash`) fills in from the left, one step per script
    dry-run (the quote's `checked` count); the cost part reads "3 of 15". Not dimmed. Before
    this, the first press only dimmed the button for about 30 s.
  - **Running:** the green (`--go`) fills in from the left, one step at a time (each script,
    then the tests), so the bar is a count, never a guess of time; the cost part reads
    "3 of 15", then "Tests" for the last step.
  - **The step under way pulses** (both presses; `--dur-pulse`, down to `--progress-pulse-low`):
    the next part of the bar, so a long step still shows the run is alive. With reduced motion
    it holds still at the low opacity.
  - **A press that does not start** reads "Test | Not started"; the hover hint gives the dev
    server's reason, or says it did not answer.
  - **Green** (`--go`) once the recordings match: "Test | Passed"; it cannot be pressed.
  - **A failed run** turns it red again: "Test | Failed" until the next press works out a new
    cost. The hover hint says what failed ("2 scripts failed, the tests fail") and what was
    spent; counts only. No key in `.env`: dimmed red, and the hint names the key.
- **The quote is a free dry run** (dev_server.py › `estimate_record`), asked for by the first
  press (before 2026-11-14 the page asked by itself). The dev server replays each kept script
  with no Claude call (about 30 s); a script that misses a reply needs recording, and its quote
  is what recording it cost last time (`agent/tests/RECORDED-COSTS.json`, written after each
  passing run). It is an over-estimate: the lines before the first changed one replay for free.
  Measured on 2026-11-13, recording all 14 scripts costs about $0.017 (Haiku 5.5, with US-only
  inference); the old fixed quote said $0.14. A quote that fails on an error ends as "failed"
  (2026-11-14), and the next press records without one.
- **Semantic:** the tab's sections under up to four headings, each with its count:
  Instructions (role, step tasks, notes), When to act (each action, then its fields), Caller
  hears, Text message. **Flow:** the same sections in the call's order, each with its tag over
  the name (Instructions, When to act, Field, Caller hears, Text message).
- **A section** is a Config row: its name in the row's name column (`--cfg-label`), its words
  beside it, up to `--prompt-max` wide so long sections wrap less. A field's name is inset under
  its action's (`--prompt-indent`); its words stay aligned with the rest. The key
  (`line.greeting`) shows on hover over the name. Under the name, the placeholders the section
  keeps ("Keeps {visit}"). Narrower than a Cartesia card, the name sits over the words.
- **Read shows the words as text; Edit shows them as boxes.** The text and the box are two
  layers in one place, with the same padding, edge width, type and wrapping, so the box fades in
  over the text and nothing moves (the help card's fade, `--dur-collapse`; none with reduced
  motion). In Read each placeholder is marked in the text with a soft wash and no padding, so
  it takes its exact width in the box.
- **What the caller hears or reads** (Caller hears, Text message) has a bar at the left of its
  words, so speech reads as speech in either order.
- **A problem** (a missing or extra placeholder, a stray brace, empty words) shows one line in
  the error color under the words, the box's edge turns the error color, its tab shows a red
  dot, and Save stays off with the reason on hover ("Verify › Read back details: Needs
  {birth}"). With no problem, no line, so nothing moves while you type.
- **An older version:** the changed sections glow (the same glow as §7.1), and each tab with a
  change shows a dot in the accent, in either order.
- **The names** are the owner's list of 2026-11-13 (`SECTIONS` in `agent.ts`; a test fails if a
  section is missing or named twice).
- **Tokens:** `--prompt-max` (1,100 px), `--prompt-indent` (new); every other value is an
  existing token. **The shared frame** (`config-kit.ts`) gained: textareas follow Read; the
  glow on any keyed section; a view may hold Save back with a reason (`blocked`); a redraw may
  go to the top (a tab change).

### 9.2 Decided inside his choices (for his review)
- The heading and tag words; "Keeps {…}" under the name; the problem lines ("Needs {visit}",
  "A { or } with no partner", "Empty", "{x} is not filled in by the agent"); the record line's
  words.
- **The placeholders are checked, not locked.** The design said a placeholder would be a tag
  you cannot delete. A locked tag inside a text box is fragile in a browser (a paste or an undo
  can still remove it), so the page names them under the section's name and refuses the save
  until the words hold them. ehr checks the same rule on save.
- **Fields stay in Semantic's "When to act"**, each under its action (the "How to fill in"
  heading went: it repeated each action's name).
- **The speech bar** is the frame's edge color, two hairlines wide; the console's type is
  already the serif, so no other type marks speech.
- **Under a cent** reads "under $0.01": most scripts cost a tenth of a cent.
- Record and test runs every kept script with `--no-record` (no new call records in the
  catalog), then the whole agent test suite except the stamp's own test, then a dry run that
  writes each script's cost for the next quote. It cannot start while a quote is being worked
  out.
- The page opens on Semantic and General the first time.

### 9.3 The store, ehr and the agent (as built, 2026-11-13)
- **A third service, `prompt`** (`ehr/migrations/0005_settings_prompt.sql`): `settings_prompt`,
  one row per section, insert-only. `settings.rs › PROMPT` lists the 81 sections (78 until
  2026-11-14) and the
  placeholders each must hold; `Kind::Prose` checks each one (not empty, at most 12,000
  characters, no control character but a line break, the exact placeholders). A test fails if
  it differs from `prompts.py › PLACEHOLDERS`.
- **The words a save stores** are each section as the agent reads it: paragraphs apart by a
  blank line, list items on their own lines, no wrapping.
- **The prompt file stays the source for git and the tests.** A save also writes the file
  (`prompts.write_file`): the notes and every unchanged section keep their own lines; a changed
  section is wrapped at 96 columns, never so that a line starts a list item or a heading.
- **The Default is the file**, the first time the store sees it. At start-up the dev server
  compares the file with the active version; when they differ (an edit by hand, another
  branch), the file becomes a new active version, saved by "file" (`sync_prompt_file`).
- **Each test call takes its own copy** of the active version when it starts
  (`dev_server.py › active_words`, `prompts.use`): every task of the call reads it, so a save
  during the call changes nothing in it. The 911 line is read from the copy when spoken
  (`flow.py › emergency_line`), no longer at import.
- **ehr's text message:** `OpenCall` carries the call's `prompt_version`; ehr reads that
  version's `text.code` once and sends the code with it (`service.rs › prompt_text_code`). 0
  (the tests, the typed harness, no store) uses the file compiled into ehr.
- **The record (schema 9):** `call_start` carries `prompt_version`, the version's label
  ("MM-DD | n", "Default", "file"); the `calls` table has a `prompt_version` column.
- **The recordings' stamp:** `agent/tests/RECORDED-PROMPT.sha256` is the hash of the words the
  kept replies were recorded and tested with. A test fails while it differs from the file, so
  `npm run check` says when Record and test is due. Record and test writes it when every script
  ran and the tests pass.

### 9.4 Open
- Who may edit which sections (ACCESS-ROLES.md: today the Agent lever covers every section); each hospital's own sections on top of
  ours; an analyst's draft and its test before it is used (PROMPT-EDITOR.md §7, §8).
- **Make active** for an older version, as on the other pages (§7.6).
- The old replies Record and test leaves behind (each changed request is a new file in
  `tests/replays/`): a cleanup that keeps only the replies the tests read.
- The audit row for each save (the activation row records who and when meanwhile).

### 9.5 Desk test
**First:** stop the dev server (Ctrl+C) and start it again with `npm run dev`. It builds the new
`ehr` and runs migration 0005. The first view makes the Default from the prompt file.

1. Config, Agent. The dropdown reads "Active | [Default]". The bars read General … Safety and
   Semantic | Flow. "Recordings match the saved words".
2. General shows "Instructions 1" with Role and rules, then "Caller hears 5". The names sit in
   a column at the left; the words read as plain text, with no boxes. Hover a name: its key.
3. Press Flow: the same sections in the call's order, each with its tag over the name. Open
   Verify: the names First name, Last name and Date of birth sit inset under Read back details. Leave and come back to
   Config: the page opens on Verify and Flow.
4. Press Edit: boxes fade in over the words. In Verify › Read back details (Caller hears),
   delete `{birth}`: "Needs {birth}" shows in red under the words, the box's edge is red, Verify's tab shows a red dot, Save stays off (hover
   says why). Put it back: all clear.
5. In Open and close › Goodbye, change the words. Press Save: "Saved as 11-13 | 1. Active from
   the next call". The Test button turns red, "Test | $?.??". `git diff` shows only that
   section changed in `AGENT-PROMPT.md`.
6. Chat, Test, a typed call that ends: the agent says the new goodbye. The call's record (the
   newest file in `agent/calls/`) has `"prompt_version": "11-13 | 1"` on its first line.
7. Config, Agent (the Test button since 2026-11-14): press Test once; after about 30 s it
   reads "Test | $0.02" or the like. Press again: the green fills in as the 15 scripts and then
   the tests finish, and the cost part counts "3 of 15". After a pass it is green, "Test |
   Passed"; after a failure, red, "Test | Failed", with what failed under the pointer.
8. Choose "Default" in the dropdown: Goodbye glows, and the Open and close tab shows a dot.
9. Read and Edit: press Edit and Read a few times on a long section (Role and rules). The box
   fades in and out over the words; no word moves. Each placeholder in Read sits exactly where
   it sits in the box.
10. Narrow the window: under one Cartesia card's width, each name moves over its words.

### 9.6 Changed by Config › System (2026-11-14)
- **The rule** is now SYSTEM-PAGE.md §3.1: a section keeps its own code values ("Keeps
  {today}" under Role and rules) and may hold any System variable. `{clinic}` is a System
  variable, so Role and rules, Step 1 and Greeting no longer list it. The problem lines add
  "{x} is not a System variable", "{x} is filled only in its own section", "`x` is not offered
  in this step" and "The caller hears this: no tool names here".
- **The lists:** in Edit, `{` and " ." open the lists (`word-picker.ts`). In Read, a tool in
  backticks shows as a chip (`.prompt-tool`).
- **The tab bar** moved to `tab-bar.ts`, shared with System.
- **Another page may show a section:** the `deets:show-section` event opens its tab, scrolls to
  it and flashes it once (`.prompt-flash`, the glow's look).

### 9.7 Arranged by the call (2026-11-13)
The five tabs and the hard-coded section list gave way to the call's steps, each line under its
tool, and a "plays when" for every section: AGENT-PAGE.md is the design, the as-built record
and the desk test. §9.1 to §9.6 hold for everything else on the page.

**One layout, Plain | Code (2026-11-14).** Semantic | Flow gave way to Plain | Code: both show a
block per tool; Code adds the tool's name and each section's key. The lines' titles now start
with a verb. AGENT-PAGE.md §8 has the decision, the as-built record and the desk test.

## 10. Config › System
> **Part:** built · 2026-11-14

The variables and the tools: SYSTEM-PAGE.md is the design, the as-built record (§6) and the
desk test (§5). Code: `console/src/pages/system.ts`, `word-picker.ts`, `tab-bar.ts`; the
`system` service in the store (`ehr/migrations/0006_settings_system.sql`). Since 2026-11-15
`system.ts` exports its table parts (`table`, `row`, `field`, `heading`, `grow`) for Config ›
Hospital.

### 10.1 Config › Hospital
> **Part:** built · 2026-11-15

The hospital's lists and numbers, after System in Config's nav (Agent, System, Hospital,
Cartesia, Anthropic): HOSPITAL-CONFIG.md is the design, the as-built record (§5) and the desk
test (§6). Code: `console/src/pages/hospital.ts`; the `hospital` service in the store
(`ehr/migrations/0009_settings_hospital.sql`). Four tabs of tables in System's style, each row
edited in place; no new CSS.

## 11. The compact layout: the nav on a narrow screen
> **Part:** built · 2026-11-14 — desk test closed 2026-11-22.

The first step toward a console on a phone. Only the shell changes here. Each page's own
narrow layout (Chat's stage and rail, the Config rows) comes after. The tracker for all of it,
page by page, is [MOBILE-UI.md](MOBILE-UI.md).

![The owner's screenshot, 2026-11-14: the console in a narrow window. The green box marks where the nav collapses to (the current page's pill, under DeetsHealth); the purple box marks where the page's own nav goes (the page title's place, at the right); the panels take the space below](images/nav-compact-layout-2026-11-14.png)

### 11.1 As built
- **When:** at a window width of `--compact` (720 px) or less. Width only: a tablet with a
  keyboard keeps the side nav. `tokens.ts › compactQuery()` reads the token; the CSS repeats
  720px in its `@media` rules (a media query cannot read a token).
- **The compact bar** (`index.html`, `#compact-bar`): a row under the top bar. At the left, the
  page dropdown (the green box). At the right, the page's own nav as a dropdown (the purple
  box). The two dropdowns share the row in equal halves: the name at the left of each pill, ▾
  at its right; a menu is at least as wide as its pill (the owner, 2026-11-14, §11.4's
  screenshot). Under the row, the page fills the full width. The side nav, each page's title and each
  page's own nav are hidden (`app.css`, `subnav.css`, `chat.css`).
- **The dropdown** (`picker.ts`): the button is the nav's accent pill with "▾" after the name.
  The menu is the theme menu's glass. Its rows are the nav's items, and the current one is the
  accent pill. The page menu puts Security, Audit Logs and Settings after a divider. The
  right-hand menu hangs from the right edge.
- **One list, two views.** A pick runs the same `go` as a click in the nav (`nav.ts`,
  `subnav.ts`, Chat's `goMode`), so the saved page, the saved view and the fade are the same.
  A page gives its dropdown through `Page.picker()`; `nav.ts` puts it in the bar when the page
  shows. A page with no own nav leaves the right end empty.
- **Keyboard:** ↓ or ↑ on the button opens the menu on the current row. ↑ ↓ Home End move.
  Enter picks. Escape closes and returns to the button. Tab or a press outside closes it.
- **Touch:** with `pointer: coarse`, the button and rows are at least `--tap-min` (44 px)
  tall. The hover washes of the nav, the menus and the app name now apply only with
  `hover: hover`, so a tap does not leave them on.

### 11.2 Decided inside his choice (for his review)
- 720 px for `--compact` and 44 px for `--tap-min` (the drafts of 2026-11-14).
- The current row in the menu is the accent pill, as in the nav, not the theme menu's dot.
- The page's title is hidden, because the left button already names the page.
- The menus open and close at once, as the theme menu does (no motion).
- Not yet: `:hover` on the other controls (Chat's buttons, the Config rows) still shows on
  touch; the safe areas (`env(safe-area-inset-*)`) and the on-screen keyboard
  (`visualViewport`) wait for each page's own narrow layout.

### 11.3 Desk test
1. `npm run dev`, open the console, and make the window narrower than 720 px (or use the
   browser's device mode, F12 › the phone icon). The side nav goes. Under **DeetsHealth**, a
   pill reads the current page with ▾. At the right of the same row, a pill reads the page's
   own choice (on Chat: Testing, Demo or PHI). The panels fill the width below.
2. Tap the left pill. The menu lists Chat, Config, Rules, Teams, Analytics, a line, then
   Security, Audit Logs, Settings. The current page is the accent pill. Pick **Config**: the
   page fades to Config, and the right pill now reads Config's view.
3. Tap the right pill and pick another view. The view changes. Reload: the same page and view
   open.
4. On Chat, pick **Demo** in the right pill. The mode changes with its wave, as from the nav.
5. Make the window wide again. The side nav comes back with its pill on the same page and
   view. An open menu closes.
6. Keyboard: Tab to a pill, press ↓, move with ↑ ↓, press Enter. Press Escape in an open menu:
   it closes and the focus returns to the pill.
7. In device mode with touch, tap a row: no hover wash stays behind on the nav or the menus.
8. The two pills in the compact bar are equal halves of the row, with a gap between them.
9. On Chat in Testing (§11.4): the canvas's top row holds only "Chat Title (Optional)", full
   width. Text | Voice and Local | Epic sit under the text box, side by side, as wide as the text
   bar. Switch Text to Voice and Local to Epic: the pills slide. Pick **Demo**: the speed bar sits
   under the canvas, full width. Pick **PHI**: no row. Make the window wide: the bars go back to
   the canvas's top right.
10. On a narrow Chat: **Start** and **End call** each take half the canvas's width. In Demo,
    **Play** and **Stop** do the same. After a code text arrives, the code button sits at the
    left and the two halves share the rest.

### 11.4 Chat on a narrow screen
> **Part:** built · 2026-11-14 — desk test closed 2026-11-22 (§11.3, step 9).

![The owner's screenshot, 2026-11-14: the compact Chat page. The arrows ask for the two pills to meet in the middle (each half the row), and for the Text | Voice and Local | Epic bars to move under the text box](images/chat-compact-toggles-2026-11-14.png)

- At `--compact` or less, `chat.ts` moves the seat bar (Text | Voice), the EHR bar (Local |
  Epic) and the speed bar (Demo) from the canvas's top row into `.chat-toggles`, a row under the
  text box. When the window grows, they move back. The bars keep their handlers; each pill
  follows on its bar's next resize.
- The row is as wide as the text bar. Its bars share it in equal parts, and each bar's parts are
  equal (decided inside his ask). PHI has no row.
- The top row keeps the call's title alone, at the full width and the text box's height. In
  Demo the top row is gone, as its only control moved.
- Start | End call fills the canvas's width, each button one half (the owner, 2026-11-14).
  Demo's Play | Stop does the same (decided inside his ask). When the code button shows, it
  keeps its own width and the split takes the rest.

## 12. Speed: the rules for every page

> **Part:** guide · 2026-11-15 (from the slow Hospital page after an import: TABLES.md §6.1;
> the owner, 2026-11-15: "doc for future build as we build out more and more pages")

Hospital froze after an import because of four habits that cost nothing on a list of 10 rows
and a lot on 400. Each new page, panel or list follows these rules from the start. Walk them
with DeetsMusic's build list (CLAUDE.md › Working style).

### 12.1 What a page draws
1. **Plan for the long list.** Ask how long each list can get in a real hospital (Places'
   limit is 2,000 rows), not how long it is in the sandbox. A list that can pass 200 rows goes
   through `fillTable` or `fillGrid` (`list-window.ts`, TABLES.md §6). Never append every row.
2. **A page that holds a long list has a search box** (the Hospital one: `Search`, the box on
   the tab bar's line, Ctrl+F). It searches the data, not the screen. Move `Search` and the box
   to a shared part the second time a page needs them.
3. **Pages are kept.** `nav.ts` and `subnav.ts` build a page once and keep it, so what a page
   holds in the page's tree costs it on every visit. Keep that tree small.
4. **One scroll per page.** Never a box that scrolls up and down inside the page's scroll
   (TABLES.md §6.3, W1). A table's box scrolls only sideways.
5. **Build a shared part once per draw,** not once per row: a select's option list, a name
   lookup, a group of rows by their parent (`departments()`, `specialtyOptions` in
   `hospital.ts`).

### 12.2 What the code does each frame
1. **Never read layout in a loop that also writes it.** Each write followed by a read
   (`offsetWidth`, `scrollHeight`, `getBoundingClientRect`) forces a layout of the whole page.
   Write all, then read all, then write all: `growAll` in `data-table.ts` is the pattern.
2. **Work that follows a key press runs once a frame, not once a key:** a check, a count, a
   search's redraw (`requestAnimationFrame`, as `changed` in `config-kit.ts`).
3. **No work in proportion to the whole draft on each key.** Compare only what changed, and
   make the saved side once: a saved version is never edited (`savedJson` in `hospital.ts`).
4. **No scan inside a scan.** A lookup inside a loop over rows becomes a `Map` built once.
5. **One observer per element.** A `ResizeObserver` or a document listener made in a draw is
   let go before the next draw, or made once for the view's life.
6. **A redraw of a part, not the page,** when only the part changed, so the cursor and the
   scroll place stay (a search redraws the tab's sections, not the card).
7. **Controls drawn after the page's draw take its mode:** `kit.apply(nodes)` on a Config page.
8. **A row's state lives on its draft object,** not on its element: an open "i", a mark, a
   height. A windowed row is drawn many times.

### 12.3 What it reads
- Every read goes through a store (LOADING.md): one read per kind of data, the newest answer
  wins, no read in a loop. A page never fetches inside a row's draw.

### 12.4 How to find a slow page (the recipe, 2026-11-15)
1. **The agent or the console?** Time the read: `curl -w "%{time_total}"` on
   `http://localhost:<vite port>/agent/<route>` (Vite adds the token), or
   `window.deetsLedger.recent()` in dev. Hospital's read was 12 ms: the cost was the console.
2. **A fresh tab:** open the page in a new tab. A fresh tab that is fast and an old tab that is
   slow point to what the old tab keeps (a kept page with its rows).
3. **The page's own work:** in the browser's console, a `PerformanceObserver` on
   `long-animation-frame` names each script that ran in a slow frame and its forced layout
   time; time a draw with `performance.now()` around it.
4. **Test in a window in front.** Chrome slows the frames of a window behind another one, on
   every tab alike, so a frame rate measured there is not the page's. Time the script, or ask
   the owner to look in his own window.
5. **Count what is in the page:** `document.querySelectorAll("*").length`, the rows, the
   `option`s, the `textarea`s. Hospital after the import, before windowing: every department's
   row, with a 118-option select in each.

## 13. Access by role in the console
> **Part:** built · 2026-11-18 · desk test passed 2026-11-19. ACCESS-ROLES.md §6, Hand-over 3;
> the owner's fork 3A (2026-11-15): None hides, View reads with a "View only" mark, Edit edits.

- **One place asks:** `access.ts` reads the `me` store (`GET /agent/me`) and answers `may(lever,
  level)`; the nav, the sub-navs, Config's frame, Chat, Auth, Roles and Audit Logs ask it, and
  each redraws when your access changes (`onAccess`). Until `me` first answers, everything
  shows (the first-load wave waits for it). The server refuses on its own either way
  (DEV-API.md §2a).
- **The nav:** a page is listed while you hold any of its levers at View (`Page.levers`; a sub-nav
  page takes them from its views). With no role at all: **No role yet** and Settings only
  (`pages/no-role.ts`). When the page or view shown is no longer yours, the nav moves to the
  first one left. Each page opened is recorded in the audit trail (`POST /agent/log`).
- **The levers:** Chat by Testing or PHI; its modes Testing (Testing Edit), Demo (Testing View),
  PHI (PHI View). Config › Agent and System by Agent, Hospital by Rules, Cartesia and Anthropic by
  Vendors. Rules by Rules. Teams by People. Analytics by Analytics. Security › Auth by People,
  Roles by Roles. Audit Logs by Audit logs. Settings: none.
- **View only:** Config's frame (`config-kit.ts`) stays in Read: no Edit, no Discard | Save, no
  Test, and a **View only** mark beside the version. Auth at People View: no Add, no ✕, the card's
  boxes read only. Roles at Roles View: the cells do not change, no Add, no Discard | Save.
- **Security › Auth:** the Role column names each person's roles ("No role" when none). In a
  person's panel, each role is a chip; at Roles Edit a chip has ✕ (revoke) and **Add a role**
  lists the roles they lack (grant). Never on your own row. ehr's refusal shows in the panel's
  status ("you may not give more access than you have").
- **Security › Roles:** the active roles from the store, in two tables (Customer, DeetsHealth
  Team). A cell click and Add change a draft; Discard | Save on the title row save the whole list
  as a new version, active at once. A new role's id comes from its name. A role's panel: its
  name, **Remove role**, and the users who hold it.
- **Audit Logs** (the trail and the page: [AUDIT.md](AUDIT.md)): Config, Calls and Security, each the trail's rows of its kind, newest first, 100
  at a time (**Older**, **Newest**), in Auth's look: when, who (a name from the People list), their
  roles, the action, what it touched, the fields that changed, the versions before → after,
  Allowed or Refused (with the lever it lacked). **Check chain** walks the whole trail. The page's
  own look is the owner's fork (ACCESS-ROLES.md §6, part 3, step 6); this is the stand-in.
- **A refusal for want of a role** reads "Your role does not allow this (needs Agent Edit)", not
  "the agent's dev server refused" (`api.ts › roleRefusal`).

### 13.1 Desk test
1. Restart `npm run dev` (migrations 0011 and 0012 run once). Open the console with the launch
   token: every page shows, as before. **Audit Logs › Security** lists your page opens.
2. **Security › Roles:** nine roles in two tables. Click a cell of Intern: it moves to the next
   level, and Discard | Save light. Save: "Saved as MM-DD | n. Active now". Discard a second
   change: it goes back.
3. **Security › Auth:** open a guest's row. Add a role: **Junior Analyst**. The chip shows, and the
   Role column reads it. Open your own row: no ✕, no Add a role.
4. As that guest (the demo, or another browser through Access): only Chat (Testing, Demo),
   Config (Agent, System, Hospital, read only with **View only**), Rules, Analytics and Settings
   show.
   Config › Agent has no Edit and no Test.
5. Revoke the guest's role (✕ on the chip). Within 30 s the guest's next page shows **No role
   yet**, with Settings.
6. Try to revoke your own Deets from another Deets holder's view, or remove the last Deets
   holder: refused, with the rule in the status line.
7. **Audit Logs:** each step above is a row (Config: the roles save; Security: the grant and
   the revoke, the refusals). **Check chain**: "Every row is as written".
