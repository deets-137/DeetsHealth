//! The call store (DATABASE.md): Postgres, shaped like production, written by `ehr` only.
//!
//! Today it files the dev call records (`agent/calls/*.jsonl`) and the annotations
//! (`agent/calls/annotations.jsonl`): `ehr ingest`, run by `npm run db:ingest` and by
//! `npm run dev`. Later the agent sends the same events over gRPC while the call runs.
//!
//! - **Idempotent, never destructive.** A record whose hash is already filed is skipped; a
//!   changed record is filed again, whole, in one transaction. An annotation line is filed once,
//!   keyed by its own hash. Nothing drops the database or its file.
//! - **Sealed.** Every column that can hold protected data is sealed with core's `seal.rs` format
//!   (seal_key.rs), bound to its table, row and column. Plain columns hold kinds, times, counts.
//! - **Errors name the step and the database's error code,** never a row or a value
//!   (SECURITY.md §4.5).

use std::path::Path;

use deetshealth_core::{
    platform::Cipher,
    seal::{is_current, open, open_text, seal, Place, CURRENT_FORMAT},
};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use zeroize::Zeroizing;

use crate::seal_key::{AwsLc, SealKey};

/// The migrations in `ehr/migrations/`, embedded in the binary: the same files in dev and in the
/// cloud.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// The file of labels, notes, stars and ratings (CHAT.md §7).
pub const ANNOTATIONS: &str = "annotations.jsonl";

/// One table's sealed columns, and its row key as SQL text: the `row` of each value's
/// [`Place`], as the code that seals it builds it.
pub struct SealedTable {
    pub table: &'static str,
    pub row: &'static str,
    pub columns: &'static [&'static str],
}

/// Every sealed column of the store. A new sealed column needs its line here: [`Store::reseal`]
/// walks this list, and a test (tests/store.rs) fails when the schema has a `bytea` column that
/// is not on it.
pub const SEALED: &[SealedTable] = &[
    SealedTable { table: "transcript_lines", row: "call_id || '/' || seq", columns: &["words"] },
    SealedTable { table: "call_events", row: "call_id || '/' || seq", columns: &["body"] },
    SealedTable { table: "staff_flags", row: "call_id || '/' || seq", columns: &["summary", "evidence"] },
    SealedTable { table: "annotations", row: "line_sha256", columns: &["words"] },
    SealedTable {
        table: "users",
        row: "encode(email_hash, 'hex')",
        columns: &["email", "name", "first_name", "last_name", "phone"],
    },
    SealedTable { table: "user_photos", row: "user_id::text", columns: &["photo"] },
];

/// What one [`Store::reseal`] did: counts only.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Resealed {
    /// Values opened in an older format and sealed again in the current one.
    pub values: usize,
    /// Values in an older format that did not open in their place. Left as they were.
    pub unopened: usize,
}

/// What one ingest did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Filed {
    pub calls: usize,
    pub unchanged: usize,
    pub annotations: usize,
}

