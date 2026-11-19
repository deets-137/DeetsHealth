//! The staff audit trail (docs/architecture/ACCESS-ROLES.md §5.1; migration 0012).
//!
//! Each row records who did what with their access, or was refused: the person, their roles and
//! the roles version, the lever and level, the action, the object (an id only), the names of the
//! fields that changed with the versions before and after, and when. A row is written in the same
//! transaction as the change it records (`write` takes the transaction), so neither is ever
//! stored without the other.
//!
//! **The chain:** each row's `hash` is SHA-256 over the row before's hash and this row's fields
//! (`digest`), so a changed, removed or inserted row breaks it, and `verify` finds the first row
//! that does not match. Rows are chained under one lock, in the order they commit.
//!
//! Never a name, an email, a phone number or a patient value: ids and field names only.

use aws_lc_rs::digest;
use sqlx::{PgConnection, PgPool, Row};

use crate::access::{Actor, Level, Lever};

/// Who a row is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A person on the list (`actor` is set).
    Person,
    /// ehr or the agent on its own: a seed, a migration, the prompt file synced.
    System,
    /// A request that named someone not on the list.
    Unknown,
}

impl Source {
    pub fn word(self) -> &'static str {
        match self {
            Source::Person => "person",
            Source::System => "system",
            Source::Unknown => "unknown",
        }
    }
}

/// One row to write.
#[derive(Clone, Debug, Default)]
pub struct Entry {
    /// The person: their id, roles and roles version. None: `Source::System` or `Unknown`.
    pub actor: Option<Actor>,
    pub unknown: bool,
    pub lever: Option<Lever>,
    pub level: Option<Level>,
    /// What was done: a lower-case word with dots (`settings.save`, `role.grant`, `page.open`).
    pub action: String,
    pub object_kind: Option<&'static str>,
    pub object_id: Option<String>,
    /// `synthetic` or `real`, for a row about a call.
    pub data_class: Option<&'static str>,
    /// The names of the fields that changed. Names only.
    pub fields: Vec<String>,
    pub before_version: Option<i64>,
    pub after_version: Option<i64>,
    pub allowed: bool,
}

impl Entry {
    pub fn by(actor: Option<&Actor>, action: &str) -> Entry {
        Entry { actor: actor.cloned(), action: action.to_string(), allowed: true, ..Entry::default() }
    }

    pub fn source(&self) -> Source {
        match (&self.actor, self.unknown) {
            (Some(_), _) => Source::Person,
            (None, true) => Source::Unknown,
            (None, false) => Source::System,
        }
    }

    pub fn lever(mut self, lever: Lever, level: Level) -> Entry {
        self.lever = Some(lever);
        self.level = Some(level);
        self
    }

    pub fn object(mut self, kind: &'static str, id: impl Into<String>) -> Entry {
        self.object_kind = Some(kind);
        self.object_id = Some(id.into());
        self
    }

    pub fn change(mut self, fields: Vec<String>, before: Option<i64>, after: Option<i64>) -> Entry {
        self.fields = fields;
        self.before_version = before;
        self.after_version = after;
        self
    }

    pub fn refused(mut self) -> Entry {
        self.allowed = false;
        self
    }
}

/// The chain's start: the `prev_hash` of the first row.
pub const GENESIS: [u8; 32] = [0u8; 32];

/// One row as stored, for `read` and `verify`.
#[derive(Clone, Debug, PartialEq)]
pub struct Stored {
    pub seq: i64,
    /// RFC 3339, UTC, to the microsecond.
    pub at: String,
    pub actor: Option<String>,
    pub source: String,
    pub roles: Vec<String>,
    pub roles_version: Option<i64>,
    pub lever: Option<String>,
    pub level: Option<String>,
    pub action: String,
    pub object_kind: Option<String>,
    pub object_id: Option<String>,
    pub data_class: Option<String>,
    pub fields: Vec<String>,
    pub before_version: Option<i64>,
    pub after_version: Option<i64>,
    pub allowed: bool,
    pub prev_hash: Vec<u8>,
    pub hash: Vec<u8>,
}

/// Appends one field: its length (4 bytes, big end first) and its bytes; a missing one is
/// `u32::MAX` alone. So no two rows encode the same.
fn put(buf: &mut Vec<u8>, field: Option<&[u8]>) {
    match field {
        Some(b) => {
            buf.extend_from_slice(&u32::try_from(b.len()).unwrap_or(u32::MAX - 1).to_be_bytes());
            buf.extend_from_slice(b);
        }
        None => buf.extend_from_slice(&u32::MAX.to_be_bytes()),
    }
}

fn put_list(buf: &mut Vec<u8>, list: &[String]) {
    buf.extend_from_slice(&u32::try_from(list.len()).unwrap_or(u32::MAX).to_be_bytes());
    for item in list {
        put(buf, Some(item.as_bytes()));
    }
}

