//! The settings store (CONSOLE.md §7.3): versions of each service's settings in Postgres,
//! insert-only (`migrations/0002_settings.sql`).
//!
//! - **One list of every setting** per service ([`VOICE`]): its name and what it may hold. Every
//!   save is checked against it here, so no value outside a range or of the wrong kind is ever
//!   stored. The agent's `config.py` holds the same ranges for the Config page; a change to one
//!   is a change to both (CONSOLE.md §7.3).
//! - **A version holds every setting.** A save sends the whole set, each setting once; it
//!   becomes a new version, numbered within its day, and the active one.
//! - **Errors name the setting, never its value** (SECURITY.md §4.5). No setting is protected
//!   data, but the rule is the same everywhere.

use serde_json::Value;
use sqlx::{PgPool, Row};

/// What one setting may hold.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// A number in a range; `null` is the provider's default.
    Number { min: f64, max: f64, nullable: bool },
    /// A whole number in a range.
    Whole { min: i64, max: i64 },
    Bool,
    /// One of a few words; `null` is the provider's default when `nullable`.
    Word { words: &'static [&'static str], nullable: bool },
    /// The voice: `{"name": …, "id": …}`, both short strings.
    Voice,
    /// A table of rows of `columns` short strings, at most `rows` rows and `chars` characters in
    /// all.
    Table { columns: usize, rows: usize, chars: usize },
}

pub struct Setting {
    pub key: &'static str,
    pub kind: Kind,
}

const fn s(key: &'static str, kind: Kind) -> Setting {
    Setting { key, kind }
}

/// The voice service: Cartesia (CARTESIA.md §9). The ranges are Cartesia's.
pub const VOICE: &[Setting] = &[
    s("voice", Kind::Voice),
    s("speed", Kind::Number { min: 0.6, max: 1.5, nullable: true }),
    s("volume", Kind::Number { min: 0.5, max: 2.0, nullable: true }),
    s("emotion", Kind::Word { words: &["neutral", "calm", "content", "sad", "angry"], nullable: true }),
    s("normalization", Kind::Word { words: &["auto", "off"], nullable: false }),
    // Each cell is a short string; 200 rows at most.
    s("pronunciations", Kind::Table { columns: 2, rows: 200, chars: 200 * 2 * SHORT }),
    s("turn_start_threshold", Kind::Number { min: 0.5, max: 0.9, nullable: true }),
    s("turn_eager_end_threshold", Kind::Number { min: 0.3, max: 0.8, nullable: true }),
    s("turn_end_threshold", Kind::Number { min: 0.05, max: 0.5, nullable: true }),
    s("turn_end_timeout_ms", Kind::Number { min: 640.0, max: 11200.0, nullable: true }),
    s("eager_end_of_turn", Kind::Bool),
    s("barge_in", Kind::Bool),
    // Cartesia: up to 100 keyterms, 1,200 characters in all (checked below).
    s("keyterms", Kind::Table { columns: 1, rows: 100, chars: 1200 }),
    s("idle_s", Kind::Whole { min: 10, max: 600 }),
    s("max_s", Kind::Whole { min: 60, max: 3600 }),
    s("plan_credits", Kind::Whole { min: 0, max: 1_000_000_000 }),
    s("offset", Kind::Whole { min: 0, max: 1_000_000_000 }),
    s("cycle_day", Kind::Whole { min: 1, max: 28 }),
];

/// A service's name, its list and its table. The names come from here only, never from a
/// request, so no request text reaches the SQL.
pub fn service(name: &str) -> Option<(&'static str, &'static [Setting], &'static str)> {
    match name {
        "voice" => Some(("voice", VOICE, "settings_voice")),
        _ => None,
    }
}

/// The longest short string a setting holds (a voice name or id).
const SHORT: usize = 64;

fn short(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty() && s.chars().count() <= SHORT)
}

/// Whether `value` fits `kind`.
pub fn fits(kind: Kind, value: &Value) -> bool {
    match kind {
        Kind::Number { min, max, nullable } => match value {
            Value::Null => nullable,
            v => v.as_f64().is_some_and(|x| x.is_finite() && x >= min - 1e-9 && x <= max + 1e-9),
        },
        Kind::Whole { min, max } => value.as_i64().is_some_and(|x| (min..=max).contains(&x)),
        Kind::Bool => value.is_boolean(),
        Kind::Word { words, nullable } => match value {
            Value::Null => nullable,
            v => v.as_str().is_some_and(|w| words.contains(&w)),
        },
        Kind::Voice => {
            value.as_object().is_some_and(|o| o.len() == 2 && o.get("name").is_some_and(short) && o.get("id").is_some_and(short))
        }
        Kind::Table { columns, rows, chars } => {
            let Some(list) = value.as_array() else { return false };
            if list.len() > rows {
                return false;
            }
            let mut total = 0usize;
            for row in list {
                let Some(cells) = row.as_array() else { return false };
                if cells.len() != columns || !cells.iter().all(short) {
                    return false;
                }
                total += cells.iter().map(|c| c.as_str().unwrap_or("").chars().count()).sum::<usize>();
            }
            total <= chars
        }
    }
}

