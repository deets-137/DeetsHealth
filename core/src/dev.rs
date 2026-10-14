//! The dev inspector (docs/ops/DEBUGGING.md): what a site's sealed store holds, for the
//! developer. Feature `dev` only: a release never builds it (the owner, 2026-10-13: "set up dev
//! mode / admin utilities to be able to read sandbox data specifically").
//!
//! Two levels, decided per site **inside this module**:
//! - **Every site:** kinds and counts, links, warnings as words (`fhir::issue_word`), and
//!   **shapes**: which JSON fields each type has, and how often. Never a value. This is safe
//!   for a real record (SECURITY.md §4.5), and it is what FHIR-SHAPES.md needs.
//! - **A sandbox site only** (its base URL is one of [`auth::sandbox_sites`], compared
//!   shielded): also warning texts, the patient row, and whole sample resources. Sandbox data
//!   is public fake data, which SECURITY.md §4.8 lets into docs and chat (the owner confirmed
//!   2026-10-13).
//!
//! [`DevPass`] is the fifth pass (shielded.rs). It is made only here: for a sandbox site, and
//! inside three helpers that let out only FHIR's own words, whatever the site: [`shape_of`]
//! (field paths, every value dropped), `plain_type` (a FHIR type with a table) and
//! `warning_word` (our codes, or FHIR's issue words).

use crate::auth;
use crate::fhir;
use crate::net::Text;
use crate::platform::Cipher;
use crate::store::{SiteStore, Store, Table};
use crate::{CoreError, Shielded};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The dev inspector's pass. Its field is private, so only this module can make one.
pub struct DevPass {
    _only_here: (),
}

impl DevPass {
    fn issue() -> Self {
        Self { _only_here: () }
    }
}

/// What to print beyond the counts.
#[derive(Default)]
pub struct Options {
    /// The fields of each type, with how many resources have each.
    pub shapes: bool,
    /// Sandbox only: up to this many whole resources of each type named in `sample_types`
    /// (all types when it is empty).
    pub sample: usize,
    pub sample_types: Vec<String>,
}

/// Our own warning codes (sync.rs `warn`): fixed words, so they can be shown as they are.
const OWN_CODES: [&str; 6] = ["network", "unreadable-page", "next-link-off-site", "token-refresh", "seal", "store"];

/// Which sandbox this site is, by its base URL, compared shielded. `None`: a real site.
fn sandbox_name<C: Cipher>(store: &SiteStore<C>) -> Result<Option<&'static str>, CoreError> {
    let Some(site) = store.site()? else { return Ok(None) };
    let Some(base) = site.base_url else { return Ok(None) };
    Ok(auth::sandbox_sites().into_iter().find(|e| Shielded::new(e.base_url.clone()) == base).map(|e| e.name))
}

