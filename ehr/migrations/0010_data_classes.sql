-- Synthetic and real rows, kept apart (DATABASE.md §7), and the kept scripts' runs (§8). The
-- owner, 2026-11-15. No real row exists yet: every call filed so far is synthetic.
--
-- - Two schemas with the same call tables: `synthetic` and `real`. `public` keeps what belongs to
--   neither (the settings store, the people list).
-- - Two roles, one per schema; ehr's pool for a class runs as that class's role and sees only
--   its schema (store.rs › ClassStore).
-- - `calls.source` becomes `origin`; `calls.data_class` is new; real only when the origin is live.
-- - Every call id starts with its origin (`test_20261115-201500`).
-- - The kept scripts' runs: test_runs, test_results, test_replies, in `synthetic` only.
-- - An index on each sealed column's format byte, so the re-seal pass at each start reads only
--   the old values (store.rs › Store::reseal).
--
-- Rule from here on: a change to a call table is written for both schemas, in the same migration
-- (ehr/tests/store.rs compares their columns).

CREATE SCHEMA synthetic;
CREATE SCHEMA real;

-- The roles. Cluster-wide, so made only once per cluster (the store tests make databases of
-- their own in the same cluster). No login: in dev, ehr's one login switches to them (SET ROLE);
-- in the cloud each gets a login of its own (DATABASE.md §7.3).
-- Two databases migrating at once (the store tests run in parallel) may both find a role missing:
-- the second CREATE then fails, and is let pass.
DO $$
BEGIN
    BEGIN
        CREATE ROLE deets_synthetic NOLOGIN;
    EXCEPTION WHEN duplicate_object OR unique_violation THEN NULL;
    END;
    BEGIN
        CREATE ROLE deets_real NOLOGIN;
    EXCEPTION WHEN duplicate_object OR unique_violation THEN NULL;
    END;
    -- The login that runs the migrations may switch to both (a superuser may already).
    IF NOT (SELECT rolsuper FROM pg_roles WHERE rolname = current_user) THEN
        EXECUTE format('GRANT deets_synthetic, deets_real TO %I', current_user);
    END IF;
END
$$;

-- What the store knows about itself: today, which test key sealed `synthetic` (a keyed
-- fingerprint, never the key; store.rs › ClassStore::rekey).
CREATE TABLE store_meta (
    key    text PRIMARY KEY,
    value  text NOT NULL
);

-- The call tables move to `synthetic`, with their rows, indexes and keys.
ALTER TABLE calls SET SCHEMA synthetic;
ALTER TABLE transcript_lines SET SCHEMA synthetic;
ALTER TABLE call_events SET SCHEMA synthetic;
ALTER TABLE tool_calls SET SCHEMA synthetic;
ALTER TABLE staff_flags SET SCHEMA synthetic;
ALTER TABLE review_flags SET SCHEMA synthetic;
ALTER TABLE annotations SET SCHEMA synthetic;
ALTER TABLE audit SET SCHEMA synthetic;

-- The two marks (DATABASE.md §7.1).
ALTER TABLE synthetic.calls RENAME COLUMN source TO origin;
ALTER TABLE synthetic.calls ADD COLUMN data_class text NOT NULL DEFAULT 'synthetic';
ALTER TABLE synthetic.calls ADD CONSTRAINT calls_origin CHECK (origin IN ('typed', 'test', 'kept', 'live'));
ALTER TABLE synthetic.calls ADD CONSTRAINT calls_real_is_live CHECK (data_class <> 'real' OR origin = 'live');
ALTER TABLE synthetic.calls ADD CONSTRAINT calls_class CHECK (data_class = 'synthetic');
-- Rows filed before this migration keep their old ids.
ALTER TABLE synthetic.calls ADD CONSTRAINT calls_id_prefix CHECK (starts_with(id, origin || '_')) NOT VALID;
CREATE INDEX calls_origin_idx ON synthetic.calls (origin);
CREATE INDEX calls_scenario_idx ON synthetic.calls (scenario);

