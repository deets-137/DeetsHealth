//! The voice catalog (docs/architecture/VOICE-VENDORS.md §2.1): each voice vendor, what it
//! offers in each direction (hearing, speaking), its levers with their ranges and defaults, and
//! its cost rates. The catalog is data: one JSON file per vendor in `catalog/`, compiled in.
//!
//! The catalog makes the voice settings: one version holds the vendor of each direction, the
//! levers every vendor shares, and every vendor's own levers under its id (`cartesia.speed`).
//! `ehr` checks each save against [`settings`]; the agent and the console read the catalog from
//! `ehr` (the `GetVoiceCatalog` RPC), so there is one copy.
//!
//! Adding a vendor: a file in `catalog/voice/`, its line in [`VENDOR_FILES`], and its adapter in the
//! agent (`agent/deets_agent/voice/`). No value here is protected data.

use std::sync::OnceLock;

pub mod cost;

use crate::{lever_ok, Kind, Lever};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The vendors, in the order the console lists them. The first is the default of each direction
/// it offers.
pub const VENDOR_FILES: &[&str] = &[
    include_str!("../../catalog/voice/cartesia.json"),
    include_str!("../../catalog/voice/elevenlabs.json"),
];
/// The levers every vendor shares.
const SHARED_FILE: &str = include_str!("../../catalog/voice/shared.json");

/// The two directions of the voice.
pub const DIRECTIONS: &[&str] = &["hearing", "speaking"];

/// What a vendor charges for one direction: `amount` of `unit` per `per`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rate {
    pub unit: String,
    /// `second` (of audio streamed) or `character` (of text spoken).
    pub per: String,
    pub amount: f64,
}

/// A voice a vendor recommends for agents: the top of the page's Voice menu. The id is a
/// fallback where the vendor's docs print one; the live list gives the rest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Voice {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// What a vendor offers in one direction.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Direction {
    /// The vendor's name for it (Ink, Sonic).
    pub product: String,
    /// Pinned by name; never a "latest" alias.
    pub model: String,
    /// Hearing only: the vendor itself says when a turn starts and ends. Without it, the agent
    /// runs Pipecat's own voice detector and turn model.
    #[serde(default)]
    pub own_turns: bool,
    pub rate: Rate,
    pub levers: Vec<Lever>,
    /// Hearing: named sets of turn levers (a page shortcut, not an API parameter).
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub profiles: serde_json::Map<String, Value>,
    /// Speaking: the recommended voices.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub voices: Vec<Voice>,
}

/// A vendor's plan: what its usage is counted in, and the levers of the account.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub unit: String,
    pub levers: Vec<Lever>,
}

/// One vendor.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Vendor {
    /// Lowercase a to z: the prefix of its levers in a version.
    pub id: String,
    pub name: String,
    /// The name of its key in `.env` (never the key).
    pub key_env: String,
    /// Where its BAA and zero retention are offered.
    pub baa: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hearing: Option<Direction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaking: Option<Direction>,
    pub account: Account,
}

impl Vendor {
    pub fn direction(&self, name: &str) -> Option<&Direction> {
        match name {
            "hearing" => self.hearing.as_ref(),
            "speaking" => self.speaking.as_ref(),
            _ => None,
        }
    }

    /// Every lever of this vendor: hearing, speaking, then the account.
    pub fn levers(&self) -> impl Iterator<Item = &Lever> {
        let h = self.hearing.iter().flat_map(|d| d.levers.iter());
        let s = self.speaking.iter().flat_map(|d| d.levers.iter());
        h.chain(s).chain(self.account.levers.iter())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Shared {
    #[allow(dead_code)]
    about: String,
    levers: Vec<Lever>,
}

/// The whole catalog.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Catalog {
    pub vendors: Vec<Vendor>,
    pub shared: Vec<Lever>,
}

impl Catalog {
    pub fn vendor(&self, id: &str) -> Option<&Vendor> {
        self.vendors.iter().find(|v| v.id == id)
    }

    /// The vendors that offer `direction`, in order.
    pub fn offering<'a>(&'a self, direction: &'a str) -> impl Iterator<Item = &'a Vendor> + 'a {
        self.vendors.iter().filter(move |v| v.direction(direction).is_some())
    }
}

