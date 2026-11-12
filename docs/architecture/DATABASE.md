---
status: project
desk_test: open
sources: [scripts/postgres.mjs, scripts/dev.mjs, agent/deets_agent/catalog.py, ehr/src/store.rs, ehr/src/seal_key.rs, ehr/migrations/0001_calls.sql, ehr/tests/store.rs]
updated: 2026-11-12
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
  - `POST /agent/annotations`: label, note, keep, rating, through `check_annotation`, still
    appended to `agent/calls/annotations.jsonl` (the newest line wins).
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
  an annotation's text or note. Plain: ids, kinds, times, counts, outcomes.
- **Running it:** `ehr ingest [--calls DIR]` (`ehr/src/main.rs`). `npm run db:ingest` files
  the records by hand; `npm run dev` files them at each start, after it builds `ehr`.
  `npm run db:test` runs `ehr/tests/store.rs` against the dev cluster in a database of its own,
  dropped after, built in `target/store-test` so a running dev server never blocks it.
  `DEETS_EHR_BIN` points `db:ingest` at another build for the same reason.
- **First run, 2026-11-12 (Postgres 18.7 on the PC):** 128 calls, 1,505 transcript lines,
  4,664 events, 25 staff flags and 46 annotation lines filed in 3.4 s; a second run filed none.
- **Not yet:** the live path (the agent sends events over gRPC as the call runs); the read API
  for the console; the audit rows; TLS to the database (the cloud adds it).

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
