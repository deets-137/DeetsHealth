//! The people who may sign in to the console (docs/ops/DEMO.md §3.5; migration 0007).
//!
//! The store is the record; the agent keeps Cloudflare's Access group in step with it. Each
//! person is one `users` row: the email and the name sealed, found by `email_hash` (a keyed hash,
//! `SealKey::lookup`). A removal sets `removed_at` and keeps the row for the audit log. The owner
//! is never removed. Every error is a fixed string; none carries an email or a name.
//!
//! A person's card (migration 0008; the owner's forks, 2026-11-14): a first and a last name, a
//! phone number (E.164, "not verified" until a text code proves it), and a picture in
//! `user_photos`. Anyone on the list may edit any card, the owner's too, until roles exist. An
//! email change moves the row to the new email's hash, so every sealed column is sealed again.

use sqlx::Row;
use zeroize::Zeroizing;

use crate::store::Store;

const TABLE: &str = "users";
/// The lookup hash's column name, so a hash of an email here never equals another column's.
const EMAIL_HASH: &str = "users.email";
pub const MAX_EMAIL: usize = 254;
pub const MAX_NAME: usize = 100;
/// A first or a last name: two of them and the space between fit `MAX_NAME`.
pub const MAX_NAME_PART: usize = 49;
/// The largest picture the store takes. The console sends a 256 px WebP, about 20 KB.
pub const MAX_PHOTO: usize = 256 * 1024;

/// One person, as the People view lists them.
pub struct User {
    pub id: String,
    pub email: Zeroizing<String>,
    pub name: Zeroizing<String>,
    /// From the card; for a row saved before the card existed, the old `name` split at its
    /// last space (empty when the name is still the email).
    pub first_name: Zeroizing<String>,
    pub last_name: Zeroizing<String>,
    /// E.164, or empty.
    pub phone: Zeroizing<String>,
    pub phone_verified: bool,
    /// Where they sign in from: `access` or `idp`.
    pub source: String,
    /// When the picture was last saved (RFC 3339, UTC); None when there is no picture.
    pub photo_at: Option<String>,
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
    /// A removed person's row holds that email: add them back instead of moving it.
    Taken,
    /// The database failed; the message names the step.
    Store(String),
}

/// What a card's Save sends. Every field is the whole new value.
pub struct Profile<'a> {
    pub first_name: &'a str,
    pub last_name: &'a str,
    /// Any common way to write it; empty clears it.
    pub phone: &'a str,
    pub email: &'a str,
}

/// What a Save changed, for the agent: when the email moved, the old one leaves Cloudflare's
/// group and its Access sessions end.
pub struct Saved {
    pub old_email: Zeroizing<String>,
    pub email_changed: bool,
}

/// A first or a last name: one line, spaces collapsed, at most `MAX_NAME_PART` characters; may
/// be empty (the card needs one of the two).
pub fn normal_name_part(raw: &str, field: &'static str) -> Result<String, UsersError> {
    let part = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if part.chars().count() > MAX_NAME_PART || part.chars().any(char::is_control) {
        return Err(UsersError::Bad(field));
    }
    Ok(part)
}

/// A phone number as the list keeps it: E.164. Spaces, dots, dashes and brackets are dropped.
/// Ten digits are a US number (+1); eleven starting with 1 too; a leading `+` takes 8 to 15
/// digits as they are. Empty stays empty (no number).
pub fn normal_phone(raw: &str) -> Result<String, UsersError> {
    const WHY: &str = "phone: a US number of 10 digits, or + and the country code";
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(String::new());
    }
    let plus = raw.starts_with('+');
    let rest = if plus { &raw[1..] } else { raw };
    if !rest.chars().all(|c| c.is_ascii_digit() || matches!(c, ' ' | '-' | '.' | '(' | ')')) {
        return Err(UsersError::Bad(WHY));
    }
    let digits: String = rest.chars().filter(char::is_ascii_digit).collect();
    let e164 = match (plus, digits.len()) {
        (true, 8..=15) if !digits.starts_with('0') => format!("+{digits}"),
        (false, 10) if !digits.starts_with(['0', '1']) => format!("+1{digits}"),
        (false, 11) if digits.starts_with('1') && !digits[1..].starts_with(['0', '1']) => format!("+{digits}"),
        _ => return Err(UsersError::Bad(WHY)),
    };
    Ok(e164)
}

/// The full name from its two parts.
fn full_name(first: &str, last: &str) -> String {
    [first, last].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ")
}

/// A row from before the card: its `name` split at the last space. A name that is still the
/// email (brought in from Cloudflare's group) is no name yet.
fn split_name(name: &str, email: &str) -> (String, String) {
    if name.is_empty() || name == email {
        return (String::new(), String::new());
    }
    match name.rsplit_once(' ') {
        Some((first, last)) => (first.to_string(), last.to_string()),
        None => (name.to_string(), String::new()),
    }
}