/// Parse and check the catalog. Every file is checked: a bad file is a build-time mistake, so
/// the tests fail on it and `catalog()` panics at start.
pub fn parse(vendor_files: &[&str], shared_file: &str) -> Result<Catalog, String> {
    let shared: Shared = serde_json::from_str(shared_file).map_err(|e| format!("shared.json: {e}"))?;
    let mut vendors: Vec<Vendor> = vec![];
    for (i, file) in vendor_files.iter().enumerate() {
        let v: Vendor = serde_json::from_str(file).map_err(|e| format!("vendor file {i}: {e}"))?;
        if v.id.is_empty() || !v.id.chars().all(|c| c.is_ascii_lowercase()) {
            return Err(format!("vendor file {i}: the id is lowercase a to z"));
        }
        if vendors.iter().any(|o| o.id == v.id) {
            return Err(format!("{}: listed twice", v.id));
        }
        if v.hearing.is_none() && v.speaking.is_none() {
            return Err(format!("{}: offers no direction", v.id));
        }
        // Every account has the same four levers: cost.rs reads them by name.
        for k in cost::ACCOUNT_LEVERS {
            if !v.account.levers.iter().any(|l| l.key == *k) {
                return Err(format!("{}.{k}: the account needs it", v.id));
            }
        }
        let mut seen: Vec<&str> = vec![];
        for lever in v.levers() {
            if seen.contains(&lever.key.as_str()) || shared.levers.iter().any(|s| s.key == lever.key) {
                return Err(format!("{}.{}: listed twice", v.id, lever.key));
            }
            seen.push(&lever.key);
            lever_ok(lever).map_err(|e| format!("{}.{}: {e}", v.id, lever.key))?;
        }
        vendors.push(v);
    }
    for lever in &shared.levers {
        lever_ok(lever).map_err(|e| format!("{}: {e}", lever.key))?;
    }
    for d in DIRECTIONS {
        if !vendors.iter().any(|v| v.direction(d).is_some()) {
            return Err(format!("no vendor offers {d}"));
        }
    }
    Ok(Catalog { vendors, shared: shared.levers })
}

/// The catalog, parsed once. Panics on a bad file: the crate's tests catch that first.
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| parse(VENDOR_FILES, SHARED_FILE).expect("the voice catalog"))
}

/// The catalog as JSON, for the `GetVoiceCatalog` RPC: the vendors, the shared levers, and
/// `defaults`, every setting of a voice version at its first value (so no reader works it out
/// again).
pub fn catalog_json() -> &'static str {
    static JSON: OnceLock<String> = OnceLock::new();
    JSON.get_or_init(|| {
        let c = catalog();
        let mut out = serde_json::to_value(c).expect("the voice catalog as JSON");
        let defaults: serde_json::Map<String, Value> = defaults(c).into_iter().collect();
        out["defaults"] = Value::Object(defaults);
        out.to_string()
    })
}

/// The key of a direction's vendor in a version: `hearing.vendor`, `speaking.vendor`.
pub fn vendor_key(direction: &str) -> String {
    format!("{direction}.vendor")
}

/// The key of a vendor's lever in a version: `cartesia.speed`.
pub fn lever_key(vendor: &str, lever: &str) -> String {
    format!("{vendor}.{lever}")
}

/// Every setting of a voice version, in the page's order, and what it may hold: the vendor of
/// each direction, the shared levers, then each vendor's levers under its id.
pub fn settings(c: &Catalog) -> Vec<(String, Kind)> {
    let mut out = vec![];
    for d in DIRECTIONS {
        let words = c.offering(d).map(|v| v.id.clone()).collect();
        out.push((vendor_key(d), Kind::Word { words, nullable: false }));
    }
    out.extend(c.shared.iter().map(|l| (l.key.clone(), l.kind.clone())));
    for v in &c.vendors {
        out.extend(v.levers().map(|l| (lever_key(&v.id, &l.key), l.kind.clone())));
    }
    out
}

/// Every setting at its first value: the Default version before any `.env` value is laid on it.
/// Each direction's vendor is the first vendor that offers it.
pub fn defaults(c: &Catalog) -> Vec<(String, Value)> {
    let mut out = vec![];
    for d in DIRECTIONS {
        let first = c.offering(d).next().map(|v| v.id.clone()).unwrap_or_default();
        out.push((vendor_key(d), Value::String(first)));
    }
    out.extend(c.shared.iter().map(|l| (l.key.clone(), l.initial())));
    for v in &c.vendors {
        out.extend(v.levers().map(|l| (lever_key(&v.id, &l.key), l.initial())));
    }
    out
}

/// The keys of a voice version saved before the catalog (2026-11-23): Cartesia's levers had no
/// prefix. Read under their new name.
const BEFORE_CATALOG: &[&str] = &[
    "voice",
    "speed",
    "volume",
    "emotion",
    "normalization",
    "turn_start_threshold",
    "turn_eager_end_threshold",
    "turn_end_threshold",
    "turn_end_timeout_ms",
    "eager_end_of_turn",
    "plan_credits",
    "plan_price_usd",
    "offset",
    "cycle_day",
];

