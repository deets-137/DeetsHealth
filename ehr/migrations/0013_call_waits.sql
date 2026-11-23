-- Analytics › Logs (docs/architecture/ANALYTICS.md §3.1; the owner, 2026-11-22): a call's seat,
-- its EHR and each caller turn's wait, as plain columns. ehr works them out when it files the
-- record (store.rs › summarize), inside the wall, from the events it seals: numbers and fixed
-- words only, never a word of the call. Both schemas, as every change to a call table
-- (DATABASE.md §7.2).
--
-- `turn_waits_ms`: one value per caller turn, in order: from the turn's start (the caller's
-- silence on a voice call, their line on a typed one) to the moment the caller hears the answer
-- (the seat speaking on a voice call, else the agent's first frame). CHAT.md §18 draws the same.

-- `requests_live` and `requests_replayed`: the call's requests to Claude, from `call_end`. A call
-- with replayed requests (a kept-script draw) waited on the replay cache, not on the model, so
-- its waits are not the caller's (latency.py leaves such rows out for the same reason).

ALTER TABLE synthetic.calls
    ADD COLUMN seat text,
    ADD COLUMN ehr text,
    ADD COLUMN turn_waits_ms double precision[] NOT NULL DEFAULT '{}',
    ADD COLUMN requests_live integer NOT NULL DEFAULT 0,
    ADD COLUMN requests_replayed integer NOT NULL DEFAULT 0;

ALTER TABLE real.calls
    ADD COLUMN seat text,
    ADD COLUMN ehr text,
    ADD COLUMN turn_waits_ms double precision[] NOT NULL DEFAULT '{}',
    ADD COLUMN requests_live integer NOT NULL DEFAULT 0,
    ADD COLUMN requests_replayed integer NOT NULL DEFAULT 0;

-- Every record already filed is filed again at the next `ehr ingest` (npm run dev runs one), so
-- its new columns are filled: an empty hash never matches a record's.
UPDATE synthetic.calls SET record_sha256 = '';
UPDATE real.calls SET record_sha256 = '';