/// The row's hash: SHA-256 over the row before's hash and every field but `seq` (a sequence may
/// skip numbers; the chain is by hash).
pub fn digest(prev: &[u8], r: &Stored) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);
    buf.extend_from_slice(b"deets.staff_audit.v1");
    put(&mut buf, Some(prev));
    put(&mut buf, Some(r.at.as_bytes()));
    put(&mut buf, r.actor.as_deref().map(str::as_bytes));
    put(&mut buf, Some(r.source.as_bytes()));
    put_list(&mut buf, &r.roles);
    put(&mut buf, r.roles_version.map(|v| v.to_string()).as_deref().map(str::as_bytes));
    put(&mut buf, r.lever.as_deref().map(str::as_bytes));
    put(&mut buf, r.level.as_deref().map(str::as_bytes));
    put(&mut buf, Some(r.action.as_bytes()));
    put(&mut buf, r.object_kind.as_deref().map(str::as_bytes));
    put(&mut buf, r.object_id.as_deref().map(str::as_bytes));
    put(&mut buf, r.data_class.as_deref().map(str::as_bytes));
    put_list(&mut buf, &r.fields);
    put(&mut buf, r.before_version.map(|v| v.to_string()).as_deref().map(str::as_bytes));
    put(&mut buf, r.after_version.map(|v| v.to_string()).as_deref().map(str::as_bytes));
    put(&mut buf, Some(if r.allowed { b"1" } else { b"0" }));
    digest::digest(&digest::SHA256, &buf).as_ref().to_vec()
}

/// `at` as text: UTC to the microsecond, the form the hash covers.
macro_rules! at_text {
    ($column:literal) => {
        concat!("to_char(", $column, r#" AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.US"Z"')"#)
    };
}

fn db(step: &str) -> impl Fn(sqlx::Error) -> String + '_ {
    move |_| format!("audit: {step}")
}

/// Writes one row in `tx`'s transaction and returns its `seq`. Rows are chained under a lock
/// held to the end of the transaction, so two writers never take the same row before.
pub async fn write(tx: &mut PgConnection, e: &Entry) -> Result<i64, String> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('deets.staff_audit'))")
        .execute(&mut *tx)
        .await
        .map_err(db("lock"))?;
    let prev: Vec<u8> = sqlx::query_scalar("SELECT hash FROM staff_audit ORDER BY seq DESC LIMIT 1")
        .fetch_optional(&mut *tx)
        .await
        .map_err(db("read the last row"))?
        .unwrap_or_else(|| GENESIS.to_vec());
    let at: String = sqlx::query_scalar(concat!("SELECT ", at_text!("clock_timestamp()")))
        .fetch_one(&mut *tx)
        .await
        .map_err(db("the time"))?;
    let mut row = Stored {
        seq: 0,
        at,
        actor: e.actor.as_ref().map(|a| a.id.clone()),
        source: e.source().word().to_string(),
        roles: e.actor.as_ref().map(|a| a.role_ids.clone()).unwrap_or_default(),
        roles_version: e.actor.as_ref().map(|a| a.roles_version),
        lever: e.lever.map(|l| l.word().to_string()),
        level: e.level.map(|l| l.word().to_string()),
        action: e.action.clone(),
        object_kind: e.object_kind.map(str::to_string),
        object_id: e.object_id.clone(),
        data_class: e.data_class.map(str::to_string),
        fields: e.fields.clone(),
        before_version: e.before_version,
        after_version: e.after_version,
        allowed: e.allowed,
        prev_hash: prev,
        hash: vec![],
    };
    row.hash = digest(&row.prev_hash, &row);
    let seq: i64 = sqlx::query_scalar(
        "INSERT INTO staff_audit (at, actor, source, roles, roles_version, lever, level, action, object_kind,
             object_id, data_class, fields, before_version, after_version, allowed, prev_hash, hash)
         VALUES ($1::timestamptz, $2::uuid, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
         RETURNING seq",
    )
    .bind(&row.at)
    .bind(&row.actor)
    .bind(&row.source)
    .bind(&row.roles)
    .bind(row.roles_version)
    .bind(&row.lever)
    .bind(&row.level)
    .bind(&row.action)
    .bind(&row.object_kind)
    .bind(&row.object_id)
    .bind(&row.data_class)
    .bind(&row.fields)
    .bind(row.before_version)
    .bind(row.after_version)
    .bind(row.allowed)
    .bind(&row.prev_hash)
    .bind(&row.hash)
    .fetch_one(&mut *tx)
    .await
    .map_err(db("write"))?;
    Ok(seq)
}

/// Writes one row on its own (a refusal, a look at a page): its own transaction.
pub async fn write_alone(pool: &PgPool, e: &Entry) -> Result<i64, String> {
    let mut tx = pool.begin().await.map_err(db("begin"))?;
    let seq = write(&mut tx, e).await?;
    tx.commit().await.map_err(db("commit"))?;
    Ok(seq)
}

