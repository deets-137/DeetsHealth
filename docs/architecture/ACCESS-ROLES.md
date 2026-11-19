---
status: built
desk_test: passed 2026-11-19
sources: [ehr/migrations/0011_roles.sql, ehr/migrations/0012_audit.sql, ehr/src/access.rs, ehr/src/audit.rs, ehr/src/service.rs, agent/deets_agent/identity.py, agent/deets_agent/dev_server.py, console/src/access.ts, console/src/nav.ts, console/src/pages/roles.ts, console/src/pages/auth.ts, console/src/pages/audit.ts]
updated: 2026-11-19
---
# DeetsHealth — Access levels by role

> **Built 2026-11-18, all three hand-overs (§6 "As built"); the owner's desk test passed
> 2026-11-19** ("looks good for now"). Moved from `docs/ideas/` the same day. **The owner designs
> this** (2026-11-12): his direction, his decisions, and a place to collect the levers as they
> appear. §4: his role templates. §5: his forks on enforcing roles. §5.1: the audit trail with
> the roles. §6: the build and what was decided inside it. §7: the routes' levers, his to edit.
> Open, his: the Audit Logs page's look; what PHI Edit allows beyond View; whether Test gets a
> lever of its own; staff sign-in as the next source (STAFF-SIGN-IN.md).

## 1. Terms
- **Role** — a set of access that a hospital creates and names itself.
- **Lever** — one thing a role can be allowed or refused. The levers are found as we build.

## 2. The direction (the owner, 2026-11-12)
- **Each role is custom.** A hospital creates a role and gives it its own name.
- **Each role is built from levers.** Which levers exist becomes clear as we build, and as the
  product gets closer to finished.
- **Open ended for now.** Nothing more is decided.

**Staff sign-in is built with the roles** (the owner, 2026-11-12): STAFF-SIGN-IN.md, its plan
and sizes in §10.

SECURITY.md §4.10's four roles (admin, supervisor, viewer, auditor) are examples, not a fixed
list.

## 3. Levers seen so far
A list to collect, not a design. Add a line when a feature shows a new thing to allow or
refuse, with a link to where it appears.
- Read the call log (DEBUGGING.md §1).
- Read a transcript (SECURITY.md §4.10; CHAT.md §2, PHI mode).
- Play a recording (VOICE-AGENT.md §8.7).
- Change the settings, and the recording rules (FUTURE-SETTINGS.md).
- Read the audit trail (SECURITY.md §4.10).
- The tools of the CLI and the MCP server (ideas/CONSOLE-MCP.md §5.2).
- Map the IdP's groups or app roles to roles; change the IdP settings (STAFF-SIGN-IN.md §5,
  M1 and M3).
- Approve our support access; hold the local emergency account (STAFF-SIGN-IN.md §5, M6).
- Export the audit trail; read the access review (STAFF-SIGN-IN.md §5, M5 and M7).
- Open a restricted call (STAFF-SIGN-IN.md §7, F2).
- Approve a change to what callers hear (STAFF-SIGN-IN.md §5, M8).

## 4. The templates (a draft, 2026-11-14)
> **Part:** built · 2026-11-18 · desk test passed 2026-11-19 — the Default roles version
> (migration 0011), edited on Security › Roles (`console/src/pages/roles.ts`).

**Decided (the owner, 2026-11-14):** each lever has three levels: **None**, **View**, **Edit**.
(Not chosen: on / off, and a fourth level, Propose, where a change waits for approval.)

The owner's starting roles, 2026-11-14: Director, Manager, Senior Analyst, Junior Analyst,
Clinician; on our side, Engineer and Intern. **Compliance** is Claude's addition (a privacy
officer who reviews access, so the Director does not audit their own changes): the owner to
keep or drop it. A template is a starting role: a hospital copies, renames or changes it
(§2).

The levers in the table: the owner's nine (PHI, Auth, Roles, Agent config, vendor config,
analytics, testing, rules, audit logs) and Claude's two (People: add and remove users; Flags:
work the flag list). The levels are Claude's draft for the owner to edit.

| Role | PHI | Auth | Roles | People | Agent | Vendors | Rules | Testing | Analytics | Flags | Audit logs |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Director | View | Edit | Edit | Edit | Edit | Edit | Edit | Edit | View | View | View |
| Manager | View | View | View | Edit | Edit | View | Edit | Edit | View | View | View |
| Senior Analyst | View | None | None | None | Edit | View | Edit | Edit | View | View | None |
| Junior Analyst | None | None | None | None | View | None | View | Edit | View | None | None |
| Clinician | View | None | None | None | None | None | None | None | None | Edit | None |
| Compliance | View | View | View | View | View | View | View | None | View | View | View |
| Engineer (ours) | None | View | View | None | View | View | View | Edit | View | None | View |
| Intern (ours) | None | None | None | None | View | None | View | Edit | View | None | None |

Notes on the draft, not decided:
- **Our side** gets into a hospital's console only when that hospital approves it, for a set
  time, in the audit log (STAFF-SIGN-IN.md M6). Engineer's PHI is None except through that
  emergency access. Intern never gets PHI.
- **No one gives more access than they have.** Edit on Roles never covers one's own role. A
  hospital always keeps one Director. Every lever change goes in the audit log.
