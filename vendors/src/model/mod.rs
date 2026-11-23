//! The model catalog (docs/architecture/VOICE-VENDORS.md §10): each model vendor (Anthropic today),
//! its models with their prices, the efforts, how thinking turns off, the cache and inference
//! factors, and the levers of the `llm` settings. One JSON file per vendor in `catalog/model/`.
//!
//! The `llm` settings' keys stay as they were (`model`, `effort`, `thinking`, `max_tokens`, …):
//! there is one model vendor. A second one (OpenAI, say) adds `model.vendor` and its own keys
//! the way the voice did (`voice::upgrade`). No value here is protected data.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{fits, lever_ok, Kind, Lever};

pub mod cost;

/// The model vendors. The first is the one the agent runs with.
pub const VENDOR_FILES: &[&str] = &[include_str!("../../catalog/model/anthropic.json")];

/// Dollars per 1M tokens of each kind.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Prices {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
}

/// One model.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// Pinned by name; never a "latest" alias.
    pub id: String,
    pub name: String,
    /// The API's own effort when none is sent.
    pub effort_default: String,
    /// What the API calls thinking turned off on this model; none when it cannot be.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_off: Option<String>,
    pub prices: Prices,
}

/// The code's starting values of the two settings that are lists, not levers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    pub model: String,
    pub effort: String,
}

/// One model vendor.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Vendor {
    pub id: String,
    pub name: String,
    /// The name of its key in `.env` (never the key).
    pub key_env: String,
    pub baa: String,
    #[serde(default, skip_serializing)]
    pub about: String,
    pub models: Vec<Model>,
    pub efforts: Vec<String>,
    /// The efforts at which thinking may be off, on a model that can turn it off.
    pub thinking_off_efforts: Vec<String>,
    /// A cache write costs this many times the input price, by the cache's life.
    pub cache_write: BTreeMap<String, f64>,
    /// Where inference runs (`inference_geo`), and what each place costs over the list price.
    pub inference_geo: String,
    pub geo_price: BTreeMap<String, f64>,
    pub defaults: Defaults,
    pub levers: Vec<Lever>,
}

impl Vendor {
    pub fn model(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }
}

/// The whole model catalog.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Catalog {
    pub vendors: Vec<Vendor>,
}

impl Catalog {
    /// The vendor the agent runs with (the first).
    pub fn active(&self) -> &Vendor {
        &self.vendors[0]
    }
}

/// The levers every model vendor's file holds: the `llm` settings and cost.rs read them by name.
pub const LEVERS: &[&str] = &["thinking", "max_tokens", "retry_max_tokens", "cache_ttl", "budget_usd", "offset_usd", "cycle_day"];

/// Parse and check the catalog. A bad file fails the crate's tests, and `catalog()` at start.
pub fn parse(files: &[&str]) -> Result<Catalog, String> {
    let mut vendors: Vec<Vendor> = vec![];
    for (i, file) in files.iter().enumerate() {
        let v: Vendor = serde_json::from_str(file).map_err(|e| format!("model file {i}: {e}"))?;
        let id = v.id.clone();
        if v.models.is_empty() {
            return Err(format!("{id}: no models"));
        }
        if v.model(&v.defaults.model).is_none() {
            return Err(format!("{id}.defaults.model: not one of its models"));
        }
        for e in std::iter::once(&v.defaults.effort).chain(&v.thinking_off_efforts).chain(v.models.iter().map(|m| &m.effort_default)) {
            if !v.efforts.contains(e) {
                return Err(format!("{id}: the effort {e} is not in its efforts"));
            }
        }
        for k in LEVERS {
            if !v.levers.iter().any(|l| l.key == *k) {
                return Err(format!("{id}.{k}: the file needs it"));
            }
        }
        for l in &v.levers {
            lever_ok(l).map_err(|e| format!("{id}.{}: {e}", l.key))?;
        }
        if let Some(Kind::Word { words, .. }) = v.levers.iter().find(|l| l.key == "cache_ttl").map(|l| &l.kind) {
            if let Some(w) = words.iter().find(|w| !v.cache_write.contains_key(*w)) {
                return Err(format!("{id}.cache_ttl: {w} has no cache_write factor"));
            }
        }
        if !v.geo_price.contains_key(&v.inference_geo) {
            return Err(format!("{id}.inference_geo: no geo_price"));
        }
        vendors.push(v);
    }
    if vendors.is_empty() {
        return Err("no model vendor".into());
    }
    Ok(Catalog { vendors })
}

/// The catalog, parsed once.
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| parse(VENDOR_FILES).expect("the model catalog"))
}

/// The catalog as JSON for `GetModelCatalog`, with `defaults`: every `llm` setting's first value.
pub fn catalog_json() -> &'static str {
    static JSON: OnceLock<String> = OnceLock::new();
    JSON.get_or_init(|| {
        let c = catalog();
        let mut out = serde_json::to_value(c).expect("the model catalog as JSON");
        out["defaults"] = Value::Object(defaults(c).into_iter().collect());
        out.to_string()
    })
}

