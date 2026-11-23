//! The call store (DATABASE.md): Postgres, shaped like production, written by `ehr` only.
//!
//! Three parts (DATABASE.md §7):
//! - **`public`** ([`Store`]): the settings store and the people list, sealed under the hospital
//!   key. The owner login; the migrations run here.
//! - **`synthetic`** ([`ClassStore`]): every call from a sandbox or a simulated caller, and the
//!   kept scripts' runs (§8), sealed under the test key. Its pool runs as the role
//!   `deets_synthetic` and sees only its own schema.
//! - **`real`**: real patients' calls, under the hospital key, as `deets_real`. Opened only in a
//!   deploy (not built): on a dev machine there is no real store, and a real row is refused.
//!
//! - **Idempotent, never destructive.** A record whose hash is already filed is skipped; a
//!   changed record is filed again, whole, in one transaction. An annotation is filed once,
//!   keyed by its own hash. The only delete is the kept-record retention (§8.6), in `synthetic`.
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
use crate::sql::Sql;

/// The migrations in `ehr/migrations/`, embedded in the binary: the same files in dev and in the
/// cloud.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// The retired file of labels, notes, stars and ratings (DATABASE.md §9): filed once if present.
pub const ANNOTATIONS: &str = "annotations.jsonl";

/// What a call's rows are: a real patient's, or a sandbox's (DATABASE.md §7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataClass {
    Synthetic,
    Real,
}

impl DataClass {
    /// The class's schema, and the role its pool runs as.
    pub fn schema(self) -> &'static str {
        match self {
            DataClass::Synthetic => "synthetic",
            DataClass::Real => "real",
        }
    }

    pub fn role(self) -> &'static str {
        match self {
            DataClass::Synthetic => "deets_synthetic",
            DataClass::Real => "deets_real",
        }
    }

    pub fn as_str(self) -> &'static str {
        self.schema()
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "synthetic" => Some(DataClass::Synthetic),
            "real" => Some(DataClass::Real),
            _ => None,
        }
    }
}

/// Who started a call (DATABASE.md §7.1). A call id starts with its origin and `_`.
pub const ORIGINS: &[&str] = &["typed", "test", "kept", "live"];

/// The origin a call id starts with, when it has one.
pub fn origin_of(id: &str) -> Option<&'static str> {
    let (prefix, rest) = id.split_once('_')?;
    if rest.is_empty() {
        return None;
    }
    ORIGINS.iter().copied().find(|o| *o == prefix)
}

/// One table's sealed columns, and its row key as SQL text: the `row` of each value's
/// [`Place`], as the code that seals it builds it.
pub struct SealedTable {
    pub table: &'static str,
    pub row: &'static str,
    pub columns: &'static [&'static str],
}

/// The sealed columns of `public`, under the hospital key.
pub const PUBLIC_SEALED: &[SealedTable] = &[
    SealedTable {
        table: "users",
        row: "encode(email_hash, 'hex')",
        columns: &["email", "name", "first_name", "last_name", "phone"],
    },
    SealedTable { table: "user_photos", row: "user_id::text", columns: &["photo"] },
    // Each card as saved (users.rs › keep_card; migration 0012), sealed under its own row.
    SealedTable {
        table: "user_card_versions",
        row: "id::text",
        columns: &["email", "name", "first_name", "last_name", "phone"],
    },
];

/// The sealed columns of the call tables, the same in both class schemas.
pub const CALL_SEALED: &[SealedTable] = &[
    SealedTable { table: "transcript_lines", row: "call_id || '/' || seq", columns: &["words"] },
    SealedTable { table: "call_events", row: "call_id || '/' || seq", columns: &["body"] },
    SealedTable { table: "staff_flags", row: "call_id || '/' || seq", columns: &["summary", "evidence"] },
    SealedTable { table: "annotations", row: "line_sha256", columns: &["words"] },
];

/// The sealed columns of the kept scripts' runs, in `synthetic` only.
pub const TEST_SEALED: &[SealedTable] = &[SealedTable {
    table: "test_results",
    row: "run_id::text || '/' || script || '/' || draw::text || '/' || mode",
    columns: &["failure"],
}];

/// Every sealed column, for the schema test: a new sealed column needs its line in one of the
/// lists above, and `ehr/tests/store.rs` fails when a `bytea` column is on none.
pub fn sealed_in(schema: &str) -> Vec<&'static SealedTable> {
    match schema {
        "public" => PUBLIC_SEALED.iter().collect(),
        "synthetic" => CALL_SEALED.iter().chain(TEST_SEALED).collect(),
        "real" => CALL_SEALED.iter().collect(),
        _ => Vec::new(),
    }
}

/// What one [`Store::reseal`] did: counts only.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Resealed {
    /// Values opened in an older format (or, once, under the old key) and sealed again.
    pub values: usize,
    /// Values that did not open in their place. Left as they were.
    pub unopened: usize,
}

impl std::ops::AddAssign for Resealed {
    fn add_assign(&mut self, o: Self) {
        self.values += o.values;
        self.unopened += o.unopened;
    }
}

/// What one ingest did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Filed {
    pub calls: usize,
    pub unchanged: usize,
    pub annotations: usize,
    /// Records whose id does not start with an origin (from before migration 0010): not filed.
    pub skipped: usize,
}

/// The plain columns of one call, worked out from its record's events.
#[derive(Debug, Default, PartialEq)]
pub struct Summary {
    pub schema_version: i32,
    pub started: Option<String>,
    /// `typed`, `test`, `kept` or `live` (the record's `source`).
    pub origin: String,
    /// `synthetic` or `real` (schema 14; a dev record before it is synthetic).
    pub data_class: String,
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
    /// `text` or `voice` (schema 7).
    pub seat: Option<String>,
    /// `local` or `epic` (schema 6).
    pub ehr: Option<String>,
    /// Each caller turn's wait, in ms (migration 0013; `turn_waits`).
    pub turn_waits_ms: Vec<f64>,
    /// The requests to Claude answered live and from the replay cache (`call_end.requests`).
    pub requests_live: i32,
    pub requests_replayed: i32,
    /// Ink's and Sonic's credits (`call_end.cartesia.credits`; migration 0014).
    pub cartesia_credits: f64,
    /// Each answered turn's stages, as `Stages::to_json` (migration 0014).
    pub turn_stages: Vec<Value>,
}

/// Each caller turn's wait, in order (ANALYTICS.md §3.1, CHAT.md §18): from the turn's start to
/// the moment the caller hears the answer. The start is Ink's `last_words_ms` (a voice turn,
/// schema 15) or the caller line's time; a caller line less than 1 s from Ink's final line is
/// the same turn. The end is the seat speaking when the record has those events, else the
/// agent's first frame (`first_ms`, schema 4), else the agent line's time. A turn the agent
/// never answered has no wait. Times only.
pub fn turn_waits(events: &[Value]) -> Vec<f64> {
    turn_spans(events).into_iter().map(|(s, e)| e - s).collect()
}

/// One answered turn's wait split by stage, in ms (migration 0014; ANALYTICS.md §3.2): `ink`
/// the caller's silence to Ink's final line, `claude` the requests to Claude, `tools` the tools'
/// own runs less the ehr calls in them, `ehr` the ehr calls, `voice` the first frame out to the
/// caller hearing it, `other` the rest. Each span counts only where it falls inside the turn.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stages {
    pub ink: f64,
    pub claude: f64,
    pub tools: f64,
    pub ehr: f64,
    pub voice: f64,
    pub other: f64,
}

impl Stages {
    pub fn to_json(self) -> Value {
        let r = |x: f64| (x * 10.0).round() / 10.0;
        serde_json::json!({"ink": r(self.ink), "claude": r(self.claude), "tools": r(self.tools), "ehr": r(self.ehr), "voice": r(self.voice), "other": r(self.other)})
    }