-- `real`: the same tables, made from `synthetic`'s (columns, defaults, checks, indexes). The keys
-- between them are not copied by LIKE; they are added below.
CREATE TABLE real.calls (LIKE synthetic.calls INCLUDING ALL);
CREATE TABLE real.transcript_lines (LIKE synthetic.transcript_lines INCLUDING ALL);
CREATE TABLE real.call_events (LIKE synthetic.call_events INCLUDING ALL);
CREATE TABLE real.tool_calls (LIKE synthetic.tool_calls INCLUDING ALL);
CREATE TABLE real.staff_flags (LIKE synthetic.staff_flags INCLUDING ALL);
CREATE TABLE real.review_flags (LIKE synthetic.review_flags INCLUDING ALL);
CREATE TABLE real.annotations (LIKE synthetic.annotations INCLUDING ALL);
CREATE TABLE real.audit (LIKE synthetic.audit INCLUDING ALL);

ALTER TABLE real.calls DROP CONSTRAINT calls_class;
ALTER TABLE real.calls ADD CONSTRAINT calls_class CHECK (data_class = 'real');
ALTER TABLE real.calls ALTER COLUMN data_class SET DEFAULT 'real';
ALTER TABLE real.calls DROP CONSTRAINT calls_id_prefix;
ALTER TABLE real.calls ADD CONSTRAINT calls_id_prefix CHECK (starts_with(id, origin || '_'));
-- audit's id came from synthetic's sequence; real gets its own.
CREATE SEQUENCE real.audit_id_seq OWNED BY real.audit.id;
ALTER TABLE real.audit ALTER COLUMN id SET DEFAULT nextval('real.audit_id_seq');
ALTER TABLE real.transcript_lines ADD FOREIGN KEY (call_id) REFERENCES real.calls (id) ON DELETE CASCADE;
ALTER TABLE real.call_events ADD FOREIGN KEY (call_id) REFERENCES real.calls (id) ON DELETE CASCADE;
ALTER TABLE real.tool_calls ADD FOREIGN KEY (call_id) REFERENCES real.calls (id) ON DELETE CASCADE;
ALTER TABLE real.staff_flags ADD FOREIGN KEY (call_id) REFERENCES real.calls (id) ON DELETE CASCADE;
ALTER TABLE real.review_flags ADD FOREIGN KEY (call_id) REFERENCES real.calls (id) ON DELETE CASCADE;

-- The kept scripts' runs (DATABASE.md §8). Synthetic only: a real call is never a test.
CREATE TABLE synthetic.test_runs (
    id                bigserial PRIMARY KEY,
    started           timestamptz NOT NULL,
    finished          timestamptz,
    run_by            text NOT NULL,
    mode              text NOT NULL DEFAULT 'replay' CHECK (mode IN ('replay', 'sample')),
    draws             integer NOT NULL DEFAULT 1,
    words_sha256      text NOT NULL,
    prompt_version    text,
    system_version    text,
    hospital_version  text,
    llm_version       text,
    model             text,
    git_commit        text,
    scripts           integer NOT NULL DEFAULT 0,
    passed            integer NOT NULL DEFAULT 0,
    failed            integer NOT NULL DEFAULT 0,
    tests_passed      boolean,
    cost_usd          double precision NOT NULL DEFAULT 0
);
CREATE INDEX test_runs_words ON synthetic.test_runs (words_sha256);

-- One row per draw. `failure` is sealed: an error line may quote the agent's words.
CREATE TABLE synthetic.test_results (
    run_id         bigint NOT NULL REFERENCES synthetic.test_runs (id) ON DELETE CASCADE,
    script         text NOT NULL,
    draw           integer NOT NULL DEFAULT 1,
    mode           text NOT NULL CHECK (mode IN ('record', 'replay')),
    passed         boolean NOT NULL,
    test           text,
    failure        bytea,
    -- The draw's record: its own, or an earlier draw's (option C, §8.2). No key: a pruned record
    -- leaves the row, which keeps its numbers.
    call_id        text,
    reused         boolean NOT NULL DEFAULT false,
    shape_sha256   text,
    duration_ms    double precision,
    tool_ms        jsonb,
    live_requests  integer NOT NULL DEFAULT 0,
    cost_usd       double precision NOT NULL DEFAULT 0,
    tags           jsonb NOT NULL DEFAULT '{}',
    PRIMARY KEY (run_id, script, draw, mode)
);
CREATE INDEX test_results_shape ON synthetic.test_results (script, shape_sha256);
CREATE INDEX test_results_call ON synthetic.test_results (call_id);

