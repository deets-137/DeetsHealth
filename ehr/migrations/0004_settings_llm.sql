-- The language model's settings (CONSOLE.md §8; the owner, 2026-11-13: "a new config subpage
-- for Anthropic", the model chosen on the page). The same store as the voice service
-- (0002_settings.sql): whole versions, one key–value table per service, insert-only.

ALTER TABLE setting_versions DROP CONSTRAINT setting_versions_service_check;
ALTER TABLE setting_versions ADD CONSTRAINT setting_versions_service_check CHECK (service IN ('voice', 'llm'));
ALTER TABLE setting_activations DROP CONSTRAINT setting_activations_service_check;
ALTER TABLE setting_activations ADD CONSTRAINT setting_activations_service_check CHECK (service IN ('voice', 'llm'));

-- The llm service: Anthropic's model, effort, thinking, the reply's token caps, the prompt
-- cache's life, and the budget.
CREATE TABLE settings_llm (
    version_id  BIGINT NOT NULL REFERENCES setting_versions (id),
    key         TEXT NOT NULL,
    value       JSONB NOT NULL,
    PRIMARY KEY (version_id, key)
);

CREATE TRIGGER settings_llm_insert_only BEFORE UPDATE OR DELETE ON settings_llm
    FOR EACH ROW EXECUTE FUNCTION settings_insert_only();
CREATE TRIGGER settings_llm_no_truncate BEFORE TRUNCATE ON settings_llm
    FOR EACH STATEMENT EXECUTE FUNCTION settings_insert_only();