/// Check a whole set: every setting of the service, once each, each value fitting its kind.
/// The error names the first setting that does not fit.
pub fn check(list: &[Setting], values: &[(String, String)]) -> Result<Vec<(&'static str, Value)>, String> {
    let mut out = Vec::with_capacity(list.len());
    for (key, _) in values {
        if !list.iter().any(|s| s.key == key) {
            return Err(format!("{}: not a setting of this service", if key.len() <= SHORT && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { key.as_str() } else { "a setting" }));
        }
    }
    for setting in list {
        let mut found = values.iter().filter(|(k, _)| k == setting.key);
        let Some((_, raw)) = found.next() else {
            return Err(format!("{}: missing", setting.key));
        };
        if found.next().is_some() {
            return Err(format!("{}: given twice", setting.key));
        }
        let value: Value = serde_json::from_str(raw).map_err(|_| format!("{}: not JSON", setting.key))?;
        if !fits(setting.kind, &value) {
            return Err(format!("{}: out of range or the wrong kind", setting.key));
        }
        out.push((setting.key, value));
    }
    Ok(out)
}

/// One saved version.
#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    pub id: i64,
    pub day: String,
    pub seq: i32,
    pub created_at: String,
    pub created_by: String,
    pub active: bool,
}

fn db(step: &str, e: sqlx::Error) -> String {
    let code = e.as_database_error().and_then(|d| d.code().map(|c| c.to_string())).unwrap_or_default();
    format!("settings: {step} failed {code}").trim_end().to_string()
}

const VERSION_COLUMNS: &str = "v.id, to_char(v.day, 'YYYY-MM-DD') AS day, v.seq, \
    to_char(v.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, v.created_by, \
    (v.id = (SELECT a.version_id FROM setting_activations a WHERE a.service = v.service ORDER BY a.id DESC LIMIT 1)) AS active";

fn version(row: &sqlx::postgres::PgRow) -> Version {
    Version {
        id: row.get("id"),
        day: row.get("day"),
        seq: row.get("seq"),
        created_at: row.get("created_at"),
        created_by: row.get("created_by"),
        active: row.get::<Option<bool>, _>("active").unwrap_or(false),
    }
}

/// Every version of a service, newest first, the Default version (number 0) last.
pub async fn versions(pool: &PgPool, service: &str) -> Result<Vec<Version>, String> {
    let rows = sqlx::query(&format!(
        "SELECT {VERSION_COLUMNS} FROM setting_versions v WHERE v.service = $1 ORDER BY (v.seq = 0), v.id DESC"
    ))
        .bind(service)
        .fetch_all(pool)
        .await
        .map_err(|e| db("list", e))?;
    Ok(rows.iter().map(version).collect())
}

/// One version and its values: the active one when `id` is 0. None when there is none.
pub async fn get(pool: &PgPool, service: &str, table: &str, id: i64) -> Result<Option<(Version, Vec<(String, String)>)>, String> {
    let row = if id == 0 {
        sqlx::query(&format!(
            "SELECT {VERSION_COLUMNS} FROM setting_versions v WHERE v.service = $1 AND v.id = \
             (SELECT a.version_id FROM setting_activations a WHERE a.service = $1 ORDER BY a.id DESC LIMIT 1)"
        ))
        .bind(service)
        .fetch_optional(pool)
        .await
    } else {
        sqlx::query(&format!("SELECT {VERSION_COLUMNS} FROM setting_versions v WHERE v.service = $1 AND v.id = $2"))
            .bind(service)
            .bind(id)
            .fetch_optional(pool)
            .await
    }
    .map_err(|e| db("get", e))?;
    let Some(row) = row else { return Ok(None) };
    let v = version(&row);
    let values = sqlx::query(&format!("SELECT key, value::text AS value FROM {table} WHERE version_id = $1 ORDER BY key"))
        .bind(v.id)
        .fetch_all(pool)
        .await
        .map_err(|e| db("values", e))?
        .iter()
        .map(|r| (r.get::<String, _>("key"), r.get::<String, _>("value")))
        .collect();
    Ok(Some((v, values)))
}

/// Why a save did not happen.
#[derive(Debug, PartialEq)]
pub enum SaveError {
    /// A seed, and the service has its Default version already.
    DefaultExists,
    Db(String),
}