/// Every `llm` setting and what it may hold, in the page's order: the model, the effort, then
/// the vendor's levers.
pub fn settings(c: &Catalog) -> Vec<(String, Kind)> {
    let v = c.active();
    let mut out = vec![
        ("model".to_string(), Kind::Word { words: v.models.iter().map(|m| m.id.clone()).collect(), nullable: false }),
        ("effort".to_string(), Kind::Word { words: v.efforts.clone(), nullable: false }),
    ];
    out.extend(v.levers.iter().map(|l| (l.key.clone(), l.kind.clone())));
    out
}

/// Every `llm` setting at its first value: the code's own (the Default version).
pub fn defaults(c: &Catalog) -> Vec<(String, Value)> {
    let v = c.active();
    let mut out = vec![
        ("model".to_string(), Value::String(v.defaults.model.clone())),
        ("effort".to_string(), Value::String(v.defaults.effort.clone())),
    ];
    out.extend(v.levers.iter().map(|l| (l.key.clone(), l.initial())));
    out
}

/// The rules between `llm` settings, after each fits its kind: thinking is off only on a model
/// that can turn it off and at an effort that allows it; the retry cap is at least the first cap.
/// The error names a setting and the model, never a value of a call.
pub fn rules(c: &Catalog, values: &[(&str, Value)]) -> Result<(), String> {
    let v = c.active();
    let get = |k: &str| values.iter().find(|(key, _)| *key == k).map(|(_, x)| x);
    let word = |k: &str| get(k).and_then(Value::as_str).unwrap_or("");
    if word("thinking") == "off" {
        let model = v.model(word("model"));
        if model.is_some_and(|m| m.thinking_off.is_none()) {
            return Err(format!("thinking: {} cannot turn thinking off", model.map_or("", |m| m.name.as_str())));
        }
        if !v.thinking_off_efforts.iter().any(|e| e == word("effort")) {
            let top = v.thinking_off_efforts.last().map_or("", String::as_str);
            return Err(format!("thinking: off needs effort {top} or below"));
        }
    }
    let n = |k: &str| get(k).and_then(Value::as_i64).unwrap_or(0);
    if n("retry_max_tokens") < n("max_tokens") {
        return Err("retry_max_tokens: below max_tokens".to_string());
    }
    Ok(())
}

/// Whether a whole `llm` set fits: every value its kind, then the rules. For tests and the agent.
pub fn check(c: &Catalog, values: &[(&str, Value)]) -> Result<(), String> {
    for (key, kind) in settings(c) {
        let Some((_, value)) = values.iter().find(|(k, _)| *k == key) else { return Err(format!("{key}: missing")) };
        if !fits(&kind, value) {
            return Err(format!("{key}: out of range or the wrong kind"));
        }
    }
    rules(c, values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set(model: &str, effort: &str, thinking: &str, max: i64, retry: i64) -> Vec<(&'static str, Value)> {
        let mut out: Vec<(&'static str, Value)> = vec![];
        for (k, v) in defaults(catalog()) {
            let k: &'static str = Box::leak(k.into_boxed_str());
            out.push((k, v));
        }
        for (k, v) in [("model", json!(model)), ("effort", json!(effort)), ("thinking", json!(thinking)), ("max_tokens", json!(max)), ("retry_max_tokens", json!(retry))] {
            out.iter_mut().find(|(key, _)| *key == k).unwrap().1 = v;
        }
        out
    }

    #[test]
    fn the_catalog_parses_and_its_defaults_fit() {
        let c = catalog();
        assert_eq!(c.active().id, "anthropic");
        assert!(check(c, &set("claude-haiku-5-5", "low", "on", 1200, 2400)).is_ok());
        assert!(catalog_json().contains("\"claude-opus-5-5\""));
    }

    #[test]
    fn the_rules() {
        let c = catalog();
        assert!(rules(c, &set("claude-haiku-5-5", "high", "off", 1200, 2400)).is_ok());
        assert!(rules(c, &set("claude-sonnet-5-5", "medium", "off", 1200, 1200)).is_ok());
        assert_eq!(rules(c, &set("claude-opus-5-5", "low", "off", 1200, 2400)).unwrap_err(), "thinking: Opus 5.5 cannot turn thinking off");
        assert_eq!(rules(c, &set("claude-sonnet-5-5", "xhigh", "off", 1200, 2400)).unwrap_err(), "thinking: off needs effort high or below");
        assert_eq!(rules(c, &set("claude-haiku-5-5", "low", "on", 2400, 1200)).unwrap_err(), "retry_max_tokens: below max_tokens");
        assert!(check(c, &set("claude-haiku-4-5", "low", "on", 1200, 2400)).unwrap_err().starts_with("model:"));
    }
}