    pub fn from_json(v: &Value) -> Stages {
        let n = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        Stages { ink: n("ink"), claude: n("claude"), tools: n("tools"), ehr: n("ehr"), voice: n("voice"), other: n("other") }
    }
}

/// Each answered turn's stages, in the order of `turn_waits`.
pub fn turn_stages(events: &[Value]) -> Vec<Stages> {
    let num = |e: &Value, k: &str| e.get(k).and_then(Value::as_f64);
    fn kind(e: &Value) -> &str {
        e.get("kind").and_then(Value::as_str).unwrap_or("")
    }
    fn stage(e: &Value) -> &str {
        e.get("stage").and_then(Value::as_str).unwrap_or("")
    }
    let speaks = events.iter().any(|e| kind(e) == "voice" && stage(e) == "speaking");
    // Every span of each kind on the call's clock.
    let (mut llm, mut tools, mut ehr) = (Vec::new(), Vec::new(), Vec::new());
    let mut open: Vec<(String, f64)> = Vec::new();
    let mut heard: Vec<(f64, f64)> = Vec::new();
    let mut first_frames: Vec<f64> = Vec::new();
    for e in events {
        let t = num(e, "t_ms").unwrap_or(0.0);
        let name = e.get("name").and_then(Value::as_str).unwrap_or("").to_owned();
        match kind(e) {
            "llm" => llm.push((t - num(e, "total_ms").unwrap_or(0.0), t)),
            "tool_call" => open.push((name, t)),
            "tool_result" => {
                if let Some(i) = open.iter().position(|(n, _)| *n == name) {
                    let (_, from) = open.remove(i);
                    tools.push((from, num(e, "ms").map_or(t, |ms| from + ms)));
                }
            }
            "ehr" => ehr.push((t - num(e, "ms").unwrap_or(0.0), t)),
            "voice" if stage(e) == "heard" => {
                if let Some(last) = num(e, "last_words_ms") {
                    heard.push((last, t));
                }
            }
            "agent" => first_frames.push(num(e, "first_ms").unwrap_or(t)),
            _ => {}
        }
    }
    let within = |spans: &[(f64, f64)], (s, end): (f64, f64)| -> f64 {
        spans.iter().map(|&(a, b)| (b.min(end) - a.max(s)).max(0.0)).sum()
    };
    turn_spans(events)
        .into_iter()
        .map(|(s, end)| {
            let ink = heard.iter().filter(|&&(a, b)| a >= s - 1.0 && b <= end).map(|&(a, b)| b - a).sum::<f64>();
            // On a voice call the wait after the first frame is Voice out; Claude's stream, a
            // tool or an ehr call still running then is not counted twice.
            let first = if speaks { first_frames.iter().copied().find(|&f| f >= s && f <= end) } else { None };
            let cut = first.unwrap_or(end);
            let claude = within(&llm, (s, cut));
            let ehr_ms = within(&ehr, (s, cut));
            let tools_ms = (within(&tools, (s, cut)) - ehr_ms).max(0.0);
            let voice = first.map_or(0.0, |f| end - f);
            let other = ((end - s) - ink - claude - tools_ms - ehr_ms - voice).max(0.0);
            Stages { ink, claude, tools: tools_ms, ehr: ehr_ms, voice, other }
        })
        .collect()
}

/// Each answered turn's start and end on the call's clock (`turn_waits`'s rule).
fn turn_spans(events: &[Value]) -> Vec<(f64, f64)> {
    let num = |e: &Value, k: &str| e.get(k).and_then(Value::as_f64);
    let stage = |e: &Value| e.get("stage").and_then(Value::as_str).map(str::to_owned);
    let kind = |e: &Value| e.get("kind").and_then(Value::as_str).unwrap_or("").to_owned();
    let speaks = events.iter().any(|e| kind(e) == "voice" && stage(e).as_deref() == Some("speaking"));
    let mut out = Vec::new();
    // The open turn's start, and when its caller line or Ink's final line came.
    let mut start: Option<f64> = None;
    let mut line_at: Option<f64> = None;
    let mut heard_at: Option<f64> = None;
    for e in events {
        let t = num(e, "t_ms").unwrap_or(0.0);
        match (kind(e).as_str(), stage(e).as_deref()) {
            ("voice", Some("heard")) => {
                let from = num(e, "last_words_ms").unwrap_or(t);
                if start.is_some() && line_at.is_some_and(|l| t - l < 1000.0) && heard_at.is_none() {
                    start = start.map(|s| s.min(from));
                } else {
                    start = Some(from);
                    line_at = None;
                }
                heard_at = Some(t);
            }
            ("caller", _) => {
                if !(start.is_some() && heard_at.is_some_and(|h| t - h < 1000.0) && line_at.is_none()) {
                    start = Some(t);
                    heard_at = None;
                }
                line_at = Some(t);
            }
            ("voice", Some("speaking")) if speaks => {
                if let Some(s) = start.take() {
                    out.push((s, t.max(s)));
                }
                line_at = None;
                heard_at = None;
            }
            ("agent", _) if !speaks => {
                // An agent line whose first frame came before this turn began is the turn
                // before's, stamped late (the harness's turn grace): it closes nothing.
                let at = num(e, "first_ms").unwrap_or(t);
                if start.is_some_and(|s| at < s) {
                    continue;
                }
                if let Some(s) = start.take() {
                    out.push((s, at));
                }
                line_at = None;
                heard_at = None;
            }
            _ => {}
        }
    }
    out
}

/// The filter bar's filters as SQL (ANALYTICS.md §3.2): the range, then `stats::Filter`, each
/// empty or false for any. `bind_filter` binds $1 to $10 in this order. A record with no seat
/// or EHR is a text call on Local, as Logs reads it.
macro_rules! stats_where {
    () => {
        " WHERE ($1 = '' OR c.started >= $1::timestamp) AND ($2 = '' OR c.started < $2::timestamp)
            AND ($3 = '' OR c.origin = $3) AND ($4 = '' OR c.outcome = $4)
            AND ($5 = '' OR COALESCE(c.ehr, 'local') = $5) AND ($6 = '' OR COALESCE(c.seat, 'text') = $6)
            AND ($7 = '' OR ($7 = 'prompt' AND c.prompt_version = $8) OR ($7 = 'system' AND c.system_version = $8)
                 OR ($7 = 'hospital' AND c.hospital_version = $8) OR ($7 = 'model' AND c.model = $8))
            AND (NOT $9 OR EXISTS (SELECT 1 FROM staff_flags sf WHERE sf.call_id = c.id)
                 OR EXISTS (SELECT 1 FROM review_flags rf WHERE rf.call_id = c.id))
            AND (NOT $10 OR (c.requests_replayed = 0 AND EXISTS (SELECT 1 FROM unnest(c.turn_waits_ms) AS w(ms) WHERE w.ms > 800)))"
    };
}
fn bind_filter<'q>(
    q: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    since: &'q str,
    until: &'q str,
    f: &'q crate::stats::Filter,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    q.bind(since)
        .bind(until)
        .bind(f.origin.as_str())
        .bind(f.outcome.as_str())
        .bind(f.ehr.as_str())
        .bind(f.seat.as_str())
        .bind(f.version_kind.as_str())
        .bind(f.version_label.as_str())
        .bind(f.flagged)
        .bind(f.slow)
}

/// One staff flag opened (`ClassStore::read_flags`): its kind and source, and its summary and
/// evidence, which are protected data, wiped when dropped.
#[derive(Debug)]
pub struct FlagWords {
    pub seq: i32,
    pub kind: String,
    pub source: String,
    pub unverified: i32,
    pub summary: Zeroizing<String>,
    pub evidence: Vec<Zeroizing<String>>,
}

