-- Analytics › Cost and Speed (docs/architecture/ANALYTICS.md §3.2; the owner, 2026-11-22): a
-- voice call's Cartesia credits, and each answered turn's wait split by stage, as plain numbers
-- worked out by ehr when it files the record (store.rs › summarize, turn_stages). Never a word of
-- the call. Both schemas (DATABASE.md §7.2).
--
-- `cartesia_credits`: Ink's and Sonic's credits from `call_end.cartesia` (0 for a text call).
-- `turn_stages`: one object per answered turn, in the order of `turn_waits_ms`, each stage in ms:
-- `ink` (the caller's silence to Ink's final line), `claude` (the requests to Claude), `tools`
-- (the tools' own runs, less the ehr calls in them), `ehr` (the ehr calls), `voice` (the first
-- frame out to the caller hearing it) and `other` (the rest of the wait).

ALTER TABLE synthetic.calls
    ADD COLUMN cartesia_credits double precision NOT NULL DEFAULT 0,
    ADD COLUMN turn_stages jsonb NOT NULL DEFAULT '[]';

ALTER TABLE real.calls
    ADD COLUMN cartesia_credits double precision NOT NULL DEFAULT 0,
    ADD COLUMN turn_stages jsonb NOT NULL DEFAULT '[]';

-- Every record already filed is filed again at the next `ehr ingest`, so the new columns fill.
UPDATE synthetic.calls SET record_sha256 = '';
UPDATE real.calls SET record_sha256 = '';
