---
status: designed
desk_test: none
sources: [scripts/dev.mjs, console/vite.config.ts, agent/deets_agent/dev_server.py, console/src/agent/transport.ts, console/public/manifest.webmanifest]
updated: 2026-11-13
---
# DeetsHealth — The demo over a tunnel

> Not built. Friends open the console from their own PCs at
> `https://deetshealth.deets.solutions/demo/` and use it as an analyst would. Every fork is
> closed; the owner said "Don't build just yet" (2026-11-13).

**Words used here.**
- **Tunnel:** `cloudflared` on the owner's PC opens an outbound link to Cloudflare. Cloudflare
  serves an `https` address that leads back to one port on this PC. No port opens to the
  internet.
- **Access:** Cloudflare Zero Trust's sign-in gate in front of that address. A person types
  their email and gets a one-time code. Only the emails on the policy get in.
- **Access token:** the signed JWT that Access adds to every request it lets through, in the
  `Cf-Access-Jwt-Assertion` header. It names the person's email.
- **Guest:** a friend on the Access policy.
- **TURN:** a relay for a WebRTC call's audio, for when two ends cannot reach each other
  directly. **STUN:** a free service that tells each end its public address, so they can try
  to connect directly first.

## 1. The owner's choices (2026-11-13)
- "imagine my friends are analysts and I want to give them full demo / full access."
- "pretend they are like me in terms of access. We can worry more when we set up RBAC." No
  spend cap for now. Roles come with staff sign-in ([ideas/ACCESS-ROLES.md](../ideas/ACCESS-ROLES.md)).
- **Several calls at once** (fork A): "this can help us test the PHI tab with each analyst / me
  spectating each others and also testing their own."
- **The address:** "deetshealth.deets.solutions/demo".
- **Hot reload:** asked for; §3.2 gives what is possible.
- **The guest list from inside the console**, later replaced by staff sign-in (§3.5).
- **No email on guest writes for now:** "we'll design when we design the audit log".
- **No "synthetic data" marker** in the top bar.

So a guest gets everything the owner gets: Testing calls (typed and voice), Demo, PHI mode,
the catalog, ratings and notes, and every Config page with its saves. This replaces the
watch-only view in the earlier idea ([ideas/README.md](../ideas/README.md)).

## 2. What the code says (read 2026-11-13)
1. **Vite's proxy adds the launch token** to every `/agent/*` request (`console/vite.config.ts`).
   Anyone who reaches the Vite address can use the whole agent. That is safe today only
   because Vite listens on this PC.
2. **The Vite dev server must never face the tunnel.** Its root is `console/`, but it serves
   files from the whole repo through `/@fs/`. Its block list covers `.env*`, `*.crt`, `*.pem`
   and `.git`. It does not cover `dev-certs/*.key` (the mTLS private keys) or `dev-dumps/`.
   Vite has also had several bugs that got around that block list. And the dev server sends
   each source file as its own request, which is slow over a tunnel.
3. **The agent's wall** (`State.allowed` in `dev_server.py`) takes the launch token and an
   Origin of `http://localhost:<port>` or `http://127.0.0.1:<port>`. A tunnel's origin fails it.
4. **One call at a time.** A second Start gets "a test call is already running"
   (`_refuse_call`). The watch stream and PHI mode follow `state.live`, the one live call.
5. **The voice seat uses WebRTC with host candidates only** ("On this PC both ends are local";
   `new RTCPeerConnection()` with no ICE servers in `transport.ts`). A guest's browser cannot
   reach this PC's UDP ports, so a voice call from a guest fails without STUN and TURN.
6. **The console assumes it sits at `/`.** 19 paths in `console/src` start with `/agent`;
   `index.html` and `manifest.webmanifest` (`id`, `start_url`, `scope`, icons) use `/`. The
   console has no URL routing, so nothing else depends on the path.
