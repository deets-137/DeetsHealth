//! The globals' declarations (GLOBALS.md §5.2): one packed format and one `File` per global.
//!
//! Every record's format starts with the same head fields, then its own. A field that holds an
//! IEN is a pointer; an index on it reads the pointer the other way. A record with an empty
//! indexed field has no entry in that index (DeetsEngine IENS-AND-INDEXES.md), so a missing
//! date is never empty: it is [`super::date::NO_DATE`].
//!
//! A change to a field list is a new format version here, and nothing else: the tree is
//! rebuilt from the sealed store at every open, so no record of an old version ever exists.

use almirah_engine::{File, Format, Index, Source};

/// The fields every record has, in this order, at the start of its format.
pub(crate) const HEAD: [&str; 12] =
    ["site", "fhir", "fhir_id", "version", "row", "kind", "status", "date", "precision", "mark", "stub", "display"];

macro_rules! record_format {
    ($name:literal, [$($f:literal),* $(,)?]) => {
        Format::new($name, &[&[
            "site", "fhir", "fhir_id", "version", "row", "kind", "status", "date", "precision", "mark", "stub", "display",
            $($f),*
        ]])
    };
}

pub(crate) static ENC_FMT: Format = record_format!("enc", [
    "concept", "concept_text", "class", "start", "end", "location", "provider", "parent", "episode",
    "diagnoses[]", "participants[]",
]);
pub(crate) static RES_FMT: Format = record_format!("res", [
    "concept", "concept_text", "value_kind", "comparator", "value", "unit", "ucum", "low", "high", "range", "flag",
    "enc", "order", "parent", "root", "seq", "specimen", "codes...",
]);
pub(crate) static ORD_FMT: Format = record_format!("ord", [
    "concept", "concept_text", "intent", "priority", "ordered", "occurrence", "enc", "parent", "requester", "number",
    "reasons[]",
]);
pub(crate) static MED_FMT: Format = record_format!("med", [
    "concept", "concept_text", "drug", "dose", "dose_unit", "route", "frequency", "as_needed", "sig", "quantity",
    "refills", "supply_days", "order", "enc", "requester", "dose_number", "lot", "reasons[]",
]);
pub(crate) static CHART_FMT: Format = record_format!("chart", [
    "concept", "concept_text", "clinical", "verification", "onset", "abatement", "recorded", "enc", "asserter",
    "performer", "category", "criticality", "reactions[substance,manifestation,severity]",
]);
pub(crate) static DOC_FMT: Format = record_format!("doc", [
    "concept", "concept_text", "description", "enc", "author", "order", "contents[type,binary]",
]);
pub(crate) static PARTY_FMT: Format = record_format!("party", ["concept", "concept_text", "name"]);

/// A link with no pointer field (GLOBALS.md §5.8): both ends as (global, IEN).
pub(crate) static LINK_FMT: Format = Format::new("link", &[&["from_global", "from_ien", "to_global", "to_ien", "role", "field"]]);
/// `^PAT(site)`: the patient as one site knows them.
pub(crate) static PAT_FMT: Format = Format::new("pat", &[&["name", "birth_date", "sex"]]);
/// `^SITE(site)`: the site's name as the user picked it, and its last fetch.
pub(crate) static SITE_FMT: Format = Format::new("site", &[&["name", "last_fetch"]]);
/// `^FETCH(site, FHIR type)`: rows stored, and the code of each warning the site sent.
pub(crate) static FETCH_FMT: Format = Format::new("fetch", &[&["rows", "warnings[]"]]);

macro_rules! p {
    ($fmt:ident, $f:literal) => {
        Source::Packed(&$fmt, $f)
    };
}

/// One record per store row (and per stub, and per component): by its row key, unique per site.
/// A stub and a component have no row, so no entry.
macro_rules! by_row {
    ($fmt:ident) => {
        Index::unique("by-row", &[p!($fmt, "site"), p!($fmt, "row")])
    };
}