/// Read a stored version in today's shape, in place (the settings tables are insert-only, so no
/// row is rewritten). A version from before the catalog has its Cartesia keys renamed and both
/// directions on Cartesia. A setting the version lacks (a vendor added after it was saved) takes
/// its first value. Values are JSON text, as the store holds them.
pub fn upgrade(c: &Catalog, values: &mut Vec<(String, String)>) {
    if values.is_empty() {
        return;
    }
    let old = !values.iter().any(|(k, _)| k.contains('.'));
    if old {
        for (k, _) in values.iter_mut() {
            if BEFORE_CATALOG.contains(&k.as_str()) {
                *k = lever_key("cartesia", k);
            }
        }
        for d in DIRECTIONS {
            values.push((vendor_key(d), "\"cartesia\"".to_string()));
        }
    }
    for (key, value) in defaults(c) {
        if !values.iter().any(|(k, _)| *k == key) {
            values.push((key, value.to_string()));
        }
    }
    // A key the catalog no longer lists (a lever or a vendor taken out) is left behind: a save
    // sends back what it read, and the check refuses an unknown key.
    let known: Vec<String> = settings(c).into_iter().map(|(k, _)| k).collect();
    values.retain(|(k, _)| known.contains(k));
    values.sort_by(|a, b| a.0.cmp(&b.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fits;
    use serde_json::json;

    #[test]
    fn the_catalog_parses() {
        let c = catalog();
        assert_eq!(c.vendors[0].id, "cartesia");
        assert!(catalog_json().contains("\"ink-2\""));
    }

    #[test]
    fn elevenlabs_offers_both_directions_without_own_turns() {
        let c = catalog();
        let e = c.vendor("elevenlabs").expect("ElevenLabs in the catalog");
        assert!(!e.hearing.as_ref().unwrap().own_turns, "Scribe: Pipecat decides the turns");
        assert_eq!(e.speaking.as_ref().unwrap().model, "eleven_flash_v2_5");
        // Cartesia stays the first vendor, so the default of each direction.
        let d: serde_json::Map<String, Value> = defaults(c).into_iter().collect();
        assert_eq!(d["hearing.vendor"], "cartesia");
        assert_eq!(d["speaking.vendor"], "cartesia");
        assert_eq!(d["elevenlabs.zero_retention"], true, "zero retention on by default (§4, item 5)");
    }

    #[test]
    fn every_default_fits_its_setting() {
        let c = catalog();
        let list = settings(c);
        let defaults = defaults(c);
        assert_eq!(list.len(), defaults.len());
        for ((k, kind), (dk, v)) in list.iter().zip(defaults.iter()) {
            assert_eq!(k, dk);
            assert!(fits(kind, v), "{k}");
        }
    }

    #[test]
    fn an_old_version_reads_in_todays_shape() {
        let c = catalog();
        let mut old: Vec<(String, String)> = vec![
            ("speed".into(), "1.2".into()),
            ("barge_in".into(), "false".into()),
            ("voice".into(), json!({"name": "Daniel", "id": "x"}).to_string()),
        ];
        upgrade(c, &mut old);
        let get = |k: &str| old.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        assert_eq!(get("cartesia.speed").as_deref(), Some("1.2"));
        assert_eq!(get("barge_in").as_deref(), Some("false"));
        assert_eq!(get("hearing.vendor").as_deref(), Some("\"cartesia\""));
        assert!(get("speed").is_none());
        // Every setting is there once.
        assert_eq!(old.len(), settings(c).len());
    }

    #[test]
    fn a_key_the_catalog_no_longer_lists_is_left_behind() {
        let c = catalog();
        let mut v: Vec<(String, String)> = defaults(c).into_iter().map(|(k, v)| (k, v.to_string())).collect();
        v.push(("cartesia.retired_lever".into(), "1".into()));
        v.push(("gone.speed".into(), "1".into()));
        upgrade(c, &mut v);
        assert_eq!(v.len(), settings(c).len());
        assert!(!v.iter().any(|(k, _)| k == "cartesia.retired_lever" || k == "gone.speed"));
    }

    #[test]
    fn a_bad_file_is_refused() {
        let shared = SHARED_FILE;
        let twice = format!("[{0}]", VENDOR_FILES[0]);
        let files: Vec<&str> = vec![VENDOR_FILES[0], VENDOR_FILES[0]];
        assert_eq!(parse(&files, shared).unwrap_err(), "cartesia: listed twice");
        assert!(parse(&[twice.as_str()], shared).is_err());
        let bad = VENDOR_FILES[0].replace("\"min\": 0.5, \"max\": 0.9", "\"min\": 0.9, \"max\": 0.5");
        assert!(parse(&[bad.as_str()], shared).unwrap_err().contains("min above max"));
    }
}
