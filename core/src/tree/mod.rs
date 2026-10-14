//! The record view (GLOBALS.md): DeetsHealth's globals in DeetsEngine, in memory.
//!
//! The sealed store stays the record on disk. [`Tree::build`] unseals it into a tree of
//! globals with declared indexes and pointers, rebuilt whole at each open and after each
//! fetch (D-A + R-A lite + L-A, HANDOFF.md › Decided). No other code touches the engine:
//! - **In:** values come from the store as [`Shielded`] and are revealed here, with the
//!   [`TreePass`], only to be written into the tree.
//! - **Inside:** the engine holds them as plain strings, wiped when they drop (its memory
//!   store, DeetsEngine STORAGE.md §9).
//! - **Out:** every read wraps the value again before it returns. An IEN is the engine's
//!   number for a record, not record data, so it comes back plain.
//! - **Errors:** an engine error becomes [`CoreError::Tree`] with the code of its redacted
//!   report. Nothing else of it is kept: its text can name record data.

mod date;
mod load;
mod normalize;
mod schema;

use crate::store::{PatientRow, ResourceRow, SiteRow, Table, Text, WarningRow};
use crate::{CoreError, Shielded};
use almirah_engine::{Data, Direction, File, Format, Globals, MemoryStore, Sub};
use std::collections::BTreeMap;

/// Proof that the caller may read plaintext: the record view's load and reads. Built only in
/// this module (the fourth pass, SECURITY.md §4.2; R-A lite: "a new pass").
pub struct TreePass {
    _only_here: (),
}

impl TreePass {
    fn issue() -> Self {
        Self { _only_here: () }
    }

    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self::issue()
    }
}

/// One global of the tree (GLOBALS.md §5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Global {
    Enc,
    Res,
    Ord,
    Med,
    Chart,
    Doc,
    Party,
    Link,
}

impl Global {
    pub const ALL: [Global; 8] =
        [Global::Enc, Global::Res, Global::Ord, Global::Med, Global::Chart, Global::Doc, Global::Party, Global::Link];

    /// The engine's name for it (`^RES` is `RES`).
    pub fn name(self) -> &'static str {
        self.file().global
    }

    fn file(self) -> &'static File {
        schema::FILES[self as usize]
    }

    fn format(self) -> &'static Format {
        match self {
            Global::Enc => &schema::ENC_FMT,
            Global::Res => &schema::RES_FMT,
            Global::Ord => &schema::ORD_FMT,
            Global::Med => &schema::MED_FMT,
            Global::Chart => &schema::CHART_FMT,
            Global::Doc => &schema::DOC_FMT,
            Global::Party => &schema::PARTY_FMT,
            Global::Link => &schema::LINK_FMT,
        }
    }

    /// The store tables it holds. `billing`, `other` and `definition` have no global
    /// (GLOBALS.md §5.9).
    fn tables(self) -> &'static [Table] {
        match self {
            Global::Enc => &[Table::Encounter],
            Global::Res => &[Table::Result],
            Global::Ord => &[Table::Order],
            Global::Med => &[Table::Medication, Table::Drug],
            Global::Chart => &[Table::ChartItem],
            Global::Doc => &[Table::Document],
            Global::Party => &[Table::Party],
            Global::Link => &[],
        }
    }

    fn of_table(t: Table) -> Option<Global> {
        Global::ALL.into_iter().find(|g| g.tables().contains(&t))
    }

    /// The standard code systems of its concept key, first choice first (GLOBALS.md §5.3).
    fn standard(self, fhir_type: &str) -> &'static [&'static str] {
        match (self, fhir_type) {
            (Global::Res | Global::Ord, _) => &["loinc"],
            (Global::Med, "Immunization" | "ImmunizationRecommendation" | "ImmunizationEvaluation") => &["cvx"],
            (Global::Med, _) => &["rxnorm"],
            (Global::Chart, _) => &["snomed", "icd10cm", "rxnorm"],
            _ => &[],
        }
    }
}