/// The report for one site's store, as text.
pub fn report<C: Cipher>(store: &SiteStore<C>, opts: &Options) -> Result<String, CoreError> {
    let sandbox = sandbox_name(store)?;
    let pass = sandbox.map(|_| DevPass::issue());
    let show = |t: &Text| -> String { pass.as_ref().map_or("[sealed]".to_owned(), |p| t.reveal(p).clone()) };
    let mut out = String::new();
    let w = &mut out;

    // ── the site ──
    match sandbox {
        Some(name) => writeln!(w, "site: {name} (sandbox: texts and samples shown)").ok(),
        None => writeln!(w, "site: a real site (counts, words and shapes only)").ok(),
    };
    writeln!(w, "index version: {}", store.index_version()?).ok();
    if let Some(site) = store.site()? {
        if let Some(s) = &site.scopes {
            // The site's words for what we may read: not record data.
            writeln!(w, "granted scopes: {}", crate::net::read_scopes(s)).ok();
        }
        let stamp = |v: &Option<Text>| v.as_ref().and_then(crate::net::read_u64).map_or("never".to_owned(), |n| n.to_string());
        writeln!(w, "last fetch: {} (seconds since 1970)", stamp(&site.last_fetch)).ok();
    }
    if let (Some(p), Some(pass)) = (store.patient()?, &pass) {
        let v = |t: &Option<Text>| t.as_ref().map_or("-".to_owned(), |t| t.reveal(pass).clone());
        writeln!(w, "patient: name {} · born {} · sex {}", v(&p.name), v(&p.birth_date), v(&p.sex)).ok();
    }

    // ── rows ──
    writeln!(w, "\nrows by table and FHIR type:").ok();
    let mut total = 0;
    for table in Table::ALL {
        let mut by_type: BTreeMap<String, u32> = BTreeMap::new();
        for (_, t, _) in store.keys(table)? {
            *by_type.entry(t).or_default() += 1;
        }
        if by_type.is_empty() {
            continue;
        }
        let n: u32 = by_type.values().sum();
        total += n;
        let list = by_type.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
        writeln!(w, "  {:<11} {n:>5}  {list}", table.name()).ok();
    }
    writeln!(w, "  {:<11} {total:>5}", "all").ok();

    // ── links ──
    let links = store.all_links()?;
    let waiting: Vec<_> = links.iter().filter(|l| l.to_id.is_none()).collect();
    writeln!(w, "\nlinks: {} ({} resolved, {} waiting)", links.len(), links.len() - waiting.len(), waiting.len()).ok();
    let mut pairs: BTreeMap<String, u32> = BTreeMap::new();
    for l in &links {
        let state = if l.to_id.is_some() { "" } else { " (waiting)" };
        *pairs.entry(format!("{} -> {}{state}", l.from_table.name(), l.to_table.name())).or_default() += 1;
    }
    for (k, v) in &pairs {
        writeln!(w, "  {k}: {v}").ok();
    }
    // Per target type: the waiting links, and the distinct records they name (compared
    // shielded): what a read of each missing record would cost, in calls.
    let mut waiting_types: BTreeMap<String, (u32, std::collections::BTreeSet<Text>)> = BTreeMap::new();
    for l in &waiting {
        let (word, id) = match crate::net::split_ref(&l.target_ref) {
            Some((t, id)) if fhir::table_for(&t).is_some() => (t, Some(id)),
            Some(_) => ("other".to_owned(), None),
            None => ("not-a-reference".to_owned(), None),
        };
        let e = waiting_types.entry(word).or_default();
        e.0 += 1;
        e.1.extend(id);
    }
    if !waiting_types.is_empty() {
        writeln!(w, "  waiting links point at (links → distinct records):").ok();
        for (k, (n, ids)) in &waiting_types {
            writeln!(w, "    {k}: {n} → {}", ids.len()).ok();
        }
    }

    // ── warnings ──
    let warnings = store.warnings()?;
    writeln!(w, "\nwarnings: {}", warnings.len()).ok();
    let mut by_word: BTreeMap<String, u32> = BTreeMap::new();
    for x in &warnings {
        *by_word.entry(format!("{} {}", plain_type(&x.fhir_type), warning_word(x))).or_default() += 1;
    }
    for (k, v) in &by_word {
        writeln!(w, "  {k}: {v}").ok();
    }
    if pass.is_some() {
        let mut texts: BTreeMap<String, u32> = BTreeMap::new();
        for x in &warnings {
            let text = x.text.as_ref().map_or("(no text)".to_owned(), &show);
            *texts.entry(format!("{} {} — {text}", plain_type(&x.fhir_type), warning_word(x))).or_default() += 1;
        }
        writeln!(w, "  texts (sandbox):").ok();
        for (k, v) in &texts {
            writeln!(w, "    ×{v}  {k}").ok();
        }
    }

    // ── shapes and samples ──
    if opts.shapes || opts.sample > 0 {
        for table in Table::ALL {
            let rows = store.resources(table)?;
            let mut by_type: BTreeMap<String, Vec<Text>> = BTreeMap::new();
            for r in rows {
                by_type.entry(plain_type(&r.fhir_type)).or_default().push(r.raw);
            }
            for (t, raws) in by_type {
                if opts.shapes {
                    writeln!(w, "\nshape of {t} ({} resources, table {}):", raws.len(), table.name()).ok();
                    let mut paths: BTreeMap<String, u32> = BTreeMap::new();
                    for raw in &raws {
                        for p in shape_of(raw) {
                            *paths.entry(p).or_default() += 1;
                        }
                    }
                    for (p, n) in &paths {
                        writeln!(w, "  {n:>5}/{:<5} {p}", raws.len()).ok();
                    }
                }
                let wanted = opts.sample_types.is_empty() || opts.sample_types.iter().any(|s| s == &t);
                if let (true, true, Some(p)) = (opts.sample > 0, wanted, &pass) {
                    for raw in raws.iter().take(opts.sample) {
                        let pretty = serde_json::from_str::<Value>(raw.reveal(p))
                            .and_then(|v| serde_json::to_string_pretty(&v))
                            .unwrap_or_else(|_| "(not JSON)".to_owned());
                        writeln!(w, "\nsample {t} (sandbox):\n{pretty}").ok();
                    }
                }
            }
        }
        if opts.sample > 0 && pass.is_none() {
            writeln!(w, "\nsamples: none (a real site shows shapes only)").ok();
        }
    }
    Ok(out)
}

