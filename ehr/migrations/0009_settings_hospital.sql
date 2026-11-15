-- The hospital's lists and numbers: Config › Hospital (HOSPITAL-CONFIG.md; the owner, 2026-11-15:
-- every section now, one hospital for every EHR source). The same store as the other services
-- (0002_settings.sql): whole versions, one key-value table per service, insert-only. One key per
-- list or number (ehr/src/hospital.rs › KEYS). A value is a fact about the hospital, never
-- protected data.

ALTER TABLE setting_versions DROP CONSTRAINT setting_versions_service_check;
ALTER TABLE setting_versions ADD CONSTRAINT setting_versions_service_check CHECK (service IN ('voice', 'llm', 'prompt', 'system', 'hospital'));
ALTER TABLE setting_activations DROP CONSTRAINT setting_activations_service_check;
ALTER TABLE setting_activations ADD CONSTRAINT setting_activations_service_check CHECK (service IN ('voice', 'llm', 'prompt', 'system', 'hospital'));

CREATE TABLE settings_hospital (
    version_id  BIGINT NOT NULL REFERENCES setting_versions (id),
    key         TEXT NOT NULL,
    value       JSONB NOT NULL,
    PRIMARY KEY (version_id, key)
);

CREATE TRIGGER settings_hospital_insert_only BEFORE UPDATE OR DELETE ON settings_hospital
    FOR EACH ROW EXECUTE FUNCTION settings_insert_only();
CREATE TRIGGER settings_hospital_no_truncate BEFORE TRUNCATE ON settings_hospital
    FOR EACH STATEMENT EXECUTE FUNCTION settings_insert_only();

-- Which hospital version each call used: the version's label, or "file". Plain: a label.
ALTER TABLE calls ADD COLUMN hospital_version TEXT;
