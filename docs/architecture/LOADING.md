---
status: built
desk_test: passed 2026-11-14
sources: [console/src/agent/store.ts, console/src/agent/stores.ts, console/src/agent/ledger.ts, console/src/reveal.ts, console/src/reveal-wave.ts, console/vite.config.ts, console/src/agent/api.ts, console/src/main.ts, console/src/nav.ts, console/src/pages/subnav.ts, console/src/pages/auth.ts, console/src/pages/people.ts, console/src/pages/config-kit.ts, console/src/pages/agent.ts, console/src/pages/chat.ts, console/public/prepaint.js, scripts/dev.mjs]
updated: 2026-11-16
---
# Loading — the console's reads, and the first-load wave

How the console reads from the agent's dev server, and how the screen comes in on a first load.
Two parts, built in this order: the read layer (§2–§5), then the wave (§6). The model for both
is DeetsMusic: its reveal (`../DeetsMusic/docs/architecture/UX-COVERUPS.md` §7, `src/reveal.ts`,
`src/reveal-wave.ts`) and its two hand-written ordering rules (`src/player.ts` load generations,
`src/favorites.ts` writes in press order). DeetsMusic has no shared read layer; this one is new.

> **Part:** built · 2026-11-14, desk test passed 2026-11-14 (§8 the reads, §9 the wave) (the owner: "let's
> design the fetch layer"; fork A for the wave; "ensure when building polling we can use it for
> meaningful debugging / telemetry / health"; "values are fine, build the wave too")

## As built (2026-11-14): read this first

Where it differs from the design below, this is the code.
- **Files:** `console/src/agent/store.ts` (the rules), `stores.ts` (every store by name),
  `ledger.ts` (the ring, `health()`). `api.ts`'s `AgentDown` now carries `status`: none when the
  agent did not answer (`down`), the HTTP status when it refused.
- **Tokens:** `--read-focus-gap` 5 s, `--read-retry-first` 0.5 s, `--read-retry-max` 8 s
  (skin.css).
- **Decided inside the owner's choices** (each open to change):
  - **A call's events are never kept in a store**: they are its transcript. Chat reads them
    with `once("events", …)`, which only times the read into the ledger. A person's picture
    is read the same way (`once("photo", …)`); its own cache in `person.ts` stays.
  - **Config's cards draw from the store's first value only.** Later values come from the
    view's own Save and version picks, which draw themselves, so an edit in progress is never
    drawn over. Config stores never read on focus.
  - **The Test button's two presses** (the quote, Record and test) are writes on
    `settings.agent` (`act`): in click order, in the ledger, and the store reads after each.
    Its status is the store's poll, every 2 s after each answer. A poll goes on while the agent
    is down (the store keeps its last value, still `running`), and stops when a read says done.
  - **An annotation** is a write on `catalog` (`act`): the call list reads again after it.
  - **A write that fails** makes the store read once more: the write had voided the reads in
    flight.
  - **Answer sizes** (`bytes`) are not filled yet: the requests do not report them. The field
    stays in the entry's shape.
  - `PEOPLE_CHANGED` is gone: Auth and People share the `people` store.
- **The wave (§6), built the same day** with the draft values (the owner: "values are fine"):
  `console/src/reveal.ts` (the hold and the wave), `reveal-wave.ts` (DeetsMusic's delay rule,
  ported), `public/prepaint.js` (marks the hold before the first paint), `app.css` (the hold,
  the safety lift, `reveal-pre`, `reveal-in`), tokens `--reveal-cap`, `--reveal-span`,
  `--reveal-safety`. Decided inside his choice:
  - **The hold's reads are found by themselves:** every store whose first read starts while the
    hold is open joins it (`store.ts` › `heldReads`). After the answers are drawn, the hold
    looks once more, so a read that a drawn answer starts (Config › Cartesia's credits) joins.
    A person's picture and a call's events (`once`) do not hold.
  - **The pieces** (`reveal.ts` › `PANELS`, `PIECES`): the nav and every glass panel; inside
    them the app title, the nav pill and items, headings, tab bars, Config's top row and rows,
    People's cards, call rows, table rows, empty lines, buttons and fields. The outermost
    match moves: a button in a row moves with its row.
  - **Lines are one control tall** (`--control-height`) when the wave groups pieces into lines:
    DeetsMusic's tiles were all one size; here a tall panel would pull the pieces in its top
    half onto its own line.
  - **"The agent did not answer" is marked by Vite:** when its proxy cannot reach the agent, and
    for every `/agent` request under `npm run dev:web`, Vite answers 503 with the header
    `x-deets-agent: unreachable` (`console/vite.config.ts`); `api.ts` reads that as `down`, not
    a refusal. A status alone could not say it: the agent answers 502 and 503 itself. Vite
    still prints its own proxy error line.
  - **Entry motion under the hold ends at the wave:** a page's own fade-in is finished at
    once, so it does not mix with the wave; Chat's mode-change wave does not play on a first
    load. The backdrop's drift runs on.

## 1. Why (2026-11-14)

- The pages read from the agent each in its own way, through the plain functions in
  `api.ts`. Nothing orders the answers. Four pages drew an older answer over a newer one:
  Security › Auth (a focus read after a Save), Teams › People, Chat › Resources, and Config ›
  Agent's Test poll. Each was fixed by hand the same day (WORKLOG, 2026-11-14 evening). The
  next page would have to remember the rule again.
- A page that fails one read says "The agent's dev server is not running" until the window
  is focused again, even after the agent is back.
- Two pages that show the same list (Auth and People) each read it and tell each other with
  an event.
- The owner's ask: every piece of the screen comes in as a gentle wave, top left to bottom
  right, on a first load and a reload; the read layer may serve as its cover.

**Terms:**
- **Store** — the console's copy of one kind of data from the agent (the people list, one
  Config section's settings, one call's use). A page reads a store; it never calls `api.ts`
  for a read.
- **Read** — one request that fills a store. **Write** — one request that changes data; its
  answer is the new value.
- **Ledger** — the list of every read and write the console made, with its time and outcome,
  and no value (§5).
- **Hold** — the moment between a first load and the wave, while the first screen's stores
  fill (§6).

## 2. The store

`console/src/agent/store.ts`. One store per kind of data:

```ts
const people = store("people", () => api.people(), { focus: true });
const resources = keyed("resources", (call: string | null) => api.resources(call));
```

A store holds its state: `loading` (no value yet), `ready` (a value), `down` (the agent did not
answer), `refused` (it answered with an error: fixed words, never a value). A page subscribes
and draws on each change; a subscription gets the current state at once.

**The rules, in the layer, so no page writes them again:**
1. **The newest asked wins.** Each read takes a number. An answer older than the newest read
   asked is dropped (the ledger says `dropped`).
2. **A write wins over reads.** A write's answer is the store's new value, and every read
   that was in flight when it was sent is dropped: it could hold the value from before.
3. **Writes to one store go one at a time, in click order** (DeetsMusic's ♥ rule). A second
   Save waits for the first.
4. **One read in flight per store.** A second ask while one is in flight marks the store to
   read once more when it lands, so the last answer is never older than the last ask.
5. **Polling:** `store.poll({ every, while })` reads again `every` ms **after each answer**, never
   on a fixed timer, while `while(value)` is true. Two polls of one store never overlap.
6. **Focus:** a store made with `focus: true` reads again when the window comes back, at most
   once in `--read-focus-gap` (draft: 5 s).
7. **Down and back:** a read that finds the agent down tries again at 0.5, 1, 2, 4, then every
   8 s, while a page shows the store. When any read reaches the agent again, every store in
   `down` reads again. Pages update in place: no second wave, no reload. A write that finds the
   agent down is not tried again; the page says "Not saved".
8. **Never kept:** values live in memory only. Nothing goes in browser storage, and a reload
   starts empty (SECURITY.md §4: a staff name or a call's use is still data to keep in).

**Keyed stores** (`keyed(name, read)`) hold one store per key: Chat's sandbox list by EHR
(`local`, `epic`), one call's use by call id, a Config section by version. The same rules hold
per key.

**Where it runs:** the console only, in any browser (this PC's dev server, the demo over the
tunnel, a deploy later). The phone call does not touch it. A store whose read leaves this
machine (`leaves: true`; today only `sandbox` with `epic`) never reads on focus and never holds
the wave (§6), and its retries keep the 5-minute cache in the agent (CHAT.md §13), so the
number of requests to Epic does not grow.

## 3. What moves, page by page

| Page | Stores | Today |
|---|---|---|
| Security › Auth, Teams › People | `people` (shared), `people` writes (add, save, remove, picture) | each reads on its own; `PEOPLE_CHANGED`; the counters added 2026-11-14 |
| Config › Cartesia, Anthropic, System, Hospital | `settings.<section>` keyed by version; its Save is the write | `config-kit.ts` loads once |
| Config › Agent | `settings.agent` keyed by version; the Test status is a poll on it while a run is `running` | `agent.ts` `follow()` |
| Config › Cartesia's credits | `resources` keyed `null` | read once |
| Config › Cartesia's voice list | `cartesia.voices` (`leaves`: it reaches Cartesia); Refresh is its write | as built 2026-11-16 (CONSOLE.md §7.8) |
| Chat | `health`, `catalog`, `sandbox` keyed by EHR, `resources` keyed by call (a poll while a call is live); a call's events by `once` (never kept: as built) | each read on its own; `RESOURCES_LIVE_MS` timer |

**Stays outside the layer:** the live streams. Each keeps its own order on one connection: the
watch socket (`watch()`), a call's socket and WebRTC (`transport.ts`), and the demo's live reload
(`reload.ts`). The watch socket's "up" tells the layer the agent is back (rule 7). The
pictures (`person.ts`) keep their own cache, read once per saved picture; their reads go in the
ledger.

## 4. The page's side

- `subscribe(store, draw)` in `build()`. The draw gets the state and shows `loading`, `ready`,
  `down` or `refused` in one shared way (the empty line's words stay the page's own).
- `PEOPLE_CHANGED` and the hand-written counters go away.
- A page that is hidden keeps its subscriptions (the nav keeps a built page), but a store polls
  and retries only while a page that shows it is on screen.

## 5. The ledger: debugging, telemetry and health later

The owner's rule for this layer (2026-11-14): its reads, and its polls above all, must serve
debugging, telemetry and health later. So every read and write, and every poll tick, writes
one ledger entry as it ends, in the shape of the EHR ledger (ops/DEBUGGING.md §3):

```ts
type Entry = {
  t: number;          // ms since the page loaded (performance.now)
  store: string;      // a fixed word: "people", "settings.cartesia", "resources"
  key?: string;       // only when the store declares its keys as fixed words ("local", "epic")
  op: "read" | "write" | "poll";
  why: "open" | "focus" | "after-write" | "poll" | "retry" | "back" | "user";
  ms: number;         // from the request to the answer
  outcome: "ok" | "dropped" | "down" | "refused" | "bad-reply";
  status?: number;    // the HTTP status
  bytes?: number;     // the answer's size
  attempt?: number;   // 1 for the first try; the retry's number after
  poll?: number;      // the tick's number within its poll
};
```

- **No protected data, by construction.** No value, no query string, no call id, no person id:
  a keyed store logs its key only when the key is one of the store's declared words. The
  refusal's words are left out too (they name a field, but the ledger has no use for them).
- **Kept:** a ring of the last 500 entries, in memory, for this page load only.
- **Read through one module,** `console/src/agent/ledger.ts`: `recent()`, `on(fn)`, and
  `health()`, which gives per store the last answer's time, the failures in a row, the median
  and the 95th percentile of `ms` over the last 50, and the count dropped.
- **Polls are named:** each poll has its store, its `every` and the tick number, so a slow
  agent shows as a rising `ms` across ticks, and a stall as a gap between ticks.
- **Later, not now** (each its own design): Analytics › Speed and Logs (ANALYTICS.md) read
  `health()` and `recent()`; a connection mark in the top bar reads `health()`; the deploy
  sends the ledger to the call log's tracing (ops/DEBUGGING.md §2, OpenTelemetry). The entry's
  shape is fixed now so those read it as it is.
- **In dev only**, `window.deetsLedger` gives the same three functions in the browser's
  console. The built console and the demo leave it out.

## 6. The first-load wave

> **Part:** built · 2026-11-14, desk test passed 2026-11-14 (§9) (fork A: hold everything, one wave; built at
> once with the read layer, at the owner's word)

**The owner's choice A (2026-11-14):** on a first load and a reload, the screen shows only the
backdrop until the first screen's data is in, at most `--reveal-cap`. Then one wave runs from
the app title at the top left to the last piece at the bottom right. Rejected: B, the top bar
and nav first and the page's pieces when their data lands (two waves, with a gap on a slow
load).

- **The hold.** `prepaint.js` marks `<html data-reveal="hold">` before the first paint, so no
  piece ever shows early. `main.ts` builds the shell and the page; the stores the page
  subscribes to while it builds are the hold. It waits for each of them to leave `loading`
  (`ready`, `down` or `refused` all count), at most `--reveal-cap`. A store with `leaves: true`
  never holds. A click or a key ends the hold at once (DeetsMusic §7.1). CSS lifts the hold at
  `--reveal-safety` whatever happens, so a page error never leaves a blank screen.
- **The pieces.** The app title, each nav item, each panel, and inside a panel each control
  group and each table or list row. Only the pieces on the first screen move; the rest show
  with no motion. A piece inside a panel never starts before its panel.
- **The wave.** DeetsMusic's rule (`reveal-wave.ts`): pieces sorted by their box, top then left,
  a piece within half its height of a line's first piece on that line. Delay = (line + column)
  ÷ (lines + columns − 2) × `--reveal-span`, so it runs left to right and top to bottom at once
  and takes the same time for 8 pieces or 80. Each piece fades in and rises
  (`--dur-wave-in`, `--wave-rise`, `--ease-wave`: the motion of Chat's mode-change wave). Each
  piece starts on its own turn, not all at once with a delay: DeetsMusic found the second way
  draws every layer in the first frame (UX-COVERUPS.md §7, `wave()`).
- **Not on a page switch.** A page switch keeps its 220 ms fade (the owner's words: "first load
  / reload"). A reload by the demo's live reload is a reload: it waves.
- **Reduced motion:** the same hold, then the screen shows at once.
- **Every place it runs:** any browser. On a guest's slow link the cap bounds the blank time.

**Tokens (the owner: "values are fine", 2026-11-14):**

| Token | Draft | From |
|---|---|---|
| `--reveal-cap` | 1.5 s | DeetsMusic `--reveal-cap` |
| `--reveal-span` | 0.5 s | DeetsMusic `--reveal-span` |
| `--reveal-safety` | 4 s | DeetsMusic `--boot-safety` (its idea) |
| `--read-focus-gap` | 5 s | new (§2 rule 6) |
| a piece's motion | `--dur-wave-in` 360 ms, `--wave-rise` 6 px | the console's own |

## 7. Build order

1. `store.ts` and `ledger.ts`, with the rules of §2 and the entry of §5.
2. Security › Auth and Teams › People onto the `people` store.
3. Config: the three settings sections, then Config › Agent and its Test poll.
4. Chat: `health`, `catalog`, `sandbox`, `resources`, `events`; the watch socket's "up" into
   rule 7.
5. **Desk test (below).** Then §6, the wave.

## 8. Desk test (the read layer)

1. `npm run dev`. Open Security › Auth. The list shows.
2. Edit a card and Save. Click into another window and back at once. The saved values stay.
3. Open Teams › People: it shows the saved card with no reload.
4. Stop `npm run dev` (Ctrl+C). Each open page says the agent is not running. Start it again.
   The page comes back by itself: Vite's own client reloads it, or, where the page outlives
   Vite (the demo, served by the agent), the layer reads again within about 8 s, with no
   reload and no focus.
5. Config › Agent › Test: the status counts up and ends on Passed or Failed, and stays.
6. Chat: start a test call; Resources counts up while it runs and stops when it ends.
7. In the browser's console: `deetsLedger.health()` lists each store with its timings;
   `deetsLedger.recent()` shows no name, email, phone or id in any entry.

## 9. Desk test (the wave)

1. `npm run dev`. Open the console on Chat. Reload (F5). Only the backdrop shows for a moment;
   then the app title, the nav and the page come in as one wave from the top left to the
   bottom right, in about half a second. No piece shows before its turn.
2. Reload on Security › Auth, on Config › Agent and on Teams › People: the same wave, with the
   rows of the table, the settings and the cards in it.
3. Click another page: it fades in as before, with no wave.
4. Reload and click at once: the wave starts at once.
5. Stop `npm run dev`; run `npm run dev:web` (the console alone) and open it: the wave runs at
   once (Vite answers "unreachable" for the agent), and each page says the agent's dev server
   is not running.
6. Windows Settings › Accessibility › Visual effects › Animation effects off: a reload shows
   the screen at once, with no wave.