macro_rules! columns {
    () => {
        concat!(
            "seq, ",
            at_text!("at"),
            " AS at, actor::text AS actor, source, roles, roles_version, lever, level, action, object_kind, object_id,
             data_class, fields, before_version, after_version, allowed, prev_hash, hash"
        )
    };
}

fn stored(r: &sqlx::postgres::PgRow) -> Stored {
    Stored {
        seq: r.get("seq"),
        at: r.get("at"),
        actor: r.get("actor"),
        source: r.get("source"),
        roles: r.get("roles"),
        roles_version: r.get("roles_version"),
        lever: r.get("lever"),
        level: r.get("level"),
        action: r.get("action"),
        object_kind: r.get("object_kind"),
        object_id: r.get("object_id"),
        data_class: r.get("data_class"),
        fields: r.get("fields"),
        before_version: r.get("before_version"),
        after_version: r.get("after_version"),
        allowed: r.get("allowed"),
        prev_hash: r.get("prev_hash"),
        hash: r.get("hash"),
    }
}

/// The most rows one read gives.
pub const READ_MAX: i64 = 500;

/// Rows newest first, before `before` (0: from the newest), at most `limit`. `changes`: only
/// the rows that changed something or were refused (the audit page's first view, §5.1).
pub async fn read(pool: &PgPool, before: i64, limit: i64, changes: bool) -> Result<Vec<Stored>, String> {
    let limit = limit.clamp(1, READ_MAX);
    let before = if before <= 0 { i64::MAX } else { before };
    let rows = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM staff_audit WHERE seq < $1
           AND (NOT $3 OR NOT allowed OR after_version IS NOT NULL OR action LIKE 'role.%' OR action LIKE 'person.%')
         ORDER BY seq DESC LIMIT $2"
    ))
    .bind(before)
    .bind(limit)
    .bind(changes)
    .fetch_all(pool)
    .await
    .map_err(db("read"))?;
    Ok(rows.iter().map(stored).collect())
}

/// Walks the whole chain, oldest first. Ok(the number of rows) when every row's hash matches its
/// fields and the row before; Err(the first row's `seq` that does not).
pub async fn verify(pool: &PgPool) -> Result<u64, i64> {
    let mut prev = GENESIS.to_vec();
    let mut after = 0i64;
    let mut count = 0u64;
    loop {
        let rows = sqlx::query(concat!("SELECT ", columns!(), " FROM staff_audit WHERE seq > $1 ORDER BY seq LIMIT 1000"))
            .bind(after)
            .fetch_all(pool)
            .await
            .map_err(|_| -1i64)?;
        if rows.is_empty() {
            return Ok(count);
        }
        for r in rows.iter().map(stored) {
            if r.prev_hash != prev || digest(&prev, &r) != r.hash {
                return Err(r.seq);
            }
            prev = r.hash.clone();
            after = r.seq;
            count += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> Stored {
        Stored {
            seq: 1,
            at: "2026-11-18T15:00:00.000001Z".into(),
            actor: Some("0b5f3c2e-8f1a-4c7d-9e2b-1a2b3c4d5e6f".into()),
            source: "person".into(),
            roles: vec!["deets".into()],
            roles_version: Some(7),
            lever: Some("agent".into()),
            level: Some("edit".into()),
            action: "settings.save".into(),
            object_kind: Some("settings".into()),
            object_id: Some("prompt".into()),
            data_class: None,
            fields: vec!["task.verify".into()],
            before_version: Some(3),
            after_version: Some(4),
            allowed: true,
            prev_hash: GENESIS.to_vec(),
            hash: vec![],
        }
    }

    #[test]
    fn every_field_changes_the_hash() {
        let base = digest(&GENESIS, &row());
        assert_eq!(base.len(), 32);
        assert_eq!(base, digest(&GENESIS, &row()), "the same row, the same hash");
        let changed: [fn(&mut Stored); 9] = [
            |r| r.at.push('0'),
            |r| r.actor = None,
            |r| r.roles.push("intern".into()),
            |r| r.roles_version = Some(8),
            |r| r.level = Some("view".into()),
            |r| r.object_id = Some("voice".into()),
            |r| r.fields.clear(),
            |r| r.after_version = None,
            |r| r.allowed = false,
        ];
        for change in changed {
            let mut r = row();
            change(&mut r);
            assert_ne!(digest(&GENESIS, &r), base);
        }
        assert_ne!(digest(&[1u8; 32], &row()), base, "the row before is in the hash");
    }

    #[test]
    fn a_field_s_bytes_cannot_move_to_the_next() {
        let mut a = row();
        a.object_kind = Some("ab".into());
        a.object_id = Some("c".into());
        let mut b = row();
        b.object_kind = Some("a".into());
        b.object_id = Some("bc".into());
        assert_ne!(digest(&GENESIS, &a), digest(&GENESIS, &b));
        let mut empty = row();
        empty.data_class = Some(String::new());
        assert_ne!(digest(&GENESIS, &empty), digest(&GENESIS, &row()), "empty is not missing");
    }
}