/// What the load reads from one site's store. The SQLite store fills it on the desktop.
pub trait TreeSource {
    fn resources(&self, table: Table) -> Result<Vec<ResourceRow>, CoreError>;
    fn type_counts(&self, table: Table) -> Result<BTreeMap<String, u64>, CoreError>;
    fn patient(&self) -> Result<Option<PatientRow>, CoreError>;
    fn site(&self) -> Result<Option<SiteRow>, CoreError>;
    fn warnings(&self) -> Result<Vec<WarningRow>, CoreError>;
}

#[cfg(feature = "sqlite")]
impl<C: crate::platform::Cipher> TreeSource for crate::store::SiteStore<C> {
    fn resources(&self, table: Table) -> Result<Vec<ResourceRow>, CoreError> {
        crate::store::SiteStore::resources(self, table)
    }
    fn type_counts(&self, table: Table) -> Result<BTreeMap<String, u64>, CoreError> {
        crate::store::Store::type_counts(self, table)
    }
    fn patient(&self) -> Result<Option<PatientRow>, CoreError> {
        crate::store::SiteStore::patient(self)
    }
    fn site(&self) -> Result<Option<SiteRow>, CoreError> {
        crate::store::SiteStore::site(self)
    }
    fn warnings(&self) -> Result<Vec<WarningRow>, CoreError> {
        crate::store::SiteStore::warnings(self)
    }
}

/// How many records one global holds. Counts only: safe for a log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Count {
    pub global: &'static str,
    pub records: u64,
    /// Records made from a reference whose target the site did not send.
    pub stubs: u64,
    /// `^RES` only: records made from an `Observation`'s components.
    pub components: u64,
}