/// One exported table: its name and its rows, the header first.
pub type ExportTable = (String, Vec<Vec<String>>);

/// The whole record as tables, **for a sandbox site only** (`None` for a real site): the data
/// to model cards on (the owner, 2026-10-13). The host writes them as CSV files ([`to_csv`])
/// and as one workbook. In this order:
/// - `records`: one row per resource, with its index columns (kind, code, dates, status).
/// - `patient`.
/// - `<Type>`, `Encounter` first: one row per resource of that type, one column per JSON field
///   path (the most common first); a field with several values (an array) joins them with ` | `.
/// - `links`, `warnings`.
///
/// `row` is our own row key, cut to 10 characters: it joins the tables.
pub fn export_tables<C: Cipher>(store: &SiteStore<C>) -> Result<Option<Vec<ExportTable>>, CoreError> {
    if sandbox_name(store)?.is_none() {
        return Ok(None);
    }
    let pass = DevPass::issue();
    let show = |t: &Option<Text>| t.as_ref().map_or(String::new(), |t| t.reveal(&pass).clone());
    let short = |r: &crate::store::RowId| r.as_str()[..10].to_owned();
    let mut files = Vec::new();
    let mut type_of_row: BTreeMap<String, String> = BTreeMap::new();

    let mut records = Sheet::new(&[
        "row", "table", "type", "fhir_id", "kind", "code_system", "code", "display", "status", "start", "end", "recorded",
        "last_updated",
    ]);
    // Per FHIR type: (row, FHIR id, its field paths and values).
    type Flat = (String, String, Vec<(String, String)>);
    let mut by_type: BTreeMap<String, Vec<Flat>> = BTreeMap::new();
    for table in Table::ALL {
        for r in store.resources(table)? {
            let t = r.fhir_type.reveal(&pass).clone();
            let row = short(&r.id);
            type_of_row.insert(row.clone(), t.clone());
            records.row(vec![
                row.clone(),
                table.name().to_owned(),
                t.clone(),
                r.fhir_id.reveal(&pass).clone(),
                r.kind.reveal(&pass).clone(),
                show(&r.code_system),
                show(&r.code),
                show(&r.code_display),
                show(&r.status),
                show(&r.start),
                show(&r.end),
                show(&r.recorded),
                show(&r.last_updated),
            ]);
            let fields = serde_json::from_str::<Value>(r.raw.reveal(&pass)).map(|v| flatten(&v)).unwrap_or_default();
            by_type.entry(t).or_default().push((row, r.fhir_id.reveal(&pass).clone(), fields));
        }
    }
    files.push(("records".to_owned(), records.rows));

    for (t, rows) in by_type {
        // Columns: every path, the most common first, then by name.
        let mut seen: BTreeMap<String, u32> = BTreeMap::new();
        for (_, _, fields) in &rows {
            let once: std::collections::BTreeSet<&String> = fields.iter().map(|(p, _)| p).collect();
            for p in once {
                *seen.entry(p.clone()).or_default() += 1;
            }
        }
        let mut cols: Vec<(String, u32)> = seen.into_iter().collect();
        cols.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut head = vec!["row".to_owned(), "fhir_id".to_owned()];
        head.extend(cols.iter().map(|(p, _)| p.clone()));
        let mut csv = Sheet::new(&head.iter().map(String::as_str).collect::<Vec<_>>());
        for (row, id, fields) in rows {
            let mut cells = vec![row, id];
            for (p, _) in &cols {
                let vals: Vec<&str> = fields.iter().filter(|(q, _)| q == p).map(|(_, v)| v.as_str()).collect();
                cells.push(vals.join(" | "));
            }
            csv.row(cells);
        }
        files.push((t, csv.rows));
    }

    let mut links = Sheet::new(&["from_row", "from_type", "role", "source_field", "to_table", "to_row", "to_type", "target", "resolved"]);
    for l in store.all_links()? {
        let from = short(&l.from_id);
        let to = l.to_id.as_ref().map(short).unwrap_or_default();
        links.row(vec![
            from.clone(),
            type_of_row.get(&from).cloned().unwrap_or_default(),
            l.role.reveal(&pass).clone(),
            l.source_field.reveal(&pass).clone(),
            l.to_table.name().to_owned(),
            to.clone(),
            type_of_row.get(&to).cloned().unwrap_or_default(),
            l.target_ref.reveal(&pass).clone(),
            if l.to_id.is_some() { "yes" } else { "no" }.to_owned(),
        ]);
    }
    files.push(("links".to_owned(), links.rows));

    let mut warnings = Sheet::new(&["type", "severity", "code", "text"]);
    for x in store.warnings()? {
        warnings.row(vec![x.fhir_type.reveal(&pass).clone(), show(&x.severity), show(&x.code), show(&x.text)]);
    }
    files.push(("warnings".to_owned(), warnings.rows));

    let mut patient = Sheet::new(&["fhir_id", "name", "birth_date", "sex"]);
    if let Some(p) = store.patient()? {
        patient.row(vec![p.fhir_id.reveal(&pass).clone(), show(&p.name), show(&p.birth_date), show(&p.sex)]);
    }
    files.push(("patient".to_owned(), patient.rows));
    // The overview first, then the visit at the centre, then the rest; the plumbing last.
    let rank = |n: &str| match n {
        "records" => 0,
        "patient" => 1,
        "Encounter" => 2,
        "links" => 4,
        "warnings" => 5,
        _ => 3,
    };
    files.sort_by_key(|(n, _)| rank(n));
    Ok(Some(files))
}