/// One call as Analytics › Logs lists it (`ClassStore::list_calls`): plain columns and names.
#[derive(Debug, Clone, Default)]
pub struct LoggedCall {
    pub id: String,
    pub started: String,
    pub origin: String,
    pub seat: Option<String>,
    pub ehr: Option<String>,
    pub mode: Option<String>,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub system_version: Option<String>,
    pub hospital_version: Option<String>,
    pub scenario: Option<String>,
    pub outcome: String,
    pub end_reason: Option<String>,
    pub reached: Vec<String>,
    pub turns: i32,
    pub duration_ms: f64,
    pub cost_usd: f64,
    pub turn_waits_ms: Vec<f64>,
    pub requests_live: i32,
    pub requests_replayed: i32,
    pub tools: Vec<String>,
    pub tool_failed: Vec<bool>,
    pub staff_flags: Vec<String>,
    pub review_flags: Vec<String>,
    /// A synthetic call's current label; none for a real call.
    pub label: Option<String>,
    pub ratings_passed: i32,
    pub ratings_failed: i32,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// The call's plain columns. The outcome is the real end state, most telling first, as the
/// dev catalog works it out (CHAT.md §7): crashed · booked · handed_off · locked · ended ·
/// abandoned.
pub fn summarize(events: &[Value]) -> Summary {
    let mut s = Summary { schema_version: 1, origin: "typed".into(), data_class: "synthetic".into(), ..Default::default() };
    let (mut crashed, mut booked, mut handed_off, mut locked, mut ended) = (false, false, false, false, false);
    for e in events {
        let kind = e.get("kind").and_then(Value::as_str).unwrap_or("");
        s.duration_ms = s.duration_ms.max(e.get("t_ms").and_then(Value::as_f64).unwrap_or(0.0));
        match kind {
            "call_start" => {
                s.schema_version = e.get("schema_version").and_then(Value::as_i64).unwrap_or(1) as i32;
                s.started = text(e, "started");
                s.origin = text(e, "source").unwrap_or_else(|| "typed".into());
                s.data_class = text(e, "data_class").unwrap_or_else(|| "synthetic".into());
                s.mode = text(e, "mode");
                s.model = text(e, "model");
                s.prompt_version = text(e, "prompt_version");
                s.system_version = text(e, "system_version");
                s.scenario = text(e, "scenario");
                s.seat = text(e, "seat");
                s.ehr = text(e, "ehr");
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
                if let Some(r) = e.get("requests") {
                    let n = |k: &str| r.get(k).and_then(Value::as_i64).unwrap_or(0) as i32;
                    s.requests_live = n("live");
                    s.requests_replayed = n("replayed");
                }
                s.cartesia_credits = e.get("cartesia").and_then(|c| c.get("credits")).and_then(Value::as_f64).unwrap_or(0.0);
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
    s.turn_waits_ms = turn_waits(events);
    s.turn_stages = turn_stages(events).into_iter().map(Stages::to_json).collect();
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

fn sha256_hex(data: &[u8]) -> Result<String, String> {
    Ok(hex(&AwsLc.sha256(data).map_err(|_| "hashing failed")?))
}

/// A record's canonical form: its non-empty lines, each without a trailing \r and ending in \n. A
/// record read from disk on Windows (\r\n) and the same record sent line by line over FileCall
/// then hash the same (2026-11-12).
fn canonical(bytes: &[u8]) -> String {
    let raw = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(raw.len());
    for line in raw.lines().map(|l| l.trim_end_matches('\r')).filter(|l| !l.trim().is_empty()) {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The data class a record names on its `call_start`; synthetic when it names none.
pub fn record_class(bytes: &[u8]) -> Result<DataClass, String> {
    for line in String::from_utf8_lossy(bytes).lines() {
        let Ok(e) = serde_json::from_str::<Value>(line.trim_end_matches('\r')) else { continue };
        if e.get("kind").and_then(Value::as_str) == Some("call_start") {
            return match e.get("data_class").and_then(Value::as_str) {
                None => Ok(DataClass::Synthetic),
                Some(c) => DataClass::parse(c).ok_or_else(|| "the record's data_class is not synthetic or real".into()),
            };
        }
    }
    Ok(DataClass::Synthetic)
}

/// A key's fingerprint for `store_meta`: a keyed hash under the key itself, so it says which key
/// sealed a schema and reveals nothing of it.
fn fingerprint(key: &SealKey, what: &str) -> String {
    hex(&key.lookup("store.key", what))[..32].to_string()
}

/// Seal again every value of `tables` that is in an older format, or that opens under `old_key`
/// and not under `key` (the move to a class's own key, once). Never destructive: a value that
/// does not open in its place keeps its bytes and is counted. One transaction per table.
///
/// With `old_key` unset, only values in an older format are read, found by the format byte's
/// index (migration 0010). The plaintext is in memory between the open and the seal, wiped when
/// it drops.
async fn reseal_tables(
    pool: &PgPool,
    key: &SealKey,
    old_key: Option<&SealKey>,
    tables: &[&SealedTable],
) -> Result<Resealed, String> {
    let mut out = Resealed::default();
    for t in tables {
        let columns = || t.columns.iter().map(|c| Sql::new().text(c));
        let select = Sql::new().text("SELECT ").text(t.row).text(" AS k, ").join(columns(), ", ").text(" FROM ").text(t.table);
        let select = if old_key.is_some() {
            select.text(" FOR UPDATE")
        } else {
            // A value is old when its first byte (the format) is not the current one.
            let old = t.columns.iter().map(|c| {
                let first = || Sql::new().text("substring(").text(c).text(" FROM 1 FOR 1)");
                Sql::new()
                    .text("((")
                    .part(first())
                    .text(" < ")
                    .byte(CURRENT_FORMAT)
                    .text(" OR ")
                    .part(first())
                    .text(" > ")
                    .byte(CURRENT_FORMAT)
                    .text(") AND length(")
                    .text(c)
                    .text(") > 0)")
            });
            select.text(" WHERE ").join(old, " OR ").text(" FOR UPDATE")
        };
        let mut tx = pool.begin().await.map_err(|e| db("begin a reseal", e))?;
        let rows = sqlx::query(select).fetch_all(&mut *tx).await.map_err(|e| db("find the older values", e))?;
        if rows.is_empty() {
            continue;
        }
        let mut keys: Vec<String> = Vec::with_capacity(rows.len());
        let mut values: Vec<Vec<Option<Vec<u8>>>> = t.columns.iter().map(|_| Vec::with_capacity(rows.len())).collect();
        for r in &rows {
            let row_key: String = r.get("k");
            let mut changed = false;
            let mut row: Vec<Option<Vec<u8>>> = Vec::with_capacity(t.columns.len());
            for column in t.columns {
                let bytes: Option<Vec<u8>> = r.get(*column);
                let place = Place { table: t.table, row: &row_key, column };
                let current = bytes.as_deref().is_some_and(is_current);
                row.push(match bytes {
                    // Only old values are looked at, unless the class is moving to its own key.
                    Some(b) if !b.is_empty() && (old_key.is_some() || !current) => {
                        let opened = match open(&AwsLc, key, &place, &b) {
                            // Current, under the right key: nothing to do.
                            Ok(_) if current => None,
                            Ok(plain) => Some(Some(plain)),
                            // Not under the right key: the old key, when moving.
                            Err(_) => Some(old_key.and_then(|old| open(&AwsLc, old, &place, &b).ok())),
                        };
                        match opened {
                            None => Some(b),
                            Some(Some(plain)) => {
                                out.values += 1;
                                changed = true;
                                Some(seal(&AwsLc, key, &place, &plain).map_err(|_| "sealing failed".to_string())?)
                            }
                            Some(None) => {
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
                keys.push(row_key);
            }
        }
        if keys.is_empty() {
            tx.commit().await.map_err(|e| db("commit a reseal", e))?;
            continue;
        }
        // One UPDATE for the table: the new values join the rows by their key.
        let n = t.columns.len();
        let sets = t.columns.iter().enumerate().map(|(i, c)| Sql::new().text(c).text(" = v.n").num(i));
        let arrays = (0..n).map(|i| Sql::new().text("$").num(i + 2).text("::bytea[]"));
        let names = (0..n).map(|i| Sql::new().text("n").num(i));
        let update = Sql::new()
            .text("UPDATE ")
            .text(t.table)
            .text(" SET ")
            .join(sets, ", ")
            .text(" FROM unnest($1::text[], ")
            .join(arrays, ", ")
            .text(") AS v(k, ")
            .join(names, ", ")
            .text(") WHERE ")
            .text(t.row)
            .text(" = v.k");
        let mut q = sqlx::query(update).bind(keys);
        for column in values {
            q = q.bind(column);
        }
        q.execute(&mut *tx).await.map_err(|e| db("seal the older values again", e))?;
        tx.commit().await.map_err(|e| db("commit a reseal", e))?;
    }
    Ok(out)
}

/// The keys a store opens with (DATABASE.md §7.4).
pub struct StoreKeys {
    /// Seals `public` (and, in a deploy, `real`): the hospital's key.
    pub hospital: SealKey,
    /// Seals `synthetic`: the test key.
    pub test: SealKey,
}

pub struct Store {
    pool: PgPool,
    key: SealKey,
    synthetic: ClassStore,
    real: Option<ClassStore>,
}

impl Store {
    /// Connect, bring the schema up to date, open the synthetic class, and seal again what is
    /// old. No real class on this path (DATABASE.md §7.4).
    pub async fn connect(url: &str, keys: StoreKeys) -> Result<Self, String> {
        let pool = PgPoolOptions::new()
            .max_connections(4)
            // A stopped dev database fails fast, so ehr starts without a store, not after 30 s.
            .acquire_timeout(std::time::Duration::from_secs(3))
            .connect(url)
            .await
            .map_err(|e| db("connect", e))?;
        MIGRATOR.run(&pool).await.map_err(|_| "the migrations failed".to_string())?;
        let synthetic = ClassStore::connect(url, DataClass::Synthetic, keys.test).await?;
        let store = Self { pool, key: keys.hospital, synthetic, real: None };
        let done = store.reseal().await?;
        if done.values > 0 {
            eprintln!("ehr: sealed {} values again (a newer format or the class's own key)", done.values);
        }
        if done.unopened > 0 {
            eprintln!("ehr: {} sealed values did not open in their place; they stay as they are", done.unopened);
        }
        Ok(store)
    }

    /// Seal again every value in an older format, in `public` and in each open class. The first
    /// time a class meets its own key (its fingerprint is not in `store_meta`), every value of
    /// the class is read: those still under the hospital key move to the class's key.
    pub async fn reseal(&self) -> Result<Resealed, String> {
        let mut out = reseal_tables(&self.pool, &self.key, None, &sealed_in("public")).await?;
        for class in [Some(&self.synthetic), self.real.as_ref()].into_iter().flatten() {
            let marker = format!("key.{}", class.class.schema());
            let print = fingerprint(&class.key, class.class.schema());
            let known: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = $1")
                .bind(&marker)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| db("read the store's keys", e))?;
            let tables = sealed_in(class.class.schema());
            if known.as_deref() == Some(print.as_str()) {
                out += reseal_tables(&class.pool, &class.key, None, &tables).await?;
                continue;
            }
            if known.is_some() {
                eprintln!("ehr: the {} key is not the one that sealed its rows; old values may not open", class.class.schema());
            }
            out += reseal_tables(&class.pool, &class.key, Some(&self.key), &tables).await?;
            sqlx::query("INSERT INTO store_meta (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value")
                .bind(&marker)
                .bind(&print)
                .execute(&self.pool)
                .await
                .map_err(|e| db("note the store's key", e))?;
        }
        Ok(out)
    }

    /// The owner login's pool: the settings store and the people list.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Close every pool: the owner's and each class's.
    pub async fn close(&self) {
        self.synthetic.pool.close().await;
        if let Some(real) = &self.real {
            real.pool.close().await;
        }
        self.pool.close().await;
    }

    /// The synthetic class: every dev call, every test.
    pub fn synthetic(&self) -> &ClassStore {
        &self.synthetic
    }

    /// A class's store, when this ehr has it. A dev machine has no real store.
    pub fn class(&self, class: DataClass) -> Option<&ClassStore> {
        match class {
            DataClass::Synthetic => Some(&self.synthetic),
            DataClass::Real => self.real.as_ref(),
        }
    }

    /// The class that holds a call, by looking: synthetic first, then real.
    pub async fn class_of_call(&self, id: &str) -> Result<Option<&ClassStore>, String> {
        for class in [Some(&self.synthetic), self.real.as_ref()].into_iter().flatten() {
            if class.has_call(id).await? {
                return Ok(Some(class));
            }
        }
        Ok(None)
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

    /// File every call record in `dir`, then the retired annotations file if it is there.
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
            if origin_of(&id).is_none() {
                filed.skipped += 1;
                continue;
            }
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
            filed.annotations = self.synthetic.file_annotations(&bytes).await?;
        }
        Ok(filed)
    }

    /// File one record in the class it names. Returns false when the same record is already filed.
    pub async fn file_record(&self, id: &str, bytes: &[u8]) -> Result<bool, String> {
        let class = record_class(bytes)?;
        let store = self.class(class).ok_or("no real store on this ehr")?;
        store.file_record(id, bytes).await
    }

    /// The retired annotations file, into `synthetic` (DATABASE.md §9).
    pub async fn file_annotations(&self, bytes: &[u8]) -> Result<usize, String> {
        self.synthetic.file_annotations(bytes).await
    }
}

/// One label, note, star or rating (DATABASE.md §9; CHAT.md §7). Checked by the agent's gate
/// (`catalog.py › check_annotation`) and again here for its shape.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Annotation {
    pub call_id: String,
    pub kind: String,
    /// When it was made, as the catalog stamped it (ISO 8601).
    pub at: String,
    pub grader_kind: Option<String>,
    pub grader: Option<String>,
    pub dimension: Option<String>,
    pub verdict: Option<String>,
    pub score: Option<i16>,
    pub rubric_version: Option<String>,
    pub keep: Option<bool>,
    /// A label's or note's text, or a rating's note. Sealed in the store.
    pub words: Option<Zeroizing<String>>,
}

/// One run of the kept scripts (DATABASE.md §8.1).
#[derive(Debug, Default)]
pub struct TestRun {
    pub started: String,
    pub finished: String,
    pub run_by: String,
    pub words_sha256: String,
    pub prompt_version: String,
    pub system_version: String,
    pub hospital_version: String,
    pub llm_version: String,
    pub model: String,
    pub git_commit: String,
    pub tests_passed: bool,
    pub cost_usd: f64,
    pub results: Vec<TestResult>,
}

/// One draw of a run.
#[derive(Debug, Default)]
pub struct TestResult {
    pub script: String,
    pub draw: i32,
    /// `record` or `replay`.
    pub mode: String,
    pub passed: bool,
    pub test: String,
    /// The first line of the error; sealed.
    pub failure: Option<Zeroizing<String>>,
    /// The id of this draw's own record, if one was written.
    pub call_id: String,
    pub shape_sha256: String,
    pub duration_ms: f64,
    /// Each tool's time, as a JSON object.
    pub tool_ms: String,
    pub live_requests: i32,
    pub cost_usd: f64,
    /// The script's tags, as a JSON object (DATABASE.md §8.4).
    pub tags: String,
    pub reply_keys: Vec<String>,
}

/// What [`ClassStore::file_test_run`] decided.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FiledRun {
    pub run_id: i64,
    /// The call ids whose records the store needs: send each over FileCall.
    pub needed: Vec<String>,
    /// Kept records deleted by the retention pass (DATABASE.md §8.6).
    pub pruned: usize,
}

/// How many words versions keep their kept records (DATABASE.md §8.6, a draft).
pub const KEPT_WORDS_VERSIONS: i64 = 3;

/// One data class's tables (DATABASE.md §7): its own pool, as its own role, seeing only its
/// schema, and its own key.
pub struct ClassStore {
    class: DataClass,
    pool: PgPool,
    key: SealKey,
}

impl ClassStore {
    async fn connect(url: &str, class: DataClass, key: SealKey) -> Result<Self, String> {
        let (role, schema) = (class.role(), class.schema());
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(std::time::Duration::from_secs(3))
            // Each connection runs as the class's role and sees its schema alone: a query here
            // cannot name the other class's tables (Postgres refuses).
            .after_connect(move |conn, _| {
                Box::pin(async move {
                    sqlx::query(Sql::new().text("SET ROLE ").text(role)).execute(&mut *conn).await?;
                    sqlx::query(Sql::new().text("SET search_path TO ").text(schema)).execute(&mut *conn).await?;
                    Ok(())
                })
            })
            .connect(url)
            .await
            .map_err(|e| db("connect a class", e))?;
        Ok(Self { class, pool, key })
    }

    pub fn class(&self) -> DataClass {
        self.class
    }

    /// The class's pool: its role, its schema.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    fn seal(&self, table: &str, row: &str, column: &str, plain: &str) -> Result<Vec<u8>, String> {
        seal(&AwsLc, &self.key, &Place { table, row, column }, plain.as_bytes()).map_err(|_| "sealing failed".to_string())
    }

    fn open(&self, table: &str, row: &str, column: &str, sealed: &[u8]) -> Result<Zeroizing<String>, String> {
        open_text(&AwsLc, &self.key, &Place { table, row, column }, sealed).map_err(|_| "opening failed".to_string())
    }

    pub async fn has_call(&self, id: &str) -> Result<bool, String> {
        let found: Option<i32> = sqlx::query_scalar("SELECT 1 FROM calls WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db("look up a call", e))?;
        Ok(found.is_some())
    }

    /// File one record. Returns false when the same record is already filed. The record's id
    /// must start with its origin, and its class must be this store's.
    pub async fn file_record(&self, id: &str, bytes: &[u8]) -> Result<bool, String> {
        let canonical = canonical(bytes);
        let sha = sha256_hex(canonical.as_bytes())?;
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
        if origin_of(id) != Some(s.origin.as_str()) {
            return Err(format!("record {id}: the id does not start with its origin"));
        }
        if DataClass::parse(&s.data_class) != Some(self.class) {
            return Err(format!("record {id}: not a {} call", self.class.as_str()));
        }

        let mut tx = self.pool.begin().await.map_err(|e| db("begin", e))?;
        sqlx::query(
            "INSERT INTO calls (id, schema_version, started, origin, data_class, mode, model, prompt_version, system_version, hospital_version, scenario, outcome, end_reason,
                reached, turns, tool_calls, duration_ms, tokens_in, tokens_out, cost_usd, record_sha256, seat, ehr, turn_waits_ms,
                requests_live, requests_replayed, cartesia_credits, turn_stages)
             VALUES ($1, $2, $3::timestamp, $4, $21, $5, $6, $18, $19, $20, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $22, $23, $24, $25, $26, $27, $28::jsonb)
             ON CONFLICT (id) DO UPDATE SET schema_version = EXCLUDED.schema_version, started = EXCLUDED.started,
                seat = EXCLUDED.seat, ehr = EXCLUDED.ehr, turn_waits_ms = EXCLUDED.turn_waits_ms,
                requests_live = EXCLUDED.requests_live, requests_replayed = EXCLUDED.requests_replayed,
                cartesia_credits = EXCLUDED.cartesia_credits, turn_stages = EXCLUDED.turn_stages,
                origin = EXCLUDED.origin, mode = EXCLUDED.mode, model = EXCLUDED.model, prompt_version = EXCLUDED.prompt_version, system_version = EXCLUDED.system_version, hospital_version = EXCLUDED.hospital_version, scenario = EXCLUDED.scenario,
                outcome = EXCLUDED.outcome, end_reason = EXCLUDED.end_reason, reached = EXCLUDED.reached,
                turns = EXCLUDED.turns, tool_calls = EXCLUDED.tool_calls, duration_ms = EXCLUDED.duration_ms,
                tokens_in = EXCLUDED.tokens_in, tokens_out = EXCLUDED.tokens_out, cost_usd = EXCLUDED.cost_usd,
                record_sha256 = EXCLUDED.record_sha256, filed_at = now()",
        )
        .bind(id)
        .bind(s.schema_version)
        .bind(&s.started)
        .bind(&s.origin)
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
        .bind(&s.data_class)
        .bind(&s.seat)
        .bind(&s.ehr)
        .bind(&s.turn_waits_ms)
        .bind(s.requests_live)
        .bind(s.requests_replayed)
        .bind(s.cartesia_credits)
        .bind(Value::Array(s.turn_stages.clone()).to_string())
        .execute(&mut *tx)
        .await
        .map_err(|e| db("file a call", e))?;

        // A changed record is filed again, whole: its rows below are worked out afresh.
        for table in ["transcript_lines", "call_events", "tool_calls", "staff_flags", "review_flags"] {
            sqlx::query(Sql::new().text("DELETE FROM ").text(table).text(" WHERE call_id = $1"))
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

    /// File each new line of the retired annotations file. Returns how many were new.
    pub async fn file_annotations(&self, bytes: &[u8]) -> Result<usize, String> {
        let mut new = 0;
        for (n, line) in String::from_utf8_lossy(bytes).lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let a: Value = serde_json::from_str(line).map_err(|_| format!("annotations: line {} is not JSON", n + 1))?;
            let sha = sha256_hex(line.as_bytes())?;
            let kind = text(&a, "kind").unwrap_or_default();
            let words = match kind.as_str() {
                "label" | "note" => text(&a, "text"),
                "rating" => text(&a, "note"),
                _ => None,
            };
            let annotation = Annotation {
                call_id: text(&a, "call").unwrap_or_default(),
                kind,
                at: text(&a, "at").unwrap_or_default(),
                grader_kind: text(&a, "grader_kind"),
                grader: text(&a, "grader"),
                dimension: text(&a, "dimension"),
                verdict: text(&a, "verdict"),
                score: a.get("score").and_then(Value::as_i64).map(|x| x as i16),
                rubric_version: text(&a, "rubric_version"),
                keep: a.get("value").and_then(Value::as_bool),
                words: words.map(Zeroizing::new),
            };
            new += self.insert_annotation(&sha, &annotation).await?;
        }
        Ok(new)
    }

    async fn insert_annotation(&self, sha: &str, a: &Annotation) -> Result<usize, String> {
        let sealed = a.words.as_ref().map(|w| self.seal("annotations", sha, "words", w)).transpose()?;
        let done = sqlx::query(
            "INSERT INTO annotations (line_sha256, call_id, kind, at, grader_kind, grader, dimension, verdict,
                score, rubric_version, keep, words)
             VALUES ($1, $2, $3, NULLIF($4, '')::timestamp, $5, $6, $7, $8, $9, $10, $11, $12)
             ON CONFLICT (line_sha256) DO NOTHING",
        )
        .bind(sha)
        .bind(&a.call_id)
        .bind(&a.kind)
        .bind(&a.at)
        .bind(&a.grader_kind)
        .bind(&a.grader)
        .bind(&a.dimension)
        .bind(&a.verdict)
        .bind(a.score)
        .bind(&a.rubric_version)
        .bind(a.keep)
        .bind(sealed)
        .execute(&self.pool)
        .await
        .map_err(|e| db("file an annotation", e))?;
        Ok(done.rows_affected() as usize)
    }

    /// File one annotation from the console or a grader (DATABASE.md §9). Its key is the hash of
    /// its fields, so a retry files it once. The call must be in this class.
    pub async fn add_annotation(&self, a: &Annotation) -> Result<bool, String> {
        if !self.has_call(&a.call_id).await? {
            return Err("call: no such call".into());
        }
        let canonical = serde_json::json!({
            "call": a.call_id, "kind": a.kind, "at": a.at, "grader_kind": a.grader_kind, "grader": a.grader,
            "dimension": a.dimension, "verdict": a.verdict, "score": a.score, "rubric_version": a.rubric_version,
            "keep": a.keep, "words": a.words.as_ref().map(|w| w.as_str()),
        });
        let sha = sha256_hex(canonical.to_string().as_bytes())?;
        Ok(self.insert_annotation(&sha, a).await? > 0)
    }

    /// Every annotation of the class, oldest first, its words opened (for the dev catalog).
    pub async fn list_annotations(&self) -> Result<Vec<Annotation>, String> {
        let rows = sqlx::query(
            "SELECT line_sha256, call_id, kind, COALESCE(to_char(at, 'YYYY-MM-DD\"T\"HH24:MI:SS'), '') AS at_text, grader_kind,
                grader, dimension, verdict, score, rubric_version, keep, words
             FROM annotations ORDER BY annotations.at NULLS FIRST, line_sha256",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db("read the annotations", e))?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let sha: String = r.get("line_sha256");
            let words: Option<Vec<u8>> = r.get("words");
            out.push(Annotation {
                call_id: r.get("call_id"),
                kind: r.get("kind"),
                at: r.get("at_text"),
                grader_kind: r.get("grader_kind"),
                grader: r.get("grader"),
                dimension: r.get("dimension"),
                verdict: r.get("verdict"),
                score: r.get("score"),
                rubric_version: r.get("rubric_version"),
                keep: r.get("keep"),
                words: words.map(|w| self.open("annotations", &sha, "words", &w)).transpose()?,
            });
        }
        Ok(out)
    }

    /// Analytics' sums (ANALYTICS.md §3.2; `stats::aggregate`): every call started in [`since`,
    /// `until`) that passes the filter, as the sums need it. Plain columns only.
    pub async fn stats_rows(&self, since: &str, until: &str, f: &crate::stats::Filter) -> Result<Vec<crate::stats::StatsRow>, String> {
        let rows = bind_filter(
            sqlx::query(concat!(
                "SELECT c.id, COALESCE(to_char(c.started, 'YYYY-MM-DD\"T\"HH24:MI:SS'), '') AS started_text, c.model,
                    c.prompt_version, c.system_version, c.hospital_version, c.outcome, c.end_reason, c.reached, c.cost_usd,
                    c.cartesia_credits, c.turn_waits_ms, c.turn_stages::text AS stages_text, c.requests_replayed,
                    COALESCE((SELECT array_agg(f.kind ORDER BY f.seq) FROM staff_flags f WHERE f.call_id = c.id), '{}') AS flag_kinds,
                    COALESCE((SELECT array_agg(f.source ORDER BY f.seq) FROM staff_flags f WHERE f.call_id = c.id), '{}') AS flag_sources,
                    COALESCE((SELECT array_agg(f.unverified ORDER BY f.seq) FROM staff_flags f WHERE f.call_id = c.id), '{}') AS flag_unverified,
                    COALESCE((SELECT array_agg(r.reason ORDER BY r.seq) FROM review_flags r WHERE r.call_id = c.id), '{}') AS review_flags
                 FROM calls c",
                stats_where!()
            )),
            since,
            until,
            f,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db("sum the calls", e))?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let kinds: Vec<String> = r.get("flag_kinds");
                let sources: Vec<String> = r.get("flag_sources");
                let unverified: Vec<i32> = r.get("flag_unverified");
                let stages: String = r.get("stages_text");
                let stages: Value = serde_json::from_str(&stages).unwrap_or(Value::Array(vec![]));
                crate::stats::StatsRow {
                    id: r.get("id"),
                    started: r.get("started_text"),
                    model: r.get("model"),
                    prompt_version: r.get("prompt_version"),
                    system_version: r.get("system_version"),
                    hospital_version: r.get("hospital_version"),
                    outcome: r.get("outcome"),
                    end_reason: r.get("end_reason"),
                    reached: r.get("reached"),
                    cost_usd: r.get("cost_usd"),
                    cartesia_credits: r.get("cartesia_credits"),
                    turn_waits_ms: r.get("turn_waits_ms"),
                    turn_stages: stages.as_array().map(|a| a.iter().map(Stages::from_json).collect()).unwrap_or_default(),
                    requests_replayed: r.get("requests_replayed"),
                    staff_flags: kinds.into_iter().zip(sources).zip(unverified).map(|((k, s), u)| (k, s, u)).collect(),
                    review_flags: r.get("review_flags"),
                }
            })
            .collect())
    }

    /// Analytics › Flags (ANALYTICS.md §3.2; the owner's A): one call's staff flags with their
    /// summary and evidence, opened. Protected data: the caller (service.rs › read_flags) holds
    /// PHI View and writes the audit row in the same request. None when the store has no call.
    pub async fn read_flags(&self, id: &str) -> Result<Option<Vec<FlagWords>>, String> {
        if !self.has_call(id).await? {
            return Ok(None);
        }
        let rows = sqlx::query("SELECT seq, kind, source, unverified, summary, evidence FROM staff_flags WHERE call_id = $1 ORDER BY seq")
            .bind(id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db("read the flags", e))?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let seq: i32 = r.get("seq");
            let row = format!("{id}/{seq}");
            let summary: Vec<u8> = r.get("summary");
            let evidence: Vec<u8> = r.get("evidence");
            let evidence = self.open("staff_flags", &row, "evidence", &evidence)?;
            let evidence: Vec<String> = serde_json::from_str::<Vec<Value>>(&evidence)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            out.push(FlagWords {
                seq,
                kind: r.get("kind"),
                source: r.get("source"),
                unverified: r.get("unverified"),
                summary: self.open("staff_flags", &row, "summary", &summary)?,
                evidence: evidence.into_iter().map(Zeroizing::new).collect(),
            });
        }
        Ok(Some(out))
    }

    /// The tool calls of the same calls as `stats_rows`: each tool, its own run, whether it failed.
    pub async fn stats_tools(&self, since: &str, until: &str, f: &crate::stats::Filter) -> Result<Vec<crate::stats::ToolRow>, String> {
        let rows = bind_filter(
            sqlx::query(concat!("SELECT t.tool, t.ms, t.failed FROM tool_calls t JOIN calls c ON c.id = t.call_id", stats_where!())),
            since,
            until,
            f,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db("sum the tool calls", e))?;
        Ok(rows.into_iter().map(|r| crate::stats::ToolRow { tool: r.get("tool"), ms: r.get("ms"), failed: r.get("failed") }).collect())
    }

    /// Analytics › Logs (ANALYTICS.md §3.1): the calls started in [`since`, `until`) (each a
    /// local date-time or a date; empty for no bound), newest first, at most `limit`. Plain
    /// columns and fixed words only, never a word of the call: the tools and flags are names, the
    /// waits are numbers. The label is a person's own words; it is given for a synthetic call
    /// only (a real call's label is read under PHI, not here).
    pub async fn list_calls(&self, since: &str, until: &str, limit: i64) -> Result<Vec<LoggedCall>, String> {
        let rows = sqlx::query(
            "SELECT c.id, COALESCE(to_char(c.started, 'YYYY-MM-DD\"T\"HH24:MI:SS'), '') AS started_text, c.origin, c.seat, c.ehr,
                c.mode, c.model, c.prompt_version, c.system_version, c.hospital_version, c.scenario, c.outcome, c.end_reason,
                c.reached, c.turns, c.duration_ms, c.cost_usd, c.turn_waits_ms, c.requests_live, c.requests_replayed,
                COALESCE((SELECT array_agg(t.tool ORDER BY t.seq) FROM tool_calls t WHERE t.call_id = c.id), '{}') AS tools,
                COALESCE((SELECT array_agg(t.failed ORDER BY t.seq) FROM tool_calls t WHERE t.call_id = c.id), '{}') AS tool_failed,
                COALESCE((SELECT array_agg(f.kind ORDER BY f.seq) FROM staff_flags f WHERE f.call_id = c.id), '{}') AS staff_flags,
                COALESCE((SELECT array_agg(r.reason ORDER BY r.seq) FROM review_flags r WHERE r.call_id = c.id), '{}') AS review_flags
             FROM calls c
             WHERE ($1 = '' OR c.started >= $1::timestamp) AND ($2 = '' OR c.started < $2::timestamp)
             ORDER BY c.started DESC NULLS LAST, c.id DESC
             LIMIT $3",
        )
        .bind(since)
        .bind(until)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db("list the calls", e))?;

        // The current label (the newest line wins, DATABASE.md §9) and the current ratings (the
        // newest per grader and dimension), for the calls listed.
        let ids: Vec<String> = rows.iter().map(|r| r.get::<String, _>("id")).collect();
        let mut labels: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        if self.class == DataClass::Synthetic && !ids.is_empty() {
            let found = sqlx::query(
                "SELECT DISTINCT ON (call_id) call_id, line_sha256, words FROM annotations
                 WHERE kind = 'label' AND call_id = ANY($1)
                 ORDER BY call_id, at DESC NULLS LAST, line_sha256 DESC",
            )
            .bind(&ids)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db("read the labels", e))?;
            for r in found {
                let words: Option<Vec<u8>> = r.get("words");
                if let Some(w) = words {
                    let sha: String = r.get("line_sha256");
                    let text = self.open("annotations", &sha, "words", &w)?;
                    if !text.trim().is_empty() {
                        labels.insert(r.get("call_id"), text.to_string());
                    }
                }
            }
        }
        let mut verdicts: std::collections::HashMap<String, (i32, i32)> = std::collections::HashMap::new();
        if !ids.is_empty() {
            let found = sqlx::query(
                "SELECT DISTINCT ON (call_id, grader, dimension) call_id, verdict FROM annotations
                 WHERE kind = 'rating' AND call_id = ANY($1)
                 ORDER BY call_id, grader, dimension, at DESC NULLS LAST, line_sha256 DESC",
            )
            .bind(&ids)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db("read the ratings", e))?;
            for r in found {
                let verdict: Option<String> = r.get("verdict");
                let n = verdicts.entry(r.get("call_id")).or_default();
                match verdict.as_deref() {
                    Some("pass") => n.0 += 1,
                    Some("fail") => n.1 += 1,
                    _ => {}
                }
            }
        }

        Ok(rows
            .into_iter()
            .map(|r| {
                let id: String = r.get("id");
                let (passed, failed) = verdicts.get(&id).copied().unwrap_or((0, 0));
                LoggedCall {
                    label: labels.remove(&id),
                    ratings_passed: passed,
                    ratings_failed: failed,
                    started: r.get("started_text"),
                    origin: r.get("origin"),
                    seat: r.get("seat"),
                    ehr: r.get("ehr"),
                    mode: r.get("mode"),
                    model: r.get("model"),
                    prompt_version: r.get("prompt_version"),
                    system_version: r.get("system_version"),
                    hospital_version: r.get("hospital_version"),
                    scenario: r.get("scenario"),
                    outcome: r.get("outcome"),
                    end_reason: r.get("end_reason"),
                    reached: r.get("reached"),
                    turns: r.get("turns"),
                    duration_ms: r.get("duration_ms"),
                    cost_usd: r.get("cost_usd"),
                    turn_waits_ms: r.get("turn_waits_ms"),
                    requests_live: r.get("requests_live"),
                    requests_replayed: r.get("requests_replayed"),
                    tools: r.get("tools"),
                    tool_failed: r.get("tool_failed"),
                    staff_flags: r.get("staff_flags"),
                    review_flags: r.get("review_flags"),
                    id,
                }
            })
            .collect())
    }

