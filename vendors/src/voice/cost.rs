//! What the voice costs (VOICE-VENDORS.md §2.4, §9): one call's use, its credits by vendor, a
//! plan's price per credit, and a vendor's billing cycle. Every formula of the voice's cost is
//! here; the rates and the plans' levers are the catalog's and the voice settings'. `ehr` calls
//! these when it files a call, sums Analytics, and answers the agent's `PriceVoiceUse`.
//!
//! **A call's use** is raw counts only, as the agent's meter takes them from Pipecat: the
//! seconds of audio sent to the hearing vendor and the characters sent to the speaking vendor.
//! The record keeps it on `call_end` as `voice_use`:
//!
//! ```json
//! {"hearing": {"vendor": "cartesia", "amount": 12.4}, "speaking": {"vendor": "cartesia", "amount": 350}}
//! ```
//!
//! A record from before (schema 7 to 15) has `cartesia: {ink_s, sonic_chars, credits}`;
//! [`call_use`] reads it as Cartesia for both directions.
//!
//! **A vendor's account** holds four levers, named the same in every vendor's catalog file:
//! `plan_credits`, `plan_price_usd` (0: not set), `offset` and `cycle_day`. Counts and dates
//! only; nothing here is protected data.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::cycle::{cycle_start, next_cycle, Day};
use crate::voice::{lever_key, Catalog, DIRECTIONS};

/// The account levers every vendor's catalog file holds (checked by `parse`).
pub const ACCOUNT_LEVERS: &[&str] = &["plan_credits", "plan_price_usd", "offset", "cycle_day"];

/// One direction's use on one call: the vendor and the amount in its rate's `per` (seconds of
/// audio heard, characters spoken).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DirectionUse {
    pub vendor: String,
    pub amount: f64,
}

/// One call's use of the voice. A text call has none.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CallUse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hearing: Option<DirectionUse>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaking: Option<DirectionUse>,
}

impl CallUse {
    fn direction(&self, d: &str) -> Option<&DirectionUse> {
        match d {
            "hearing" => self.hearing.as_ref(),
            "speaking" => self.speaking.as_ref(),
            _ => None,
        }
    }
}

/// A call's use from its `call_end` event (or any object that holds `voice_use` or the older
/// `cartesia`). None for a text call or a shape it does not know.
pub fn call_use(end: &Value) -> Option<CallUse> {
    if let Some(u) = end.get("voice_use") {
        let parsed: CallUse = serde_json::from_value(u.clone()).ok()?;
        return (parsed.hearing.is_some() || parsed.speaking.is_some()).then_some(parsed);
    }
    let old = end.get("cartesia")?.as_object()?;
    let num = |k: &str| old.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    Some(CallUse {
        hearing: Some(DirectionUse { vendor: "cartesia".into(), amount: num("ink_s") }),
        speaking: Some(DirectionUse { vendor: "cartesia".into(), amount: num("sonic_chars") }),
    })
}

/// A call's credits by vendor: each direction's amount times its vendor's rate, summed by vendor
/// and rounded up to a whole credit (as the vendors bill). A vendor not in the catalog counts
/// nothing.
pub fn credits(c: &Catalog, u: &CallUse) -> BTreeMap<String, f64> {
    let mut raw: BTreeMap<String, f64> = BTreeMap::new();
    for d in DIRECTIONS {
        let Some(du) = u.direction(d) else { continue };
        let Some(rate) = c.vendor(&du.vendor).and_then(|v| v.direction(d)).map(|x| &x.rate) else { continue };
        *raw.entry(du.vendor.clone()).or_default() += du.amount.max(0.0) * rate.amount;
    }
    raw.into_iter().map(|(k, v)| (k, ceil(v))).collect()
}

/// Up to a whole credit. Rounded to 9 places first, so 180.00000000000003 credits is 180.
fn ceil(x: f64) -> f64 {
    ((x * 1e9).round() / 1e9).ceil().max(0.0)
}

/// A voice version's values by key (as the store or the agent holds them).
pub type Values = serde_json::Map<String, Value>;

fn account(values: &Values, vendor: &str, lever: &str) -> f64 {
    values.get(&lever_key(vendor, lever)).and_then(Value::as_f64).unwrap_or(0.0)
}

/// Dollars per credit for a vendor: the plan's price over its credits. None while the price or
/// the credits are not set.
pub fn usd_per_credit(values: &Values, vendor: &str) -> Option<f64> {
    let price = account(values, vendor, "plan_price_usd");
    let plan = account(values, vendor, "plan_credits");
    (price > 0.0 && plan > 0.0).then(|| price / plan)
}

/// One vendor's cycle: its plan, what was used, what is left, and the use in its own amounts.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Cycle {
    pub vendor: String,
    pub name: String,
    /// What the plan counts in (`credits`).
    pub unit: String,
    pub plan: f64,
    pub offset: f64,
    /// This cycle's calls and the live calls.
    pub used: f64,
    /// The plan less the offset less the use; may go below zero.
    pub left: f64,
    /// Calls this cycle that used this vendor (the live ones not counted).
    pub calls: i64,
    /// The amounts by direction (`hearing`: seconds, `speaking`: characters), with each one's
    /// product and unit, for the page's facts.
    pub amounts: Vec<CycleAmount>,
    pub cycle_start: String,
    pub renews: String,
    pub usd_per_credit: Option<f64>,
    /// The use of the live calls alone.
    pub live: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CycleAmount {
    pub direction: String,
    pub product: String,
    /// The rate's `per`: `second`, `character`.
    pub per: String,
    pub amount: f64,
}

