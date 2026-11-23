//! DeetsHealth's vendors (docs/architecture/VOICE-VENDORS.md): every outside service the agent
//! pays for, as data and as the functions that read it. One crate, by the owner's word
//! (2026-11-23, "A: one vendors crate").
//!
//! - [`voice`]: the voice vendors (hearing, speaking), their levers, and the voice's cost.
//! - [`model`]: the model vendors (Anthropic today), their models, prices and rules, and the
//!   model's cost.
//! - [`cycle`]: a plan's billing cycle, shared by both.
//!
//! The catalogs are JSON files in `catalog/`, compiled in. `ehr` checks every save against them
//! and serves them to the agent and the console (`GetVoiceCatalog`, `GetModelCatalog`); the agent
//! prices nothing itself (`PriceVoiceUse`, `PriceModelUse`). No value here is protected data.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod cycle;
pub mod model;
pub mod voice;

/// What one lever may hold.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Kind {
    /// A number in a range; `null` is the vendor's default when `nullable`.
    Number {
        min: f64,
        max: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        step: Option<f64>,
        #[serde(default)]
        nullable: bool,
    },
    /// A whole number in a range.
    Whole { min: i64, max: i64 },
    Bool,
    /// One of a few words; `null` is the vendor's default when `nullable`.
    Word {
        words: Vec<String>,
        #[serde(default)]
        nullable: bool,
    },
    /// A voice: `{"name": …, "id": …}`.
    Voice,
    /// Rows of `columns` short strings: at most `rows` rows, `chars` characters in all.
    Table { columns: usize, rows: usize, chars: usize },
}

/// One lever: its key, what it holds, and the vendor's default.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Lever {
    pub key: String,
    #[serde(flatten)]
    pub kind: Kind,
    /// The vendor's own default. For a nullable lever, the value a version starts with is
    /// `null` (we send nothing), and this is where the page marks the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

impl Lever {
    /// The value a new version holds for this lever.
    pub fn initial(&self) -> Value {
        let nullable = match &self.kind {
            Kind::Number { nullable, .. } | Kind::Word { nullable, .. } => *nullable,
            _ => false,
        };
        if nullable {
            return Value::Null;
        }
        match (&self.default, &self.kind) {
            (Some(v), _) => v.clone(),
            (None, Kind::Table { .. }) => Value::Array(vec![]),
            (None, Kind::Bool) => Value::Bool(false),
            (None, Kind::Whole { min, .. }) => Value::from(*min),
            (None, Kind::Number { min, .. }) => Value::from(*min),
            (None, Kind::Word { words, .. }) => Value::from(words.first().cloned().unwrap_or_default()),
            (None, Kind::Voice) => Value::Null,
        }
    }
}

/// A lever's own rules: a key of a to z and _, a range the right way round, and a default
/// that fits.
pub(crate) fn lever_ok(lever: &Lever) -> Result<(), String> {
    if lever.key.is_empty() || !lever.key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err("the key is a to z, 0 to 9 and _".into());
    }
    match &lever.kind {
        Kind::Number { min, max, .. } if min > max => return Err("min above max".into()),
        Kind::Whole { min, max } if min > max => return Err("min above max".into()),
        Kind::Word { words, .. } if words.is_empty() => return Err("no words".into()),
        _ => {}
    }
    if let Some(d) = &lever.default {
        if !fits(&lever.kind, d) {
            return Err("the default does not fit".into());
        }
    }
    if !fits(&lever.kind, &lever.initial()) {
        return Err("needs a default".into());
    }
    Ok(())
}

/// The longest short string a lever holds (a voice's name or id, a table's cell).
pub const SHORT: usize = 64;

fn short(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty() && s.chars().count() <= SHORT)
}

/// Whether `value` fits `kind`.
pub fn fits(kind: &Kind, value: &Value) -> bool {
    match kind {
        Kind::Number { min, max, nullable, .. } => match value {
            Value::Null => *nullable,
            v => v.as_f64().is_some_and(|x| x.is_finite() && x >= min - 1e-9 && x <= max + 1e-9),
        },
        Kind::Whole { min, max } => value.as_i64().is_some_and(|x| (*min..=*max).contains(&x)),
        Kind::Bool => value.is_boolean(),
        Kind::Word { words, nullable } => match value {
            Value::Null => *nullable,
            v => v.as_str().is_some_and(|w| words.iter().any(|x| x == w)),
        },
        Kind::Voice => value.as_object().is_some_and(|o| o.len() == 2 && o.get("name").is_some_and(short) && o.get("id").is_some_and(short)),
        Kind::Table { columns, rows, chars } => {
            let Some(list) = value.as_array() else { return false };
            if list.len() > *rows {
                return false;
            }
            let mut total = 0usize;
            for row in list {
                let Some(cells) = row.as_array() else { return false };
                if cells.len() != *columns || !cells.iter().all(short) {
                    return false;
                }
                total += cells.iter().map(|c| c.as_str().unwrap_or("").chars().count()).sum::<usize>();
            }
            total <= *chars
        }
    }
}

