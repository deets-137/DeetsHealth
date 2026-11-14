-- The people who may sign in to the console (docs/ops/DEMO.md §3.5; the owner, 2026-11-14:
-- "a user profile record in our db to eventually store more details to be used for audits").
-- One row per person. The email and the name are sealed (seal.rs, bound to this table, the
-- row's email_hash and the column); `email_hash` is a keyed hash of the email (SealKey::lookup),
-- so a row is found by email without the email in plain text. A removal sets `removed_at`; the
-- row stays, for the audit log to point at. `role` waits for roles (ACCESS-ROLES.md).
CREATE TABLE users (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email_hash    bytea NOT NULL UNIQUE,
    email         bytea NOT NULL,
    name          bytea NOT NULL,
    -- Where the person signs in from: `access` (Cloudflare Access, the demo); `idp` later.
    source        text NOT NULL DEFAULT 'access' CHECK (source IN ('access', 'idp')),
    -- The owner: never removed (a lockout guard).
    owner         boolean NOT NULL DEFAULT false,
    added_by      uuid REFERENCES users (id),
    added_at      timestamptz NOT NULL DEFAULT now(),
    last_seen_at  timestamptz,
    removed_at    timestamptz,
    role          text
);

-- At most one owner.
CREATE UNIQUE INDEX users_one_owner ON users (owner) WHERE owner;