/// One filed call for a cycle: its day and its use.
pub struct DatedUse<'a> {
    pub day: Day,
    pub used: &'a CallUse,
}

/// The cycle of each vendor the version chose (hearing first, then speaking, each once), from
/// the calls given and the live calls' use so far.
pub fn cycles(c: &Catalog, values: &Values, today: Day, calls: &[DatedUse<'_>], live: &[CallUse]) -> Vec<Cycle> {
    let mut chosen: Vec<String> = vec![];
    for d in DIRECTIONS {
        if let Some(id) = values.get(&format!("{d}.vendor")).and_then(Value::as_str) {
            if !chosen.iter().any(|x| x == id) && c.vendor(id).is_some() {
                chosen.push(id.to_string());
            }
        }
    }
    chosen
        .into_iter()
        .filter_map(|id| {
            let v = c.vendor(&id)?;
            let start = cycle_start(today, account(values, &id, "cycle_day") as u32);
            let renews = next_cycle(start);
            let mut used = 0.0;
            let mut count = 0;
            let mut amounts: BTreeMap<&str, f64> = BTreeMap::new();
            let mut add = |u: &CallUse| -> f64 {
                for d in DIRECTIONS {
                    if let Some(du) = u.direction(d).filter(|du| du.vendor == id) {
                        *amounts.entry(d).or_default() += du.amount;
                    }
                }
                credits(c, u).get(&id).copied().unwrap_or(0.0)
            };
            for call in calls.iter().filter(|x| x.day >= start && x.day < renews) {
                let n = add(call.used);
                if call.used.hearing.as_ref().is_some_and(|x| x.vendor == id) || call.used.speaking.as_ref().is_some_and(|x| x.vendor == id) {
                    count += 1;
                }
                used += n;
            }
            let live_used: f64 = live.iter().map(&mut add).sum();
            used += live_used;
            let plan = account(values, &id, "plan_credits");
            let offset = account(values, &id, "offset");
            let amounts = DIRECTIONS
                .iter()
                .filter_map(|d| {
                    let dir = v.direction(d)?;
                    Some(CycleAmount { direction: d.to_string(), product: dir.product.clone(), per: dir.rate.per.clone(), amount: amounts.get(d).copied().unwrap_or(0.0) })
                })
                .collect();
            Some(Cycle {
                vendor: id.clone(),
                name: v.name.clone(),
                unit: v.account.unit.clone(),
                plan,
                offset,
                used,
                left: plan - offset - used,
                calls: count,
                amounts,
                cycle_start: start.iso(),
                renews: renews.iso(),
                usd_per_credit: usd_per_credit(values, &id),
                live: live_used,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::{catalog, defaults};
    use serde_json::json;

    fn d(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    #[test]
    fn credits_follow_the_catalogs_rates() {
        let c = catalog();
        let u = |h: f64, s: f64| CallUse {
            hearing: Some(DirectionUse { vendor: "cartesia".into(), amount: h }),
            speaking: Some(DirectionUse { vendor: "cartesia".into(), amount: s }),
        };
        assert_eq!(credits(c, &u(0.0, 0.0)).get("cartesia"), Some(&0.0));
        assert_eq!(credits(c, &u(60.0, 0.0))["cartesia"], 180.0, "Ink: 3 a second");
        assert_eq!(credits(c, &u(0.0, 750.0))["cartesia"], 750.0, "Sonic: 1 a character");
        assert_eq!(credits(c, &u(0.1, 0.0))["cartesia"], 1.0, "rounded up");
        assert_eq!(credits(c, &u(0.1 + 0.2, 0.0))["cartesia"], 1.0, "0.9 credits is 1");
        assert!(credits(c, &CallUse::default()).is_empty(), "a text call costs nothing");
    }

    #[test]
    fn an_old_record_reads_as_cartesia() {
        let u = call_use(&json!({"kind": "call_end", "cartesia": {"ink_s": 100.0, "sonic_chars": 500, "credits": 800}})).unwrap();
        assert_eq!(credits(catalog(), &u)["cartesia"], 800.0);
        let new = call_use(&json!({"voice_use": {"hearing": {"vendor": "cartesia", "amount": 2.0}}})).unwrap();
        assert_eq!(credits(catalog(), &new)["cartesia"], 6.0);
        assert!(call_use(&json!({"kind": "call_end"})).is_none());
    }

    #[test]
    fn a_vendors_cycle_counts_this_cycles_calls_and_the_live_ones() {
        let c = catalog();
        let mut values: Values = defaults(c).into_iter().collect();
        values.insert("cartesia.offset".into(), json!(270));
        values.insert("cartesia.plan_price_usd".into(), json!(5));
        let old = call_use(&json!({"cartesia": {"ink_s": 100.0, "sonic_chars": 500}})).unwrap();
        let calls = [DatedUse { day: d("2026-11-10"), used: &old }, DatedUse { day: d("1999-01-01"), used: &old }];
        let live = [call_use(&json!({"voice_use": {"speaking": {"vendor": "cartesia", "amount": 30}}})).unwrap()];
        let got = cycles(c, &values, d("2026-11-13"), &calls, &live);
        assert_eq!(got.len(), 1, "both directions on Cartesia: one cycle");
        let x = &got[0];
        assert_eq!((x.used, x.live, x.calls, x.left), (830.0, 30.0, 1, 20000.0 - 270.0 - 830.0));
        assert_eq!(x.usd_per_credit, Some(5.0 / 20000.0));
        assert_eq!(x.amounts.iter().map(|a| a.amount).collect::<Vec<_>>(), vec![100.0, 530.0]);
    }
}
