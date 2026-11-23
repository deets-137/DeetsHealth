//! What the model costs (VOICE-VENDORS.md §10): tokens times the model's prices, the cache
//! write factor by the cache's life, the place inference runs; and the budget's cycle. Every
//! formula of the model's cost is here; `ehr` calls these when it files a call and when the
//! agent asks (`PriceModelUse`). Counts, dollars and dates only.
//!
//! **A call's use** is what the record already holds: `call_start` › `llm` (`model`,
//! `cache_ttl`) and `inference_geo`, and `call_end` › `tokens` (Anthropic's usage names:
//! `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cycle::{cycle_start, next_cycle, Day};
use crate::model::Catalog;

/// Tokens of each kind.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Tokens {
    pub input: f64,
    pub output: f64,
    pub cache_read: f64,
    pub cache_write: f64,
}

impl Tokens {
    /// From Anthropic's usage object (or a record's `tokens`).
    pub fn from_usage(u: &Value) -> Tokens {
        let n = |k: &str| u.get(k).and_then(Value::as_f64).unwrap_or(0.0).max(0.0);
        Tokens {
            input: n("input_tokens"),
            output: n("output_tokens"),
            cache_read: n("cache_read_input_tokens"),
            cache_write: n("cache_creation_input_tokens"),
        }
    }
}

/// One priced use: the model, the cache's life, where inference ran, and the tokens.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Use {
    pub model: String,
    #[serde(default = "five_minutes")]
    pub cache_ttl: String,
    /// `us`, or empty for the list price.
    #[serde(default)]
    pub geo: String,
    pub tokens: Tokens,
}

fn five_minutes() -> String {
    "5m".into()
}

/// Dollars for these tokens: input, cache writes at the input price times the factor for the
/// cache's life, cache reads at the model's read price, output; times the place's factor. A model
/// or a place not in the catalog costs its list price, or nothing for an unknown model. An
/// estimate for the record, not a bill.
pub fn usd(c: &Catalog, u: &Use) -> f64 {
    let v = c.active();
    let Some(m) = v.model(&u.model) else { return 0.0 };
    let p = &m.prices;
    let write = v.cache_write.get(&u.cache_ttl).copied().unwrap_or(1.25);
    let t = &u.tokens;
    let dollars = t.input * p.input + t.cache_write * p.input * write + t.cache_read * p.cache_read + t.output * p.output;
    let geo = v.geo_price.get(&u.geo).copied().unwrap_or(1.0);
    dollars / 1_000_000.0 * geo
}

/// A use as the agent sends it to `PriceModelUse`: `{model, cache_ttl, geo, usage}`, `usage` in
/// Anthropic's names. None without a model.
pub fn use_from(v: &Value) -> Option<Use> {
    let model = v.get("model")?.as_str()?.to_string();
    let word = |k: &str, or: &str| v.get(k).and_then(Value::as_str).unwrap_or(or).to_string();
    Some(Use {
        model,
        cache_ttl: word("cache_ttl", "5m"),
        geo: word("geo", ""),
        tokens: Tokens::from_usage(v.get("usage").unwrap_or(&Value::Null)),
    })
}

/// A recorded call's use from its `call_start` and `call_end` events. None without an end.
pub fn call_use(start: &Value, end: &Value) -> Option<Use> {
    let tokens = end.get("tokens")?;
    let llm = start.get("llm");
    let model = llm.and_then(|l| l.get("model")).or_else(|| start.get("model")).and_then(Value::as_str).unwrap_or("");
    Some(Use {
        model: model.to_string(),
        cache_ttl: llm.and_then(|l| l.get("cache_ttl")).and_then(Value::as_str).unwrap_or("5m").to_string(),
        geo: start.get("inference_geo").and_then(Value::as_str).unwrap_or("").to_string(),
        tokens: Tokens::from_usage(tokens),
    })
}

/// The budget's cycle: the budget, the offset, what this cycle's calls and the live ones spent,
/// what is left (it may go below zero), the calls, the days.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Cycle {
    pub budget: f64,
    pub offset: f64,
    pub spent: f64,
    pub left: f64,
    pub calls: i64,
    pub cycle_start: String,
    pub renews: String,
    /// The live calls' dollars alone; none when no call runs.
    pub live: Option<f64>,
}

/// The cycle that holds `today`, from the `llm` settings (`budget_usd`, `offset_usd`,
/// `cycle_day`), the filed calls' dollars by day, and the live calls' dollars.
pub fn cycle(values: &serde_json::Map<String, Value>, today: Day, calls: &[(Day, f64)], live: Option<f64>) -> Cycle {
    let num = |k: &str| values.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let start = cycle_start(today, num("cycle_day") as u32);
    let renews = next_cycle(start);
    let mine: Vec<f64> = calls.iter().filter(|(d, _)| *d >= start && *d < renews).map(|(_, x)| *x).collect();
    let spent = mine.iter().sum::<f64>() + live.unwrap_or(0.0);
    let round = |x: f64| (x * 10_000.0).round() / 10_000.0;
    Cycle {
        budget: num("budget_usd"),
        offset: num("offset_usd"),
        spent: round(spent),
        left: round(num("budget_usd") - num("offset_usd") - spent),
        calls: mine.len() as i64,
        cycle_start: start.iso(),
        renews: renews.iso(),
        live,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::catalog;
    use serde_json::json;

    fn u(model: &str, ttl: &str, geo: &str, t: Tokens) -> Use {
        Use { model: model.into(), cache_ttl: ttl.into(), geo: geo.into(), tokens: t }
    }

    #[test]
    fn the_prices() {
        let c = catalog();
        let million = Tokens { input: 1e6, ..Tokens::default() };
        assert_eq!(usd(c, &u("claude-opus-5-5", "5m", "", million.clone())), 4.0);
        assert!((usd(c, &u("claude-opus-5-5", "5m", "us", million)) - 4.4).abs() < 1e-9, "US-only, 1.1×");
        let write = Tokens { cache_write: 1e6, ..Tokens::default() };
        assert_eq!(usd(c, &u("claude-sonnet-5-5", "1h", "", write)), 4.0, "an hour's write is 2× input");
        assert_eq!(usd(c, &u("claude-unknown", "5m", "", Tokens { input: 1e6, ..Tokens::default() })), 0.0);
    }

    #[test]
    fn a_record_prices_from_its_start_and_end() {
        let start = json!({"kind": "call_start", "llm": {"model": "claude-haiku-5-5", "cache_ttl": "5m"}, "inference_geo": "us"});
        let end = json!({"kind": "call_end", "tokens": {"input_tokens": 1000000, "output_tokens": 0}});
        let got = call_use(&start, &end).unwrap();
        assert!((usd(catalog(), &got) - 0.11).abs() < 1e-9);
        assert!(call_use(&start, &json!({})).is_none());
    }

    #[test]
    fn the_budget_cycle() {
        let values = json!({"budget_usd": 100, "offset_usd": 10, "cycle_day": 1});
        let d = |s: &str| Day::parse(s).unwrap();
        let c = cycle(values.as_object().unwrap(), d("2026-11-13"), &[(d("2026-11-02"), 0.032), (d("2026-10-30"), 5.0)], Some(0.01));
        assert_eq!((c.spent, c.calls, c.left), (0.042, 1, 89.958));
        assert_eq!((c.cycle_start.as_str(), c.renews.as_str()), ("2026-11-01", "2026-12-01"));
    }
}
