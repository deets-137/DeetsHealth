//! The people who may sign in to the console (docs/ops/DEMO.md §3.5; migration 0007).
//!
//! The store is the record; the agent keeps Cloudflare's Access group in step with it. Each
//! person is one `users` row: the email and the name sealed, found by `email_hash` (a keyed hash,
//! `SealKey::lookup`). A removal sets `removed_at` and keeps the row for the audit log. The owner
//! is never removed. Every error is a fixed string; none carries an email or a name.

use sqlx::Row;
use zeroize::Zeroizing;

use crate::store::Store;

const TABLE: &str = "users";
/// The lookup hash's column name, so a hash of an email here never equals another column's.
const EMAIL_HASH: &str = "users.email";
pub const MAX_EMAIL: usize = 254;
pub const MAX_NAME: usize = 100;

/// One person, as the People view lists them.
pub struct User {
    pub id: String,
    pub email: Zeroizing<String>,
    pub name: Zeroizing<String>,
    pub owner: bool,
    /// The name of the person who added them; empty for the owner and for people who were in
    /// Cloudflare's group before the list existed.
    pub added_by: Zeroizing<String>,
    /// RFC 3339, UTC.
    pub added_at: String,
    pub last_seen_at: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UsersError {
    /// The request does not fit; the message names the field, never its value.
    Bad(&'static str),
    /// The email is already on the list.
    Exists,
    /// No such person on the list.
    NotFound,
    /// The owner cannot be removed.
    Owner,
    /// The database failed; the message names the step.
    Store(String),
}

/// An email as the list keeps it: trimmed and lower case. Refused unless it has one `@` with
/// text on both sides, no space or control character, and at most `MAX_EMAIL` characters.
pub fn normal_email(raw: &str) -> Result<String, UsersError> {
    let email = raw.trim().to_lowercase();
    let ok = !email.is_empty()
        && email.len() <= MAX_EMAIL
        && email.chars().all(|c| !c.is_whitespace() && !c.is_control())
        && matches!(email.split_once('@'), Some((a, b)) if !a.is_empty() && b.contains('.') && !b.contains('@') && !b.starts_with('.') && !b.ends_with('.'));
    if ok {
        Ok(email)
    } else {
        Err(UsersError::Bad("email: not an email address"))
    }
}

/// A name as the list keeps it: one line, spaces collapsed, 1 to `MAX_NAME` characters.
pub fn normal_name(raw: &str) -> Result<String, UsersError> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(UsersError::Bad("name: 1 to 100 characters on one line"));
    }
    Ok(name)
}

fn store_err(step: &str) -> impl Fn(sqlx::Error) -> UsersError + '_ {
    move |_| UsersError::Store(format!("users: {step}"))
}

fn sealed(store: &Store, row: &str, column: &str, plain: &str) -> Result<Vec<u8>, UsersError> {
    store.seal(TABLE, row, column, plain).map_err(|_| UsersError::Store("users: sealing".into()))
}

fn opened(store: &Store, row: &str, column: &str, bytes: &[u8]) -> Result<Zeroizing<String>, UsersError> {
    store.open(TABLE, row, column, bytes).map_err(|_| UsersError::Store("users: opening".into()))
}

const TIME: &str = r#"'YYYY-MM-DD"T"HH24:MI:SS"Z"'"#;

