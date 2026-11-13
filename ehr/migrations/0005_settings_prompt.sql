-- The agent's words (CONSOLE.md §9; the owner, 2026-11-13: the Agent page holds the prompt's
-- sections). The same store as the voice and llm services (0002_settings.sql): whole versions,
-- one key–value table per service, insert-only. Each key is a section of
-- agent/prompts/AGENT-PROMPT.md; each value is its words, as the agent reads them.

ALTER TABLE setting_versions DROP CONSTRAINT setting_versions_service_check;
ALTER TABLE setting_versions ADD CONSTRAINT setting_versions_service_check CHECK (service IN ('voice', 'llm', 'prompt'));
ALTER TABLE setting_activations DROP CONSTRAINT setting_activations_service_check;
ALTER TABLE setting_activations ADD CONSTRAINT setting_activations_service_check CHECK (service IN ('voice', 'llm', 'prompt'));

CREATE TABLE settings_prompt (
    version_id  BIGINT NOT NULL REFERENCES setting_versions (id),
    key         TEXT NOT NULL,
    value       JSONB NOT NULL,
    PRIMARY KEY (version_id, key)
);

CREATE TRIGGER settings_prompt_insert_only BEFORE UPDATE OR DELETE ON settings_prompt
    FOR EACH ROW EXECUTE FUNCTION settings_insert_only();
CREATE TRIGGER settings_prompt_no_truncate BEFORE TRUNCATE ON settings_prompt
    FOR EACH STATEMENT EXECUTE FUNCTION settings_insert_only();

-- Which words each call used (PROMPT-EDITOR.md §6.5): the version's label ("MM-DD | n",
-- "Default"), or "file" when the agent ran without a store. Plain: a label, never a value.
ALTER TABLE calls ADD COLUMN prompt_version TEXT;