-- The reply index: which recorded reply (tests/replays/<key>.json) each script read in a run.
CREATE TABLE synthetic.test_replies (
    run_id     bigint NOT NULL REFERENCES synthetic.test_runs (id) ON DELETE CASCADE,
    script     text NOT NULL,
    reply_key  text NOT NULL,
    PRIMARY KEY (run_id, script, reply_key)
);

-- The roles' rights: each its own schema, nothing else.
GRANT USAGE ON SCHEMA synthetic TO deets_synthetic;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA synthetic TO deets_synthetic;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA synthetic TO deets_synthetic;
ALTER DEFAULT PRIVILEGES IN SCHEMA synthetic GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO deets_synthetic;
ALTER DEFAULT PRIVILEGES IN SCHEMA synthetic GRANT USAGE, SELECT ON SEQUENCES TO deets_synthetic;
GRANT USAGE ON SCHEMA real TO deets_real;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA real TO deets_real;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA real TO deets_real;
ALTER DEFAULT PRIVILEGES IN SCHEMA real GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO deets_real;
ALTER DEFAULT PRIVILEGES IN SCHEMA real GRANT USAGE, SELECT ON SEQUENCES TO deets_real;

-- Plain columns of both classes side by side, for analysts (DATABASE.md §7.2). Never a sealed
-- column. Read by the owner login; neither class role can read the other's rows through it.
CREATE VIEW public.calls_by_class AS
    SELECT id, data_class, origin, started, mode, model, scenario, outcome, end_reason, turns, tool_calls,
           duration_ms, tokens_in, tokens_out, cost_usd, prompt_version, system_version, hospital_version
      FROM synthetic.calls
    UNION ALL
    SELECT id, data_class, origin, started, mode, model, scenario, outcome, end_reason, turns, tool_calls,
           duration_ms, tokens_in, tokens_out, cost_usd, prompt_version, system_version, hospital_version
      FROM real.calls;

-- The format byte of each sealed column, so the re-seal pass finds the old values by an index
-- (store.rs › Store::reseal), not by reading every value at each start.
CREATE INDEX users_email_format ON public.users ((substring(email FROM 1 FOR 1)));
CREATE INDEX users_name_format ON public.users ((substring(name FROM 1 FOR 1)));
CREATE INDEX users_first_name_format ON public.users ((substring(first_name FROM 1 FOR 1)));
CREATE INDEX users_last_name_format ON public.users ((substring(last_name FROM 1 FOR 1)));
CREATE INDEX users_phone_format ON public.users ((substring(phone FROM 1 FOR 1)));
CREATE INDEX user_photos_photo_format ON public.user_photos ((substring(photo FROM 1 FOR 1)));
CREATE INDEX transcript_lines_words_format ON synthetic.transcript_lines ((substring(words FROM 1 FOR 1)));
CREATE INDEX call_events_body_format ON synthetic.call_events ((substring(body FROM 1 FOR 1)));
CREATE INDEX staff_flags_summary_format ON synthetic.staff_flags ((substring(summary FROM 1 FOR 1)));
CREATE INDEX staff_flags_evidence_format ON synthetic.staff_flags ((substring(evidence FROM 1 FOR 1)));
CREATE INDEX annotations_words_format ON synthetic.annotations ((substring(words FROM 1 FOR 1)));
CREATE INDEX test_results_failure_format ON synthetic.test_results ((substring(failure FROM 1 FOR 1)));
CREATE INDEX transcript_lines_words_format ON real.transcript_lines ((substring(words FROM 1 FOR 1)));
CREATE INDEX call_events_body_format ON real.call_events ((substring(body FROM 1 FOR 1)));
CREATE INDEX staff_flags_summary_format ON real.staff_flags ((substring(summary FROM 1 FOR 1)));
CREATE INDEX staff_flags_evidence_format ON real.staff_flags ((substring(evidence FROM 1 FOR 1)));
CREATE INDEX annotations_words_format ON real.annotations ((substring(words FROM 1 FOR 1)));
