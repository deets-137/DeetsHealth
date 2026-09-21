//! The sealed cache (DATA-MODEL.md, CORE-CRATE.md §5).
//!
//! One store per site. Every protected value is sealed on its own, bound to its table, row
//! and column (`crate::seal`). Only opaque row keys, table names and the schema version are
//! plain. A missing value is sealed too, so even "this result has no end date" is not visible.
//!
//! This module is one of the two places that may read a [`crate::Shielded`] value: the
//! cache writer seals each value just before its write and unseals it just after its read.
//!
//! The row types here are platform-free. The SQLite store (desktop) is behind the `sqlite`
//! feature; a web version adds an IndexedDB store over the same rows.

#[cfg(feature = "sqlite")]
mod sqlite;
#[cfg(feature = "sqlite")]
pub use sqlite::SiteStore;

use crate::platform::{is_hex_key, random_hex, Cipher};
use crate::{CoreError, Shielded};

/// A protected text value, as the cache holds it.
pub type Text = Shielded<String>;

/// The cache writer's pass. Its field is private, so only this module can make one.
pub struct CachePass {
    _only_here: (),
}

impl CachePass {
    /// The writer makes one per read or write. (No writer yet on the web, hence the allow.)
    #[cfg_attr(not(feature = "sqlite"), allow(dead_code))]
    pub(crate) fn issue() -> Self {
        Self { _only_here: () }
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self::issue()
    }
}

/// A row's opaque key: 128 random bits as hex, made when the row is first written. It is
/// plain in the file, carries no meaning, and is part of every value's associated data.
/// No `Debug` or `Display`: logs carry no id (SECURITY.md §4.5).
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RowId(String);

impl RowId {
    pub fn generate<C: Cipher>(cipher: &C) -> Result<Self, CoreError> {
        random_hex(cipher).map(Self)
    }

    /// Read back from the file. Refuses anything that is not a row key.
    pub fn from_stored(s: String) -> Result<Self, CoreError> {
        if is_hex_key(&s) {
            Ok(Self(s))
        } else {
            Err(CoreError::Io)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The tables that hold one FHIR resource per row (DATA-MODEL.md §3, §5, §7).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Table {
    Encounter,
    Result,
    Medication,
    Document,
    Order,
    ChartItem,
    Party,
    Billing,
    Drug,
    Other,
    Definition,
}

impl Table {
    pub const ALL: [Table; 11] = [
        Table::Encounter,
        Table::Result,
        Table::Medication,
        Table::Document,
        Table::Order,
        Table::ChartItem,
        Table::Party,
        Table::Billing,
        Table::Drug,
        Table::Other,
        Table::Definition,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Table::Encounter => "encounter",
            Table::Result => "result",
            Table::Medication => "medication",
            Table::Document => "document",
            Table::Order => "order_",
            Table::ChartItem => "chart_item",
            Table::Party => "party",
            Table::Billing => "billing",
            Table::Drug => "drug",
            Table::Other => "other",
            Table::Definition => "definition",
        }
    }
}

/// One FHIR resource: the index columns and the raw resource (DATA-MODEL.md §6.2). Every
/// field is sealed. Dates are the FHIR text as sent, so a partial date keeps its precision.
pub struct ResourceRow {
    pub id: RowId,
    pub fhir_type: Text,
    pub fhir_id: Text,
    pub kind: Text,
    pub code_system: Option<Text>,
    pub code: Option<Text>,
    pub code_display: Option<Text>,
    pub status: Option<Text>,
    pub start: Option<Text>,
    pub end: Option<Text>,
    pub recorded: Option<Text>,
    /// The site's `meta.lastUpdated`.
    pub last_updated: Option<Text>,
    /// `definition` rows only: the canonical URL and version they are keyed by.
    pub canonical: Option<Text>,
    pub version: Option<Text>,
    /// The resource as the site sent it (JSON).
    pub raw: Text,
}

/// The patient: name, birth date and sex only; no raw resource (FHIR-SHAPES.md §5, fork 1).
pub struct PatientRow {
    pub fhir_id: Text,
    /// The `HumanName` entries as JSON.
    pub name: Option<Text>,
    pub birth_date: Option<Text>,
    /// `Patient.gender` (FHIR-SHAPES.md §5, fork 4).
    pub sex: Option<Text>,
}

/// This site's facts and credentials (DATA-MODEL.md §7.3). One row per file.
#[derive(Default)]
pub struct SiteRow {
    /// The site's name as the user picked it. Protected: it says where a person gets care.
    pub display_name: Option<Text>,
    pub base_url: Option<Text>,
    /// The client id the app signed in with; the refresh needs it.
    pub client_id: Option<Text>,
    pub patient_fhir_id: Option<Text>,
    pub smart_config: Option<Text>,
    pub capability: Option<Text>,
    pub software_version: Option<Text>,
    pub scopes: Option<Text>,
    pub access_token: Option<Text>,
    pub refresh_token: Option<Text>,
    pub token_expiry: Option<Text>,
    pub last_fetch: Option<Text>,
    pub last_full_fetch: Option<Text>,
}

/// One `OperationOutcome` issue found in a search reply (DATA-MODEL.md §7.3).
pub struct WarningRow {
    pub id: RowId,
    pub fhir_type: Text,
    pub severity: Option<Text>,
    pub code: Option<Text>,
    pub text: Option<Text>,
    pub at: Text,
}

/// One reference between rows (DATA-MODEL.md §4). The row keys and table names are plain,
/// so the store can join on them; what the link means is sealed.
pub struct LinkRow {
    pub id: RowId,
    pub from_table: Table,
    pub from_id: RowId,
    pub to_table: Table,
    /// `None` until the target row is fetched.
    pub to_id: Option<RowId>,
    /// Why the rows are tied: ordered, collected, contains, … (DATA-MODEL.md §4).
    pub role: Text,
    /// The FHIR field the link came from, e.g. `Observation.basedOn`.
    pub source_field: Text,
    /// The FHIR reference as sent, e.g. `ServiceRequest/…`, so a later fetch can resolve it.
    pub target_ref: Text,
}
