-- The System variables (SYSTEM-PAGE.md; the owner, 2026-11-13: analysts see and configure the
-- variables the agent's words use, and add their own). The same store as the other services
-- (0002_settings.sql): whole versions, one key-value table per service, insert-only. One key,
-- `variables`: a list of {name, meaning, value}. A value is the clinic's own text, never
-- protected data.

ALTER TABLE setting_versions DROP CONSTRAINT setting_versions_service_check;
ALTER TABLE setting_versions ADD CONSTRAINT setting_versions_service_check CHECK (service IN ('voice', 'llm', 'prompt', 'system'));
ALTER TABLE setting_activations DROP CONSTRAINT setting_activations_service_check;
ALTER TABLE setting_activations ADD CONSTRAINT setting_activations_service_check CHECK (service IN ('voice', 'llm', 'prompt', 'system'));

CREATE TABLE settings_system (
    version_id  BIGINT NOT NULL REFERENCES setting_versions (id),
    key         TEXT NOT NULL,
    value       JSONB NOT NULL,
    PRIMARY KEY (version_id, key)
);

CREATE TRIGGER settings_system_insert_only BEFORE UPDATE OR DELETE ON settings_system
    FOR EACH ROW EXECUTE FUNCTION settings_insert_only();
CREATE TRIGGER settings_system_no_truncate BEFORE TRUNCATE ON settings_system
    FOR EACH STATEMENT EXECUTE FUNCTION settings_insert_only();

-- Which System variables each call used: the version's label, or "file". Plain: a label.
ALTER TABLE calls ADD COLUMN system_version TEXT;