7. **A full console build takes about 1.7 s** on this PC (`vite build`, 2026-11-13).
8. **Cloudflare, read with `cf` 2026-11-13:** the zone `deets.solutions` is active; the Access
   team is `deetssolutions.cloudflareaccess.com` with email codes (set up for DeetsMusic,
   DeetsMusic's `docs/ops/LIVE-MCP.md` §8); no tunnel exists yet.

## 3. The design

### 3.1 The address and the path of a request
**The address:** `https://deetshealth.deets.solutions/demo/`.
- One level under `deets.solutions` is covered by the free Universal SSL certificate. A deeper
  name such as `demo.deetshealth.deets.solutions` would need Advanced Certificate Manager
  ($10 a month). The path gives the nesting for free.
- The console is built with Vite's `base: "/demo/"`. Its 19 `/agent` paths go through one
  helper that adds the base, and the manifest's `id`, `start_url`, `scope` and icons move under
  `/demo/`. On this PC, `npm run dev` still serves the console at `/`.
- **The Access app covers the whole hostname**, not only `/demo`. So nothing on
  `deetshealth.deets.solutions` is ever outside Access. The agent answers only under
  `/demo/`; every other path gives 404. If the real console comes to this hostname later, it
  gets its own Access app or staff sign-in then.

**The path of a request:** guest's browser → Access (sign-in) → the tunnel → the agent's dev
server on `127.0.0.1:<agent port>`. Vite's dev server is not on this path (§2.2).

### 3.2 Live reload (what "hot reload" can be here)
True hot reload (Vite's HMR, which swaps a module in place) needs the Vite dev server on the
tunnel, and §2.2 rules that out. The design gives **live reload** instead:
1. `npm run demo` runs `vite build --watch` into its own folder. A saved change rebuilds in
   about 2 s or less (§2.7).
2. The agent serves that folder. When a build finishes, it tells each open page over the watch
   stream.
3. A page reloads itself at once when it is idle: no call running in that tab and no unsaved
   edit on a Config page. Otherwise it reloads when the call ends or the edit is saved or
   dropped. A guest never loses a call or a draft to a reload.
4. A change to the agent (Python) or to `ehr` (Rust) still needs a restart of `npm run demo`,
   as with `npm run dev` today.

The owner's own Vite tab on this PC keeps true hot reload.

### 3.3 A second door in the wall
A request passes if it has either:
- the launch token and a local origin (the owner's Vite, as today); or
- a valid Access token and the origin `https://deetshealth.deets.solutions`.

The agent checks the Access token itself: the signature against the team's keys
(`/cdn-cgi/access/certs`, cached), the issuer, the app's `aud` tag and the expiry. It also
checks that the token's email is still on the guest list (§3.5, cached 30 s). So a request
that slips past Access gets a 403, and a removed guest is out within 30 s, even with a live
Access session. The same check runs on every WebSocket upgrade and on the voice route. The
launch token never leaves this PC.

### 3.4 The tunnel and the Access app
- **The Access app:** `DeetsHealthDemo`, self-hosted, on `deetshealth.deets.solutions`. Its
  policy allows one Access group, *DeetsHealth demo guests*: the owner's email and each
  friend's. The emails live only in that group, never in a doc, a commit or this repo. Session
  length: 24 hours.
- **The tunnel:** a named tunnel `deetshealth-demo`, locally managed. Its credentials file sits
  in `~/.cloudflared/`, outside the repo. `npm run demo` (`scripts/demo.mjs`) does what
  `npm run dev` does, then starts the build watch (§3.2) and runs
  `cloudflared tunnel run --url http://127.0.0.1:<agent port> deetshealth-demo`. The port is
  free-picked at each launch. Ctrl+C stops the tunnel with everything else. When the demo is
  not running, the address shows Cloudflare's error page and reaches nothing.

### 3.5 The guest list from the console
Yes, the console can manage it.
- **Now:** a console page lists the guests' emails, with Add and Remove. The agent makes the
  change through Cloudflare's API on the one Access group (§3.4). A Remove also revokes that
  person's Access sessions, and the wall's own check (§3.3) ends their requests within 30 s.
- **The Cloudflare API token** sits in `.env` and never reaches a browser. Its scope is
  "Access: Apps and Policies Edit" on this account. Cloudflare cannot narrow a token to one
  group, so the agent's code touches only the one group id. The token could in principle
  edit DeetsMusic's Access apps too; a separate Cloudflare account for DeetsHealth would close
  that, at the cost of a second account.
- **Because guests are "like me"**, a guest can add or remove guests too. Roles change this
  later (ACCESS-ROLES.md).
- **Later, with staff sign-in** ([ideas/STAFF-SIGN-IN.md](../ideas/STAFF-SIGN-IN.md)): the
  same page sits on an interface, "who may sign in". Its first backend is the Access group. Its
  next is the hospital's IdP (Entra ID, Okta) and our roles. The page does not change; the
  backend does.
- **Not now:** the page's place in the nav, its name and its controls are forks for when it
  is built.

### 3.6 Several calls at once (fork A)
- The agent holds a set of live calls, each with its own record, keyed by call id.
  `_refuse_call` no longer refuses a second call; it keeps the per-call checks.
- The watch stream carries each event with its call id. A watcher asks for one call, or for
  the list of live calls.
- **PHI mode picks the call to watch**, so the owner and each guest can watch any live call,
  their own or another's. How the picker looks is a fork for when it is built (CHAT.md).
- The cost line on Config › Anthropic sums every live call.

### 3.7 Voice through STUN and TURN
- For each voice call the agent asks Cloudflare for short-lived TURN credentials
  (`POST https://rtc.live.cloudflare.com/v1/turn/keys/<key id>/credentials/generate-ice-servers`,
  TTL one hour). The browser and the agent get the same `iceServers`. The port-53 URL is
  dropped, because browsers block it and the voice seat does not trickle candidates.
- ICE tries a direct link first (STUN, free). It uses the TURN relay only when the direct link
  fails.
- The TURN key id and its API token sit in `.env`. The owner's local calls keep host
  candidates only.
- **Privacy:** WebRTC encrypts the audio end to end with DTLS. Cloudflare relays encrypted
  packets and sees only addresses, ports and times. Every caller in the demo is synthetic.
  TURN is not on the production path: real calls come in through Twilio.

**The cost (read 2026-11-13 from Cloudflare's Realtime pricing and TURN FAQ):**
- $0.05 per GB, metered on data sent from Cloudflare to each TURN client. The first 1,000 GB
  each month are free (shared with Cloudflare's SFU, which we do not use). STUN is free.
- One voice call is about 50 kbit/s each way with headers. If both ends use the relay, that is
  about 0.75 MB a minute, or 45 MB an hour.
- So 1,000 GB is more than 20,000 hours of relayed calls a month. The demo costs nothing here.
  Past the free GB, an hour of relayed voice costs about $0.002.
- Still to check when the TURN key is made: whether Cloudflare asks for a payment method.

### 3.8 Later: Twilio over the same tunnel
TURN and Twilio sit at different layers (the owner asked, 2026-11-13):
- **The swap point is the transport.** `Call` (`call.py`) takes any Pipecat `BaseTransport`,
  and `_run_call` (`dev_server.py`) passes one in. The typed seat uses a WebSocket; the voice
  seat uses `SmallWebRTCTransport`. Twilio becomes a third seat: a route that builds
  `FastAPIWebsocketTransport` with Pipecat's Twilio serializer (8 kHz μ-law, Media Streams)
  and calls the same `_run_call`. The call loop does not change.
- **TURN serves only the browser seat.** It stays under that seat when Twilio comes; phone
  calls never use it. So TURN stays small (§3.7): no interface of its own.
- **The tunnel carries over.** BUILD-ORDER.md plans "a real Twilio number and a tunnel". The
  same tunnel brings Twilio's webhook and its Media Streams WebSocket to this PC. That route
  cannot sit behind Access (Twilio cannot sign in): it gets its own path outside Access and
  checks Twilio's request signature (`X-Twilio-Signature`) instead.
- **The BAA first.** A real person's phone audio is protected data. Until the BAA is signed,
  only synthetic test calls reach the number. Guests could then call a phone number, which is
  closer to the product than the browser seat.
- **Before a number exists**, the fake Twilio caller (VOICE-AGENT.md §9, layer 5) tests the
  Twilio seat, the 8 kHz path and barge-in from WAV files.

### 3.9 The route ledger
New routes (the build watch's reload signal, the guest list, the TURN credentials) each get
their row in [architecture/DEV-API.md](../architecture/DEV-API.md) in the same commit.

## 4. Each place it runs
- **The call:** no change. The phone line is not part of the demo.
- **The console:** any recent browser, on a PC or a phone. A hospital network may block
  WebSockets or WebRTC to an unknown site; TURN over TLS on port 443 gets through most of them.
- **Deploy targets:** none. This runs on the owner's PC only and goes away with the cloud
  deploy ([integrations/CLOUD.md](../integrations/CLOUD.md)).
- **EHRs:** Local and Epic's public sandbox, as on Chat today. Several guests on Epic add
  sandbox reads; the 5-minute cache on `/agent/sandbox` keeps them low.

## 5. Build steps
1. `cf`: create the tunnel, the DNS record, the Access group and app, and the TURN key, with
   the owner's go for each (they face the internet).
2. The wall's second door and the Access token check, with tests in `agent/tests/`.
3. The base path (`/demo/`), `--static` in the dev server, the build watch and live reload,
   `scripts/demo.mjs`, `npm run demo` in `package.json`.
4. Several calls at once (§3.6), then PHI mode's picker after its fork.
5. STUN and TURN for guest voice calls (§3.7).
6. The guest list page (§3.5), after its forks.
7. The desk test (§6). Then this doc's status becomes `built`.

## 6. Desk test (draft)
1. `npm run demo`. The output names `https://deetshealth.deets.solutions/demo/`.
2. On a phone on mobile data, open the address: Access asks for an email. A non-listed email
   gets no code.
3. Sign in with a listed email. The console opens.
4. Start a typed call on Local, then a voice call. Both run.
5. On the PC, start a second call at the same time. In PHI mode on each side, watch the other
   call.
6. Change a console file and save it. The phone's page reloads within a few seconds when no
   call runs; during a call it waits for the call to end.
7. `https://deetshealth.deets.solutions/` and `/demo/@fs/` give 404.
8. Remove the phone's email from the guest list. Within 30 s its page stops getting answers.
9. Ctrl+C. The address stops answering.
