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
    seal::{seal, Place},
};
use serde_json::Value;
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::seal_key::{AwsLc, SealKey};

/// The migrations in `ehr/migrations/`, embedded in the binary: the same files in dev and in the
/// cloud.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// The file of labels, notes, stars and ratings (CHAT.md §7).
pub const ANNOTATIONS: &str = "annotations.jsonl";

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
                s.scenario = text(e, "scenario");
            }
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
        Ok(Self { pool, key })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    fn seal(&self, table: &str, row: &str, column: &str, plain: &str) -> Result<Vec<u8>, String> {
        seal(&AwsLc, &self.key, &Place { table, row, column }, plain.as_bytes()).map_err(|_| "sealing failed".to_string())
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
            "INSERT INTO calls (id, schema_version, started, source, mode, model, prompt_version, scenario, outcome, end_reason,
                reached, turns, tool_calls, duration_ms, tokens_in, tokens_out, cost_usd, record_sha256)
             VALUES ($1, $2, $3::timestamp, $4, $5, $6, $18, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
             ON CONFLICT (id) DO UPDATE SET schema_version = EXCLUDED.schema_version, started = EXCLUDED.started,
                source = EXCLUDED.source, mode = EXCLUDED.mode, model = EXCLUDED.model, prompt_version = EXCLUDED.prompt_version, scenario = EXCLUDED.scenario,
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
