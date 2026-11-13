-- The Default version (CONSOLE.md §7.3; the owner, 2026-11-13: "The 'Default' can be a
-- version, the first ever in the dropdown for every new implementation, but it's still a
-- version"). It is number 0 of its day: each service has at most one, made by SaveSettings with
-- `seed`, holding the code's defaults. It is active only when it is the first version saved.

ALTER TABLE setting_versions DROP CONSTRAINT setting_versions_seq_check;
ALTER TABLE setting_versions ADD CONSTRAINT setting_versions_seq_check CHECK (seq >= 0);
CREATE UNIQUE INDEX setting_versions_one_default ON setting_versions (service) WHERE seq = 0;