/// The record view built from this store (GLOBALS.md), as counts: records, stubs and
/// components per global, and the entries of every index. Numbers only, so safe for any site.
pub fn tree_report<C: Cipher>(store: &SiteStore<C>) -> Result<String, CoreError> {
    use crate::tree::{Global, Tree};
    let started = std::time::Instant::now();
    let tree = Tree::build(&[store], || 0)?;
    let ms = started.elapsed().as_millis();
    let mut out = String::new();
    let w = &mut out;
    writeln!(w, "\nrecord view (GLOBALS.md): built in {ms} ms").ok();
    for (g, c) in Global::ALL.into_iter().zip(tree.counts()) {
        let extra = if c.components > 0 { format!(", {} components", c.components) } else { String::new() };
        writeln!(w, "  ^{:<6} {:>6} records ({} stubs{extra})", c.global, c.records, c.stubs).ok();
        let sizes: Vec<String> = Tree::indexes(g)
            .into_iter()
            .map(|ix| format!("{ix} {}", tree.index_size(g, ix).map_or("?".to_owned(), |n| n.to_string())))
            .collect();
        if !sizes.is_empty() {
            writeln!(w, "          {}", sizes.join(" · ")).ok();
        }
    }
    Ok(out)
}

/// The record view's globals as tables, **for a sandbox site only** (`None` for a real site):
/// one sheet per global (`^ENC`, `^RES`, …), one row per record, one column per field. The
/// first column is the IEN; a pointer field holds the IEN of a record in its own global.
pub fn export_tree<C: Cipher>(store: &SiteStore<C>) -> Result<Option<Vec<ExportTable>>, CoreError> {
    use crate::tree::{Global, Tree};
    if sandbox_name(store)?.is_none() {
        return Ok(None);
    }
    let pass = DevPass::issue();
    let tree = Tree::build(&[store], || 0)?;
    let mut files = Vec::new();
    for g in Global::ALL {
        let records = tree.records(g)?;
        let Some((_, first)) = records.first() else { continue };
        let mut head = vec!["ien"];
        head.extend(first.iter().map(|(n, _)| *n));
        let mut sheet = Sheet::new(&head);
        for (ien, cells) in &records {
            let mut row = vec![ien.to_string()];
            row.extend(cells.iter().map(|(_, v)| v.reveal(&pass).clone()));
            sheet.row(row);
        }
        files.push((format!("^{}", g.name()), sheet.rows));
    }
    Ok(Some(files))
}

/// A table as CSV text (RFC 4180: a field with a comma, a quote or a line break is quoted).
pub fn to_csv(rows: &[Vec<String>]) -> String {
    let mut text = String::new();
    for cells in rows {
        let line: Vec<String> = cells
            .iter()
            .map(|c| if c.contains([',', '"', '\n', '\r']) { format!("\"{}\"", c.replace('"', "\"\"")) } else { c.clone() })
            .collect();
        text.push_str(&line.join(","));
        text.push_str("\r\n");
    }
    text
}

/// Every leaf of a resource as (field path, value), in document order.
fn flatten(v: &Value) -> Vec<(String, String)> {
    fn go(v: &Value, path: String, out: &mut Vec<(String, String)>) {
        match v {
            Value::Object(map) => {
                for (k, child) in map {
                    go(child, if path.is_empty() { k.clone() } else { format!("{path}.{k}") }, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    go(item, format!("{path}[]"), out);
                }
            }
            Value::String(s) => out.push((path, s.clone())),
            Value::Null => {}
            other => out.push((path, other.to_string())),
        }
    }
    let mut out = Vec::new();
    go(v, String::new(), &mut out);
    out
}

/// A table being built, the header first.
struct Sheet {
    rows: Vec<Vec<String>>,
}

impl Sheet {
    fn new(head: &[&str]) -> Self {
        Self { rows: vec![head.iter().map(|s| (*s).to_owned()).collect()] }
    }

    fn row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }
}

