-- The call store, first cut (DATABASE.md §4; the draft schema, the owner's to edit).
-- Plain columns hold no protected data: names, kinds, times, counts. Every column that can hold
-- protected data is `bytea`, sealed by ehr with core's seal.rs format, bound to its table, row
-- and column (SECURITY.md §4.6). Only ehr writes here (DATABASE.md §1).

-- One row per call. The record's hash makes the ingest idempotent: an unchanged record is
-- skipped; a changed one is filed again, whole.
CREATE TABLE calls (
    id              text PRIMARY KEY,
    hospital        text NOT NULL DEFAULT 'dev',
    schema_version  integer NOT NULL,
    -- The record's own clock. Dev records hold local time; the live path will write UTC.
    started         timestamp,
    source          text NOT NULL,
    mode            text,
    model           text,
    scenario        text,
    outcome         text NOT NULL,
    end_reason      text,
    reached         text[] NOT NULL DEFAULT '{}',
    turns           integer NOT NULL DEFAULT 0,
    tool_calls      integer NOT NULL DEFAULT 0,
    duration_ms     double precision NOT NULL DEFAULT 0,
    tokens_in       bigint NOT NULL DEFAULT 0,
    tokens_out      bigint NOT NULL DEFAULT 0,
    cost_usd        double precision NOT NULL DEFAULT 0,
    record_sha256   text NOT NULL,
    filed_at        timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX calls_started ON calls (started);
CREATE INDEX calls_outcome ON calls (outcome);

-- What was said, in order. The words are sealed.
CREATE TABLE transcript_lines (
    call_id  text NOT NULL REFERENCES calls (id) ON DELETE CASCADE,
    seq      integer NOT NULL,
    t_ms     double precision NOT NULL,
    speaker  text NOT NULL CHECK (speaker IN ('caller', 'agent')),
    words    bytea NOT NULL,
    PRIMARY KEY (call_id, seq)
);

-- Every event of the record. The kind and time are plain; the whole event is sealed.
CREATE TABLE call_events (
    call_id  text NOT NULL REFERENCES calls (id) ON DELETE CASCADE,
    seq      integer NOT NULL,
    t_ms     double precision NOT NULL,
    kind     text NOT NULL,
    body     bytea NOT NULL,
    PRIMARY KEY (call_id, seq)
);
CREATE INDEX call_events_kind ON call_events (kind);

-- One row per tool result: names and numbers only.
CREATE TABLE tool_calls (
    call_id  text NOT NULL REFERENCES calls (id) ON DELETE CASCADE,
    seq      integer NOT NULL,
    t_ms     double precision NOT NULL,
    tool     text NOT NULL,
    ms       double precision,
    failed   boolean NOT NULL DEFAULT false,
    PRIMARY KEY (call_id, seq)
);

-- Staff flags (FLAGS.md): the kind leaves the wall; the summary and the evidence do not.
CREATE TABLE staff_flags (
    call_id     text NOT NULL REFERENCES calls (id) ON DELETE CASCADE,
    seq         integer NOT NULL,
    t_ms        double precision NOT NULL,
    kind        text NOT NULL,
    source      text NOT NULL CHECK (source IN ('code', 'model')),
    unverified  integer NOT NULL DEFAULT 0,
    summary     bytea NOT NULL,
    evidence    bytea NOT NULL,
    PRIMARY KEY (call_id, seq)
);
CREATE INDEX staff_flags_kind ON staff_flags (kind);

-- Review flags (FLAGS.md §8): for an analyst or an engineer. A reason, never a value.
CREATE TABLE review_flags (
    call_id  text NOT NULL REFERENCES calls (id) ON DELETE CASCADE,
    seq      integer NOT NULL,
    t_ms     double precision NOT NULL,
    reason   text NOT NULL,
    final    boolean NOT NULL DEFAULT false,
    PRIMARY KEY (call_id, seq)
);

-- Labels, notes, stars and ratings (CHAT.md §7). One row per line of annotations.jsonl, keyed by
-- the line's hash, so a merged file files each line once. No foreign key: a line may name a call
-- filed later. The free text (a label's or note's text, a rating's note) is sealed.
CREATE TABLE annotations (
    line_sha256     text PRIMARY KEY,
    call_id         text NOT NULL,
    kind            text NOT NULL CHECK (kind IN ('label', 'note', 'keep', 'rating')),
    at              timestamp,
    grader_kind     text,
    grader          text,
    dimension       text,
    verdict         text,
    score           smallint CHECK (score BETWEEN 0 AND 5),
    rubric_version  text,
    keep            boolean,
    words           bytea
);
CREATE INDEX annotations_call ON annotations (call_id);

-- Who read what, and when (SECURITY.md §4.10). Plain ids only. Nothing writes here yet.
CREATE TABLE audit (
    id       bigserial PRIMARY KEY,
    at       timestamptz NOT NULL DEFAULT now(),
    actor    text NOT NULL,
    action   text NOT NULL,
    call_id  text
);