/// Everyone on the list now (not removed): the owner first, then by when they were added.
pub async fn list(store: &Store) -> Result<Vec<User>, UsersError> {
    let sql = format!(
        "SELECT u.id::text AS id, u.email_hash, u.email, u.name, u.owner,
                a.email_hash AS by_hash, a.name AS by_name,
                to_char(u.added_at AT TIME ZONE 'UTC', {TIME}) AS added_at,
                to_char(u.last_seen_at AT TIME ZONE 'UTC', {TIME}) AS last_seen_at
         FROM users u LEFT JOIN users a ON a.id = u.added_by
         WHERE u.removed_at IS NULL
         ORDER BY u.owner DESC, u.added_at, u.id"
    );
    let rows = sqlx::query(&sql).fetch_all(store.pool()).await.map_err(store_err("list"))?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let row = Store::hex(&r.get::<Vec<u8>, _>("email_hash"));
        let by_name = match (r.get::<Option<Vec<u8>>, _>("by_hash"), r.get::<Option<Vec<u8>>, _>("by_name")) {
            (Some(h), Some(n)) => opened(store, &Store::hex(&h), "name", &n)?,
            _ => Zeroizing::new(String::new()),
        };
        out.push(User {
            id: r.get("id"),
            email: opened(store, &row, "email", &r.get::<Vec<u8>, _>("email"))?,
            name: opened(store, &row, "name", &r.get::<Vec<u8>, _>("name"))?,
            owner: r.get("owner"),
            added_by: by_name,
            added_at: r.get("added_at"),
            last_seen_at: r.get("last_seen_at"),
        });
    }
    Ok(out)
}

/// The id of the person whose email this is, if they are on the list now.
async fn active_id(store: &Store, email: &str) -> Result<Option<String>, UsersError> {
    let hash = store.lookup(EMAIL_HASH, email);
    let row = sqlx::query("SELECT id::text AS id FROM users WHERE email_hash = $1 AND removed_at IS NULL")
        .bind(hash.as_slice())
        .fetch_optional(store.pool())
        .await
        .map_err(store_err("find"))?;
    Ok(row.map(|r| r.get("id")))
}

/// The adder's own row: the owner when `by` is empty (the console on the owner's PC), else the
/// person with that email. None when they are not on the list.
async fn adder(store: &Store, by: &str) -> Result<Option<String>, UsersError> {
    if by.trim().is_empty() {
        let row = sqlx::query("SELECT id::text AS id FROM users WHERE owner AND removed_at IS NULL")
            .fetch_optional(store.pool())
            .await
            .map_err(store_err("find the owner"))?;
        return Ok(row.map(|r| r.get("id")));
    }
    active_id(store, &normal_email(by)?).await
}

/// Puts a person on the list. Someone removed before comes back on the same row, with the new
/// name and adder. `by`: the adder's email, or empty for the owner's PC.
pub async fn add(store: &Store, email: &str, name: &str, by: &str) -> Result<String, UsersError> {
    let email = normal_email(email)?;
    let name = normal_name(name)?;
    if active_id(store, &email).await?.is_some() {
        return Err(UsersError::Exists);
    }
    let added_by = adder(store, by).await?;
    let hash = store.lookup(EMAIL_HASH, &email);
    let row = Store::hex(&hash);
    let id: String = sqlx::query(
        "INSERT INTO users (email_hash, email, name, added_by) VALUES ($1, $2, $3, $4::uuid)
         ON CONFLICT (email_hash) DO UPDATE SET name = EXCLUDED.name, added_by = EXCLUDED.added_by,
             added_at = now(), removed_at = NULL, last_seen_at = NULL
         RETURNING id::text AS id",
    )
    .bind(hash.as_slice())
    .bind(sealed(store, &row, "email", &email)?)
    .bind(sealed(store, &row, "name", &name)?)
    .bind(added_by)
    .fetch_one(store.pool())
    .await
    .map_err(store_err("add"))?
    .get("id");
    Ok(id)
}

