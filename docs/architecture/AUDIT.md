---
status: built
desk_test: passed 2026-11-19
sources: [ehr/migrations/0012_audit.sql, ehr/src/audit.rs, ehr/src/service.rs, ehr/src/settings.rs, ehr/src/users.rs, ehr/tests/store.rs, agent/deets_agent/dev_server.py, agent/deets_agent/ehr_client.py, console/src/pages/audit.ts, console/src/nav.ts, console/src/agent/api.ts, console/src/agent/stores.ts]
updated: 2026-11-19
---
# DeetsHealth — The audit trail and the Audit Logs page

> The one reference for the staff audit trail: what a row holds, what writes one, how the chain
> proves nothing was changed, and how the console reads it. Built 2026-11-18 with the roles
> (ACCESS-ROLES.md §6, commit `0f417e0`); the owner's desk test passed 2026-11-19.
> The owner's ask (2026-11-18): "tracking which user made a change, what change, and what they
> did with their role logged as well as time". The design and its reasons: ACCESS-ROLES.md §5.1.

## 1. Terms
- **Row**: one thing a person did with their access, or was refused, in `staff_audit`.
- **Actor**: the person behind a request, by their user id. The agent sends it to `ehr` as the
  gRPC metadata `deets-actor` (EHR-SERVICE.md §3, "Who is asking").
- **Look**: a use that changes nothing in the store (a page opened, a transcript read). It is
  recorded through `LogAccess`, not by the change it would make.
- **Chain**: each row's hash covers the row before it, so a changed, removed or inserted row
  shows.

## 2. What a row holds
> **Part:** built · 2026-11-18 · `ehr/migrations/0012_audit.sql`, `ehr/src/audit.rs`

One table, `public.staff_audit`, for the whole store. It is never reset: a reset of the
synthetic data does not touch it (DATABASE.md §7, §11).

| Column | What it holds |
|---|---|
| `seq` | The row's number. A number may be skipped (a transaction that rolled back); the chain is by hash, not by number. |
| `at` | When, from the database's own clock (`clock_timestamp()`), stored in UTC to the microsecond. The page shows the reader's local time. |
| `actor` | The person's user id; empty for the system and for an unknown asker. |
| `source` | `person`, `system` (a seed, a migration, the prompt file synced, a call reading its settings) or `unknown` (a request that named no person on the list). |
| `roles`, `roles_version` | The ids of the actor's live roles at that moment, and the active roles version. With the versions in the store, a reader sees what each role allowed on that day. |
| `lever`, `level` | The lever and level the action used, or lacked when it was refused. |
| `action` | What was done: one word of §3. |
| `object_kind`, `object_id` | What it touched: `settings` and the service, `person` and their id, `page` and its id, `call` and its id, `test_run`. An id only. |
| `data_class` | `synthetic` or `real`, for a row about one call. |
| `fields` | The names of the fields that changed (`task.verify`, `phone`, `role:intern`). Names only, never a value. |
| `before_version`, `after_version` | The versions before and after a change: a settings version, or a person's card version (`user_card_versions`). The values are read from those versions, not from the row. |
| `allowed` | Allowed, or refused. |
| `prev_hash`, `hash` | The chain (§4). |

**Never in a row:** a name, an email, a phone number, a picture, or any patient value. A person
is an id; the console names them from the People list (§5). So an export of the trail carries no
PII (§7).

## 3. What writes a row
> **Part:** built · 2026-11-18

**Changes**, written by `ehr` in the same transaction as the change: neither is ever stored
without the other.

| Action | Written by | Lever | Fields, versions |
|---|---|---|---|
| `settings.save` | `settings.rs › save` | the service's (`settings.rs › lever`) | the settings that changed; the active version before, the new one |
| `settings.seed` | the same, for a Default version | the service's | the settings; none before |
| `person.add` | `users.rs › add` | People Edit | `email`, `name`; the card versions |
| `person.card` | `users.rs › update` | People Edit | the card's fields that changed; the card versions |
| `person.photo` | `users.rs › set_photo`, `clear_photo` | People Edit | `photo` (the picture is not kept in the history) |
| `person.remove` | `users.rs › remove` | People Edit | each role revoked with the person (`role:<id>`) |
| `person.seed` | `users.rs › seed` (people brought in from Cloudflare's group) | the system | `email`, and `role:deets` for the first person |
| `role.grant`, `role.revoke` | `service.rs › change_role` | Roles Edit | `role:<id>` |

A roles list save is a `settings.save` on the service `roles`.

**Refusals**, written by `ehr` on their own: `rpc.<name>` (`rpc.list_users`,
`rpc.grant_role`), with the lever and level that were missing, or `unknown` when no person on the
list was named (`service.rs › gate`, `refuse`). A rule's refusal (not your own roles, above you,
the last manager) is the change's own action word with `allowed` false.

