-- Roles (docs/ideas/ACCESS-ROLES.md §5, §6; the owner's forks, 2026-11-15).
--
-- - `roles`: a settings service like the others (0002_settings.sql): whole versions,
--   insert-only. One key, `roles`: a list of {id, name, domain, levels}. `domain` is `hospital`
--   or `team`; `levels` maps each lever to `none`, `view` or `edit` (access.rs checks it).
-- - `user_roles`: who holds which role. A grant is a row; a revoke stamps `revoked_at` and
--   `revoked_by`. A row is never deleted, so who held what, and when, stays readable.
-- - The Default roles version: the templates of ACCESS-ROLES.md §4, and Deets (the owner's own
--   role, 2026-11-15; Edit on every lever, PHI too since the owner's word of 2026-11-18). The
--   owner's row gets Deets; every other person gets no role (deny by default).
--
-- Nothing here is protected data: ids and role names only.

ALTER TABLE setting_versions DROP CONSTRAINT setting_versions_service_check;
ALTER TABLE setting_versions ADD CONSTRAINT setting_versions_service_check CHECK (service IN ('voice', 'llm', 'prompt', 'system', 'hospital', 'roles'));
ALTER TABLE setting_activations DROP CONSTRAINT setting_activations_service_check;
ALTER TABLE setting_activations ADD CONSTRAINT setting_activations_service_check CHECK (service IN ('voice', 'llm', 'prompt', 'system', 'hospital', 'roles'));

CREATE TABLE settings_roles (
    version_id  BIGINT NOT NULL REFERENCES setting_versions (id),
    key         TEXT NOT NULL,
    value       JSONB NOT NULL,
    PRIMARY KEY (version_id, key)
);

CREATE TRIGGER settings_roles_insert_only BEFORE UPDATE OR DELETE ON settings_roles
    FOR EACH ROW EXECUTE FUNCTION settings_insert_only();
CREATE TRIGGER settings_roles_no_truncate BEFORE TRUNCATE ON settings_roles
    FOR EACH STATEMENT EXECUTE FUNCTION settings_insert_only();

CREATE TABLE user_roles (
    id          BIGSERIAL PRIMARY KEY,
    user_id     uuid NOT NULL REFERENCES users (id),
    role_id     text NOT NULL CHECK (role_id ~ '^[a-z][a-z0-9_]{0,31}$'),
    granted_by  uuid REFERENCES users (id),
    granted_at  timestamptz NOT NULL DEFAULT now(),
    -- NULL with a `revoked_at`: revoked by the system, not a person.
    revoked_by  uuid REFERENCES users (id),
    revoked_at  timestamptz
);

-- One live grant per person and role.
CREATE UNIQUE INDEX user_roles_live ON user_roles (user_id, role_id) WHERE revoked_at IS NULL;

-- A row is never deleted; only a revoke may change one, once.
CREATE FUNCTION user_roles_keep() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE' THEN
        RAISE EXCEPTION 'user_roles rows are never deleted';
    END IF;
    IF OLD.revoked_at IS NOT NULL
       OR NEW.revoked_at IS NULL
       OR (NEW.id, NEW.user_id, NEW.role_id, NEW.granted_by, NEW.granted_at)
          IS DISTINCT FROM (OLD.id, OLD.user_id, OLD.role_id, OLD.granted_by, OLD.granted_at) THEN
        RAISE EXCEPTION 'a user_roles row changes only by its one revoke';
    END IF;
    RETURN NEW;
END
$$;

CREATE TRIGGER user_roles_keep BEFORE UPDATE OR DELETE ON user_roles
    FOR EACH ROW EXECUTE FUNCTION user_roles_keep();
CREATE TRIGGER user_roles_no_truncate BEFORE TRUNCATE ON user_roles
    FOR EACH STATEMENT EXECUTE FUNCTION settings_insert_only();

-- `users.role` waited for roles; `user_roles` replaces it. `owner` now only says which row the
-- launch token speaks for (the owner's PC): it gives no access (ACCESS-ROLES.md §5).
ALTER TABLE users DROP COLUMN role;
COMMENT ON COLUMN users.owner IS 'the row the launch token speaks for; gives no access (roles do)';

-- The Default roles version (seq 0) and its activation.
WITH v AS (
    INSERT INTO setting_versions (service, day, seq, created_by)
    VALUES ('roles', CURRENT_DATE, 0, 'system')
    RETURNING id
), r AS (
    INSERT INTO settings_roles (version_id, key, value)
    SELECT id, 'roles', $json$[
  {"id": "deets", "name": "Deets", "domain": "team", "levels": {"phi": "edit", "auth": "edit", "roles": "edit", "people": "edit", "agent": "edit", "vendors": "edit", "rules": "edit", "testing": "edit", "analytics": "edit", "flags": "edit", "audit_logs": "edit"}},
  {"id": "director", "name": "Director", "domain": "hospital", "levels": {"phi": "view", "auth": "edit", "roles": "edit", "people": "edit", "agent": "edit", "vendors": "edit", "rules": "edit", "testing": "edit", "analytics": "view", "flags": "view", "audit_logs": "view"}},
  {"id": "manager", "name": "Manager", "domain": "hospital", "levels": {"phi": "view", "auth": "view", "roles": "view", "people": "edit", "agent": "edit", "vendors": "view", "rules": "edit", "testing": "edit", "analytics": "view", "flags": "view", "audit_logs": "view"}},
  {"id": "senior_analyst", "name": "Senior Analyst", "domain": "hospital", "levels": {"phi": "view", "auth": "none", "roles": "none", "people": "none", "agent": "edit", "vendors": "view", "rules": "edit", "testing": "edit", "analytics": "view", "flags": "view", "audit_logs": "none"}},
  {"id": "junior_analyst", "name": "Junior Analyst", "domain": "hospital", "levels": {"phi": "none", "auth": "none", "roles": "none", "people": "none", "agent": "view", "vendors": "none", "rules": "view", "testing": "edit", "analytics": "view", "flags": "none", "audit_logs": "none"}},
  {"id": "clinician", "name": "Clinician", "domain": "hospital", "levels": {"phi": "view", "auth": "none", "roles": "none", "people": "none", "agent": "none", "vendors": "none", "rules": "none", "testing": "none", "analytics": "none", "flags": "edit", "audit_logs": "none"}},
  {"id": "compliance", "name": "Compliance", "domain": "hospital", "levels": {"phi": "view", "auth": "view", "roles": "view", "people": "view", "agent": "view", "vendors": "view", "rules": "view", "testing": "none", "analytics": "view", "flags": "view", "audit_logs": "view"}},
  {"id": "engineer", "name": "Engineer", "domain": "team", "levels": {"phi": "none", "auth": "view", "roles": "view", "people": "none", "agent": "view", "vendors": "view", "rules": "view", "testing": "edit", "analytics": "view", "flags": "none", "audit_logs": "view"}},
  {"id": "intern", "name": "Intern", "domain": "team", "levels": {"phi": "none", "auth": "none", "roles": "none", "people": "none", "agent": "view", "vendors": "none", "rules": "view", "testing": "edit", "analytics": "view", "flags": "none", "audit_logs": "none"}}
]$json$::jsonb
    FROM v
    RETURNING version_id
)
INSERT INTO setting_activations (service, version_id, by_who)
SELECT 'roles', version_id, 'system' FROM r;

-- The owner's row holds Deets (the owner, 2026-11-15). Granted by no one: the system.
INSERT INTO user_roles (user_id, role_id)
SELECT id, 'deets' FROM users WHERE owner AND removed_at IS NULL;