/// Save a checked set as a new version, in one transaction. A plain save is the day's next
/// number and becomes the active one. A `seed` is the Default version (number 0, one per
/// service), active only when nothing is active yet. The day's number comes under a lock, so two
/// saves at once never share one.
pub async fn save(
    pool: &PgPool,
    service: &str,
    table: &str,
    values: &[(&str, Value)],
    by: &str,
    seed: bool,
) -> Result<Version, SaveError> {
    let mut tx = pool.begin().await.map_err(|e| SaveError::Db(db("begin", e)))?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('deets.settings'))")
        .execute(&mut *tx)
        .await
        .map_err(|e| SaveError::Db(db("lock", e)))?;
    let has_default: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM setting_versions WHERE service = $1 AND seq = 0)")
        .bind(service)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| SaveError::Db(db("default", e)))?;
    if seed && has_default {
        return Err(SaveError::DefaultExists);
    }
    let has_active: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM setting_activations WHERE service = $1)")
        .bind(service)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| SaveError::Db(db("active", e)))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO setting_versions (service, day, seq, created_by) \
         SELECT $1, CURRENT_DATE, CASE WHEN $3 THEN 0 ELSE COALESCE(MAX(seq), 0) + 1 END, $2 \
         FROM setting_versions WHERE service = $1 AND day = CURRENT_DATE \
         RETURNING id",
    )
    .bind(service)
    .bind(by)
    .bind(seed)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| SaveError::Db(db("version", e)))?;
    let activate = !seed || !has_active;
    for (key, value) in values {
        sqlx::query(&format!("INSERT INTO {table} (version_id, key, value) VALUES ($1, $2, $3::jsonb)"))
            .bind(id)
            .bind(key)
            .bind(value.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| SaveError::Db(db("value", e)))?;
    }
    if activate {
        sqlx::query("INSERT INTO setting_activations (service, version_id, by_who) VALUES ($1, $2, $3)")
            .bind(service)
            .bind(id)
            .bind(by)
            .execute(&mut *tx)
            .await
            .map_err(|e| SaveError::Db(db("activate", e)))?;
    }
    tx.commit().await.map_err(|e| SaveError::Db(db("commit", e)))?;
    let (v, _) = get(pool, service, table, id)
        .await
        .map_err(SaveError::Db)?
        .ok_or_else(|| SaveError::Db("settings: the new version is missing".to_string()))?;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn full() -> Vec<(String, String)> {
        let v = |k: &str, x: Value| (k.to_string(), x.to_string());
        vec![
            v("voice", json!({"name": "Skylar", "id": "db6b0ed5-d5d3-463d-ae85-518a07d3c2b4"})),
            v("speed", json!(null)),
            v("volume", json!(1.2)),
            v("emotion", json!("calm")),
            v("normalization", json!("auto")),
            v("pronunciations", json!([["Lisinopril", "lie-SIN-oh-pril"]])),
            v("turn_start_threshold", json!(0.7)),
            v("turn_eager_end_threshold", json!(null)),
            v("turn_end_threshold", json!(0.4)),
            v("turn_end_timeout_ms", json!(4500)),
            v("eager_end_of_turn", json!(false)),
            v("barge_in", json!(true)),
            v("keyterms", json!([["Main Street Clinic"]])),
            v("idle_s", json!(60)),
            v("max_s", json!(360)),
            v("plan_credits", json!(20000)),
            v("offset", json!(270)),
            v("cycle_day", json!(12)),
        ]
    }

    #[test]
    fn a_whole_set_passes() {
        assert_eq!(check(VOICE, &full()).unwrap().len(), VOICE.len());
    }

    #[test]
    fn each_refusal_names_the_setting() {
        let mut set = full();
        set.pop();
        assert_eq!(check(VOICE, &set).unwrap_err(), "cycle_day: missing");

        let mut set = full();
        set.push(("speed".into(), "1.0".into()));
        assert_eq!(check(VOICE, &set).unwrap_err(), "speed: given twice");

        let mut set = full();
        set.push(("colour".into(), "1".into()));
        assert_eq!(check(VOICE, &set).unwrap_err(), "colour: not a setting of this service");

        let mut set = full();
        set[1].1 = "1.6".into(); // speed above 1.5
        assert_eq!(check(VOICE, &set).unwrap_err(), "speed: out of range or the wrong kind");

        let mut set = full();
        set[17].1 = "29".into(); // cycle_day above 28
        assert!(check(VOICE, &set).unwrap_err().starts_with("cycle_day:"));
    }

    #[test]
    fn the_kinds() {
        assert!(fits(Kind::Number { min: 0.5, max: 0.9, nullable: true }, &json!(null)));
        assert!(!fits(Kind::Word { words: &["auto", "off"], nullable: false }, &json!(null)));
        assert!(!fits(Kind::Voice, &json!({"name": "Skylar"})));
        assert!(!fits(Kind::Table { columns: 2, rows: 1, chars: 64 }, &json!([["a", "b"], ["c", "d"]])));
        assert!(!fits(Kind::Table { columns: 1, rows: 100, chars: 1200 }, &json!([["   "]])));
        let long: Vec<Value> = (0..30).map(|_| json!(["x".repeat(50)])).collect();
        assert!(!fits(Kind::Table { columns: 1, rows: 100, chars: 1200 }, &Value::Array(long)), "1,500 characters");
    }
}
