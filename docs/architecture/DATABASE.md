---
status: project
desk_test: open
sources: [scripts/postgres.mjs, scripts/dev.mjs, agent/deets_agent/catalog.py, ehr/src/store.rs, ehr/migrations/0010_data_classes.sql, ehr/src/edge.rs, agent/tests/test_typed_call.py, agent/harness/typed_call.py, agent/deets_agent/call_record.py, ehr/src/seal_key.rs, core/src/seal.rs, ehr/src/service.rs, ehr/migrations/0001_calls.sql, ehr/migrations/0002_settings.sql, ehr/src/settings.rs, ehr/tests/store.rs, agent/deets_agent/ehr_client.py, agent/deets_agent/ehr_process.py, agent/deets_agent/dev_server.py]
updated: 2026-11-15
---
# The database — dev today, shaped for production

The plan for one database that holds every call's data the way the production store will
(VOICE-AGENT.md §8.7), starting on each dev machine and moving to the cloud without a rewrite.
The current dev catalog (`agent/calls/catalog.db`, CHAT.md §7) is the thing it replaces.

## 1. Decisions
- **Postgres, shaped like the production store** (the owner, 2026-10-19 and 2026-11-12: "mimic
  a production db … be ready to take it to the cloud / scale"). The search columns are plain;
  every field that can hold protected data is sealed (`seal.rs`), as in production.
- **Native Postgres on each dev machine** (the owner, 2026-11-12: Docker Desktop and WSL2 are
  fiddly on the PC). Docker Compose stays for the later rehearsal of the cloud stack
  (CLOUD.md §5a), on the Mac.
- **A script files each call record into the database** (the owner, 2026-11-12). The ingest is
  idempotent: it upserts by call id and never deletes, so a rebuild never needs to drop the file
  that another process holds open (the Windows catalog bug).
- **The Rust `ehr` service is the store's only writer, and the agent never opens the database**
  (VOICE-AGENT.md §8.7; confirmed by the owner, 2026-11-12: "still true if it'd work at scale",
  see §2.1). The ingest is a command of `ehr`.

## 2. Writers and readers
- **Writes:** `ehr` only (§1). Today, a backfill command files the dev call records;
  later the agent sends each event over gRPC as the call runs. Sealing happens in Rust, under a
  local dev key here and the hospital's KMS key in the cloud.
- **Reads:** the console through a read API, never the database directly. Until that API exists,
  the dev catalog keeps serving the Chat page.
- **The contract the Chat page depends on** (from the session that built it, 2026-11-12). The move
  must keep these shapes, or change the page with them:
  - `GET /agent/catalog`: `calls[]` with `id`, `label`, `note`, `keep` and `ratings[]` from the
    current ratings (`grader_kind`, `grader`, `dimension`, `verdict`, `score`, `note`,
    `rubric_version`, `rated_at`); plus `dimensions`, `outcomes`, `rubric_version`. The Review
    panel finds the owner's rating by grader `owner` and the model's by grader kind `model`.
  - `POST /agent/annotations`: label, note, keep, rating, through `check_annotation`, then kept
    in the call store through `ehr` (§9; the newest one wins).
  - `GET /agent/latency`: the `tool_calls` table (one row per tool call, spans in ms, exact /
    replayed / failed as 0 or 1) and `Catalog.latency()`. Its SQL uses SQLite placeholders, and
    `latency.py` builds the rows (tests: `agent/tests/test_latency.py`).

### 2.1 Why one writer still scales
"Only writer" means only the `ehr` service writes, not one process. What keeps it fast as calls
grow:
- **Many `ehr` replicas.** Each one writes; Postgres takes many writers at once. A call's
  session lives in one replica's memory (`ehr/src/session.rs`), so the load balancer keeps a
  call on its replica; new calls spread across all of them.
- **One database per hospital** (A2). The load splits by hospital before it reaches Postgres.
- **The call never waits for a log write.** Chart reads and bookings stay synchronous; the
  record's events (lines, tool timings, flags) go to the store from a buffer in the background,
  in batches. A slow write never adds silence to a call. When the event stream grows past what
  a buffer holds, a queue sits between the agent and `ehr` (CLOUD.md plans one for the watch
  stream).
- **A connection pool in each replica,** and a pooler in front of Postgres (the cloud's proxy, or
  PgBouncer) when the replicas grow, so connections do not grow with replicas.
- **Reads off the writer:** the console's lists and dashboards can read from a replica of the
  database later. The big table (`call_events`) is split by month, so old months drop as one
  piece when retention ends.
- **What it buys:** sealing and `Shielded<T>` stay in one language, one place checks every
  write, and the audit trail cannot be skipped by a second writer.

## 3. Dev setup (built 2026-11-12)
> **Part:** built · 2026-11-12

- **Install once:** Postgres at the pinned major version (`PG_MAJOR` in `scripts/postgres.mjs`).
  Windows: `winget install --id PostgreSQL.PostgreSQL.<major>`; the Mac:
  `brew install postgresql@<major>`. The version matches what the cloud's managed Postgres
  serves: AWS RDS has served 18 since November 2025, with standard support to February 2031
  (pinned 2026-11-12, when the owner installed 18). Moving up is one upgrade later. If winget's
  download fails (it did twice on 2026-11-12, from EDB's CDN), use EDB's installer from
  postgresql.org.
- **The installer's own server is not used.** On Windows the installer also makes a Windows
  service on port 5432. It may be set to start by hand; ours never touches it.
- **`npm run dev` starts our cluster if it is not up** (`ensurePostgres` in
  `scripts/postgres.mjs`): one cluster per repo in the git-ignored `.pg/`, run as the signed-in
  user by `pg_ctl` (no admin rights, no service), on 127.0.0.1 only, scram passwords, a random
  dev password made at first start. A running cluster keeps its port; a new start takes the
  first free port from 5433 up (never a fixed port). It stays up after Ctrl+C; `npm run db:stop`
  stops it, `npm run db:status` says where it runs.
- **The agent gets `DEETS_DATABASE_URL`** for `ehr`. Nothing reads it yet. A machine without
  Postgres goes on without the database, with a line that says how to install it.

## 4. The schema (a draft for the owner to edit)
> **Part:** built · 2026-11-12 — as `ehr/migrations/0001_calls.sql`, still the owner's to edit

The migration is the one copy (DOCS-ORG.md §1, rule 8); the list below is the intent.

Plain columns hold no protected data; sealed columns are bytes under `seal.rs`.
- `calls`: id, hospital, started, source (typed · test · live), mode, model, scenario, outcome,
  end reason, nodes reached, turns, duration, tokens, cost. All plain.
- `transcript_lines`: call, order, speaker, time; the words sealed.
- `call_events`: call, order, time, kind, plain fields; the body sealed.
- `tool_calls`: call, order, tool, node, the timing spans, failed. Plain (names and numbers).
- `staff_flags`: call, kind, source, time, unverified count plain; summary and evidence sealed
  (FLAGS.md).
- `review_flags`: call, reason, time, final. Plain.
- `annotations`: call, kind, grader, dimension, verdict, score, rubric version, time; the note
  sealed.
- `audit`: who read what and when (each transcript view, each flag's notes). Plain ids only.
- **`users`** (`0007_users.sql`, 2026-11-14; ops/DEMO.md §3.5): who may sign in to the console.
  The email and the name sealed; `email_hash` a keyed hash of the email (unique) to find a row;
  `source`, `owner` (at most one), `added_by`, `added_at`, `last_seen_at`, `removed_at` plain
  (a removal keeps the row, for the audit log); `role` empty until roles. Written by `ehr`'s
  `users.rs` only.
  Since `0008_user_profile.sql` (2026-11-14; the owner's forks for the card): `first_name`,
  `last_name` and `phone` (E.164) sealed like the email; `phone_verified` (false until a text
  code proves it, which is not built; any change to the number sets it back); `updated_at`.
  `name` stays, kept equal to "first last". Rows from before have no parts: `users.rs` splits
  `name` at its last space until the card is saved. An email change moves the row to the new
  email's hash and seals every column again under it.
- **`user_photos`** (`0008_user_profile.sql`): one picture per person, a 256 px WebP the
  console makes (at most 256 KB), sealed and bound to the person's id, so an email change does
  not touch it.
- **The settings store** (`0002_settings.sql`, 2026-11-13; CONSOLE.md §7.3): `setting_versions`,
  one key–value table per service (`settings_voice`; `settings_llm` since `0004_settings_llm.sql`,
  CONSOLE.md §8.3; `settings_prompt`, the agent's words, since `0005_settings_prompt.sql`,
  CONSOLE.md §9.3; `settings_system`, the System variables, since `0006_settings_system.sql`,
  SYSTEM-PAGE.md; `settings_hospital`, Config › Hospital's lists and numbers, since
  `0009_settings_hospital.sql`, HOSPITAL-CONFIG.md), and `setting_activations`.
  `calls.prompt_version` (0005) names the words each call ran with, `calls.system_version`
  (0006) its System variables, and `calls.hospital_version` (0009) its hospital, each by the
  version's label. All plain and
  insert-only: a trigger refuses every UPDATE, DELETE and TRUNCATE. Written by `ehr`'s
  `SaveSettings` (`ehr/src/settings.rs`), checked there against the list of every setting.
- **Since `0010_data_classes.sql` (2026-11-15):** the call tables above live in two schemas,
  `synthetic` and `real`; `calls.source` is `origin`, and `calls.data_class` is new (§7). The kept
  scripts' runs are `test_runs`, `test_results` and `test_replies` in `synthetic` (§8).
  `store_meta` (public) records which test key sealed `synthetic`.
- **Migrations** in the repo, applied in order, the same files on both dev machines and in the
  cloud.

## 4a. The store in `ehr` (as built, 2026-11-12)
> **Part:** built · 2026-11-12 · desk test open

- **The store:** `ehr/src/store.rs`. `Store::connect` runs the embedded migrations
  (`MIGRATOR`). `file_dir` files every record in a folder, then the annotations.
  `file_record` skips a record whose hash is filed, and files a changed one again, whole, in one
  transaction. `file_annotations` files each line once, by its hash. `summarize` works out the
  plain columns (the outcome as the dev catalog does).
- **Sealing:** `ehr/src/seal_key.rs`. `AwsLc` fills core's `Cipher` with AES-256-GCM from
  AWS-LC (its FIPS module under `--features fips`); core's `seal.rs` owns the format and binds
  each value to its table, row and column. The dev key is `.pg/seal.key`, made by
  `scripts/postgres.mjs`; a deploy refuses a key file (`SealKey::from_dev_file`).
- **Sealed columns:** transcript words, every event's body, a staff flag's summary and evidence,
  an annotation's text or note. Plain: ids, kinds, times, counts, outcomes. The one list of
  them, with each table's row key, is `SEALED` in `store.rs`. A new sealed column needs its
  line there; a test fails when the schema has a `bytea` column that the list lacks.
- **The format change of 2026-11-14 (the owner):** the associated data's domain string was
  `deetshealth.cache.v1`, a name from the patient app's cache. It is now
  `deetshealth.store.v1`, under a new format byte (3). Formats 1 and 2 still open with the old
  string and are never written.
- **The re-seal pass:** `Store::reseal`, run by `Store::connect` after the migrations. It finds
  each value whose first byte is not the current format, opens it in its place, and seals it
  again. One transaction per table. A value that does not open keeps its bytes and is counted.
  `ehr` prints the two counts on stderr, and nothing when both are zero. Two runs at once are
  safe (the rows are locked). Each machine's database changes at its first start after the
  pull; there is no step to do by hand. Key rotation (SECURITY.md §4.6) can use the same pass.
- **Running it:** `ehr ingest [--calls DIR]` (`ehr/src/main.rs`). `npm run db:ingest` files
  the records by hand; `npm run dev` files them at each start, after it builds `ehr`.
  `npm run db:test` runs `ehr/tests/store.rs` against the dev cluster in a database of its own,
  dropped after, built in `target/debug` like every build (CLAUDE.md › Conventions). A running
  dev server does not block it: it runs a copy of ehr from `target/run/` (`DEETS_EHR_BIN`).
  `DEETS_EHR_BIN` also points `db:ingest` at another build.
- **First run, 2026-11-12 (Postgres 18.7 on the PC):** 128 calls, 1,505 transcript lines,
  4,664 events, 25 staff flags and 46 annotation lines filed in 3.4 s; a second run filed none.
- **The live path (built 2026-11-12):** when a call ends, the agent streams its record to `ehr`
  line by line (`FileCall` in `ehr.proto`; `file_call` in `ehr/src/service.rs` and in
  `agent/deets_agent/ehr_client.py`), and `ehr` files it at once. The dev server does this for
  every Chat-page test call (`_file` in `dev_server.py`); the typed harness for every typed call.
  `ehr` opens the store at start when it has the settings: `npm run dev` passes them; a typed
  call run by hand reads `.pg/ehr.env`, which `scripts/postgres.mjs` rewrites at each start.
  The tests' `ehr` runs without a store (`running_ehr(store=False)`), so a test never writes
  the dev database. If the store is missing or down, the call is still served; it is filed by
  the next `ehr ingest`.
- **SQL is text from the code only** (sqlx 0.9, 2026-11-16; DEPENDENCIES.md §4): a query is a
  `&'static str`, with `concat!` for a fixed fragment shared by several queries. A query joined
  from pieces (a class's schema and role, a table, its sealed columns, a `$n`) goes through
  `ehr/src/sql.rs`'s `Sql`, which takes `&'static str`, numbers and bytes only, never a
  `String`. Every value is a `$n` bind. A new `AssertSqlSafe` in `ehr/src` outside `sql.rs` is a
  review stop.
- **One hash for both paths:** the hash is over the record's canonical form (non-empty lines,
  no `\r`), so a record read from disk with Windows line ends and the same record streamed over
  `FileCall` are one record. The ingest skips what the live path filed.
- **Not yet:** each event filed as it happens (today the whole record at the end); the read API
  for the console; the audit rows; TLS to the database (the cloud adds it).

### 4b. Working on the Mac (or any new machine)
The call store **adds to the database; it never deletes it or rebuilds it.** Each call is filed
once, when it ends, and `npm run dev` files only what is new or changed since the last run.
- **First time:** `brew install postgresql@18` (the pinned major, `PG_MAJOR` in
  `scripts/postgres.mjs`), then `npm run dev`. It makes the repo's cluster in `.pg/`, the
  database, the dev password and the sealing key, and files every record in `agent/calls/`.
- **Each machine has its own database and its own sealing key** (`.pg/` is git-ignored). The
  call records in `agent/calls/` are committed and shared; each machine files them into its own
  database. A call made on the PC reaches the Mac's database after a pull and an `npm run dev`.
- **Never copy `.pg/` between machines.** The key in it opens the sealed columns; a copy is a
  second key to guard.
- **To start over** (not needed day to day): `npm run db:stop`, delete `.pg/`, `npm run dev`.
  The records rebuild everything, because the records, not the database, are the dev source.
- **Not the same thing: the Chat page's catalog** (`agent/calls/catalog.db`, SQLite, CHAT.md §7)
  is still rebuilt from the records when the dev server starts. It retires when the Chat page
  reads the store through `ehr` (§6).

## 5. To be demo ready (synthetic data in the cloud)
It is more than Docker and a cloud account. In order:
1. **The store in `ehr`:** the schema, its migrations, the sealing and the ingest are built
   (§4a). Left: the key from the KMS in place of the dev key file; a read API for the console;
   the live event path from the agent.
2. **Container images** for `ehr`, the agent and the console (Docker builds them; the cloud runs
   them). CLOUD.md §5 holds the deploy shape.
3. **The cloud's pieces** (C1: AWS `us-east-1`): managed Postgres at the pinned version (RDS);
   a KMS key; Secrets Manager for the database password and vendor keys; the network with the
   database in private subnets, open only to `ehr`, TLS forced; ECS with Fargate and a load
   balancer with a certificate. Cost: CLOUD.md §5a (about $60–75 a month for staging).
4. **Migrations at each deploy,** as a one-off task before `ehr` starts, with a snapshot first.
5. **Backups:** automated backups with point-in-time restore (the managed service's default),
   and one restore tried by hand.
6. **The demo's data:** the synthetic calls filed by the same ingest; nothing real.
7. **Watching it:** the cloud's metrics and logs, with no protected data in them (SECURITY.md
   §4.5).

**Beyond the demo, before a real hospital:** every vendor's BAA signed; one cloud account and
database per hospital (A2); the hospital's own key (BYOK); retention jobs (transcripts 30 days, a
draft) and crypto-shredding; the audit trail live; the paperwork of SECURITY.md §6.

## 6. Open
- The schema (§4): the owner's to edit.
- When the Chat page's reads move from the dev catalog to `ehr`'s read API.
- The script tags (§8.4): a draft for the owner to edit.
- The kept-record retention values (§8.6): a draft for the owner to edit.
- How the console marks a synthetic row (§7.6): the owner's, when a page that lists calls from
  the store is built.

## 7. Synthetic and real rows, kept apart
> **Part:** built · 2026-11-15 · desk test open. The owner, 2026-11-15: "anything we can do to
> better differentiate test rows from real runs / rows?", then "doc it thoroughly, and build out
> this". No real call exists yet (none before the BAAs), so the separation is built before the
> first real row, not moved in after.

**Terms.** A **real** row comes from a real patient's call. A **synthetic** row comes from a
sandbox patient or a simulated caller: every row on a dev machine, every kept-script draw, every
console Test call, every demo call. The **data class** of a row is one of the two.

### 7.1 Two marks on every call
- **`origin`**: who started the call. `typed` (the typed harness), `test` (the console's Test
  seat, the owner's or a guest's over the demo tunnel), `kept` (a kept-script draw, §8), `live`
  (a phone call over Twilio). It was `source`; migration 0010 renamed the column. Demo replays
  start no call, so they have no origin of their own.
- **`data_class`**: `synthetic` or `real`.
- **The rule, in the database:** `real` only when `origin = live` (a CHECK on `calls`). A
  kept-script draw or a test call cannot be filed as real, even by a bug. A synthetic call may be
  live: a simulated caller who dials the real number is `live` and `synthetic`.
- **In the record:** `call_start` carries `source` (the origin) and, since schema 14,
  `data_class`. A dev record is always `synthetic` (call_record.py).

### 7.2 Two schemas, the same tables
- **`synthetic`** and **`real`** each hold the call tables: `calls`, `transcript_lines`,
  `call_events`, `tool_calls`, `staff_flags`, `review_flags`, `annotations`, `audit`.
  `synthetic` also holds the test-run tables (§8): a real call is never a test.
- **`public`** holds what belongs to neither: the settings store and the people list (`users`,
  `user_photos`). A setting or a staff card is not call data.
- **One template.** `real`'s tables are made `LIKE` `synthetic`'s (migration 0010). **Every later
  change to a call table is written for both schemas in the same migration;** a test
  (`ehr/tests/store.rs`) fails when the two schemas' columns differ.
- **Analysts compare across classes** through a view of plain columns only, never a sealed one:
  `public.calls_by_class` (the id, the origin, the class, the outcome, the counts, the cost).

### 7.3 Two database roles, enforced by Postgres
- `deets_synthetic` may read and write the `synthetic` schema only; `deets_real`, the `real`
  schema only. Neither may touch the other (no grant; Postgres refuses).
- `ehr` stays the only writer (§1). It holds one pool per class, and each pool's connections run
  `SET ROLE` to that class's role and set `search_path` to that schema alone. A query on the
  synthetic pool cannot name a real table.
- The owner login runs the migrations and the `public` tables (settings, people).
- **In the cloud** each role gets its own login and password (Secrets Manager), so a leaked
  synthetic credential opens nothing real; on a dev machine one login switches roles (`SET
  ROLE`), which is enough because no real row exists there. The cloud step is in §5.

### 7.4 Two sealing keys
- **The hospital key** seals `public` and `real`: the people list and real calls. In dev it is
  `.pg/seal.key` (`DEETS_SEAL_KEY_FILE`); in the cloud, the hospital's KMS key.
- **The test key** seals `synthetic`: `.pg/seal-test.key` (`DEETS_TEST_SEAL_KEY_FILE`), made by
  `scripts/postgres.mjs` like the other. A leak from the test side opens nothing real, and the
  test key never needs the hospital's key service.
- **The move (built):** the synthetic rows filed before migration 0010 were sealed under the
  hospital key. At its first connect, `ehr` opens each with the test key, else with the hospital
  key, and seals it again under the test key, one transaction per table. A marker in
  `public.store_meta` (`key.synthetic`, a keyed fingerprint of the test key, never the key)
  records that it is done; the pass does not run again. A value that opens under neither key keeps
  its bytes and is counted, as in the format pass (§4a).
- **The real pool opens only in a deploy,** with the hospital's KMS key (not built). On a dev
  machine `ehr` has no real store: a call or an annotation for a real row is refused
  (FAILED_PRECONDITION, "no real store on this ehr").

### 7.5 What else marks a row
- **A prefix on every call id:** `typed_`, `test_`, `kept_`, `live_`, then the start time
  (`test_20261115-201500`). Every log line, URL and ledger entry shows the origin. A CHECK on
  `calls` makes the prefix match the origin; rows filed before 0010 keep their old ids (the CHECK
  is `NOT VALID` for them), and `ehr ingest` skips a record without a prefix, with a count.
- **`ehr` checks the EHR against the class** at `OpenCall` (`data_class` in the request): a
  synthetic call may reach only a sandbox (the Local fakes, Epic's sandbox); a real call only the
  hospital's production EHR. Every source today is a sandbox, so every real call is refused
  until a production adapter exists. This blocks the worst mistake: a test path that books into
  a real schedule.
- **Retention per schema:** the kept-record pass (§8.6) runs in `synthetic` only; the hospital's
  transcript rule (VOICE-AGENT.md §8.7) will run in `real`. A cleanup of test rows cannot reach a
  real row.

### 7.6 The console (not built)
Every list and chart that reads the store names its class, and a synthetic row carries a mark.
The look of the mark (its words, color and place) is the owner's, when the console reads the
store (§2). Today the console reads the dev catalog, which holds synthetic calls only.

### 7.7 What it costs, in every place it runs
- **Dev:** a second key file, made once; the move above, once.
- **Our cloud (A2) and a hospital's cloud (B):** two roles with their own logins, and two keys
  (the test key need not come from the hospital's KMS). Part of the deploy's checklist (§5).
- **Each EHR:** an adapter says whether it is a sandbox (`edge.rs › is_sandbox`). A new source
  must answer that before it compiles.
- **API dollars:** none.

## 8. The kept scripts' runs
> **Part:** built · 2026-11-15 · desk test open. The owner, 2026-11-15: run history in Postgres
> through `ehr` (A), and "doc it". Option C for a passing replay draw (below).

**Terms.** A **kept script** is a caller script in `agent/tests/scripts/`. A **run** is one press
of Test on Config › Agent: it records the scripts that need new replies, then runs the agent's
tests. A **draw** is one script played once in a run: a **record draw** (the typed harness, which
calls Claude for a line not yet recorded) or a **replay draw** (the tests, from the recorded
replies, with their checks).

### 8.1 The tables (`synthetic` schema, migration 0010)
- **`test_runs`**: one row per run. `started`, `finished`, `run_by` (`dev` on a dev machine; a
  staff id later), `words_sha256` (the prompt file's digest, the same as
  `tests/RECORDED-PROMPT.sha256`), `prompt_version`, `system_version`, `hospital_version`,
  `llm_version` (each a label), `model`, `git_commit`, `scripts`, `passed`, `failed`,
  `tests_passed` (the agent's other tests), `cost_usd` (the replies the run recorded).
- **`test_results`**: one row per draw. `run_id`, `script`, `draw`, `mode` (`record` ·
  `replay`), `passed`, `test` (the pytest test's name), `failure` (the first line of the
  error, **sealed**: it may quote the agent's words), `call_id` (the record of this draw's call,
  maybe an earlier draw's, §8.2), `reused`, `shape_sha256` (§8.2), `duration_ms`, `tool_ms` (each
  tool's time, as JSON), `live_requests`, `cost_usd`, `tags` (the script's tags, §8.4). Plain columns hold names and numbers only.
- **`test_replies`**: the reply index. One row per run, script and recorded reply it read
  (`reply_key`, the file name in `tests/replays/`). Pruning is one query: a reply that no script
  of the newest run read is no longer needed.
- **Each draw's call record** is filed in `synthetic.calls` with `origin = kept` (§8.2), so the
  timeline, the tool spans and the costs work on it with no new code. Call lists hide `kept`
  unless a filter shows it.

### 8.2 What a draw keeps (option C, the owner, 2026-11-15)
On replay the model's replies are fixed, so two passing replay draws of one script on the same
code make the same call. Filing each one would add a copy per press.
- **The shape:** a hash of a record without its times and ids (`call_record.py › shape_digest`:
  every `t_ms`, every key ending in `_ms`, `ms`, `started`, `call_id`; the record's own name).
  The same call, made again, has the same shape.
- **A passing replay draw whose shape matches the script's last filed draw** files no record.
  Its `test_results` row points to that record (`call_id`, `reused = true`) and keeps this
  run's timings (`duration_ms`, `tool_ms`): the code's speed on every run, as numbers.
- **Every other draw files its record:** a failed draw, a draw whose shape changed (the code
  changed what the call does), and every record draw that made a live request to Claude (a fresh
  draw). A record draw that replayed every line files nothing and gets no row: its replay draw
  follows.
- `ehr` decides, in `FileTestRun`: it answers with the call ids whose records it needs, and the
  dev server files those over `FileCall`.

### 8.3 The run, end to end
1. Test on Config › Agent (`record_and_test` in `dev_server.py`).
2. Each script through the typed harness in record mode (`--origin kept --calls <tmp>`): its
   record goes to a temporary folder, never to `agent/calls/`; `--keys-out` lists the replies it
   read.
3. `pytest` with `DEETS_KEPT_DIR=<tmp>`: each kept-script test (`tests/test_typed_call.py`)
   writes its draw's record there and a line to `kept.jsonl` (the script, the test, the record,
   the shape, the replies read, the timings); `--junitxml` gives every test's outcome and first
   error line.
4. The dev server sends the run to `ehr` (`FileTestRun`), files the records `ehr` asks for
   (`FileCall`), and keeps the failures for the Test button's hint (§8.5).
5. Without a store (Postgres down), steps 1 to 3 and the hint still work; nothing is filed.

### 8.4 The script tags (a draft for the owner to edit)
A header at the top of each script, as `#` lines: `# safety: yes`, `# area: book`, `# ehr:
local`, `# channel: text`, `# caller: cooperative`. The harness and the tests skip `#` lines.
- `safety`: yes or no. A safety script (emergency, verification) must pass every draw.
- `area`: greet · verify · book · change · emergency · medical · hand-off · off-topic.
- `ehr`: local · epic. `channel`: text · voice. `caller`: cooperative · confused · hostile.

Git holds the scripts; a run copies the tags into `test_results.tags` (JSON), so a query groups
by any tag. Today's 15 scripts carry the draft tags.

### 8.5 The Test button's hint
Each failed test's name and the first line of its error, from the run's `--junitxml` (the agent's
other tests too), shown in the hint of Test on Config › Agent. Synthetic callers only, so the line
may quote the agent's words; it never leaves the dev server except to the console and `ehr`.
GET `/agent/prompt` carries it as `recording.failures` (DEV-API.md).

### 8.6 Retention (a draft for the owner to edit)
- `test_runs`, `test_results` and `test_replies` are kept: they are small.
- A `kept` call record is deleted when no draw of a run with one of the 3 newest words versions
  (`words_sha256`) points to it, unless it is starred (`keep`). The pass runs in `FileTestRun`,
  in `synthetic` only.

### 8.7 Later
- Sampling: `draws` on a run, k fresh draws per script, pass^k (AGENT.md §4). The tables take it
  as is.
- Code graders: each check that fails could file a rating (`grader_kind = code`, §9) on the
  draw's record, so one query counts the owner's, a model's and the code's verdicts.

## 9. Annotations: the store is the source
> **Part:** built · 2026-11-15 · desk test open (the owner, 2026-11-15, "this", for the plan that
> the Review panel writes to Postgres).

- **Why:** a rating's note on a real call is protected data, and protected data never goes in
  git; a hospital's deploy has no git file. So `ehr` holds every label, note, star and rating.
- **Writes:** `POST /agent/annotations` checks the body (`check_annotation`, unchanged), stamps
  it, and sends it to `ehr` (`AddAnnotation`), which seals the words and files it in the call's
  class (by the call id's prefix: `live_` is real). Then the catalog applies it. Without a store
  the POST answers 503 ("the store is not running"); nothing is written anywhere else.
- **Reads:** the dev catalog reads them back from `ehr` (`ListAnnotations`, synthetic only) each
  time it is built. The `GET /agent/catalog` shape is unchanged (§2).
- **`agent/calls/annotations.jsonl` is retired.** Nothing writes it. If one is present (from the
  tag `archive/calls-2026-11-15`), `ehr ingest` still files it once, and the catalog still reads
  it; then it can be deleted.
- **Cost:** ratings no longer move between the PC and the Mac through git, like the run history
  (§8). A dev-only export and import is a small script, if wanted later.

## 10. Desk test (§7 to §9)
> **Part:** built · 2026-11-15 · desk test open

Restart `npm run dev` first: its `ehr` and the migration are new.
1. The first start after the pull: the `[db]` lines say the store sealed values again (the move
   to the test key, §7.4). `.pg/seal-test.key` now exists beside `.pg/seal.key`. The next start
   says nothing about sealing.
2. Chat › Testing: make a test call and end it. Its id starts with `test_` (the Review panel's
   call, the record's file name in `agent/calls/`).
3. Star it, label it, and set one verdict. Restart `npm run dev`: all three are still there, and
   `agent/calls/annotations.jsonl` was not written (§9).
4. Config › Agent: press Test (once for the cost, once to run). It passes, as the recordings
   already match the words. Hover Test: the hint has no failures.
5. To see a failure in the hint: in `agent/tests/test_typed_call.py`, change `assert d.booked()
   == 1` in `test_book_a_visit` to `== 2`, save, edit any line on Config › Agent so Test is due,
   and run Test. The hint lists `test_book_a_visit` with its first error line. Undo both.
6. Optional, in a terminal: `node scripts/postgres.mjs status`, then `psql` with the dev password
   (`.pg/password`) to the printed port, database `deetshealth_dev`, user `deets`:
   `SELECT id, origin, data_class FROM calls_by_class;` lists the test call as `synthetic`;
   `SELECT script, mode, passed, reused FROM synthetic.test_results ORDER BY run_id DESC LIMIT 15;`
   shows the last run's draws (`reused` true for a passing replay draw that matched the last one).
