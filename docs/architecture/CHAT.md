---
status: built
desk_test: open
sources: [console/src/pages/chat.ts, console/src/styles/chat.css, console/src/agent/transport.ts, console/src/agent/api.ts, agent/deets_agent/dev_server.py, agent/deets_agent/catalog.py, agent/deets_agent/rtvi_json.py, scripts/dev.mjs, docs/architecture/images/chat-sketch-2026-11-11.png]
updated: 2026-11-12
---
# DeetsHealth — The Chat page

> The console's Chat page: one call in a chat view, in three modes. The owner's design,
> narrated 2026-11-11 over his sketch. The shell it sits in is [CONSOLE.md](CONSOLE.md). The
> connection is the hybrid (§5), built 2026-11-12 (§6); the catalog of calls is §7.

![The owner's sketch of the Chat page, 2026-11-11](images/chat-sketch-2026-11-11.png)

## 1. The parts (the owner's words, by the sketch's colors)
- **The chat canvas** (black): a panel stacked on the stage. A little translucent, about 85 %
  opaque, for the stacked-glass feel. The same rounding as every other panel. The color of
  the main panel, pushed darker: toward black (Dark), coffee beige (Light), navy (Moonlight).
- **The agent's lines** (green): bubbles on the left.
- **The caller's lines** (orange): bubbles on the right.
- **The line facts** (dark blue): a row under each bubble. For the agent: the time to make the
  text, the time to make the speech. For the caller: the time to transcribe, the time to reach
  the agent.
- **The mode toggle** (yellow): above the canvas, at its top right. One button that flips its
  word, gently, through three modes:
  - **PHI** — the watch view: observe a call. Read only.
  - **Test** — the analyst is the caller.
  - **Demo** — a recorded call plays back as if it were live.
  It is tied to Pipecat's PHI setting.
- **The help box** (light blue): bottom left, in Test only. The sandbox patients the tester may
  use.
- **The text box** (red) with **the send button** (green): the caller's line. Not shown in PHI
  mode.

## 2. The forks and the answers (the owner, 2026-11-11)
1. **Where a test call starts and ends:** in the canvas, at the bottom right, mirroring the help
   box. One rounded rectangle split in two by a hairline: **Start** on the left, tinted green,
   **End call** on the right, tinted red. The greens and reds by theme: forest and scarlet
   (Dark), grass and burgundy (Light), turquoise and purple (Moonlight). The first press on End
   call turns its word to **Confirm?**; the second ends the call. The buttons are large, and
   nothing moves when a word changes ("make the buttons large enough where jitter does not ever
   occur").
2. **The help box:** inside the canvas, at the bottom left (his sketch). One press opens it,
   the next closes it, gently. The canvas keeps a row at its foot for these controls; the help
   box opens upward from it, over the lines, because it is not open all the time.
3. **Demo's controls:** the text bar becomes a player, in the same button style: Play | Stop in
   the canvas where Start | End sit, and in the bar a call picker and the speed, 1× | 2×.
4. **"Tied to Pipecat's PHI setting"** (confirmed by the owner, 2026-11-12: "Yup, works"). The
   toggle picks where the data comes from and how much protected data the watch stream may
   carry:
   - **PHI:** live calls. In development every field shows, because the data is synthetic. In
     production each field shows only as far as the viewer's role allows, and each view writes
     an audit row (SECURITY.md §4).
   - **Test and Demo:** synthetic data only; every field shows.
   - **The caller's seat** keeps RTVI's `function_call_report_level` at `name` in every mode, so
     the browser never gets a tool's arguments or result through RTVI. The rail panels get them
     from our watch stream, where each field carries its PHI mark.
   - Why not RTVI's own setting alone: RTVI belongs to the caller's seat, and PHI mode has no
     caller seat in the browser, so that setting would control nothing there.
   **To change it:** the levels live in the agent's dev server (the RTVI observer's parameters)
   and in the watch stream's per-field marks; the toggle itself only picks a mode.

## 3. As built
> **Part:** built · 2026-11-12 — the UI. Its connection is §6.

`console/src/pages/chat.ts`, `console/src/styles/chat.css`; the tokens in `skin.css` (chat),
the roles in `themes.css`, the six call paints in `palette.css`.
- **The toggle** sits right of the title. A press turns the word over on its horizontal axis
  (`--dur-flip`, 240 ms: half out, half in); instant with reduced motion. The mode is kept per
  browser (`deets.console.chat.mode`); Test is the default. Each mode keeps its own lines.
- **The canvas:** `--canvas` per theme, 85 % opaque, `--radius`. The lines scroll above the
  control row.
- **Start | End call:** each half is 132 px by 44 px (`--split-half`, `--control-height`).
  Both words of End call sit in one grid cell, so "Confirm?" never resizes it. Confirm? waits
  3 s (`--confirm-hold`), then turns back; Esc turns it back at once.
- **A test call:** Start calls the agent (§6). The greeting comes in, the text box opens, and
  each line you send comes back as a caller bubble, then the agent's answer. End call, then
  Confirm?, hangs up; when the agent ends the call, the canvas says why.
- **The help box:** "Sandbox patients": Jordan Ellis, Priya Raman, Casey Morgan (with the
  Morgen twin), the code 246810. It opens over the lines with a row-height and fade
  transition (`--dur-collapse`, 220 ms). The list is in `chat.ts`, kept in step with
  `ehr/src/verify.rs` until the dev server sends it.
- **Demo:** the picker lists every call in the catalog (§7), starred first, then newest: a
  call's label, or its time, outcome and scenario. Play fetches its events and draws each one
  at its own time, divided by the speed (1× or 2×); Stop ends it.
- **PHI:** the live call, whoever is its caller (in dev, a test call in another tab or
  window); "No live call to watch" when there is none.
- **The wave** (the owner, 2026-11-12: "animate the buttons appearing to be gentle and like a
  wave as a whole ... when we change between surfaces, all visual buttons go"). On a mode
  change, every visible control (the help button, the split buttons, the text box, the send
  button, the picker, the speed pair) fades out together in 120 ms (`--dur-wave-out`) while
  the toggle's word turns away. Then the new mode's controls come in as one wave: top row
  first, left to right, each 60 ms after the one before (`--wave-step`), each rising 6 px
  (`--wave-rise`) and fading in over 360 ms (`--dur-wave-in`, `--ease-wave`). The same wave
  plays each time the Chat page comes into view. The animation is not held, so at rest
  nothing is transformed. With reduced motion there is no wave. A control joins the wave by
  carrying `data-wave` (`chat.ts`); the toggle itself only flips.

**Decided inside his choices** (for his review): the theme-to-color order (his list's order
against Dark, Light, Moonlight); the exact shades (`palette.css`), each dark enough for white
text; "Confirm?" turns back after 3 s; the send button is a 44 px square in the theme's green
with an arrow; the help button's label "Sandbox patients"; the speed's chosen half takes the
accent color; the drafts below are built as drafted.

## 4. Drafts for the owner
- **Bubble colors:** the agent's is a light glass wash; the caller's is the theme's accent.
- **The line facts:** agent "text 0.8 s · speech 0.3 s" and a "fixed line" mark; caller
  "transcribe 0.2 s · to agent 0.05 s". Only numbers that exist show; a typed call has none
  yet, so no facts row shows.
- **The toggle's flip:** 240 ms, on its horizontal axis.

## 5. The connection (the owner, 2026-11-11: the hybrid)
The caller's seat (Test) is Pipecat's RTVI over a WebSocket, one per call; later SmallWebRTC
for voice in the browser, and Twilio in production. The watchers (the rail panels, the line
facts, PHI mode, Demo) read our own event stream, any number per call. The dev server listens
on this PC only. The demo for an analyst team over a Cloudflare Tunnel is an idea
([ideas/README.md](../ideas/README.md)).

## 6. The connection as built
> **Part:** built · 2026-11-12 — the owner's forks 1A (Test and the watch stream together) and
> 2A (`npm run dev` starts both).

**Run it:** `npm run dev`. `scripts/dev.mjs` takes a free port for Vite (from 1420) and one
for the agent (from 8790), makes a fresh 256-bit token, starts the agent's dev server
(`agent/deets_agent/dev_server.py`), then starts Vite. Vite's proxy sends `/agent/*` to the
agent and adds the token. Ctrl+C stops both. `npm run dev:web` starts the console alone; then
the Chat page says the dev server is not running. **Test calls are always live** (the owner,
2026-11-12: "limit replaying conversations only to the Demo view"): every line goes to Claude,
with no replay cache, so each line costs tokens (about $0.0003 for a short Haiku turn). Without
an Anthropic key in `.env` there is no test call ("a test call needs an Anthropic key"); Demo
still works. The replay cache stays for the typed harness and the tests (AGENT.md §1).

**The parts:**
- **The caller's seat (Test):** `@pipecat-ai/client-js` 1.16 with our own text transport
  (`console/src/agent/transport.ts`): RTVI as JSON text, one message per WebSocket frame, no
  audio. The server's half is `agent/deets_agent/rtvi_json.py`. The seat sends; it draws
  nothing from RTVI.
- **The watch stream** (`/agent/watch`): each event of the live call, as the dev call record
  writes it. Test, PHI and Demo draw from the same events with the same code (`CallView` in
  `chat.ts`), so a recorded call plays exactly as it was drawn live. Fixed lines (the 911
  line, the code lines) come this way: they are not model text, so RTVI's text events do not
  carry them.
- **One test call at a time.** A second Start while a call runs is refused ("a test call is
  already running"). Each test call writes a record (`source: test`) that joins the catalog
  when it ends.
- **The line facts, for now:** an agent bubble shows "text 0.8 s" (the time to the first word
  of its first live request to Claude). A recorded call from the typed harness can show
  "replayed" in Demo; a test call never does. Speech and transcription times come with the
  voice.
- **The bar** (the owner, 2026-11-12): the text box and the send button together are exactly
  as wide as the canvas above them. They share one height: the bar sets 44 px and each fills
  it, and both have the same 1 px edge. The text box has `appearance: none` for Safari. The
  line facts start where the bubble's text starts (`--bubble-pad-x`).
- **The control row is fluid** (after two narrow-window desk tests): the help button keeps its
  own width ("Help" when the canvas is 480 px or less); the split takes the room left, up to
  its full size, its halves always equal and never narrower than "Confirm?". It never runs out
  of the canvas.

**The wall** (SECURITY.md §4; dev only, synthetic callers only):
- The dev server listens on 127.0.0.1 only.
- A WebSocket is not covered by CORS, so any web page could open one to a local port. So every
  request needs the launch token, compared in constant time (only Vite's proxy has it; the
  page never does), and must come from the console's own origin. Checked on 2026-11-12: no
  token, 403; another origin through the proxy, 403.
- **RTVI's tool reports are at `NONE`** in the seat: tool events carry only an id, with no
  name, arguments or result. That is stricter than §2.4's `name`, because nothing in the seat
  uses the names; the watch stream carries them.
- **No remote code.** Pipecat's own browser WebSocket transport is not used: it pulls in
  `daily-js`, which loads Daily's call-machine bundle from Daily's CDN. Checked 2026-11-12: the
  built bundle has no `daily` code. Voice in the browser (SmallWebRTC) needs the same check.
- The watch stream carries caller lines and tool arguments. It is safe only because every
  caller is synthetic. A real call has no path to a dev call record (call_record.py).

**Levers seen on the way** (Pipecat 1.16, not used yet): RTVI's `metrics` messages (time to
first byte per service, token usage) now flow to the seat with `enable_metrics` on; the record
times its own requests, so the line facts use those. Pipecat ships an RTVI client transport
and an "eval transport" for simulated callers (`pipecat.transports.websocket.rtvi_client`),
which fits the evals (§7) and test layer 4 (VOICE-AGENT.md §9).

**Desk test:**
1. `npm run dev`; open the address Vite prints, in Safari.
2. Test: Start. The greeting shows. Type as a sandbox patient (the Help box) and book a
   visit. Each answer shows "text n.n s" under it (a live request: about $0.0003 a line), and
   the call ends with the goodbye line.
3. Check the edges: the text box starts at the canvas's left edge and the send button ends
   at its right edge; the two are the same height. The facts under a bubble start where its text starts. Narrow the
   window: "Sandbox patients" becomes "Help", and Start | End call narrows, never past the
   canvas's edge.
4. Open a second window on PHI during a test call: the same lines show as they happen.
5. Demo: pick the call you just made (the newest), Play at 2×. It plays as it ran.
6. Stop the dev server (Ctrl+C) with the page open: the Chat page says the dev server is not
   running, and comes back by itself when you start it again.

## 7. The catalog of calls (the owner, 2026-11-12: SQLite; ratings per dimension)
> **Part:** built · 2026-11-12 — the store and its API; the rating controls in the console are
> not designed yet.

`agent/deets_agent/catalog.py`. Every dev call record is one row; ratings are kept for evals
later (Anthropic's "Demystifying evals for AI agents": grade the outcome, one rubric per
dimension, an "unknown" answer, calibrate model graders against human ones).
- **Two sources, one database.** A row is worked out from its record's events every time the
  catalog is built, so it never drifts. What a person or a grader adds (a label, a note, a
  star, a rating) goes in `agent/calls/annotations.jsonl`, committed and append only, so the
  PC and the Mac merge cleanly; the last line for a field wins. `agent/calls/catalog.db` is
  git-ignored and rebuilt from both at each start. The column names and types follow Postgres,
  for the store (VOICE-AGENT.md §8.7).
- **`calls`:** `id` (the record's file name), `schema_version`, `started`, `source` (typed ·
  test), `mode`, `model`, `scenario` (a kept script's name, so repeated runs group for pass^k),
  `outcome`, `end_reason`, `reached` (the Flow's nodes in order), `turns`, `tool_calls`,
  `duration_ms`, `tokens_in`, `tokens_out`, `cost_usd`, `flags`, `label`, `note`, `keep`.
- **The outcome is the real end state, not the agent's words:** `crashed` · `booked` (ehr's
  `Book` answered `booked`) · `handed_off` · `locked` · `ended` · `abandoned`, the first that
  holds, in that order. Records since 2026-11-12 (schema 2) carry ehr's domain answer on each
  `CheckCode` and `Book` event (an enum's name, never a value). The 47 records of 2026-11-11
  (schema 1) do not; for them, a `Book` that returned ok with no tool error counts as booked.
- **`flags`:** `emergency`, `screen_911`, `invented_value`, `words_before_tool`,
  `library_error`.
- **`ratings`:** one row per call, grader and dimension: `grader_kind` (human · model ·
  code), `grader` (`owner`, a model id, a check's name), `dimension`, `verdict` (pass · fail ·
  unknown), an optional `score` from 0 to 5 (the owner), `note`, `rubric_version`,
  `rated_at`. All are kept; the view `current_ratings` holds the latest from each grader on
  each dimension, so a human and a model rating of the same dimension sit side by side.
- **The dimensions** (a draft for the owner to edit; `DIMENSIONS` in catalog.py):
  `right_outcome`, `verification`, `no_invented_values`, `emergency`, `hand_off`,
  `phone_style`, and `intent_drift` (the owner, 2026-11-12: the agent going off to solve math
  or doing things away from the clinic's business). Change `RUBRIC_VERSION` when a
  dimension's meaning changes.
- **The API:** `GET /agent/catalog` (every row with its current ratings, the dimensions),
  `GET /agent/calls/{id}` (the events, for Demo), `POST /agent/annotations` (one gate checks
  every field: the kind, the call, the dimension, the verdict, the score 0–5).
- **Not built:** where the rating controls sit in the console (the owner designs them), the
  model and code graders.
