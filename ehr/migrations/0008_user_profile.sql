-- A person's card on Teams › People (docs/ops/DEMO.md §3.5; the owner's sketch and forks,
-- 2026-11-14): a first and a last name, a phone number, and a picture.
--
-- `first_name`, `last_name` and `phone` are sealed like `email` and `name` (seal.rs, bound to
-- this table, the row's email_hash and the column). Empty on the rows from before: users.rs
-- splits the old `name` until someone saves the card. `name` stays, kept equal to "first last",
-- for the "added by" line and the audit log. `phone` is E.164 (`+15551234567`).
-- `phone_verified` is false until a text code proves the number (not built; the owner, 2026-11-14:
-- "save, mark unverified"); any change to the number sets it back to false.
ALTER TABLE users
    ADD COLUMN first_name     bytea,
    ADD COLUMN last_name      bytea,
    ADD COLUMN phone          bytea,
    ADD COLUMN phone_verified boolean NOT NULL DEFAULT false,
    ADD COLUMN updated_at     timestamptz;

-- The picture (the owner, 2026-11-14: "sealed in Postgres"). The console crops it to a square
-- WebP of 256 px (about 20 KB) before it is sent. A face is PII: the bytes are sealed, bound to
-- this table, the person's id and the column, so an email change does not touch them.
CREATE TABLE user_photos (
    user_id    uuid PRIMARY KEY REFERENCES users (id),
    photo      bytea NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now()
);
