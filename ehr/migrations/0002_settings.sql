-- The settings store (CONSOLE.md §7.3; the owner, 2026-11-13: key–value, "A per service", and
-- a record of exactly which setting was live and when).
--
-- - setting_versions: one row per saved version of one service's settings. Its label is
--   "MM-DD | seq": the day it was saved and its number that day.
-- - settings_<service>: one key–value table per service; every version holds every setting,
--   one row each, so a version is the whole set the agent ran with.
-- - setting_activations: one row each time a version became the active one, with when and who.
--   The active version is the newest row. Which setting was live at a moment is the version of
--   the last activation before it.
--
-- All three are insert-only: a trigger refuses every UPDATE, DELETE and TRUNCATE. Nothing here
-- is protected data: plain columns only.

CREATE TABLE setting_versions (
    id          BIGSERIAL PRIMARY KEY,
    service     TEXT NOT NULL CHECK (service IN ('voice')),
    day         DATE NOT NULL,
    seq         INTEGER NOT NULL CHECK (seq > 0),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by  TEXT NOT NULL,
    UNIQUE (service, day, seq)
);

-- The voice service: Cartesia's Ink and Sonic, the test-call guard, the plan's credits.
CREATE TABLE settings_voice (
    version_id  BIGINT NOT NULL REFERENCES setting_versions (id),
    key         TEXT NOT NULL,
    value       JSONB NOT NULL,
    PRIMARY KEY (version_id, key)
);

CREATE TABLE setting_activations (
    id          BIGSERIAL PRIMARY KEY,
    service     TEXT NOT NULL CHECK (service IN ('voice')),
    version_id  BIGINT NOT NULL REFERENCES setting_versions (id),
    at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    by_who      TEXT NOT NULL
);

CREATE INDEX setting_activations_latest ON setting_activations (service, id DESC);

CREATE FUNCTION settings_insert_only() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'the settings history is insert-only';
END
$$;

CREATE TRIGGER setting_versions_insert_only BEFORE UPDATE OR DELETE ON setting_versions
    FOR EACH ROW EXECUTE FUNCTION settings_insert_only();
CREATE TRIGGER setting_versions_no_truncate BEFORE TRUNCATE ON setting_versions
    FOR EACH STATEMENT EXECUTE FUNCTION settings_insert_only();
CREATE TRIGGER settings_voice_insert_only BEFORE UPDATE OR DELETE ON settings_voice
    FOR EACH ROW EXECUTE FUNCTION settings_insert_only();
CREATE TRIGGER settings_voice_no_truncate BEFORE TRUNCATE ON settings_voice
    FOR EACH STATEMENT EXECUTE FUNCTION settings_insert_only();
CREATE TRIGGER setting_activations_insert_only BEFORE UPDATE OR DELETE ON setting_activations
    FOR EACH ROW EXECUTE FUNCTION settings_insert_only();
CREATE TRIGGER setting_activations_no_truncate BEFORE TRUNCATE ON setting_activations
    FOR EACH STATEMENT EXECUTE FUNCTION settings_insert_only();