/// One record with every field of its format, in order ([`Tree::records`]).
pub type RecordRow = (u64, Vec<(&'static str, Text)>);

/// A value to walk an index by: an IEN (a pointer) or a value from the record.
pub enum Key {
    Ien(u64),
    Value(Text),
}

pub(crate) fn engine(e: almirah_engine::Error) -> CoreError {
    CoreError::Tree(e.redacted().code)
}

/// The record view of one patient's record, from one or more sites. `Send`, not `Sync`.
pub struct Tree {
    db: Globals<MemoryStore>,
    counts: Vec<Count>,
    pass: TreePass,
}

impl Tree {
    /// Build the tree from each site's store; site numbers are 1, 2, … in the order given.
    /// `now_ms` is the platform's clock, in milliseconds since 1970 (the engine stamps nothing
    /// the app reads with it). Off the UI thread: it unseals the whole record.
    pub fn build<S: TreeSource>(sites: &[&S], now_ms: impl Fn() -> i64 + Send + 'static) -> Result<Tree, CoreError> {
        let pass = TreePass::issue();
        let mut db = Globals::new(MemoryStore::new(now_ms));
        for f in schema::FILES {
            db.declare(f).map_err(engine)?;
        }
        let mut b = load::Builder::new(&pass);
        for (i, src) in sites.iter().enumerate() {
            b.site(i as u32 + 1, *src)?;
        }
        b.finish();
        let counts = b.write(&mut db)?;
        Ok(Tree { db, counts, pass })
    }

    /// Records per global.
    pub fn counts(&self) -> &[Count] {
        &self.counts
    }

    fn unpacked(&self, g: Global, ien: u64) -> Result<Option<almirah_engine::Record>, CoreError> {
        let Some(v) = self.db.get(g.name(), &[Sub::from(ien as i64)]).map_err(engine)? else { return Ok(None) };
        let v = Shielded::new(v);
        g.format().unpack(v.reveal(&self.pass)).map(Some).map_err(engine)
    }

    /// One field of a record. `None` when the record does not exist; an empty value when the
    /// record does not have the field.
    pub fn field(&self, g: Global, ien: u64, name: &str) -> Result<Option<Text>, CoreError> {
        Ok(self.unpacked(g, ien)?.map(|r| Shielded::new(r.get(name).unwrap_or_default().to_owned())))
    }

    /// A pointer field: the IEN it holds, in its own global.
    pub fn pointer(&self, g: Global, ien: u64, name: &str) -> Result<Option<u64>, CoreError> {
        Ok(self.unpacked(g, ien)?.and_then(|r| r.get(name)?.parse().ok()))
    }

    /// A list field's values (`codes`, `diagnoses`), or one sub-field across a list's groups
    /// (`reactions.substance`).
    pub fn list(&self, g: Global, ien: u64, name: &str) -> Result<Vec<Text>, CoreError> {
        Ok(self.unpacked(g, ien)?.map(|r| r.list(name).into_iter().map(|v| Shielded::new(v.to_owned())).collect()).unwrap_or_default())
    }

    /// A result's notes, in order (`^RES(ien,"note",n)`).
    pub fn notes(&self, ien: u64) -> Result<Vec<Text>, CoreError> {
        let mut path = vec![Sub::from(ien as i64), Sub::Str("note".into())];
        let mut out = Vec::new();
        let mut at = None;
        while let Some(n) = self.db.order("RES", &path, at.as_ref(), Direction::Forward).map_err(engine)? {
            path.push(n.clone());
            if let Some(v) = self.db.get("RES", &path).map_err(engine)? {
                out.push(Shielded::new(v));
            }
            path.pop();
            at = Some(n);
        }
        Ok(out)
    }

    fn sub(&self, k: &Key) -> Sub {
        match k {
            Key::Ien(n) => Sub::from(*n as i64),
            Key::Value(v) => almirah_engine::index_value(v.reveal(&self.pass)),
        }
    }

    /// The IENs under `prefix` in one index, in index order (or reversed, newest first for a
    /// date). `walk(Res, "by-trend", [kind, concept])` is one test's values by date.
    pub fn walk(&self, g: Global, index: &str, prefix: &[Key], newest_first: bool) -> Result<Vec<u64>, CoreError> {
        let mut path: Vec<Sub> = std::iter::once(Sub::Str(index.into())).chain(prefix.iter().map(|k| self.sub(k))).collect();
        let dir = if newest_first { Direction::Backward } else { Direction::Forward };
        let mut out = Vec::new();
        self.collect(g, &mut path, dir, &mut out)?;
        Ok(out)
    }

    fn collect(&self, g: Global, path: &mut Vec<Sub>, dir: Direction, out: &mut Vec<u64>) -> Result<(), CoreError> {
        let mut at = None;
        while let Some(s) = self.db.order(g.name(), path, at.as_ref(), dir).map_err(engine)? {
            path.push(s.clone());
            match self.db.data(g.name(), path).map_err(engine)? {
                Data::Value => {
                    if let Ok(ien) = s.to_m_string().parse() {
                        out.push(ien);
                    }
                }
                Data::Children | Data::ValueAndChildren => self.collect(g, path, dir, out)?,
                Data::None => {}
            }
            path.pop();
            at = Some(s);
        }
        Ok(())
    }

    /// The distinct values at the next level of an index under `prefix`, in order.
    /// `values(Res, "by-trend", [kind])` is every test of one kind.
    pub fn values(&self, g: Global, index: &str, prefix: &[Key]) -> Result<Vec<Text>, CoreError> {
        let path: Vec<Sub> = std::iter::once(Sub::Str(index.into())).chain(prefix.iter().map(|k| self.sub(k))).collect();
        let mut out = Vec::new();
        let mut at = None;
        while let Some(s) = self.db.order(g.name(), &path, at.as_ref(), Direction::Forward).map_err(engine)? {
            out.push(Shielded::new(s.to_m_string()));
            at = Some(s);
        }
        Ok(out)
    }

    /// The record of one store row, by its row key (`by-row`): how a card finds its record
    /// again after a rebuild.
    pub fn by_row(&self, g: Global, site: u32, row: &str) -> Result<Option<u64>, CoreError> {
        let prefix = [Key::Ien(u64::from(site)), Key::Value(Shielded::new(row.to_owned()))];
        Ok(self.walk(g, "by-row", &prefix, false)?.first().copied())
    }

    /// One field of a site node: `^PAT(site)` (`name`, `birth_date`, `sex`) or `^SITE(site)`
    /// (`name`, `last_fetch`).
    pub fn site_field(&self, node: SiteNode, site: u32, name: &str) -> Result<Option<Text>, CoreError> {
        let (global, format) = match node {
            SiteNode::Patient => ("PAT", &schema::PAT_FMT),
            SiteNode::Site => ("SITE", &schema::SITE_FMT),
        };
        let Some(v) = self.db.get(global, &[Sub::from(i64::from(site))]).map_err(engine)? else { return Ok(None) };
        let v = Shielded::new(v);
        let r = format.unpack(v.reveal(&self.pass)).map_err(engine)?;
        Ok(r.get(name).map(|x| Shielded::new(x.to_owned())))
    }

    /// What the fetch saw for one FHIR type at one site: rows stored, and each warning's code.
    pub fn fetch(&self, site: u32, fhir_type: &str) -> Result<Option<(u64, Vec<Text>)>, CoreError> {
        let Some(v) = self.db.get("FETCH", &[Sub::from(i64::from(site)), Sub::Str(fhir_type.into())]).map_err(engine)? else {
            return Ok(None);
        };
        let v = Shielded::new(v);
        let r = schema::FETCH_FMT.unpack(v.reveal(&self.pass)).map_err(engine)?;
        let rows = r.get("rows").and_then(|n| n.parse().ok()).unwrap_or(0);
        Ok(Some((rows, r.list("warnings").into_iter().map(|c| Shielded::new(c.to_owned())).collect())))
    }

    /// How many entries one index holds (the inspector's shape view).
    pub fn index_size(&self, g: Global, index: &str) -> Result<u64, CoreError> {
        Ok(self.walk(g, index, &[], false)?.len() as u64)
    }

    /// Every record of a global, in IEN order, with every field of its format in order: a
    /// list's values joined with ` | `, a group's sub-fields with `, ` (the inspector's export).
    pub fn records(&self, g: Global) -> Result<Vec<RecordRow>, CoreError> {
        let specs = g.format().versions[g.format().versions.len() - 1];
        let mut out = Vec::new();
        let mut at = None;
        while let Some(s) = self.db.order(g.name(), &[], at.as_ref(), Direction::Forward).map_err(engine)? {
            // Records are numbers; index names are strings, and sort after them.
            let Ok(ien) = s.to_m_string().parse::<u64>() else { break };
            if let Some(r) = self.unpacked(g, ien)? {
                let cells = specs
                    .iter()
                    .map(|spec| {
                        let name = spec.split(['[', '.']).next().unwrap_or(spec);
                        let value = if spec.contains('[') && spec.contains(',') {
                            r.groups(name).iter().map(|grp| grp.join(", ")).collect::<Vec<_>>().join(" | ")
                        } else if spec.contains('[') || spec.ends_with("...") {
                            r.list(name).join(" | ")
                        } else {
                            r.get(name).unwrap_or_default().to_owned()
                        };
                        (name, Shielded::new(value))
                    })
                    .collect();
                out.push((ien, cells));
            }
            at = Some(s);
        }
        Ok(out)
    }

    /// The index names of a global, in declared order.
    pub fn indexes(g: Global) -> Vec<&'static str> {
        g.file().indexes.iter().map(|i| i.name).collect()
    }
}

/// The two per-site nodes outside the declared globals.
#[derive(Clone, Copy)]
pub enum SiteNode {
    Patient,
    Site,
}

#[cfg(test)]
mod tests;