/// The plain columns of one call, worked out from its record's events.
#[derive(Debug, Default, PartialEq)]
pub struct Summary {
    pub schema_version: i32,
    pub started: Option<String>,
    pub source: String,
    pub mode: Option<String>,
    pub model: Option<String>,
    /// The prompt version's label (schema 9), or "file".
    pub prompt_version: Option<String>,
    /// The System version's label (SYSTEM-PAGE.md §3.4), or "file".
    pub system_version: Option<String>,
    /// The hospital version's label (HOSPITAL-CONFIG.md), or "file".
    pub hospital_version: Option<String>,
    pub scenario: Option<String>,
    pub outcome: &'static str,
    pub end_reason: Option<String>,
    pub reached: Vec<String>,
    pub turns: i32,
    pub tool_calls: i32,
    pub duration_ms: f64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_usd: f64,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// The call's plain columns. The outcome is the real end state, most telling first, as the
/// dev catalog works it out (CHAT.md §7): crashed · booked · handed_off · locked · ended ·
/// abandoned.
pub fn summarize(events: &[Value]) -> Summary {
    let mut s = Summary { schema_version: 1, source: "typed".into(), ..Default::default() };
    let (mut crashed, mut booked, mut handed_off, mut locked, mut ended) = (false, false, false, false, false);
    for e in events {
        let kind = e.get("kind").and_then(Value::as_str).unwrap_or("");
        s.duration_ms = s.duration_ms.max(e.get("t_ms").and_then(Value::as_f64).unwrap_or(0.0));
        match kind {
            "call_start" => {
                s.schema_version = e.get("schema_version").and_then(Value::as_i64).unwrap_or(1) as i32;
                s.started = text(e, "started");
                s.source = text(e, "source").unwrap_or_else(|| "typed".into());
                s.mode = text(e, "mode");
                s.model = text(e, "model");
                s.prompt_version = text(e, "prompt_version");
                s.system_version = text(e, "system_version");
                s.scenario = text(e, "scenario");
            }
            // The hospital version the call opened with (schema 13): known only once ehr
            // answers OpenCall, so its own event.
            "hospital" => s.hospital_version = text(e, "version"),
            "node" | "step" => {
                if let Some(name) = text(e, "name") {
                    if s.reached.last() != Some(&name) {
                        s.reached.push(name);
                    }
                }
            }
            "caller" => s.turns += 1,
            "tool_call" => {
                s.tool_calls += 1;
                match e.get("name").and_then(Value::as_str) {
                    Some("hand_off" | "change_visit") => handed_off = true,
                    Some("end_call") => ended = true,
                    _ => {}
                }
            }
            "ehr" => match (e.get("call").and_then(Value::as_str), e.get("result").and_then(Value::as_str)) {
                (Some("Book"), Some("booked")) => booked = true,
                (Some("CheckCode"), Some("locked")) => locked = true,
                _ => {}
            },
            "error" => crashed = true,
            "llm" if e.get("error").is_some() => crashed = true,
            "call_end" => {
                s.end_reason = text(e, "reason");
                if let Some(t) = e.get("tokens") {
                    s.tokens_in = t.get("input_tokens").and_then(Value::as_i64).unwrap_or(0);
                    s.tokens_out = t.get("output_tokens").and_then(Value::as_i64).unwrap_or(0);
                }
                s.cost_usd = e.get("cost_usd").and_then(Value::as_f64).unwrap_or(0.0);
            }
            _ => {}
        }
    }
    s.outcome = if crashed {
        "crashed"
    } else if booked {
        "booked"
    } else if handed_off {
        "handed_off"
    } else if locked {
        "locked"
    } else if ended {
        "ended"
    } else {
        "abandoned"
    };
    s
}

/// A database error as the step and its code, never a value.
fn db(step: &str, e: sqlx::Error) -> String {
    let code = e.as_database_error().and_then(|d| d.code().map(|c| c.into_owned()));
    match code {
        Some(code) => format!("{step}: database error {code}"),
        None => format!("{step}: the database could not be reached"),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub struct Store {
    pool: PgPool,
    key: SealKey,
}

impl Store {
    /// Connect and bring the schema up to date.
    pub async fn connect(url: &str, key: SealKey) -> Result<Self, String> {
        let pool = PgPoolOptions::new()
            .max_connections(4)
            // A stopped dev database fails fast, so ehr starts without a store, not after 30 s.
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect(url)
            .await
            .map_err(|e| db("connect", e))?;
        MIGRATOR.run(&pool).await.map_err(|_| "the migrations failed".to_string())?;
        let store = Self { pool, key };
        let done = store.reseal().await?;
        if done.values > 0 {
            eprintln!("ehr: sealed {} values again in the current format", done.values);
        }
        if done.unopened > 0 {
            eprintln!("ehr: {} sealed values in an older format did not open; they stay as they are", done.unopened);
        }
        Ok(store)
    }

    /// Seal again every value that is in an older format (core's seal.rs: a new format byte).
    /// Runs at each connect; with nothing old, it is one query per table that finds no row.
    ///
    /// - **Never destructive.** A value that does not open in its place keeps its bytes and is
    ///   counted. One transaction per table: all of a table's values change, or none.
    /// - **Safe to run twice at once** (`ehr` and `ehr ingest` start together): the rows are
    ///   locked, and the second run finds them current.
    /// - The plaintext is in memory between the open and the seal, wiped when it drops.
    pub async fn reseal(&self) -> Result<Resealed, String> {
        let mut out = Resealed::default();
        for t in SEALED {
            let old: Vec<String> =
                t.columns.iter().map(|c| format!("(length({c}) > 0 AND get_byte({c}, 0) <> {CURRENT_FORMAT})")).collect();
            let select = format!("SELECT {} AS k, {} FROM {} WHERE {} FOR UPDATE", t.row, t.columns.join(", "), t.table, old.join(" OR "));
            let mut tx = self.pool.begin().await.map_err(|e| db("begin a reseal", e))?;
            let rows = sqlx::query(&select).fetch_all(&mut *tx).await.map_err(|e| db("find the older values", e))?;
            if rows.is_empty() {
                continue;
            }
            let mut keys: Vec<String> = Vec::with_capacity(rows.len());
            let mut values: Vec<Vec<Option<Vec<u8>>>> = t.columns.iter().map(|_| Vec::with_capacity(rows.len())).collect();
            for r in &rows {
                let key: String = r.get("k");
                let mut changed = false;
                let mut row: Vec<Option<Vec<u8>>> = Vec::with_capacity(t.columns.len());
                for column in t.columns {
                    let bytes: Option<Vec<u8>> = r.get(*column);
                    row.push(match bytes {
                        Some(b) if !b.is_empty() && !is_current(&b) => {
                            let place = Place { table: t.table, row: &key, column };
                            match open(&AwsLc, &self.key, &place, &b) {
                                Ok(plain) => {
                                    out.values += 1;
                                    changed = true;
                                    Some(seal(&AwsLc, &self.key, &place, &plain).map_err(|_| "sealing failed".to_string())?)
                                }
                                Err(_) => {
                                    out.unopened += 1;
                                    Some(b)
                                }
                            }
                        }
                        other => other,
                    });
                }
                // A row with nothing sealed again is not written.
                if changed {
                    for (i, v) in row.into_iter().enumerate() {
                        values[i].push(v);
                    }
                    keys.push(key);
                }
            }
            if keys.is_empty() {
                continue;
            }
            // One UPDATE for the table: the new values join the rows by their key.
            let sets: Vec<String> = t.columns.iter().enumerate().map(|(i, c)| format!("{c} = v.n{i}")).collect();
            let arrays: Vec<String> = (0..t.columns.len()).map(|i| format!("${}::bytea[]", i + 2)).collect();
            let names: Vec<String> = (0..t.columns.len()).map(|i| format!("n{i}")).collect();
            let update = format!(
                "UPDATE {} SET {} FROM unnest($1::text[], {}) AS v(k, {}) WHERE {} = v.k",
                t.table,
                sets.join(", "),
                arrays.join(", "),
                names.join(", "),
                t.row
            );
            let mut q = sqlx::query(&update).bind(keys);
            for column in values {
                q = q.bind(column);
            }
            q.execute(&mut *tx).await.map_err(|e| db("seal the older values again", e))?;
            tx.commit().await.map_err(|e| db("commit a reseal", e))?;
        }
        Ok(out)
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub(crate) fn seal(&self, table: &str, row: &str, column: &str, plain: &str) -> Result<Vec<u8>, String> {
        seal(&AwsLc, &self.key, &Place { table, row, column }, plain.as_bytes()).map_err(|_| "sealing failed".to_string())
    }

    /// A sealed value back as text, wiped when it drops.
    pub(crate) fn open(&self, table: &str, row: &str, column: &str, sealed: &[u8]) -> Result<Zeroizing<String>, String> {
        open_text(&AwsLc, &self.key, &Place { table, row, column }, sealed).map_err(|_| "opening failed".to_string())
    }

    /// Bytes sealed the same way (a picture).
    pub(crate) fn seal_bytes(&self, table: &str, row: &str, column: &str, plain: &[u8]) -> Result<Vec<u8>, String> {
        seal(&AwsLc, &self.key, &Place { table, row, column }, plain).map_err(|_| "sealing failed".to_string())
    }

    /// Sealed bytes back, wiped when they drop.
    pub(crate) fn open_bytes(&self, table: &str, row: &str, column: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
        open(&AwsLc, &self.key, &Place { table, row, column }, sealed).map_err(|_| "opening failed".to_string())
    }

    /// The keyed hash a sealed column is found by (SealKey::lookup), as hex for a seal's row.
    pub(crate) fn lookup(&self, what: &str, value: &str) -> [u8; 32] {
        self.key.lookup(what, value)
    }

    pub(crate) fn hex(bytes: &[u8]) -> String {
        hex(bytes)
    }

    /// File every call record in `dir`, then its annotations.
    pub async fn file_dir(&self, dir: &Path) -> Result<Filed, String> {
        let mut records: Vec<_> = std::fs::read_dir(dir)
            .map_err(|_| "cannot read the calls folder")?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl") && p.file_name().is_some_and(|n| n != ANNOTATIONS))
            .collect();
        records.sort();
        let mut filed = Filed::default();
        for path in records {
            let id = path.file_stem().and_then(|s| s.to_str()).ok_or("a record has no name")?.to_owned();
            let bytes = std::fs::read(&path).map_err(|_| format!("cannot read the record {id}"))?;
            if self.file_record(&id, &bytes).await? {
                filed.calls += 1;
            } else {
                filed.unchanged += 1;
            }
        }
        let annotations = dir.join(ANNOTATIONS);
        if annotations.exists() {
            let bytes = std::fs::read(&annotations).map_err(|_| "cannot read the annotations")?;
            filed.annotations = self.file_annotations(&bytes).await?;
        }
        Ok(filed)
    }

    /// File one record. Returns false when the same record is already filed.
    pub async fn file_record(&self, id: &str, bytes: &[u8]) -> Result<bool, String> {
        // The hash is over the record's canonical form: its non-empty lines, each without a
        // trailing \r and ending in \n. A record read from disk on Windows (\r\n) and the same
        // record sent line by line over FileCall then hash the same (2026-11-12).
        let raw = String::from_utf8_lossy(bytes);
        let mut canonical = String::with_capacity(raw.len());
        for line in raw.lines().map(|l| l.trim_end_matches('\r')).filter(|l| !l.trim().is_empty()) {
            canonical.push_str(line);
            canonical.push('\n');
        }
        let sha = hex(&AwsLc.sha256(canonical.as_bytes()).map_err(|_| "hashing failed")?);
        let known: Option<String> = sqlx::query_scalar("SELECT record_sha256 FROM calls WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db("look up a call", e))?;
        if known.as_deref() == Some(sha.as_str()) {
            return Ok(false);
        }
        let mut events = Vec::new();
        for (n, line) in canonical.lines().enumerate() {
            events.push(serde_json::from_str::<Value>(line).map_err(|_| format!("record {id}: line {} is not JSON", n + 1))?);
        }
        let s = summarize(&events);

        let mut tx = self.pool.begin().await.map_err(|e| db("begin", e))?;
        sqlx::query(
            "INSERT INTO calls (id, schema_version, started, source, mode, model, prompt_version, system_version, hospital_version, scenario, outcome, end_reason,
                reached, turns, tool_calls, duration_ms, tokens_in, tokens_out, cost_usd, record_sha256)
             VALUES ($1, $2, $3::timestamp, $4, $5, $6, $18, $19, $20, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
             ON CONFLICT (id) DO UPDATE SET schema_version = EXCLUDED.schema_version, started = EXCLUDED.started,
                source = EXCLUDED.source, mode = EXCLUDED.mode, model = EXCLUDED.model, prompt_version = EXCLUDED.prompt_version, system_version = EXCLUDED.system_version, hospital_version = EXCLUDED.hospital_version, scenario = EXCLUDED.scenario,
                outcome = EXCLUDED.outcome, end_reason = EXCLUDED.end_reason, reached = EXCLUDED.reached,
                turns = EXCLUDED.turns, tool_calls = EXCLUDED.tool_calls, duration_ms = EXCLUDED.duration_ms,
                tokens_in = EXCLUDED.tokens_in, tokens_out = EXCLUDED.tokens_out, cost_usd = EXCLUDED.cost_usd,
                record_sha256 = EXCLUDED.record_sha256, filed_at = now()",
        )
        .bind(id)
        .bind(s.schema_version)
        .bind(&s.started)
        .bind(&s.source)
        .bind(&s.mode)
        .bind(&s.model)
        .bind(&s.scenario)
        .bind(s.outcome)
        .bind(&s.end_reason)
        .bind(&s.reached)
        .bind(s.turns)
        .bind(s.tool_calls)
        .bind(s.duration_ms)
        .bind(s.tokens_in)
        .bind(s.tokens_out)
        .bind(s.cost_usd)
        .bind(&sha)
        .bind(&s.prompt_version)
        .bind(&s.system_version)
        .bind(&s.hospital_version)
        .execute(&mut *tx)
        .await
        .map_err(|e| db("file a call", e))?;

        // A changed record is filed again, whole: its rows below are worked out afresh.
        for table in ["transcript_lines", "call_events", "tool_calls", "staff_flags", "review_flags"] {
            sqlx::query(&format!("DELETE FROM {table} WHERE call_id = $1"))
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| db("clear a call's rows", e))?;
        }

        for (seq, e) in events.iter().enumerate() {
            let seq = seq as i32;
            let row = format!("{id}/{seq}");
            let t_ms = e.get("t_ms").and_then(Value::as_f64).unwrap_or(0.0);
            let kind = e.get("kind").and_then(Value::as_str).unwrap_or("unknown");
            let body = self.seal("call_events", &row, "body", &e.to_string())?;
            sqlx::query("INSERT INTO call_events (call_id, seq, t_ms, kind, body) VALUES ($1, $2, $3, $4, $5)")
                .bind(id)
                .bind(seq)
                .bind(t_ms)
                .bind(kind)
                .bind(body)
                .execute(&mut *tx)
                .await
                .map_err(|e| db("file an event", e))?;
            match kind {
                "caller" | "agent" => {
                    if let Some(words) = e.get("text").and_then(Value::as_str) {
                        let sealed = self.seal("transcript_lines", &row, "words", words)?;
                        sqlx::query(
                            "INSERT INTO transcript_lines (call_id, seq, t_ms, speaker, words) VALUES ($1, $2, $3, $4, $5)",
                        )
                        .bind(id)
                        .bind(seq)
                        .bind(t_ms)
                        .bind(kind)
                        .bind(sealed)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| db("file a transcript line", e))?;
                    }
                }
                "tool_result" => {
                    sqlx::query("INSERT INTO tool_calls (call_id, seq, t_ms, tool, ms, failed) VALUES ($1, $2, $3, $4, $5, $6)")
                        .bind(id)
                        .bind(seq)
                        .bind(t_ms)
                        .bind(text(e, "name").unwrap_or_default())
                        .bind(e.get("ms").and_then(Value::as_f64))
                        .bind(e.get("error").is_some_and(|x| !x.is_null()))
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| db("file a tool call", e))?;
                }
                "staff_flag" => {
                    let summary = text(e, "summary").unwrap_or_default();
                    let evidence = e.get("evidence").cloned().unwrap_or(Value::Array(vec![])).to_string();
                    sqlx::query(
                        "INSERT INTO staff_flags (call_id, seq, t_ms, kind, source, unverified, summary, evidence)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                    )
                    .bind(id)
                    .bind(seq)
                    .bind(t_ms)
                    .bind(text(e, "flag").unwrap_or_else(|| "other".into()))
                    .bind(text(e, "source").unwrap_or_else(|| "code".into()))
                    .bind(e.get("unverified").and_then(Value::as_i64).unwrap_or(0) as i32)
                    .bind(self.seal("staff_flags", &row, "summary", &summary)?)
                    .bind(self.seal("staff_flags", &row, "evidence", &evidence)?)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| db("file a staff flag", e))?;
                }
                "review_flag" => {
                    sqlx::query("INSERT INTO review_flags (call_id, seq, t_ms, reason, final) VALUES ($1, $2, $3, $4, $5)")
                        .bind(id)
                        .bind(seq)
                        .bind(t_ms)
                        .bind(text(e, "reason").unwrap_or_else(|| "other".into()))
                        .bind(e.get("final").and_then(Value::as_bool).unwrap_or(false))
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| db("file a review flag", e))?;
                }
                _ => {}
            }
        }
        tx.commit().await.map_err(|e| db("commit a call", e))?;
        Ok(true)
    }

    /// File each new line of annotations.jsonl. Returns how many were new.
    pub async fn file_annotations(&self, bytes: &[u8]) -> Result<usize, String> {
        let mut new = 0;
        for (n, line) in String::from_utf8_lossy(bytes).lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let a: Value = serde_json::from_str(line).map_err(|_| format!("annotations: line {} is not JSON", n + 1))?;
            let sha = hex(&AwsLc.sha256(line.as_bytes()).map_err(|_| "hashing failed")?);
            let kind = text(&a, "kind").unwrap_or_default();
            let words = match kind.as_str() {
                "label" | "note" => text(&a, "text"),
                "rating" => text(&a, "note"),
                _ => None,
            };
            let sealed = words.map(|w| self.seal("annotations", &sha, "words", &w)).transpose()?;
            let done = sqlx::query(
                "INSERT INTO annotations (line_sha256, call_id, kind, at, grader_kind, grader, dimension, verdict,
                    score, rubric_version, keep, words)
                 VALUES ($1, $2, $3, $4::timestamp, $5, $6, $7, $8, $9, $10, $11, $12)
                 ON CONFLICT (line_sha256) DO NOTHING",
            )
            .bind(&sha)
            .bind(text(&a, "call").unwrap_or_default())
            .bind(&kind)
            .bind(text(&a, "at"))
            .bind(text(&a, "grader_kind"))
            .bind(text(&a, "grader"))
            .bind(text(&a, "dimension"))
            .bind(text(&a, "verdict"))
            .bind(a.get("score").and_then(Value::as_i64).map(|x| x as i16))
            .bind(text(&a, "rubric_version"))
            .bind(a.get("value").and_then(Value::as_bool))
            .bind(sealed)
            .execute(&self.pool)
            .await
            .map_err(|e| db("file an annotation", e))?;
            new += done.rows_affected() as usize;
        }
        Ok(new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_outcome_is_the_real_end_state() {
        let events = vec![
            json!({"t_ms": 0, "kind": "call_start", "schema_version": 3, "source": "test", "started": "2026-11-12T12:58:17"}),
            json!({"t_ms": 5, "kind": "node", "name": "greet"}),
            json!({"t_ms": 9, "kind": "caller", "text": "hi"}),
            json!({"t_ms": 12, "kind": "tool_call", "name": "book"}),
            json!({"t_ms": 13, "kind": "ehr", "call": "Book", "result": "booked"}),
            json!({"t_ms": 20, "kind": "tool_call", "name": "change_visit"}),
            json!({"t_ms": 30, "kind": "call_end", "reason": "the agent ended it",
                   "tokens": {"input_tokens": 4, "output_tokens": 50}, "cost_usd": 0.001}),
        ];
        let s = summarize(&events);
        assert_eq!(s.outcome, "booked", "a booking outranks the hand-off after it");
        assert_eq!((s.turns, s.tool_calls, s.schema_version), (1, 2, 3));
        assert_eq!(s.reached, vec!["greet"]);
        assert_eq!((s.tokens_in, s.tokens_out, s.duration_ms), (4, 50, 30.0));
    }

    #[test]
    fn an_empty_record_is_abandoned() {
        assert_eq!(summarize(&[]).outcome, "abandoned");
    }
}
