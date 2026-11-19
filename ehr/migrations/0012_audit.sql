-- The staff audit trail (docs/ideas/ACCESS-ROLES.md §5.1; the owner, 2026-11-18: "tracking which
-- user made a change, what change, and what they did with their role logged as well as time").
--
-- One row per thing a person did with their access, or was refused: who (`actor`, a user id),
-- with which roles (`roles`, `roles_version`), the lever and level used, the action, the object
-- (an id only), the names of the fields that changed and the versions before and after, allowed
-- or refused, and when (`at`, the database's clock). audit.rs writes each row in the same
-- transaction as the change it records, and chains it: `hash` = SHA-256 of the row before's hash
-- and this row's fields, so a changed or missing row shows (`audit::verify`).
--
-- One chain for the whole store, never reset: a reset of the synthetic data never touches it.
-- `data_class` marks the rows about a call. No name, email, phone or patient value in any
-- column: `object_id` is a call id, a user id, a role id or a version id; `fields` holds names.

CREATE TABLE staff_audit (
    seq             bigserial PRIMARY KEY,
    at              timestamptz NOT NULL DEFAULT now(),
    -- NULL: no person (the system, or a refused request that named no known person).
    actor           uuid REFERENCES users (id),
    -- `person`, `system` (seeds, migrations, the dev server itself), `unknown` (a request that
    -- named no known person). Later: `idp`, `scim` (STAFF-SIGN-IN.md M4).
    source          text NOT NULL CHECK (source IN ('person', 'system', 'unknown')),
    roles           text[] NOT NULL DEFAULT '{}',
    roles_version   bigint REFERENCES setting_versions (id),
    lever           text,
    level           text CHECK (level IS NULL OR level IN ('none', 'view', 'edit')),
    action          text NOT NULL CHECK (action ~ '^[a-z][a-z0-9_.]{0,63}$'),
    object_kind     text CHECK (object_kind IS NULL OR object_kind ~ '^[a-z][a-z_]{0,31}$'),
    object_id       text CHECK (object_id IS NULL OR length(object_id) <= 64),
    data_class      text CHECK (data_class IS NULL OR data_class IN ('synthetic', 'real')),
    fields          text[] NOT NULL DEFAULT '{}',
    before_version  bigint,
    after_version   bigint,
    allowed         boolean NOT NULL,
    prev_hash       bytea NOT NULL,
    hash            bytea NOT NULL UNIQUE
);

CREATE INDEX staff_audit_actor ON staff_audit (actor, seq DESC);
CREATE INDEX staff_audit_object ON staff_audit (object_kind, object_id, seq DESC);

CREATE FUNCTION staff_audit_insert_only() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'the audit trail is insert-only';
END
$$;

CREATE TRIGGER staff_audit_insert_only BEFORE UPDATE OR DELETE ON staff_audit
    FOR EACH ROW EXECUTE FUNCTION staff_audit_insert_only();
CREATE TRIGGER staff_audit_no_truncate BEFORE TRUNCATE ON staff_audit
    FOR EACH STATEMENT EXECUTE FUNCTION staff_audit_insert_only();

-- Each save of a person's card (§5.1, point 3): the card as it was saved, so a change row has a
-- before and an after. Each value is sealed again under its own place (this table, the row's id,
-- the column; store.rs › PUBLIC_SEALED), so it opens on its own and a key change seals it again.
-- users.rs keeps a card's first version at its first save after this migration. A row is never
-- deleted, and changes only when the store seals its values again (the plain columns stay).
CREATE TABLE user_card_versions (
    id              bigserial PRIMARY KEY,
    user_id         uuid NOT NULL REFERENCES users (id),
    saved_at        timestamptz NOT NULL DEFAULT now(),
    saved_by        uuid REFERENCES users (id),
    email           bytea NOT NULL,
    name            bytea NOT NULL,
    first_name      bytea,
    last_name       bytea,
    phone           bytea,
    phone_verified  boolean NOT NULL
);

CREATE INDEX user_card_versions_user ON user_card_versions (user_id, id DESC);

CREATE FUNCTION user_card_versions_keep() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP <> 'UPDATE'
       OR (NEW.id, NEW.user_id, NEW.saved_at, NEW.saved_by, NEW.phone_verified)
          IS DISTINCT FROM (OLD.id, OLD.user_id, OLD.saved_at, OLD.saved_by, OLD.phone_verified) THEN
        RAISE EXCEPTION 'a card version changes only when its values are sealed again';
    END IF;
    RETURN NEW;
END
$$;

CREATE TRIGGER user_card_versions_keep BEFORE UPDATE OR DELETE ON user_card_versions
    FOR EACH ROW EXECUTE FUNCTION user_card_versions_keep();
CREATE TRIGGER user_card_versions_no_truncate BEFORE TRUNCATE ON user_card_versions
    FOR EACH STATEMENT EXECUTE FUNCTION staff_audit_insert_only();

-- The two empty `audit` tables of 0001 (moved and copied by 0010): nothing ever wrote them.
-- staff_audit replaces both.
DROP TABLE synthetic.audit;
DROP TABLE real.audit;