- **PHI has parts** (a transcript, the audio, the caller's number, a restricted call) and a
  scope (a Clinician sees the flagged calls only). Both wait for the work that needs them.
- Testing is with synthetic patients only.

**Kept (the owner, 2026-11-15):** Compliance stays. **Added (the owner, 2026-11-15):**
**Deets**, a DeetsHealth Team role: Edit on every lever except PHI, which is None. The owner
holds it in place of the `owner` flag.

| Role | PHI | Auth | Roles | People | Agent | Vendors | Rules | Testing | Analytics | Flags | Audit logs |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Deets (ours) | Edit | Edit | Edit | Edit | Edit | Edit | Edit | Edit | Edit | Edit | Edit |

**Changed (the owner, 2026-11-18):** "give Deets PHI Edit permissions as well". Deets now has
Edit on every lever, so it covers every template and its holder may grant any of them (the
fork of §6 "As built"). Until 2026-11-18 Deets had PHI None.

## 5. Enforcing roles: the forks and the answers
> **Part:** built · 2026-11-18 · desk test passed 2026-11-19 · the owner's forks

Brought 2026-11-15. **Enforce** means the server refuses a request that the user's roles do
not allow, not only the console's screen.

- **Fork 1, the order: A, with a seam for staff sign-in.** Roles are built now, on the
  identity we already have (the Cloudflare Access email matched to a row in `users`, or the
  launch token on the owner's PC). Who a request is comes through one interface with one
  source per way in. Staff sign-in (OIDC, STAFF-SIGN-IN.md M2) is a new source behind it; the
  checks, the store and the console do not change. (Not chosen: B, staff sign-in first.)
- **Fork 2, roles per person: A, several.** A person holds any number of roles. Each lever
  takes the highest level of the person's roles. A conflict rule refuses a mix that lets
  someone audit their own changes (§6, step 3). (Not chosen: B, one role per person.)
- **Fork 3, the screen: A.** A lever at None hides its page from the nav. A lever at View
  shows the page with no Save, Add or Remove, and a "View only" mark in its title row. (Not
  chosen: B, every page shown with greyed controls.)
- **The owner (the owner, 2026-11-15):** "make me a Deets role on the DeetsHealth Team with
  edit access to everything except PHI". The `owner` flag no longer gives any access.

**What follows from these, without a fork** (brought with them):
1. The server enforces in two layers: the dev server checks each route; `ehr` checks again on
   each RPC that returns protected data or changes settings, people, roles or the audit.
2. Deny by default. A person with no role sees only a "No role yet" page.
3. Every route and every RPC names its lever and level. `docs:check` refuses one that does
   not, as check 12 refuses an unlisted route today. The WebSockets are checked too.
4. No one edits their own roles. A hospital always keeps one holder of a role with Edit on
   Roles.
5. A change to a role or to a person's roles takes effect within seconds for everyone signed
   in.
6. An append-only audit table, hash-chained, is built with this (STAFF-SIGN-IN.md M5 at its
   smallest).
7. Role definitions are saved as versions in the settings store, as Config is.

**Decided inside his choices (Claude, 2026-11-15; listed so nothing ships unseen):**
- **You change only people at or below you.** To grant, revoke or remove a person, or edit
  their card, your level must be at least theirs on every lever. So only a Deets holder can
  change a Deets holder. This replaces the `owner` flag's lockout guard. The last holder of
  Deets cannot lose it.
- **The launch token is still the owner's PC.** It resolves to the user row it resolves to
  today (the `owner` column, kept only to say which row that is, with no power). That row's
  access is its roles, as for anyone.
- **What PHI None means for the owner:** Chat › PHI (watching another person's live call)
  leaves his nav. Testing and Demo stay: their patients are synthetic and their lever is
  Testing. *(Replaced 2026-11-18: Deets has PHI Edit, §4, so Chat › PHI stays in his nav.)*
- **The conflict rule:** a person who holds a role with View or Edit on Audit logs and no Edit
  on Roles (Compliance) cannot also hold a role with Edit on Roles or People.
- **The two domains:** each role is Hospital or DeetsHealth Team. In the demo both live in
  one `users` table. The hospital's approval and time limit for a Team role (STAFF-SIGN-IN.md
  M6) wait for a hospital deploy.

### 5.1 The audit trail, with the roles
> **Part:** built · 2026-11-18 · desk test passed 2026-11-19 · the owner's ask, Claude's design (no fork left)

**His ask (2026-11-18):** "integrating audit trails with our RBAC build, ie tracking which user
made a change, what change, and what they did with their role logged as well as time". The
design of §5 and §6 had who, the lever, the object and the time. It did not have what changed,
or which role gave the access. The fork test left one option for each point below, so none was
asked. He saw this list before it was written here.

1. **Which role gave the access.** Each row holds the ids of the actor's live roles at that
   moment and the id of the active roles version. Roles are versioned, so a reader can see what
   each role allowed on that day, also after a later edit.
2. **What changed: field names in the row, values from the versions.** A change row lists the
   names of the fields that changed (for example `task.verify`, `line.ask_reason`) and points to
   the version before and the version after. The audit page shows the change from the two
   versions. Not chosen: the values in the row. A people change would then put a staff name or
   email in the hash chain and in a SIEM export, for no gain on the page.
3. **Every audited object keeps its "before".** Settings are versioned today. `user_roles`
   rows are never deleted (§6). A person's card gets a history table (one row per save), so a
   card change also has a before and an after. A removal already keeps the row (`removed_at`,
   `users.rs`), so an old row can always name its actor.
4. **What is logged: all that a person does with their access.**
   - Each change: settings saves and Make active, role saves, grants and revokes, people
     changes, annotations.
   - Each refusal (the lever and level that were missing).
   - Each look at protected data: a transcript view, a recording played, a live call watched,
     an export (SECURITY.md §4.10).
   - Each page opened, and each Test run (it spends tokens).
   - Not each poll: one row per page opened, not per two-second read.
   - The audit page opens on Changes, with a filter for the rest.
5. **The time** is the database's `now()`, stored in UTC: one clock for every row. The page
   shows the reader's local time.
6. **Where a row is written:** in `ehr`, in the same transaction as the change. A change with
   no row, or a row with no change, cannot be stored. The agent never writes the database.
7. **The source** of each row: a person, the call loop, the system (seeds, migrations); later
   staff sign-in and SCIM (STAFF-SIGN-IN.md M4).
8. **One chain, never reset.** The staff audit is one table in the shared schema, with a
   `data_class` column (synthetic or real) for the rows about a call. Resetting the synthetic
   data never touches it. The two empty `synthetic.audit` and `real.audit` tables (0001, 0010;
   nothing writes them) are dropped.
9. **Small corrections to §6 and §8:** `setting_versions.created_by` holds the actor's user id,
   not `"dev"`. The migrations are 0011 (roles) and 0012 (audit): 0010 is now
   `0010_data_classes.sql`.

Later, each in its own work: the audit page's look (a fork when it is built), how long rows are
kept (SECURITY.md's draft: 6 years), the write-once copy and the SIEM export (at a deploy), and
what an exported row may carry (STAFF-SIGN-IN.md §8).

## 6. The build, scoped
> **Part:** built · 2026-11-18 (commit `0f417e0`) · desk test passed 2026-11-19

**One build, all three parts at once** (the owner, 2026-11-15: "can aim to build all three
handovers at once and test along the path at reasonable points"), locally on this PC. The
checkpoints are in §8.5. The parts below say what each part does; §8 maps it to the code.
Sizes, at this project's pace (2026-11-12 to 11-15: the settings store, the demo door, People
each took one sitting): about 2 sittings to build, 1 to fix after the desk test. The day
counts in the part titles are a team's without Claude, kept for comparison. **Cost: none** (no FHIR calls, no minutes, no tokens).

### Hand-over 1 — the store and `ehr` (about 6 days)
1. **Migration 0011** (0010 is `0010_data_classes.sql`, §5.1). A `roles` settings service:
   a versioned list of `{id, name, domain, levels}`, the id stable across versions. A
   `user_roles` table: `user_id`, `role_id`, `granted_by`, `granted_at`, `revoked_at` (a row is
   never deleted). `users.role` is dropped. Seeded with the nine templates of §4; the `owner`
   row gets Deets; every other person gets no role (deny by default).
2. **Migration 0012, the audit table.** One row per event: when, who (the user id), the
   actor's roles and the roles version, the lever and level used, the object (a call id, a user
   id, a role id; never a name or an email), the fields changed and the versions before and
   after, the result (allowed or refused), and the hash of the row before it (§5.1). A database
   trigger refuses UPDATE and DELETE. `audit.rs` writes the rows and checks the chain. A
   history table for people's cards (§5.1, point 3).
3. **`ehr/src/access.rs`:** a user's levels (the highest per lever across their live roles);
   `may(actor, lever, level)`; the conflict rule; the "at or below you" rule; the last-Deets
   guard. Unit tests for each rule.
4. **The actor on every RPC.** The agent sends the user id in the gRPC metadata. `ehr` reads
   the levels itself (it never trusts levels sent by the agent). Each RPC in the service names
   its lever and level in one table; a test fails when an RPC has no entry. New RPCs:
   `ListRoles`, `SaveRoles`, `GrantRole`, `RevokeRole`, `Levels` (one user's), `ReadAudit`.
   EHR-SERVICE.md §3's RPC ledger gets a row for each.
5. Audit rows: each grant, revoke, role save, settings save, people change, and each refusal;
   each look at protected data, page opened and Test run (§5.1, point 4).

Desk test: `cargo test` for the rules; `npm run check`.

**As built (2026-11-18; checkpoint 2 of §8.5 reached, not committed).** `ehr/migrations/0011_roles.sql`,
`0012_audit.sql`; `ehr/src/access.rs`, `audit.rs`; `settings.rs`, `users.rs`, `service.rs`,
`store.rs`; `ehr.proto`. Tests: 143 unit tests (`cargo test -p deetshealth-ehr --lib`), 8
store tests (`npm run db:test`), clippy with warnings as errors, and cargo-deny's bans pass. The tree's other checks wait for Hand-over 2: the agent
sends no `deets-actor` yet, so every staff RPC from the console is refused, and `docs:check` 13
fails on the six new RPCs with no agent method.

Decided inside his choices (Claude, 2026-11-18; listed so nothing ships unseen):
- **The roles list is the settings service `roles`**, not RPCs `ListRoles` and `SaveRoles`
  (§8.1 left this to the builder): it gets versions, Make active and the audit row for free.
- **`SeenOwner` keeps its name** (§8.1 said rename it `Seen`). It now answers with the owner's
  id, and `AdmitUser` with the person's id: the agent needs the id to send as `deets-actor`.
  Stamping last seen stays on those two RPCs.
- **Settings with no person are the agent itself**: a call reading its settings, the prompt
  file synced, a Default seed. Allowed and audited as the system. With a person: the lever by
  service, View to read and Edit to save. Every other staff RPC refuses a request with no
  person.
- **A new RPC, `LogAccess`**, for what changes nothing in the store (a page opened, a
  transcript viewed, a test run; §5.1, point 4): ehr checks the lever and writes an allowed or
  a refused row; the agent refuses the look when it was refused.
- **A removal revokes the person's roles** in the same transaction; a person added back
  comes back with no role.
- **SHA-256 from AWS-LC** (already in `ehr`), not the `sha2` crate §8.1 named: no new
  dependency.
- **A card change needs People Edit, one's own card too**, and the card's owner at or below
  you.
- **Seeds and migrations are audited as the system**, except the migration's own roles seed
  (SQL cannot chain a row; its version says `created_by` `system`).

**A fork found while building, answered:** the rule "you change only people at or below you"
compares every lever, PHI too. With PHI None, Deets could not grant the five roles with PHI View.
The owner's answer (2026-11-18): the rule stays, and Deets gets PHI Edit (§4). So Chat › PHI is
in his nav after all, and what PHI Edit allows beyond View is still open (§7).

### Hand-over 2 — the dev server (about 4 days)
1. **The seam (`agent/deets_agent/identity.py`):** an interface, `who(request) → Principal`
   (the user id, the source). Sources now: the launch token, Cloudflare Access (`access.py`
   moves behind it). Later: OIDC. `State.admit` asks the sources in order and nothing else.
2. **`Principal.levels`** from `ehr`'s `Levels`, cached 30 s, the cache dropped at once when a
   grant, revoke or role save goes through this server.
3. **Every route names its lever and level** with a decorator (`@needs("people", "edit")`).
   A route with none is refused at start. A refusal is HTTP 403 `{"error": "forbidden", "lever":
   …}`. The WebSockets are checked on connect; a live watch closes (1008) when its viewer's
   PHI level drops.
4. **New routes:** `GET /agent/me` (your user id, your roles, your levels), `GET/POST
   /agent/roles`, `POST/DELETE /agent/people/{id}/roles/{role_id}`, `GET /agent/audit`.
5. **DEV-API.md:** the ledger gets a Lever column; `docs:check` 12 compares it with the
   decorators. §7 below is the draft of that column.
6. **DEMO.md §3.5 changes:** a guest is no longer "like the owner". Adding a guest needs Edit
   on People; the new guest has no role until someone grants one.

Desk test: tests in `agent/tests/` for each source and the 403s; a guest with no role gets
only `/agent/me`.

**As built (2026-11-18; checkpoint 3 of §8.5 reached, not committed).** `agent/deets_agent/identity.py`
(new), `ehr_client.py`, `dev_server.py`; `agent/tests/test_roles.py` (new) and the people,
door and live-call tests; DEV-API.md §2 (the new routes) and §2a (what each route needs).
`npm run check -- agent` passes (lint, every agent test, the kept scripts against a real
`ehr`), and `docs:check` has no failure.

Decided inside his choices (Claude, 2026-11-18; listed so nothing ships unseen):
- **One table, not a decorator per route:** `dev_server.py › ROUTE_NEEDS` maps each method and
  path to its lever and level. The wall finds the route a request reaches and checks the table
  before the route runs, and `build_app` refuses to start when a route and the table differ.
  The WebSockets check the same table when they open. DEV-API.md §2a is the table in words;
  `test_roles.py` compares them, in place of a new `docs:check` step.
- **The person travels in a context variable** (`ehr_client.ACTOR`), set by the wall for each
  request; every staff call sends it as `deets-actor`. Work the agent does on its own runs in
  `as_system()` and sends `deets-actor: system`: the start-up sync, seeding a Default version,
  and the settings a call runs with (so a guest who may test, but not read Config › Agent, still
  gets a working call). `ehr` takes `system` only for settings, the People list and the
  annotations (`Need::LeverOrSystem`); a request with no `deets-actor` at all is still refused.
- **No roles store, no roles:** without `ehr` or its Postgres, the launch token is the owner
  with Edit everywhere (as before roles), and a guest gets nothing. The same holds for the
  launch token on a store with no owner row yet (a new machine with no Cloudflare group). Dev
  only: a deploy always has its store.
- **Levels are kept 30 s** per person, and dropped at once after a grant, a revoke, a roles
  save or a removal through this server. Another server's change takes up to 30 s, as the
  guest list does (DEMO.md §3.3).
- **Looks recorded so far** (`POST /agent/log` is ready for the console's page opens): a
  transcript read (`GET /agent/calls/{id}`), a watch opened, a Test run (it spends tokens).
- **The roles list's routes** are `GET` and `POST /agent/roles` (the whole list, versioned);
  grants and revokes are `POST` and `DELETE /agent/people/{id}/roles/{role_id}`.
- **A 403 says what was missing:** `{error: "forbidden", lever, level}`, so the console can
  say which permission is needed.

### Hand-over 3 — the console (about 5 days)
1. **A `me` store** (LOADING.md): your levels, read on start and again when the server says
   they changed.
2. **The nav** hides each page whose lever is None. The "No role yet" page when nothing is
   left.
3. **View only:** Config's pages through `config-kit.ts` (one place); Security › Auth, Roles;
   Teams › People; Chat's modes (PHI by the PHI lever, Testing and Demo by Testing).
4. **Security › Roles wired to the store:** the role table edits and saves as a version; a
   role's panel lists its holders.
5. **Security › Auth's Role cell:** a list of the person's roles, with Add and Remove, each
   refused when the rules of step 3 above say no (the reason in the status line).
6. **The audit page** (`audit.ts`, empty today) shows the rows: when, who, what, the object's
   id, allowed or refused. Its look is a fork for the owner when it is built.

Desk test: written into CONSOLE.md with the build; sign in as the owner and as a guest with
Junior Analyst, and see the pages and controls differ.

**As built (2026-11-18; checkpoints 4 and 5 of §8.5).** `console/src/access.ts` (new),
`pages/no-role.ts` (new), `nav.ts`, `pages/subnav.ts`, `pages/page.ts`, `pages/config-kit.ts`,
`pages/config.ts`, `security.ts`, `teams.ts`, `rules.ts`, `analytics.ts`, `chat.ts`, `auth.ts`,
`roles.ts` (rewritten on the store), `audit.ts` (the three views filled); `agent/api.ts`,
`agent/stores.ts`; `styles/auth.css`, `styles/config.css`. CONSOLE.md §13 (with the desk test),
LOADING.md (the `me`, `roles` and `audit` stores).

Decided inside his choices (Claude, 2026-11-18; listed so nothing ships unseen):
- **Security › Auth needs People** (it lists and edits the People list), not Auth. Auth has
  nothing of its own to guard until staff sign-in exists (§7's open point).
- **Chat's modes:** Testing needs Testing Edit (it makes calls), Demo Testing View (it plays the
  records), PHI PHI View. Chat is in the nav while any mode is yours.
- **A view's lever rides on the view** (`View.lever`), and Config's frame reads it from the view
  it builds, so the five Config views needed no change of their own.
- **Before `me` answers, everything shows**, so the first paint never flashes "No role yet"; the
  first-load wave waits for `me` anyway.
- **Security › Roles saves the whole list from the page's title row** (Config's Discard | Save
  look), not per role: a roles version is the whole list. A new role's id is made from its name
  at Save and never changes after. **Remove role** takes a role out of the draft; ehr refuses it
  while someone holds it.
- **Audit Logs:** your three views (Config, Calls, Security) sort the rows by kind; 100 rows a
  page with Older and Newest, so no list passes CONSOLE.md §12's 200 and needs no window or
  search box yet. Names come from the People list when your role shows it, else the start of
  the id. The page's own look stays your fork.
- **A page open is recorded once per visit** to the page, by the first lever you hold it by.

### Then
Staff sign-in (STAFF-SIGN-IN.md §10) as a new source behind the seam: OIDC on an Entra test
tenant first.

## 7. Each route's lever (for the owner to edit)
> **Part:** built · 2026-11-18 — Claude's draft, built as drafted (`dev_server.py ›
> ROUTE_NEEDS`; DEV-API.md §2a is the table as built). Edit both together.

| Route | Lever and level |
|---|---|
| `GET /agent/health`, `/agent/build`, `/agent/me`, `POST /agent/csp-report`, `/demo/…` | any signed-in person, no lever |
| `WS /agent/watch` (another person's live call) | PHI View |
| `WS /agent/call`, `POST /agent/voice`, `GET /agent/ice`, `GET /agent/sandbox` | Testing Edit |
| `GET /agent/catalog`, `GET /agent/calls/{id}` | Testing View |
| `POST /agent/annotations` | Testing Edit |
| `GET /agent/prompt`, `GET /agent/system` | Agent View |
| `POST /agent/prompt`, `POST /agent/system`, `POST /agent/prompt/estimate`, `POST /agent/prompt/record` | Agent Edit |
| `GET /agent/cartesia`, `GET /agent/anthropic` | Vendors View |
| `POST /agent/cartesia`, `POST /agent/anthropic` | Vendors Edit |
| `GET /agent/resources`, `GET /agent/latency` | Analytics View |
| `GET /agent/people`, `GET /agent/people/{id}/photo` | People View |
| `POST`, `PATCH`, `DELETE /agent/people…`, the photo's `PUT` and `DELETE` | People Edit |
| `GET /agent/roles` | Roles View |
| `POST /agent/roles`, a grant or revoke | Roles Edit |
| `GET /agent/audit` | Audit logs View |
| Config › Hospital (the hospital settings, when its route exists) | Rules Edit |

Open in the draft: what Edit means on PHI (an export, or a note on a call that shows protected
data); what Auth covers before staff sign-in exists (the Cloudflare group link only); and
whether Record and test, which spends tokens, needs its own lever.

Also in the code but not in the ledger yet: `GET /agent/hospital` and `POST /agent/hospital`
(the hospital settings, uncommitted on 2026-11-15). Draft: Rules View and Rules Edit.

## 8. The code map
> **Part:** built · 2026-11-18 · the plan as it was read against the tree of 2026-11-15 (main
> at `8a4e234`). Where the build differs, §6's "As built" notes say so; they are the code.

Every file to add or change, by part. "New" is a file to add. A name in backticks is a symbol
in the code today, unless it is marked new.

### 8.0 Before the session starts
- **Run locally on this PC** (the owner, 2026-11-15: "we'll run it in the morning locally"),
  not in a cloud session.
- **Start from a clean tree.** Done 2026-11-18: the Config › Hospital work is committed, and
  so is everything after it (`fe5828a`). The roles build's commits hold only roles.
- A full check runs with the dev server on since 2026-11-16 (it runs a copy of `ehr` from
  `target/run/`). The tests use the fakes that
  exist (`agent/tests/test_demo_door.py`'s Access fakes) and the local Postgres. The owner's
  desk test is the last checkpoint.

### 8.1 Part 1 — the store and `ehr` (Rust)
**Migrations (`ehr/migrations/`)**
- **New `0011_roles.sql`** (0010 is taken, §5.1):
  - Adds `'roles'` to the two `CHECK (service IN …)` lists, as `0006_settings_system.sql`
    does for `'system'`.
  - `settings_roles (version_id, key, value jsonb)` with the insert-only triggers, copied
    from `settings_system`. One key, `roles`: a list of `{id, name, domain, levels}`.
    `domain` is `hospital` or `team`. `levels` maps each lever to `none`, `view` or `edit`.
  - `user_roles`: `user_id uuid REFERENCES users`, `role_id text`, `granted_by uuid REFERENCES
    users`, `granted_at`, `revoked_at`, `revoked_by`. A row is never deleted. One live row per
    pair: a partial unique index `WHERE revoked_at IS NULL`.
  - Drops `users.role`. Keeps `users.owner`, with a new comment: it says which row the launch
    token speaks for, nothing more.
- **New `0012_audit.sql`** (§5.1):
  - `public.staff_audit (seq bigserial, at timestamptz DEFAULT now(), actor uuid, source text,
    roles text[], roles_version bigint, lever text, level text, action text, object_kind text,
    object_id text, data_class text, fields text[], before_version bigint, after_version bigint,
    allowed boolean, prev_hash bytea, hash bytea)`. The name is the builder's to settle.
  - A trigger that refuses UPDATE, DELETE and TRUNCATE, as `settings_insert_only` does.
  - No name, email or patient value in any column. `object_id` holds a call id, a user id or a
    role id; `fields` holds field names only.
  - `user_profile_versions`: one row per save of a person's card, so a card change has a
    before and an after.
  - Drops the empty `synthetic.audit` and `real.audit`.
  - `setting_versions.created_by` takes the actor's user id.

**`ehr/src/settings.rs`**
- `service()`: add `"roles" => Some(("roles", ROLES, "settings_roles"))`, with a `ROLES` list
  of one setting, `roles`.
- `coherent("roles", …)`: every role has an id and a name, ids are unique, every lever named,
  every level one of three words. At least one role holds Edit on Roles.
- The seed (where `0003_settings_default.sql`'s defaults and `file_variables` seed today): the
  nine templates of §4, Deets among them, as version 1.

**New `ehr/src/access.rs`** (add `pub mod access;` in `lib.rs`)
- `Lever` (the 11 levers) and `Level` (`None < View < Edit`) as enums. Their words come from
  one table, so the console and the agent use the same strings.
- `Levels`: a map lever → level. `Levels::of(roles)` takes the highest level per lever.
- `async fn levels(store, user_id) -> Levels`: the user's live `user_roles` rows joined with
  the active `roles` version.
- `fn may(levels, lever, level) -> bool`.
- `fn conflict(roles) -> Option<&'static str>`: the Compliance rule of §5.
- `fn at_or_below(actor, target) -> bool`: the "at or below you" rule of §5.
- The last-Deets guard: a revoke or a removal that leaves no live holder of a Team role with
  Edit on Roles is refused.
- `async fn grant`, `async fn revoke`: check `may(Roles, Edit)`, `at_or_below`, no grant to
  oneself, `conflict`, then write `user_roles` and an audit row in one transaction.
- Unit tests in the same file, one per rule.

**New `ehr/src/audit.rs`** (add to `lib.rs`)
- `async fn write(tx, row)`: reads the last row's hash in the same transaction (lock the last
  row), computes `hash = SHA-256(prev_hash ‖ the row's fields)`, inserts.
- `async fn read(store, after, limit)` for `ReadAudit`.
- `async fn verify(store) -> Result<(), u64>`: walks the chain; returns the first bad `seq`.
- Needs the `sha2` crate: add it to `ehr/Cargo.toml` (it is not there on 2026-11-15).

**`ehr/src/users.rs`**
- `remove`: the `owner` refusal goes. In its place, `access::at_or_below` and the last-Deets
  guard, checked by the caller (`service.rs`) with the actor's levels.
- `update`, `set_photo`, `clear_photo`: the same "at or below you" check.
- `seed`: the first person (today's `owner` row) gets a `user_roles` row for Deets.
- `list`: each person's role ids, for the console's Role cell.
- The module comment (lines 1–13): "anyone may edit any card" is replaced by the rules.

**`ehr/proto/deets/ehr/v1/ehr.proto`**
- New RPCs: `ListRoles`, `SaveRoles` (or `SaveSettings` with service `roles`; the builder picks
  one and says why in EHR-SERVICE.md), `GrantRole`, `RevokeRole`, `Levels`, `ReadAudit`,
  `VerifyAudit`.
- `User` gains `repeated string role_ids`.
- The actor is not a field. It travels in gRPC metadata (`deets-actor: <user uuid>`), so no
  message changes shape for it.

**`ehr/src/service.rs`**
- **New: one table of every RPC's lever and level** (`const RPC_LEVERS: &[(&str, Need)]`).
  `Need` is `Call` (the call loop: `OpenCall` … `FileCall`, no staff actor), `Any` (any signed
  in user: `Levels` for oneself), or `Lever(lever, level)`. `SaveSettings` and `GetSettings`
  look up the lever by `service`: `voice` and `llm` → Vendors, `prompt` and `system` → Agent,
  `hospital` → Rules, `roles` → Roles.
- **New: `async fn actor(&self, req) -> Result<(Uuid, Levels), Status>`** reads the metadata and
  the levels from the store (never from the agent). A missing or unknown actor on a staff RPC
  → `permission_denied`. Each staff handler calls it first, then `may`. A refusal writes an
  audit row.
- The users handlers (`list_users` … `seen_owner`, lines ~724–820) take the actor; `add_user`'s
  `by` field is then the actor, not an email.
- `seen_owner` is renamed `seen` and stamps the actor's own row (anyone, not only the owner).
- **New test:** every `rpc` name in `ehr.proto` has a row in `RPC_LEVERS` (read the proto file
  in the test, as `scripts/docs-check.mjs` check 13 does).
- Tests: a staff RPC with no actor is refused; Junior Analyst cannot save `voice`; Compliance
  cannot be granted Director; no one grants themself; the last Deets cannot be revoked.

**`ehr/tests/store.rs`**: migration tests for 0011 and 0012: the audit table refuses UPDATE
and DELETE; `verify` finds a changed row (changed with the trigger dropped in the test).

**Docs:** EHR-SERVICE.md §3 (a row per new RPC, and the actor in metadata); DATABASE.md (the
two tables); SECURITY.md §4.10 (staff access as built).

### 8.2 Part 2 — the dev server (Python)
**`agent/scripts/gen_proto.py`**: run it after the proto change (the Python stubs are
git-ignored).

**New `agent/deets_agent/identity.py`** — the seam for staff sign-in
- `@dataclass Principal`: `user_id`, `source` (`launch`, `access`; later `oidc`), `levels`.
- `class Source(Protocol)`: `async def who(headers) -> str | None` (a user id, or None).
- `LaunchSource`: wraps `State.allowed`; returns the `owner` row's id (asked of `ehr` once at
  start, through `ListUsers`).
- `AccessSource`: wraps `access.AccessGate.email`, then the email → user id (a new
  `ehr.admit_user` reply field: the id, not a boolean).
- `Identity`: the sources in order; `async def principal(headers) -> Principal | None`; the
  levels from `ehr.levels(user_id)`, cached 30 s (`GUEST_TTL`'s value), with `forget(user_id)`
  and `forget_all()`.
- Later, `OidcSource` (STAFF-SIGN-IN.md M2) is one more class here. Nothing else changes.

**`agent/deets_agent/access.py`**: `AccessGate` stays the token check; its guest cache moves
to `Identity` (one cache, by user id). `set_guests` and `forget` follow.

**`agent/deets_agent/ehr_client.py`**
- `_call`, `_users_call`, `_settings_call`: take `actor: str | None` and send it as the
  `deets-actor` metadata.
- New methods, one per new RPC (names in EHR-SERVICE.md §3, so `docs:check` 13 holds).
- `admit_user` returns the id or None.

**`agent/deets_agent/dev_server.py`**
- `State.admit` (line ~285) returns a `Principal` through `Identity`. `OWNER` and the email
  strings go. `request.state.who` becomes `request.state.me` (a `Principal`).
- **New: `needs(lever, level)`**, a FastAPI dependency on each route: refuses with 403
  `{"error": "forbidden", "lever": …, "level": …}` and writes nothing else. `NO_LEVER` marks the
  open routes (health, build, me, csp-report, demo pages).
- **At start, `build_app` refuses to start** when a route has neither `needs` nor `NO_LEVER`
  (walk `app.routes` after they are declared).
- The `wall` middleware (line ~323): the `seen_owner` stamp becomes `seen` for anyone.
- `/agent/watch` (line ~671): checks PHI View on connect. Each watcher keeps its user id; a
  levels change (`forget`) closes the sockets of a viewer who lost PHI View (close 1008).
- `/agent/call` and `/agent/voice`: Testing Edit. `_record_who` (line ~1630): `started_by`
  stays `owner` or `guest` in the record for now; the user id goes in the audit row instead.
- The people routes (lines ~841–962): People View or Edit; `by` becomes the actor's id.
- **New routes:** `GET /agent/me` (id, roles, levels, source); `GET /agent/roles`; `POST
  /agent/roles` (a new version); `POST /agent/people/{id}/roles/{role_id}` (grant); `DELETE
  /agent/people/{id}/roles/{role_id}` (revoke); `GET /agent/audit` (`?after=`, `?limit=`). A
  grant, revoke or role save calls `Identity.forget` and closes affected watchers.
- `_people_view`: `you` becomes the asker's user id; each person gains `roles`.

**Tests (`agent/tests/`)**
- New `test_identity.py`: the launch source; the Access source with the fakes; the cache
  and `forget`; an unknown email → None.
- New `test_levers.py`: every route has a lever or `NO_LEVER` (the start check); a Junior
  Analyst gets 403 on `POST /agent/cartesia`; a user with no role gets only `/agent/me`; the
  watch closes after a revoke.
- Update `test_demo_door.py`, `test_people.py`, `test_access.py`, `test_dev_server.py`: their
  fakes now answer `Levels`, and `who` is a `Principal`.

**Docs**
- DEV-API.md §2: a **Lever** column, and the new routes.
- `scripts/docs-check.mjs` check 12: also compares each row's lever with the `needs(…)` on its
  handler.
- DEV-API.md §3 (the wall): the identity sources and `needs`.
- ops/DEMO.md §3.3 and §3.5: guests have their roles' access, not the owner's.
- AGENT.md: `identity.py`.

### 8.3 Part 3 — the console (TypeScript)
**`console/src/agent/api.ts`**: `me()`, `roles()`, `saveRoles()`, `grantRole()`,
`revokeRole()`, `audit()`; types `Lever`, `Level`, `Me`, `Role`. `Person` gains `roles`. A 403
with a `lever` is its own error, so a page can say "Your role does not allow this", not "the
agent's dev server refused".

**`console/src/agent/stores.ts`**: `me` (read on start and on focus), `roles`, `audit`. A
grant, revoke or role save reads `me` and `people` again.

**New `console/src/access.ts`**: `level(lever)`, `may(lever, level)`, `onMe(fn)`; the lever
words from `api.ts`. Every page asks here. No page reads `me` itself.

**`console/src/pages/page.ts`**: `Page` gains `lever?: Lever` (absent = always shown).
`subnavPage`'s `View` (`pages/subnav.ts`) gains the same, for one view of a page.

**`console/src/nav.ts`** (`initNav`): filters `pages` by `may(p.lever, "view")`, and again when
`me` changes. When no page is left: the new "No role yet" page (**new
`pages/no-role.ts`**). When the page shown loses its lever, the nav moves to the first page
left.

**`console/src/main.ts`**: give each page its lever:
- Chat: none at the page. Its modes do the check: Testing and Demo by Testing, PHI by PHI.
- Config: Agent and System views → Agent; Cartesia and Anthropic → Vendors.
- Rules → Rules. Teams → People. Analytics → Analytics. Audit Logs → Audit logs.
- Security: Auth view → Auth (and People, which it edits); Roles view → Roles.
- Settings: none.

**View only, page by page**
- `pages/config-kit.ts` already has `Mode = "read" | "edit"`. At View, it stays in `read`:
  the Edit button and Discard | Save do not show, and a "View only" mark shows in the title row.
  This covers Agent, System, Cartesia and Anthropic at once.
- `pages/chat.ts`: `MODES` (line ~45) filtered by lever. PHI mode shows only with PHI View.
  Without Testing Edit: no text bar, no call button, no rating.
- `pages/auth.ts`: at People View, no Add, no Remove and no Save. The Role cell (line ~235,
  "Role · set by roles, later") becomes the person's roles as chips. At Roles Edit it gets an
  Add list and a Remove on each chip, refused (with the server's reason) by the §5 rules.
- `pages/people.ts`: the Role row shows the role names (the `PLACEHOLDER.role` comment).
- `pages/roles.ts`: `CUSTOMER` and `TEAM` (line ~41) go; the tables read the `roles` store.
  A cell click and Add change a draft; the page gets Config's Discard | Save (the
  `config-kit.ts` frame) and saves a version. At Roles View, read only. A role's panel lists
  its holders from `people`.
- `pages/audit.ts` (21 lines, empty): a table of the rows, newest first, with More. Its look
  is a fork for the owner (§6, part 3, step 6). Until he picks, a plain table in Auth's look.
- Styles: the "View only" mark and the role chips in `styles/` with theme roles and skin
  tokens only (CLAUDE.md); `npm run tokens` if a token is added.

**Docs:** CONSOLE.md (the nav and View only), CHAT.md §14 (the modes by lever), LOADING.md (the
`me`, `roles` and `audit` stores), MOBILE-UI.md (the role chips on a narrow screen),
ACCESS-ROLES.md "as built".

### 8.4 What does not change
- The call loop's RPCs (`OpenCall` … `FileCall`) and caller verification: no staff actor.
- `seal.rs` and the sealed columns of `users`.
- The Cloudflare group sync (`cloudflare.py`, `sync_people`): who may sign in stays the
  group; what they may do is now their roles.

### 8.5 The checkpoints
Run `npm run check` at each one; `npm run check:all` before the commit.
1. **After 8.1's migrations and `access.rs`:** `cargo test` for the rules and the audit chain.
2. **After `service.rs`'s table and actor:** the `RPC_LEVERS` coverage test and the refusal
   tests pass. The agent still works: it does not send an actor yet, so staff RPCs refuse;
   expected until checkpoint 3.
3. **After `identity.py`, `ehr_client.py` and `needs`:** `pytest agent/tests` green, the start
   check passes, `docs:check` 12 and 13 green.
4. **After the console's `me`, nav and config-kit:** `npm run check` (types) green.
5. **After roles.ts, auth.ts, chat.ts and audit.ts:** the full check; commit.
6. **The owner's desk test, on this PC:**
   - Sign in with the launch token: everything shows, Chat › PHI too (Deets has PHI Edit since
     2026-11-18).
   - On a phone through the demo, a guest with no role sees "No role yet".
   - Grant the guest Junior Analyst. Within seconds the guest sees Config › Agent as View
     only, Testing, and Analytics.
   - Revoke it. The guest is back to "No role yet".
   - Try to revoke your own Deets. It is refused.
   - Audit Logs shows each step.