/// Takes a person off the list and returns their email, so the agent can take it out of
/// Cloudflare's group and end their sessions. The owner is refused.
pub async fn remove(store: &Store, id: &str) -> Result<Zeroizing<String>, UsersError> {
    if uuid_text(id).is_none() {
        return Err(UsersError::Bad("id: not a person's id"));
    }
    let row = sqlx::query("SELECT email_hash, email, owner FROM users WHERE id = $1::uuid AND removed_at IS NULL")
        .bind(id)
        .fetch_optional(store.pool())
        .await
        .map_err(store_err("find"))?
        .ok_or(UsersError::NotFound)?;
    if row.get::<bool, _>("owner") {
        return Err(UsersError::Owner);
    }
    let email = opened(store, &Store::hex(&row.get::<Vec<u8>, _>("email_hash")), "email", &row.get::<Vec<u8>, _>("email"))?;
    sqlx::query("UPDATE users SET removed_at = now() WHERE id = $1::uuid AND removed_at IS NULL")
        .bind(id)
        .execute(store.pool())
        .await
        .map_err(store_err("remove"))?;
    Ok(email)
}

/// Whether this email is on the list now; when it is, its `last_seen_at` becomes now.
pub async fn admit(store: &Store, email: &str) -> Result<bool, UsersError> {
    let Ok(email) = normal_email(email) else { return Ok(false) };
    let hash = store.lookup(EMAIL_HASH, &email);
    let done = sqlx::query("UPDATE users SET last_seen_at = now() WHERE email_hash = $1 AND removed_at IS NULL")
        .bind(hash.as_slice())
        .execute(store.pool())
        .await
        .map_err(store_err("admit"))?;
    Ok(done.rows_affected() == 1)
}

/// Brings in the emails already in Cloudflare's group that the list does not have (people added
/// in Cloudflare's dashboard), with their email as their name until someone renames them. On an
/// empty list with exactly one email, that person is the owner: the group was made with the
/// owner's email alone (DEMO.md §7). Returns how many were added. A removed person is not
/// brought back: the agent takes them out of the group instead.
pub async fn seed(store: &Store, emails: &[String]) -> Result<u32, UsersError> {
    let empty: bool = sqlx::query("SELECT NOT EXISTS (SELECT 1 FROM users) AS empty")
        .fetch_one(store.pool())
        .await
        .map_err(store_err("count"))?
        .get("empty");
    let mut normal: Vec<String> = emails.iter().filter_map(|e| normal_email(e).ok()).collect();
    normal.sort();
    normal.dedup();
    let owner = empty && normal.len() == 1;
    let mut added = 0;
    for email in &normal {
        let hash = store.lookup(EMAIL_HASH, email);
        let row = Store::hex(&hash);
        let done = sqlx::query(
            "INSERT INTO users (email_hash, email, name, owner) VALUES ($1, $2, $3, $4)
             ON CONFLICT (email_hash) DO NOTHING",
        )
        .bind(hash.as_slice())
        .bind(sealed(store, &row, "email", email)?)
        .bind(sealed(store, &row, "name", email)?)
        .bind(owner)
        .execute(store.pool())
        .await
        .map_err(store_err("seed"))?;
        added += done.rows_affected() as u32;
    }
    Ok(added)
}

/// A uuid in its text form, or None.
fn uuid_text(s: &str) -> Option<&str> {
    let ok = s.len() == 36
        && s.char_indices().all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { c == '-' } else { c.is_ascii_hexdigit() });
    ok.then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_are_trimmed_lowered_and_checked() {
        assert_eq!(normal_email("  Priya@Example.COM ").unwrap(), "priya@example.com");
        for bad in ["", "no-at", "@example.com", "a@", "a@b", "a b@example.com", "a@@b.com", "a@.com", "a@b.com."] {
            assert!(normal_email(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn names_are_one_line() {
        assert_eq!(normal_name("  Priya \n Shah ").unwrap(), "Priya Shah");
        assert!(normal_name("   ").is_err());
        assert!(normal_name(&"x".repeat(101)).is_err());
    }

    #[test]
    fn ids_are_uuids() {
        assert!(uuid_text("0b5f3c2e-8f1a-4c7d-9e2b-1a2b3c4d5e6f").is_some());
        assert!(uuid_text("0b5f3c2e8f1a4c7d9e2b1a2b3c4d5e6f").is_none());
        assert!(uuid_text("'; DROP TABLE users; --").is_none());
    }
}