**Looks**, through `LogAccess` (allowed or refused, by the person's own levels):

| Action | From | Lever |
|---|---|---|
| `page.open` | the console's nav, once per visit to a page (`nav.ts`, `POST /agent/log`) | the first lever the person holds the page by |
| `transcript.view` | `GET /agent/calls/{id}` (a call's events) | Testing View |
| `call.watch` | `WS /agent/watch` opened (another person's live call) | PHI View |
| `test.run` | `POST /agent/prompt/record` (it spends tokens) | Agent Edit |

**Not written today** (see §7): a refusal by the dev server's own wall (a route your role does not
allow, refused before any request reaches `ehr`); a sign-in; a recording played or an export
(neither exists yet).

## 4. The chain
> **Part:** built · 2026-11-18 · `audit.rs › write`, `digest`, `verify`

- **The hash:** SHA-256 (AWS-LC, the provider `ehr` already uses) over a fixed prefix
  (`deets.staff_audit.v1`), the row before's hash, and every field but `seq`. Each field goes in
  with its length first, and a missing field has its own mark, so no two rows encode the same and
  no field's bytes can pass for the next one's. The first row's "row before" is 32 zero bytes.
- **One writer at a time:** `write` takes a transaction lock (`pg_advisory_xact_lock`), reads the
  last row's hash, takes the time, and inserts. Two writers never chain to the same row.
- **Insert-only:** a trigger refuses UPDATE, DELETE and TRUNCATE. Only a superuser who turns the
  trigger off can change a row, and then the chain shows it.
- **The check:** `audit::verify` walks the whole trail, oldest first, 1,000 rows a read, and
  answers the row count, or the first `seq` whose hash does not match. The console's Check chain
  runs it (§5).
- **The card history** (`user_card_versions`) is not chained: it is sealed (each version under its
  own row, so a key change seals it again; `store.rs › PUBLIC_SEALED`), and a row changes only
  when its sealed values are sealed again. The chain holds the version ids that point into it.

## 5. Reading it
> **Part:** built · 2026-11-18 · desk test passed 2026-11-19

- **`ehr`:** `ReadAudit` (newest first, `before` a `seq`, at most 500, `changes` for the changes
  and refusals only) and `VerifyAudit`; both need Audit logs View (EHR-SERVICE.md §3).
- **The dev server:** `GET /agent/audit` and `GET /agent/audit/verify`; `POST /agent/log` records a
  look (DEV-API.md §2, §2a).
- **The console:** the store `audit` keyed `all` holds the newest 100 rows; older pages and Check
  chain read with `once` (LOADING.md).
- **The Audit Logs page** (`pages/audit.ts`; CONSOLE.md §13), the owner's three views
  (2026-11-13), each the rows of its kind, newest first:
  - **Config:** `settings.*`.
  - **Calls:** rows about a call or a test run (`transcript.view`, `call.watch`, `test.run`).
  - **Security:** the rest: people, roles, page opens, and every refusal.

  Columns: When, Who (a name from the People list when your role shows it, else the start of the
  id), Roles, Action (in words), What, Fields, Versions (before → after), Result (Allowed, or
  Refused with the lever it lacked). 100 rows a page with **Older** and **Newest**, so no list
  passes the 200-row rule (CONSOLE.md §12). **Check chain** says "Every row is as written (n)" or
  names the first row that is not. The page is a plain table in Auth's look: its own look is the
  owner's fork (§7).
- **Who may read it:** Audit logs View. The Compliance shape (Audit logs View, no Edit on Roles)
  may not also hold a role that edits roles or people (ACCESS-ROLES.md §5, the conflict rule).

## 6. Tests and desk test
- `ehr/src/audit.rs` (unit): every field changes the hash; a field's bytes cannot move to the next.
- `ehr/tests/store.rs` (`npm run db:test`): a settings save's row (fields, versions); the card
  saves' rows and that no value reaches the trail; grants, refusals and the rows through the gRPC
  service; the triggers; a row changed with the trigger off, found by `verify` at that row.
- `agent/tests/test_roles.py`: a transcript read with no such call writes nothing.
- **Desk test** (passed 2026-11-19): CONSOLE.md §13.1, step 7. Make a change, a grant and a
  refusal; each is a row on its view; Check chain reads "Every row is as written".

## 7. Open
- **The dev server's own refusals are not recorded.** A route refused by the wall
  (`dev_server.py › ROUTE_NEEDS`) answers 403 before any request reaches `ehr`, so no row is
  written. §5.1 of ACCESS-ROLES.md asks for each refusal. The fix: the wall records it through
  `LogAccess` (or a refusal RPC) before it answers. Not built.
- **The page's own look:** the owner's fork; the plain table stands in.
- **Retention:** rows are never deleted. SECURITY.md's draft is 6 years (HIPAA's record rule;
  STAFF-SIGN-IN.md, M5).
- **The write-once copy and the export** to a hospital's SIEM and privacy tools: at a deploy
  (STAFF-SIGN-IN.md, M5; REGULATIONS.md R6). What an exported row may carry: STAFF-SIGN-IN.md §8.
- **Sign-ins** as rows: with staff sign-in (STAFF-SIGN-IN.md, M2).
- **Reasons for access** (a reason asked before a look at protected data): STAFF-SIGN-IN.md F2.
