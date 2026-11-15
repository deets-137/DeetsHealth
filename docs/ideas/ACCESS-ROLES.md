---
status: idea
desk_test: none
sources: [console/src/pages/roles.ts]
updated: 2026-11-15
---
# DeetsHealth — Access levels by role (idea)

> Not built, and open ended on purpose. **The owner designs and builds this** (2026-11-12).
> This doc holds his direction, his decisions, and a place to collect the levers as they
> appear. Since 2026-11-14 it also holds his first role templates (§4), as a draft. Since
> 2026-11-15 it holds his forks on enforcing roles (§5) and the scope of the build (§6).

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
> **Part:** idea · 2026-11-14 — shown on Security › Roles (`console/src/pages/roles.ts`),
> not wired to a store.

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
| Deets (ours) | None | Edit | Edit | Edit | Edit | Edit | Edit | Edit | Edit | Edit | Edit |

## 5. Enforcing roles: the forks and the answers
> **Part:** designed · 2026-11-15 · the owner

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
  Testing.
- **The conflict rule:** a person who holds a role with View or Edit on Audit logs and no Edit
  on Roles (Compliance) cannot also hold a role with Edit on Roles or People.
- **The two domains:** each role is Hospital or DeetsHealth Team. In the demo both live in
  one `users` table. The hospital's approval and time limit for a Team role (STAFF-SIGN-IN.md
  M6) wait for a hospital deploy.

## 6. The build, scoped
> **Part:** designed · 2026-11-15 · not started

**One build, all three parts at once** (the owner, 2026-11-15: "can aim to build all three
handovers at once and test along the path at reasonable points"), locally on this PC. The
checkpoints are in §8.5. The parts below say what each part does; §8 maps it to the code.
Sizes, at this project's pace (2026-11-12 to 11-15: the settings store, the demo door, People
each took one sitting): about 2 sittings to build, 1 to fix after the desk test. The day
counts in the part titles are a team's without Claude, kept for comparison. **Cost: none** (no FHIR calls, no minutes, no tokens).

### Hand-over 1 — the store and `ehr` (about 6 days)
1. **Migration 0010** (0009 is the uncommitted hospital settings). A `roles` settings service:
   a versioned list of `{id, name, domain, levels}`, the id stable across versions. A
   `user_roles` table: `user_id`, `role_id`, `granted_by`, `granted_at`, `revoked_at` (a row is
   never deleted). `users.role` is dropped. Seeded with the nine templates of §4; the `owner`
   row gets Deets; every other person gets no role (deny by default).
2. **Migration 0011, the audit table.** One row per event: when, who (the user id), the lever
   and level used, the object (a call id, a user id, a role id; never a name or an email), the
   result (allowed or refused), and the hash of the row before it. A database trigger refuses
   UPDATE and DELETE. `audit.rs` writes the rows and checks the chain.
3. **`ehr/src/access.rs`:** a user's levels (the highest per lever across their live roles);
   `may(actor, lever, level)`; the conflict rule; the "at or below you" rule; the last-Deets
   guard. Unit tests for each rule.
4. **The actor on every RPC.** The agent sends the user id in the gRPC metadata. `ehr` reads
   the levels itself (it never trusts levels sent by the agent). Each RPC in the service names
   its lever and level in one table; a test fails when an RPC has no entry. New RPCs:
   `ListRoles`, `SaveRoles`, `GrantRole`, `RevokeRole`, `Levels` (one user's), `ReadAudit`.
   EHR-SERVICE.md §3's RPC ledger gets a row for each.
5. Audit rows: each grant, revoke, role save, people change, and each refusal.

Desk test: `cargo test` for the rules; `npm run check`.

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

### Then
Staff sign-in (STAFF-SIGN-IN.md §10) as a new source behind the seam: OIDC on an Entra test
tenant first.

## 7. Draft: each route's lever (for the owner to edit)
> **Part:** idea · 2026-11-15 — Claude's draft

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
> **Part:** designed · 2026-11-15 · read against the tree of 2026-11-15 (main at `8a4e234`,
> with the uncommitted hospital settings work)

Every file to add or change, by part. "New" is a file to add. A name in backticks is a symbol
in the code today, unless it is marked new.

### 8.0 Before the session starts
- **Run locally on this PC** (the owner, 2026-11-15: "we'll run it in the morning locally"),
  not in a cloud session.
- **Commit the Config › Hospital work first.** It holds `0009_settings_hospital.sql`,
  `settings.rs`, `store.rs` and the `/agent/hospital` routes (with their missing DEV-API.md
  rows). The roles build then starts from a clean tree, and its commit holds only roles.
- Stop `npm run dev` before a full check (it locks the `ehr` exe). The tests use the fakes that
  exist (`agent/tests/test_demo_door.py`'s Access fakes) and the local Postgres. The owner's
  desk test is the last checkpoint.

### 8.1 Part 1 — the store and `ehr` (Rust)
**Migrations (`ehr/migrations/`)**
- **New `0010_roles.sql`:**
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
- **New `0011_audit.sql`:**
  - `audit (seq bigserial, at timestamptz, actor uuid, source text, lever text, level text,
    action text, object_kind text, object_id text, allowed boolean, prev_hash bytea, hash
    bytea)`.
  - A trigger that refuses UPDATE, DELETE and TRUNCATE, as `settings_insert_only` does.
  - No name, email or patient value in any column. `object_id` holds a call id, a user id or a
    role id.

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

**`ehr/tests/store.rs`**: migration tests for 0010 and 0011: the audit table refuses UPDATE
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
   - Sign in with the launch token: everything shows except Chat › PHI.
   - On a phone through the demo, a guest with no role sees "No role yet".
   - Grant the guest Junior Analyst. Within seconds the guest sees Config › Agent as View
     only, Testing, and Analytics.
   - Revoke it. The guest is back to "No role yet".
   - Try to revoke your own Deets. It is refused.
   - Audit Logs shows each step.
