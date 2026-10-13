//! Card payloads (CORE-CRATE.md §9): what the front end receives, with plain values, built
//! here and nowhere else. Every struct here derives `TS` under the `ts` feature, and
//! `npm run types` writes its TypeScript type into `src/model/` (never edited by hand).
//!
//! This module is one of the three places that may read a [`crate::Shielded`] value. Today it
//! reveals one protected value, the site's name, for the Sites row. The page puts the name in
//! with `textContent` only, and never into a hover hint (CORE-CRATE.md §2, S2).
//!
//! The events are here too, so their types are generated with the rest: `fetch-progress`,
//! `fetch-done` and `sign-in`. They carry counts, fixed words and the app's own random site
//! key only (SECURITY.md §4.3).

use crate::auth;
use crate::platform::SiteKey;
use crate::store::{Store, Table};
use crate::CoreError;
use serde::Serialize;
use std::collections::BTreeMap;
#[cfg(feature = "ts")]
use ts_rs::TS;

/// The card payload's pass. Its field is private, so only this module can make one.
pub struct CardPass {
    _only_here: (),
}

impl CardPass {
    /// Made while one payload is built, then dropped.
    fn issue() -> Self {
        Self { _only_here: () }
    }
}

/// The five counts on a Sites row (CORE-CRATE.md §2, S6). Each is a number of resources of
/// the FHIR types in [`SiteCounts::from_types`].
#[derive(Serialize, Default, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct SiteCounts {
    pub results: u32,
    pub medicines: u32,
    pub allergies: u32,
    pub conditions: u32,
    pub visits: u32,
}

impl SiteCounts {
    /// The words from counts by FHIR type. A type not named here is not counted on the row.
    pub fn from_types(by_type: &BTreeMap<String, u64>) -> Self {
        let n = |t: &str| by_type.get(t).copied().unwrap_or(0).min(u32::MAX as u64) as u32;
        Self {
            results: n("Observation"),
            medicines: n("MedicationRequest").saturating_add(n("MedicationStatement")),
            allergies: n("AllergyIntolerance"),
            conditions: n("Condition"),
            visits: n("Encounter"),
        }
    }
}

/// What a site's row can do now, from its saved data.
#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum SiteState {
    /// Signed in: Refresh works.
    Ready,
    /// The site refused the refresh token: the row offers Sign in.
    SignInAgain,
    /// The file will not open (its key is gone, or DPAPI refused it). Sign out erases it.
    Unreadable,
    /// A newer build wrote the file.
    Newer,
}

/// One connected site, for the Settings › Sites row.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct SiteView {
    /// The app's own random site key: not protected (SECURITY.md §4.3).
    pub key: String,
    /// The site's name. **Protected** for a real site: `textContent` only, never a hint.
    /// `None` when the file will not open.
    pub name: Option<String>,
    pub counts: SiteCounts,
    /// When the last fetch ended, in seconds since 1970. `None`: never.
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub updated: Option<u64>,
    pub state: SiteState,
}

/// One site in the Connect menu. Directory data, not the user's: the sandbox names are public.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct CatalogSite {
    /// The index `sign_in_start` takes.
    pub index: u32,
    pub name: String,
}

/// The whole Sites section: the Connect menu and the connected sites.
#[derive(Serialize)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct SitesPayload {
    pub catalog: Vec<CatalogSite>,
    pub sites: Vec<SiteView>,
}

/// The Connect menu's sites.
pub fn catalog() -> Vec<CatalogSite> {
    auth::sandbox_sites().iter().enumerate().map(|(i, s)| CatalogSite { index: i as u32, name: s.name.to_owned() }).collect()
}

/// The tables that hold the five counted types.
const COUNTED: [Table; 3] = [Table::Result, Table::Medication, Table::ChartItem];

/// The row for a site whose store opened.
pub fn site_view<S: Store>(key: &SiteKey, store: &S, now: u64) -> Result<SiteView, CoreError> {
    let pass = CardPass::issue();
    let site = store.site()?.ok_or(CoreError::Io)?;
    let mut by_type = store.type_counts(Table::Encounter)?;
    for t in COUNTED {
        by_type.extend(store.type_counts(t)?);
    }
    // A refused refresh drops the refresh token (the host, at `SignInAgain`). With no refresh
    // token and an access token past its time, only a new sign-in helps.
    let state = if site.refresh_token.is_none() && auth::needs_refresh(&site, now) {
        SiteState::SignInAgain
    } else {
        SiteState::Ready
    };
    Ok(SiteView {
        key: key.as_str().to_owned(),
        name: site.display_name.as_ref().map(|n| n.reveal(&pass).clone()),
        counts: SiteCounts::from_types(&by_type),
        updated: site.last_fetch.as_ref().and_then(|t| t.reveal(&pass).parse().ok()),
        state,
    })
}

/// The row for a site whose store would not open.
pub fn broken_site_view(key: &SiteKey, e: Option<CoreError>) -> SiteView {
    SiteView {
        key: key.as_str().to_owned(),
        name: None,
        counts: SiteCounts::default(),
        updated: None,
        state: if e == Some(CoreError::Version) { SiteState::Newer } else { SiteState::Unreadable },
    }
}

/// `fetch-progress`: after every page and every job.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct FetchProgress {
    pub site: String,
    /// Counts of this fetch so far (they grow from 0; a Refresh starts again at 0).
    pub counts: SiteCounts,
    pub jobs_done: u32,
    pub jobs_total: u32,
    pub calls: u32,
}

/// How a fetch ended.
#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum FetchEndWord {
    Done,
    SignInAgain,
    Failed,
}

/// `fetch-done`: once, at the end of a fetch.
#[derive(Serialize, Clone)]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub struct FetchDone {
    pub site: String,
    pub end: FetchEndWord,
}

/// `sign-in`: what a sign-in is doing now. The one waiting sign-in has no site key yet.
#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts", derive(TS), ts(export))]
pub enum SignInStatus {
    /// The browser is open; the app waits for the link.
    Waiting,
    Cancelled,
    /// The link came after the 10-minute wait.
    Expired,
    /// The site did not grant access.
    Refused,
    Failed,
    /// Sealed; the fetch starts.
    Connected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_name_their_types() {
        let by: BTreeMap<String, u64> = [
            ("Observation", 40),
            ("DiagnosticReport", 9),
            ("MedicationRequest", 3),
            ("MedicationStatement", 2),
            ("AllergyIntolerance", 1),
            ("Condition", 7),
            ("Encounter", 12),
            ("Procedure", 5),
        ]
        .into_iter()
        .map(|(t, n)| (t.to_owned(), n))
        .collect();
        let c = SiteCounts::from_types(&by);
        assert_eq!(c, SiteCounts { results: 40, medicines: 5, allergies: 1, conditions: 7, visits: 12 });
    }

    #[test]
    fn events_are_kebab_words() {
        assert_eq!(serde_json::to_string(&SignInStatus::Waiting).unwrap(), r#""waiting""#);
        assert_eq!(serde_json::to_string(&FetchEndWord::SignInAgain).unwrap(), r#""sign-in-again""#);
        let p = FetchProgress { site: "k".into(), counts: SiteCounts::default(), jobs_done: 1, jobs_total: 2, calls: 3 };
        let v = serde_json::to_value(&p).unwrap();
        assert!(v.get("jobsDone").is_some() && v["counts"].get("results").is_some());
    }
}