pub(crate) static ENC: File = File {
    global: "ENC",
    key_depth: 1,
    indexes: &[
        Index::new("by-date", &[p!(ENC_FMT, "date")]),
        Index::new("by-kind", &[p!(ENC_FMT, "kind"), p!(ENC_FMT, "date")]),
        Index::new("by-diagnosis", &[Source::Each(&ENC_FMT, "diagnoses")]),
        Index::new("by-parent", &[p!(ENC_FMT, "parent")]),
        by_row!(ENC_FMT),
    ],
};

pub(crate) static RES: File = File {
    global: "RES",
    key_depth: 1,
    indexes: &[
        Index::new("by-trend", &[p!(RES_FMT, "kind"), p!(RES_FMT, "concept"), p!(RES_FMT, "date")]),
        Index::new("by-enc", &[p!(RES_FMT, "enc"), p!(RES_FMT, "date")]),
        Index::new("by-parent", &[p!(RES_FMT, "parent"), p!(RES_FMT, "seq")]),
        Index::new("by-root", &[p!(RES_FMT, "root")]),
        Index::new("by-order", &[p!(RES_FMT, "order")]),
        Index::new("by-date", &[p!(RES_FMT, "date")]),
        Index::new("by-code", &[Source::Each(&RES_FMT, "codes")]),
        by_row!(RES_FMT),
    ],
};

pub(crate) static ORD: File = File {
    global: "ORD",
    key_depth: 1,
    indexes: &[
        Index::new("by-enc", &[p!(ORD_FMT, "enc"), p!(ORD_FMT, "date")]),
        Index::new("by-date", &[p!(ORD_FMT, "date")]),
        Index::new("by-parent", &[p!(ORD_FMT, "parent")]),
        Index::new("by-number", &[p!(ORD_FMT, "number")]),
        by_row!(ORD_FMT),
    ],
};

pub(crate) static MED: File = File {
    global: "MED",
    key_depth: 1,
    indexes: &[
        Index::new("by-medicine", &[p!(MED_FMT, "concept"), p!(MED_FMT, "date")]),
        Index::new("by-kind", &[p!(MED_FMT, "kind"), p!(MED_FMT, "date")]),
        Index::new("by-order", &[p!(MED_FMT, "order")]),
        Index::new("by-enc", &[p!(MED_FMT, "enc"), p!(MED_FMT, "date")]),
        Index::new("by-reason", &[Source::Each(&MED_FMT, "reasons")]),
        by_row!(MED_FMT),
    ],
};

pub(crate) static CHART: File = File {
    global: "CHART",
    key_depth: 1,
    indexes: &[
        Index::new("by-kind", &[p!(CHART_FMT, "kind"), p!(CHART_FMT, "date")]),
        Index::new("by-enc", &[p!(CHART_FMT, "enc"), p!(CHART_FMT, "date")]),
        Index::new("by-date", &[p!(CHART_FMT, "date")]),
        by_row!(CHART_FMT),
    ],
};

pub(crate) static DOC: File = File {
    global: "DOC",
    key_depth: 1,
    indexes: &[
        Index::new("by-enc", &[p!(DOC_FMT, "enc"), p!(DOC_FMT, "date")]),
        Index::new("by-date", &[p!(DOC_FMT, "date")]),
        by_row!(DOC_FMT),
    ],
};

pub(crate) static PARTY: File = File { global: "PARTY", key_depth: 1, indexes: &[by_row!(PARTY_FMT)] };

pub(crate) static LINK: File = File {
    global: "LINK",
    key_depth: 1,
    indexes: &[
        Index::new("by-from", &[p!(LINK_FMT, "from_global"), p!(LINK_FMT, "from_ien"), p!(LINK_FMT, "role")]),
        Index::new("by-to", &[p!(LINK_FMT, "to_global"), p!(LINK_FMT, "to_ien"), p!(LINK_FMT, "role")]),
    ],
};

pub(crate) static FILES: [&File; 8] = [&ENC, &RES, &ORD, &MED, &CHART, &DOC, &PARTY, &LINK];