/// A picture as the store takes it: a WebP (`RIFF`, its size, `WEBP`) of at most `MAX_PHOTO`.
fn check_photo(bytes: &[u8]) -> Result<(), UsersError> {
    if bytes.len() > MAX_PHOTO {
        return Err(UsersError::Bad("photo: at most 256 KB"));
    }
    if bytes.len() < 16 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return Err(UsersError::Bad("photo: a WebP image"));
    }
    Ok(())
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
                u.first_name, u.last_name, u.phone, u.phone_verified, u.source,
                a.email_hash AS by_hash, a.name AS by_name,
                to_char(u.added_at AT TIME ZONE 'UTC', {TIME}) AS added_at,
                to_char(u.last_seen_at AT TIME ZONE 'UTC', {TIME}) AS last_seen_at,
                to_char(p.updated_at AT TIME ZONE 'UTC', {TIME}) AS photo_at
         FROM users u LEFT JOIN users a ON a.id = u.added_by
              LEFT JOIN user_photos p ON p.user_id = u.id
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
        let email = opened(store, &row, "email", &r.get::<Vec<u8>, _>("email"))?;
        let name = opened(store, &row, "name", &r.get::<Vec<u8>, _>("name"))?;
        let column = |c: &str| -> Result<Option<Zeroizing<String>>, UsersError> {
            r.get::<Option<Vec<u8>>, _>(c).map(|b| opened(store, &row, c, &b)).transpose()
        };
        let (first_name, last_name) = match (column("first_name")?, column("last_name")?) {
            (Some(f), Some(l)) => (f, l),
            _ => {
                let (f, l) = split_name(&name, &email);
                (Zeroizing::new(f), Zeroizing::new(l))
            }
        };
        out.push(User {
            id: r.get("id"),
            first_name,
            last_name,
            phone: column("phone")?.unwrap_or_default(),
            phone_verified: r.get("phone_verified"),
            source: r.get("source"),
            photo_at: r.get("photo_at"),
            email,
            name,
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
             added_at = now(), removed_at = NULL, last_seen_at = NULL,
             first_name = NULL, last_name = NULL
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

/// Saves a person's card: the whole profile at once. A changed phone number is not verified.
/// A changed email moves the row to the new email's hash and seals every column again under
/// it; the old email comes back so the agent can take it out of Cloudflare's group.
pub async fn update(store: &Store, id: &str, p: &Profile<'_>) -> Result<Saved, UsersError> {
    if uuid_text(id).is_none() {
        return Err(UsersError::Bad("id: not a person's id"));
    }
    let first = normal_name_part(p.first_name, "first_name: at most 49 characters on one line")?;
    let last = normal_name_part(p.last_name, "last_name: at most 49 characters on one line")?;
    let name = full_name(&first, &last);
    if name.is_empty() {
        return Err(UsersError::Bad("first_name: a first or a last name"));
    }
    let phone = normal_phone(p.phone)?;
    let email = normal_email(p.email)?;

    let row = sqlx::query("SELECT email_hash, email, phone FROM users WHERE id = $1::uuid AND removed_at IS NULL")
        .bind(id)
        .fetch_optional(store.pool())
        .await
        .map_err(store_err("find"))?
        .ok_or(UsersError::NotFound)?;
    let old_hex = Store::hex(&row.get::<Vec<u8>, _>("email_hash"));
    let old_email = opened(store, &old_hex, "email", &row.get::<Vec<u8>, _>("email"))?;
    let old_phone = row
        .get::<Option<Vec<u8>>, _>("phone")
        .map(|b| opened(store, &old_hex, "phone", &b))
        .transpose()?
        .unwrap_or_default();

    let hash = store.lookup(EMAIL_HASH, &email);
    let email_changed = email != old_email.as_str();
    if email_changed {
        let holder = sqlx::query("SELECT removed_at IS NULL AS active FROM users WHERE email_hash = $1")
            .bind(hash.as_slice())
            .fetch_optional(store.pool())
            .await
            .map_err(store_err("find"))?;
        match holder.map(|r| r.get::<bool, _>("active")) {
            Some(true) => return Err(UsersError::Exists),
            Some(false) => return Err(UsersError::Taken),
            None => {}
        }
    }
    let hex = Store::hex(&hash);
    let phone_sealed = if phone.is_empty() { None } else { Some(sealed(store, &hex, "phone", &phone)?) };
    // A number that did not change keeps its mark; any other loses it.
    let keep_verified = phone == old_phone.as_str() && !phone.is_empty();
    sqlx::query(
        "UPDATE users SET email_hash = $2, email = $3, name = $4, first_name = $5, last_name = $6,
             phone = $7, phone_verified = phone_verified AND $8, updated_at = now()
         WHERE id = $1::uuid AND removed_at IS NULL",
    )
    .bind(id)
    .bind(hash.as_slice())
    .bind(sealed(store, &hex, "email", &email)?)
    .bind(sealed(store, &hex, "name", &name)?)
    .bind(sealed(store, &hex, "first_name", &first)?)
    .bind(sealed(store, &hex, "last_name", &last)?)
    .bind(phone_sealed)
    .bind(keep_verified)
    .execute(store.pool())
    .await
    .map_err(|e| match e {
        // Another save took the email between the check and this write.
        sqlx::Error::Database(d) if d.is_unique_violation() => UsersError::Exists,
        _ => UsersError::Store("users: save".into()),
    })?;
    Ok(Saved { old_email, email_changed })
}

/// Whether a person is on the list now.
async fn is_active(store: &Store, id: &str) -> Result<bool, UsersError> {
    if uuid_text(id).is_none() {
        return Err(UsersError::Bad("id: not a person's id"));
    }
    let row = sqlx::query("SELECT 1 AS one FROM users WHERE id = $1::uuid AND removed_at IS NULL")
        .bind(id)
        .fetch_optional(store.pool())
        .await
        .map_err(store_err("find"))?;
    Ok(row.is_some())
}

/// Saves a person's picture (a WebP of at most `MAX_PHOTO`), sealed and bound to their id.
pub async fn set_photo(store: &Store, id: &str, bytes: &[u8]) -> Result<(), UsersError> {
    check_photo(bytes)?;
    if !is_active(store, id).await? {
        return Err(UsersError::NotFound);
    }
    let sealed = store
        .seal_bytes("user_photos", id, "photo", bytes)
        .map_err(|_| UsersError::Store("users: sealing".into()))?;
    sqlx::query(
        "INSERT INTO user_photos (user_id, photo) VALUES ($1::uuid, $2)
         ON CONFLICT (user_id) DO UPDATE SET photo = EXCLUDED.photo, updated_at = now()",
    )
    .bind(id)
    .bind(sealed)
    .execute(store.pool())
    .await
    .map_err(store_err("save the picture"))?;
    Ok(())
}

/// A person's picture, or None when they have none.
pub async fn photo(store: &Store, id: &str) -> Result<Option<Zeroizing<Vec<u8>>>, UsersError> {
    if !is_active(store, id).await? {
        return Err(UsersError::NotFound);
    }
    let row = sqlx::query("SELECT photo FROM user_photos WHERE user_id = $1::uuid")
        .bind(id)
        .fetch_optional(store.pool())
        .await
        .map_err(store_err("read the picture"))?;
    row.map(|r| {
        store
            .open_bytes("user_photos", id, "photo", &r.get::<Vec<u8>, _>("photo"))
            .map_err(|_| UsersError::Store("users: opening".into()))
    })
    .transpose()
}

/// Deletes a person's picture. No picture is no error.
pub async fn clear_photo(store: &Store, id: &str) -> Result<(), UsersError> {
    if !is_active(store, id).await? {
        return Err(UsersError::NotFound);
    }
    sqlx::query("DELETE FROM user_photos WHERE user_id = $1::uuid")
        .bind(id)
        .execute(store.pool())
        .await
        .map_err(store_err("delete the picture"))?;
    Ok(())
}

/// The owner was seen (the console on the owner's PC, through the launch token).
pub async fn seen_owner(store: &Store) -> Result<(), UsersError> {
    sqlx::query("UPDATE users SET last_seen_at = now() WHERE owner AND removed_at IS NULL")
        .execute(store.pool())
        .await
        .map_err(store_err("stamp the owner"))?;
    Ok(())
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
    fn phones_become_e164() {
        assert_eq!(normal_phone("(555) 234-5678").unwrap(), "+15552345678");
        assert_eq!(normal_phone("1 555.234.5678").unwrap(), "+15552345678");
        assert_eq!(normal_phone("+44 20 7946 0958").unwrap(), "+442079460958");
        assert_eq!(normal_phone("  ").unwrap(), "");
        for bad in ["555-1234", "055-234-5678", "+0123456789", "555-234-5678 x2", "+1234567", "abc"] {
            assert!(normal_phone(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn old_names_split_at_the_last_space() {
        assert_eq!(split_name("Mary Ann Lee", "m@x.com"), ("Mary Ann".into(), "Lee".into()));
        assert_eq!(split_name("Cher", "c@x.com"), ("Cher".into(), String::new()));
        assert_eq!(split_name("o@x.com", "o@x.com"), (String::new(), String::new()));
        assert_eq!(full_name("", "Lee"), "Lee");
        assert!(normal_name_part(&"x".repeat(50), "first_name").is_err());
    }

    #[test]
    fn photos_are_small_webps() {
        let mut webp = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
        assert!(check_photo(&webp).is_ok());
        assert!(check_photo(b"\x89PNG\r\n\x1a\n0000000000").is_err());
        webp.resize(MAX_PHOTO + 1, 0);
        assert!(check_photo(&webp).is_err());
    }

    #[test]
    fn ids_are_uuids() {
        assert!(uuid_text("0b5f3c2e-8f1a-4c7d-9e2b-1a2b3c4d5e6f").is_some());
        assert!(uuid_text("0b5f3c2e8f1a4c7d9e2b1a2b3c4d5e6f").is_none());
        assert!(uuid_text("'; DROP TABLE users; --").is_none());
    }
}