    /// File one run of the kept scripts (DATABASE.md §8), decide which draws' records the store
    /// needs (option C, §8.2), and run the kept-record retention (§8.6). Synthetic only.
    pub async fn file_test_run(&self, run: &TestRun) -> Result<FiledRun, String> {
        if self.class != DataClass::Synthetic {
            return Err("a test run is synthetic only".into());
        }
        let mut tx = self.pool.begin().await.map_err(|e| db("begin a test run", e))?;
        let mut scripts: Vec<&str> = run.results.iter().map(|r| r.script.as_str()).collect();
        scripts.sort_unstable();
        scripts.dedup();
        let failed = scripts.iter().filter(|s| run.results.iter().any(|r| r.script == **s && !r.passed)).count();
        let run_id: i64 = sqlx::query_scalar(
            "INSERT INTO test_runs (started, finished, run_by, words_sha256, prompt_version, system_version, hospital_version,
                llm_version, model, git_commit, scripts, passed, failed, tests_passed, cost_usd)
             VALUES ($1::timestamptz, NULLIF($2, '')::timestamptz, $3, $4, NULLIF($5, ''), NULLIF($6, ''), NULLIF($7, ''),
                NULLIF($8, ''), NULLIF($9, ''), NULLIF($10, ''), $11, $12, $13, $14, $15)
             RETURNING id",
        )
        .bind(&run.started)
        .bind(&run.finished)
        .bind(&run.run_by)
        .bind(&run.words_sha256)
        .bind(&run.prompt_version)
        .bind(&run.system_version)
        .bind(&run.hospital_version)
        .bind(&run.llm_version)
        .bind(&run.model)
        .bind(&run.git_commit)
        .bind(scripts.len() as i32)
        .bind((scripts.len() - failed) as i32)
        .bind(failed as i32)
        .bind(run.tests_passed)
        .bind(run.cost_usd)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db("file a test run", e))?;

        let mut needed = Vec::new();
        for r in &run.results {
            // Option C: a passing replay draw that makes the same call as the script's last filed
            // draw points to that record and files none.
            let earlier: Option<String> = if r.mode == "replay" && r.passed && !r.shape_sha256.is_empty() {
                sqlx::query_scalar(
                    "SELECT r.call_id FROM test_results r JOIN calls c ON c.id = r.call_id
                     WHERE r.script = $1 AND r.shape_sha256 = $2 ORDER BY r.run_id DESC LIMIT 1",
                )
                .bind(&r.script)
                .bind(&r.shape_sha256)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| db("find an earlier draw", e))?
            } else {
                None
            };
            let reused = earlier.is_some();
            let call_id = match earlier {
                Some(id) => Some(id),
                None if !r.call_id.is_empty() => {
                    if origin_of(&r.call_id) != Some("kept") {
                        return Err("a draw's call id does not start with kept_".into());
                    }
                    needed.push(r.call_id.clone());
                    Some(r.call_id.clone())
                }
                None => None,
            };
            let row = format!("{run_id}/{}/{}/{}", r.script, r.draw, r.mode);
            let failure = r.failure.as_ref().map(|f| self.seal("test_results", &row, "failure", f)).transpose()?;
            sqlx::query(
                "INSERT INTO test_results (run_id, script, draw, mode, passed, test, failure, call_id, reused, shape_sha256,
                    duration_ms, tool_ms, live_requests, cost_usd, tags)
                 VALUES ($1, $2, $3, $4, $5, NULLIF($6, ''), $7, $8, $9, NULLIF($10, ''), $11, NULLIF($12, '')::jsonb, $13, $14,
                    COALESCE(NULLIF($15, '')::jsonb, '{}'::jsonb))",
            )
            .bind(run_id)
            .bind(&r.script)
            .bind(r.draw)
            .bind(&r.mode)
            .bind(r.passed)
            .bind(&r.test)
            .bind(failure)
            .bind(&call_id)
            .bind(reused)
            .bind(&r.shape_sha256)
            .bind(r.duration_ms)
            .bind(&r.tool_ms)
            .bind(r.live_requests)
            .bind(r.cost_usd)
            .bind(&r.tags)
            .execute(&mut *tx)
            .await
            .map_err(|e| db("file a draw", e))?;
            for key in &r.reply_keys {
                sqlx::query("INSERT INTO test_replies (run_id, script, reply_key) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
                    .bind(run_id)
                    .bind(&r.script)
                    .bind(key)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| db("file a reply key", e))?;
            }
        }
        tx.commit().await.map_err(|e| db("commit a test run", e))?;
        let pruned = self.prune_kept(&needed).await?;
        Ok(FiledRun { run_id, needed, pruned })
    }

    /// The kept-record retention (DATABASE.md §8.6): delete a `kept` call that no draw of a run
    /// with one of the newest words versions points to, unless it is starred or about to be
    /// filed. Synthetic only; never another origin.
    pub async fn prune_kept(&self, keep_ids: &[String]) -> Result<usize, String> {
        let done = sqlx::query(
            "WITH recent AS (
                 SELECT words_sha256 FROM test_runs GROUP BY words_sha256 ORDER BY max(started) DESC LIMIT $1
             ), used AS (
                 SELECT r.call_id FROM test_results r JOIN test_runs u ON u.id = r.run_id
                 WHERE u.words_sha256 IN (SELECT words_sha256 FROM recent) AND r.call_id IS NOT NULL
             ), starred AS (
                 SELECT call_id FROM (
                     SELECT DISTINCT ON (call_id) call_id, keep FROM annotations WHERE kind = 'keep'
                     ORDER BY call_id, at DESC NULLS LAST
                 ) k WHERE keep
             )
             DELETE FROM calls c WHERE c.origin = 'kept'
               AND c.id NOT IN (SELECT call_id FROM used)
               AND c.id NOT IN (SELECT call_id FROM starred)
               AND NOT (c.id = ANY($2))",
        )
        .bind(KEPT_WORDS_VERSIONS)
        .bind(keep_ids.to_vec())
        .execute(&self.pool)
        .await
        .map_err(|e| db("prune the kept records", e))?;
        Ok(done.rows_affected() as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_turns_wait_runs_from_its_start_to_the_answer() {
        // A typed call: from the caller's line to the agent's first frame; the greeting is no turn.
        let typed = vec![
            json!({"t_ms": 0, "kind": "call_start", "seat": "text", "ehr": "local"}),
            json!({"t_ms": 900, "kind": "agent", "first_ms": 400}),
            json!({"t_ms": 1000, "kind": "caller", "text": "hi"}),
            json!({"t_ms": 2500, "kind": "agent", "first_ms": 1700}),
            json!({"t_ms": 3000, "kind": "caller", "text": "no answer"}),
        ];
        assert_eq!(turn_waits(&typed), vec![700.0]);
        // A late agent line whose first frame came before the next caller line closes nothing.
        let late = vec![
            json!({"t_ms": 1000, "kind": "caller", "text": "a"}),
            json!({"t_ms": 1500, "kind": "caller", "text": "b"}),
            json!({"t_ms": 1600, "kind": "agent", "first_ms": 1400}),
            json!({"t_ms": 2600, "kind": "agent", "first_ms": 2100}),
        ];
        assert_eq!(turn_waits(&late), vec![600.0]);
        let s = summarize(&typed);
        assert_eq!((s.seat.as_deref(), s.ehr.as_deref()), (Some("text"), Some("local")));

        // A voice call: from the caller's silence to the seat speaking, in either order of Ink's
        // final line and the caller event; a typed line in it starts at the line.
        let voice = vec![
            json!({"t_ms": 0, "kind": "call_start", "seat": "voice"}),
            json!({"t_ms": 500, "kind": "voice", "stage": "speaking"}),
            json!({"t_ms": 2000, "kind": "voice", "stage": "heard", "last_words_ms": 1600}),
            json!({"t_ms": 2005, "kind": "caller", "text": "a"}),
            json!({"t_ms": 2400, "kind": "agent", "first_ms": 2300}),
            json!({"t_ms": 2600, "kind": "voice", "stage": "speaking"}),
            json!({"t_ms": 5000, "kind": "caller", "text": "b"}),
            json!({"t_ms": 5004, "kind": "voice", "stage": "heard", "last_words_ms": 4700}),
            json!({"t_ms": 5900, "kind": "voice", "stage": "speaking"}),
            json!({"t_ms": 8000, "kind": "caller", "text": "typed"}),
            json!({"t_ms": 8450, "kind": "voice", "stage": "speaking"}),
        ];
        assert_eq!(turn_waits(&voice), vec![1000.0, 1200.0, 450.0]);

        // The stages of a voice turn add up to its wait: Claude's stream past the first frame is
        // Voice out, not Claude as well.
        let staged = vec![
            json!({"t_ms": 0, "kind": "call_start", "seat": "voice"}),
            json!({"t_ms": 2000, "kind": "voice", "stage": "heard", "last_words_ms": 1600}),
            json!({"t_ms": 2005, "kind": "caller", "text": "a"}),
            json!({"t_ms": 2100, "kind": "tool_call", "name": "find_slots"}),
            json!({"t_ms": 2400, "kind": "ehr", "call": "FindSlots", "ms": 250}),
            json!({"t_ms": 2450, "kind": "tool_result", "name": "find_slots", "ms": 350}),
            json!({"t_ms": 2700, "kind": "voice", "stage": "speaking"}),
            json!({"t_ms": 2750, "kind": "agent", "first_ms": 2500}),
            json!({"t_ms": 2900, "kind": "llm", "total_ms": 400}),
        ];
        let st = turn_stages(&staged);
        assert_eq!(st.len(), 1);
        let s = st[0];
        assert_eq!((s.ink, s.ehr, s.tools, s.voice), (400.0, 250.0, 100.0, 200.0));
        assert_eq!(s.claude, 0.0, "the request ran from 2500, the first frame: all of it is Voice out");
        let sum = s.ink + s.claude + s.tools + s.ehr + s.voice + s.other;
        assert!((sum - turn_waits(&staged)[0]).abs() < 0.01, "the parts add up to the wait");
    }

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
        assert_eq!((s.origin.as_str(), s.data_class.as_str()), ("test", "synthetic"));
    }

    #[test]
    fn an_empty_record_is_abandoned() {
        assert_eq!(summarize(&[]).outcome, "abandoned");
    }

    #[test]
    fn a_call_id_starts_with_its_origin() {
        assert_eq!(origin_of("kept_20261115-201500"), Some("kept"));
        assert_eq!(origin_of("live_20261115-201500-2"), Some("live"));
        assert_eq!(origin_of("20261115-201500-test"), None, "an id from before the prefixes");
        assert_eq!(origin_of("demo_20261115"), None, "not an origin");
        assert_eq!(origin_of("test_"), None);
    }

    #[test]
    fn a_record_names_its_class() {
        let real = b"{\"kind\": \"call_start\", \"source\": \"live\", \"data_class\": \"real\"}\n";
        assert_eq!(record_class(real).unwrap(), DataClass::Real);
        let old = b"{\"kind\": \"call_start\", \"source\": \"typed\"}\r\n{\"kind\": \"caller\"}\n";
        assert_eq!(record_class(old).unwrap(), DataClass::Synthetic, "a record before schema 14 is synthetic");
        assert!(record_class(b"{\"kind\": \"call_start\", \"data_class\": \"other\"}").is_err());
    }
}