/// A resource's FHIR type is FHIR's word, but stored sealed; let it out only when it is a type
/// with a table.
fn plain_type(t: &Text) -> String {
    let pass = DevPass::issue();
    let s = t.reveal(&pass);
    if fhir::table_for(s).is_some() || s == "Patient" || s == "OperationOutcome" { s.clone() } else { "other".to_owned() }
}

/// A stored warning as a word: our own code as it is, or the site's issue as
/// [`fhir::issue_word`] gives it.
fn warning_word(x: &crate::store::WarningRow) -> String {
    if let Some(code) = &x.code {
        let own = OWN_CODES.iter().any(|c| *code == Shielded::new((*c).to_owned()));
        let http = (100..600).any(|n| *code == Shielded::new(format!("http-{n}")));
        if own || http {
            return code.reveal(&DevPass::issue()).clone();
        }
    }
    fhir::issue_word(&fhir::Issue { severity: x.severity.clone(), code: x.code.clone(), text: None, detail: None })
}

/// The field paths of one resource, values dropped: `code.coding[].system`. Only JSON keys
/// leave this function, and FHIR's keys are FHIR's own field names. A key that is not a plain
/// field name (letters, digits, `_`) is written `?`, so nothing odd gets out.
pub fn shape_of(raw: &Text) -> Vec<String> {
    let pass = DevPass::issue();
    let Ok(v) = serde_json::from_str::<Value>(raw.reveal(&pass)) else { return Vec::new() };
    let mut out = Vec::new();
    walk(&v, String::new(), &mut out);
    out.sort();
    out.dedup();
    out
}

fn walk(v: &Value, path: String, out: &mut Vec<String>) {
    match v {
        Value::Object(map) => {
            for (k, child) in map {
                let k = if k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { k.as_str() } else { "?" };
                let p = if path.is_empty() { k.to_owned() } else { format!("{path}.{k}") };
                walk(child, p, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(item, format!("{path}[]"), out);
            }
        }
        leaf => {
            let kind = match leaf {
                Value::String(_) => "text",
                Value::Number(_) => "number",
                Value::Bool(_) => "bool",
                _ => "null",
            };
            out.push(format!("{path}: {kind}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shape_has_paths_and_no_values() {
        let raw = Shielded::new(r#"{"resourceType":"Observation","code":{"coding":[{"system":"http://loinc.org","code":"4548-4"}]},"valueQuantity":{"value":6.1},"weird key":"x"}"#.to_owned());
        let s = shape_of(&raw);
        assert!(s.contains(&"code.coding[].system: text".to_owned()));
        assert!(s.contains(&"valueQuantity.value: number".to_owned()));
        assert!(s.contains(&"?: text".to_owned()));
        assert!(!s.iter().any(|p| p.contains("loinc") || p.contains("4548") || p.contains("6.1") || p.contains("weird")));
    }

    #[test]
    fn csv_quotes_and_flatten_joins_paths() {
        let mut c = Sheet::new(&["a", "b"]);
        c.row(vec!["x,y".into(), "say \"hi\"".into()]);
        assert_eq!(to_csv(&c.rows), "a,b\r\n\"x,y\",\"say \"\"hi\"\"\"\r\n");
        let v: Value = serde_json::from_str(r#"{"code":{"coding":[{"code":"1"},{"code":"2"}]},"n":5,"z":null}"#).unwrap();
        assert_eq!(flatten(&v), vec![("code.coding[].code".into(), "1".into()), ("code.coding[].code".into(), "2".into()), ("n".into(), "5".into())]);
    }

    #[test]
    fn our_codes_and_site_issues_become_words() {
        let row = |code: &str, sev: &str| crate::store::WarningRow {
            id: crate::store::RowId::from_stored("0".repeat(32)).unwrap(),
            fhir_type: Shielded::new("CarePlan".into()),
            severity: Some(Shielded::new(sev.into())),
            code: Some(Shielded::new(code.into())),
            text: Some(Shielded::new("a site's text".into())),
            at: Shielded::new("0".into()),
        };
        assert_eq!(warning_word(&row("http-400", "error")), "http-400");
        assert_eq!(warning_word(&row("required", "error")), "error:required");
        assert_eq!(warning_word(&row("a name", "error")), "error:other");
    }
}
