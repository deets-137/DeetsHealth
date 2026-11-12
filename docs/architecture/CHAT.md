---
status: built
desk_test: open
sources: [console/src/pages/chat.ts, console/src/styles/chat.css, docs/architecture/images/chat-sketch-2026-11-11.png]
updated: 2026-11-12
---
# DeetsHealth — The Chat page

> The console's Chat page: one call in a chat view, in three modes. The owner's design,
> narrated 2026-11-11 over his sketch. The shell it sits in is [CONSOLE.md](CONSOLE.md). The
> connection is the hybrid (§4). Not built.

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
> **Part:** built · 2026-11-12 — the UI only; nothing is connected yet.

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
- **A test call, for now:** Start enables the text box and says the agent is not connected;
  each line you send shows as a caller bubble; End call, then Confirm?, ends it. Nothing
  answers until the connection is built.
- **The help box:** "Sandbox patients": Jordan Ellis, Priya Raman, Casey Morgan (with the
  Morgen twin), the code 246810. It opens over the lines with a row-height and fade
  transition (`--dur-collapse`, 220 ms). The list is in `chat.ts`, kept in step with
  `ehr/src/verify.rs` until the dev server sends it.
- **Demo:** Play | Stop and the call picker are disabled ("No recorded calls yet") until the
  dev call records are served. The speed pair works.
- **PHI:** the canvas alone: "No live call to watch".
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
