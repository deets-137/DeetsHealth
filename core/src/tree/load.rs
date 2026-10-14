//! The load (GLOBALS.md §5.7): the sealed store's rows become the tree's records.
//!
//! 1. **Number** every row of every table that has a global. The map from (site, FHIR type,
//!    FHIR id) to IEN lives only for the load; its ids are shielded, and wiped when it drops.
//! 2. **Plan** each record: unseal the raw resource, normalize it, and fill each pointer from
//!    its references. A target the site did not send becomes a stub (§5.5).
//! 3. **Finish** what needs every record first: a result's parent and place in its group, its
//!    root, and a medicine's concept from its drug.
//! 4. **Write** each global in batches of a few hundred records (DeetsEngine request doc §8).
//!    Each record is written once, so the engine keeps its indexes once per record.

use super::normalize::{self, sh, Fields};
use super::schema::{self, HEAD};
use super::{engine, Count, Global, TreePass, TreeSource};
use crate::index::{self, Ref};
use crate::store::{Table, Text};
use crate::CoreError;
use almirah_engine::{Format, Globals, MemoryStore, Sub};
use serde_json::Value;
use std::collections::BTreeMap;

/// Records per transaction. The undo log holds one entry per node until its transaction
/// commits, so a smaller batch keeps the load's peak memory low.
const BATCH: usize = 300;

/// How far up a chain of parents the root is looked for. A real culture is 3 or 4 levels.
const MAX_DEPTH: usize = 32;

/// One record before it is written.
#[derive(Default)]
struct Plan {
    fields: Fields,
    notes: Vec<Text>,
}

impl Plan {
    fn get(&self, name: &str) -> Option<&Text> {
        self.fields.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
    }

    fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
}

#[derive(Clone, Copy)]
struct Target {
    global: Global,
    ien: u64,
}

/// A node outside the declared globals: `^PAT(site)`, `^SITE(site)`, `^FETCH(site, type)`.
struct Node {
    global: &'static str,
    subs: Vec<Sub>,
    value: Text,
}

pub(super) struct Builder<'p> {
    pass: &'p TreePass,
    next: [u64; Global::ALL.len()],
    /// (site, FHIR type, FHIR id) → the record. Stubs are added as they are made.
    ids: BTreeMap<(u32, String, Text), Target>,
    plans: [BTreeMap<u64, Plan>; Global::ALL.len()],
    /// A result's groups, in the order found: (group IEN, place, the group is an Observation).
    groups: BTreeMap<u64, Vec<(u64, u32, bool)>>,
    nodes: Vec<Node>,
    stubs: [u64; Global::ALL.len()],
    components: u64,
}

impl<'p> Builder<'p> {
    pub(super) fn new(pass: &'p TreePass) -> Self {
        Self {
            pass,
            next: [0; Global::ALL.len()],
            ids: BTreeMap::new(),
            plans: Default::default(),
            groups: BTreeMap::new(),
            nodes: Vec::new(),
            stubs: [0; Global::ALL.len()],
            components: 0,
        }
    }

    fn alloc(&mut self, g: Global) -> u64 {
        self.next[g as usize] += 1;
        self.next[g as usize]
    }

    fn reveal<'a>(&self, t: &'a Text) -> &'a str {
        t.reveal(self.pass)
    }

    /// Steps 1 and 2 for one site's store.
    pub(super) fn site<S: TreeSource>(&mut self, site: u32, src: &S) -> Result<(), CoreError> {
        let mut rows = Vec::new();
        for g in Global::ALL {
            for table in g.tables() {
                for row in src.resources(*table)? {
                    let ien = self.alloc(g);
                    let fhir_type = self.reveal(&row.fhir_type).to_owned();
                    self.ids.insert((site, fhir_type, row.fhir_id.clone()), Target { global: g, ien });
                    rows.push((g, ien, row));
                }
            }
        }
        for (g, ien, row) in &rows {
            self.record(site, *g, *ien, row)?;
        }
        drop(rows);
        self.site_nodes(site, src)
    }

    /// The patient, the site and what the fetch saw, per site.
    fn site_nodes<S: TreeSource>(&mut self, site: u32, src: &S) -> Result<(), CoreError> {
        let s = Sub::from(i64::from(site));
        if let Some(p) = src.patient()? {
            let name = match &p.name {
                Some(n) => serde_json::from_str::<Value>(self.reveal(n)).ok().and_then(|v| normalize::human_name(&v)),
                None => None,
            };
            let fields = vec![
                ("name", sh(name.unwrap_or_default())),
                ("birth_date", sh(p.birth_date.as_ref().map(|b| self.reveal(b).to_owned()).unwrap_or_default())),
                ("sex", sh(p.sex.as_ref().map(|x| self.reveal(x).to_owned()).unwrap_or_default())),
            ];
            let value = self.pack(&schema::PAT_FMT, &fields)?;
            self.nodes.push(Node { global: "PAT", subs: vec![s.clone()], value });
        }
        if let Some(r) = src.site()? {
            let text = |t: &Option<Text>| sh(t.as_ref().map(|v| self.reveal(v).to_owned()).unwrap_or_default());
            let fields = vec![("name", text(&r.display_name)), ("last_fetch", text(&r.last_fetch))];
            let value = self.pack(&schema::SITE_FMT, &fields)?;
            self.nodes.push(Node { global: "SITE", subs: vec![s.clone()], value });
        }
        let mut fetch: BTreeMap<String, (u64, Vec<String>)> = BTreeMap::new();
        for table in Table::ALL {
            for (t, n) in src.type_counts(table)? {
                fetch.entry(t).or_default().0 += n;
            }
        }
        for w in src.warnings()? {
            let code = w.code.as_ref().map(|c| self.reveal(c).to_owned()).unwrap_or_default();
            fetch.entry(self.reveal(&w.fhir_type).to_owned()).or_default().1.push(code);
        }
        for (t, (rows, warnings)) in fetch {
            let mut fields = vec![("rows", sh(rows.to_string()))];
            fields.extend(warnings.into_iter().map(|c| ("warnings", sh(c))));
            let value = self.pack(&schema::FETCH_FMT, &fields)?;
            self.nodes.push(Node { global: "FETCH", subs: vec![s.clone(), Sub::Str(t)], value });
        }
        Ok(())
    }

    /// Step 2 for one row.
    fn record(&mut self, site: u32, g: Global, ien: u64, row: &crate::store::ResourceRow) -> Result<(), CoreError> {
        let t = self.reveal(&row.fhir_type).to_owned();
        let raw: Value = serde_json::from_str(self.reveal(&row.raw)).map_err(|_| CoreError::Parse)?;
        let ix = index::extract(&t, &raw);
        let refs = index::references(&t, &raw);
        let opt = |v: &Option<Text>| v.as_ref().map(|x| self.reveal(x).to_owned());

        let mut p = Plan::default();
        let (date, precision, mark) = [(&ix.start, "care"), (&ix.recorded, "recorded"), (&ix.last_updated, "updated")]
            .into_iter()
            .find_map(|(v, mark)| super::date::key(&opt(v)?).map(|k| (k.key, k.precision, mark)))
            .unwrap_or_else(|| (super::date::NO_DATE.to_owned(), "", "none"));
        p.fields.extend([
            ("site", sh(site.to_string())),
            ("fhir", sh(t.clone())),
            ("fhir_id", row.fhir_id.clone()),
            ("version", sh(raw.pointer("/meta/versionId").and_then(Value::as_str).unwrap_or_default())),
            ("row", sh(row.id.as_str())),
            ("kind", sh(opt(&ix.kind).unwrap_or_default())),
            ("status", sh(opt(&ix.status).unwrap_or_default())),
            ("date", sh(date)),
            ("precision", sh(precision)),
            ("mark", sh(mark)),
            ("stub", sh("0")),
            ("display", sh("")),
        ]);

        let cc = index::code_field(&t).iter().find_map(|f| raw.get(*f));
        let concept = cc.map(|cc| normalize::concept(site, g.standard(&t), cc));
        if let Some(c) = &concept {
            p.fields.push(("concept", sh(c.key.clone().unwrap_or_default())));
            p.fields.push(("concept_text", sh(c.text.clone().unwrap_or_default())));
        }
        match g {
            Global::Enc => {
                p.fields.extend(normalize::encounter(&raw));
                p.fields.push(("start", sh(opt(&ix.start).unwrap_or_default())));
                p.fields.push(("end", sh(opt(&ix.end).unwrap_or_default())));
            }
            Global::Res => {
                if t == "Observation" {
                    p.fields.extend(normalize::value(&raw));
                }
                if let Some(c) = &concept {
                    p.fields.extend(c.codes.iter().map(|code| ("codes", sh(code.clone()))));
                }
                p.notes = normalize::notes(&raw);
            }
            Global::Ord => p.fields.extend(normalize::order(&raw)),
            Global::Med => p.fields.extend(normalize::medicine(&raw)),
            Global::Chart => {
                p.fields.extend(normalize::chart(&raw));
                p.fields.push(("onset", sh(opt(&ix.start).unwrap_or_default())));
                p.fields.push(("abatement", sh(opt(&ix.end).unwrap_or_default())));
                p.fields.push(("recorded", sh(opt(&ix.recorded).unwrap_or_default())));
            }
            Global::Doc => p.fields.extend(normalize::document(&raw)),
            Global::Party => p.fields.extend(normalize::party(&t, &raw)),
            Global::Link => {}
        }
        self.pointers(site, g, ien, &t, &refs, &mut p);

        if g == Global::Res && t == "Observation" {
            self.components(site, ien, &raw, &p);
        }
        self.plans[g as usize].insert(ien, p);
        Ok(())
    }

    /// A component (G2 A): a child record with no FHIR id of its own. Its key is its parent's
    /// FHIR id and its place; it shares its parent's kind, status, date and visit.
    fn components(&mut self, site: u32, parent: u64, raw: &Value, p: &Plan) {
        let list = raw.get("component").and_then(Value::as_array).cloned().unwrap_or_default();
        for (i, c) in list.iter().enumerate() {
            let ien = self.alloc(Global::Res);
            let mut child = Plan::default();
            for name in HEAD {
                let v = match name {
                    "row" => sh(""),
                    _ => p.get(name).cloned().unwrap_or_else(|| sh("")),
                };
                child.fields.push((name, v));
            }
            if let Some(cc) = c.get("code") {
                let k = normalize::concept(site, Global::Res.standard("Observation"), cc);
                child.fields.push(("concept", sh(k.key.unwrap_or_default())));
                child.fields.push(("concept_text", sh(k.text.unwrap_or_default())));
                child.fields.extend(k.codes.into_iter().map(|code| ("codes", sh(code))));
            }
            child.fields.extend(normalize::value(c));
            if let Some(enc) = p.get("enc") {
                child.fields.push(("enc", enc.clone()));
            }
            child.fields.push(("parent", sh(parent.to_string())));
            child.fields.push(("seq", sh((i + 1).to_string())));
            self.components += 1;
            self.plans[Global::Res as usize].insert(ien, child);
        }
    }

    /// The record a reference points at: a stored row, or a stub made once per target. `None`
    /// when the target's table has no global (GLOBALS.md §5.9).
    fn resolve(&mut self, site: u32, r: &Ref) -> Option<Target> {
        let key = (site, r.target_type.clone(), r.target_id.clone());
        if let Some(t) = self.ids.get(&key) {
            return Some(*t);
        }
        let g = Global::of_table(r.target_table)?;
        let ien = self.alloc(g);
        let mut stub = Plan::default();
        stub.fields.extend([
            ("site", sh(site.to_string())),
            ("fhir", sh(r.target_type.clone())),
            ("fhir_id", r.target_id.clone()),
            ("version", sh("")),
            ("row", sh("")),
            ("kind", sh(stub_kind(&r.target_type))),
            ("status", sh("")),
            ("date", sh(super::date::NO_DATE)),
            ("precision", sh("")),
            ("mark", sh("none")),
            ("stub", sh("1")),
            ("display", r.display.clone().unwrap_or_else(|| sh(""))),
        ]);
        self.plans[g as usize].insert(ien, stub);
        self.stubs[g as usize] += 1;
        let target = Target { global: g, ien };
        self.ids.insert(key, target);
        Some(target)
    }

    /// Each pointer field from its references (GLOBALS.md §5.8). A reference no pointer field
    /// takes becomes a `^LINK` record, when its target is in the tree.
    fn pointers(&mut self, site: u32, g: Global, ien: u64, t: &str, refs: &[Ref], p: &mut Plan) {
        let mut used = vec![false; refs.len()];
        let path = |r: &Ref| r.source_field.split_once('.').map_or("", |(_, rest)| rest).to_owned();
        let to = |r: &Ref| Global::of_table(r.target_table);
        let mut take = |b: &mut Self, p: &mut Plan, name: &'static str, many: bool, pred: &dyn Fn(&Ref) -> bool| {
            for (i, r) in refs.iter().enumerate() {
                if used[i] || !pred(r) {
                    continue;
                }
                if let Some(target) = b.resolve(site, r) {
                    used[i] = true;
                    p.fields.push((name, sh(target.ien.to_string())));
                    if !many {
                        return;
                    }
                }
            }
        };
        if g != Global::Enc {
            let visit = |r: &Ref| to(r) == Some(Global::Enc) && matches!(path(r).as_str(), "encounter" | "context" | "context.encounter");
            take(self, p, "enc", false, &visit);
        }
        let role = |want: &'static str, target: Global| move |r: &Ref| r.role == want && to(r) == Some(target);
        match g {
            Global::Enc => {
                take(self, p, "location", false, &role("location", Global::Party));
                take(self, p, "provider", false, &role("service-provider", Global::Party));
                take(self, p, "parent", false, &role("parent", Global::Enc));
                take(self, p, "episode", false, &role("episode", Global::Enc));
                take(self, p, "diagnoses", true, &role("diagnosed", Global::Chart));
                take(self, p, "participants", true, &role("participant", Global::Party));
            }
            Global::Res => {
                take(self, p, "order", false, &role("fulfils", Global::Ord));
                take(self, p, "specimen", false, &role("specimen", Global::Res));
                let mut seq = 0;
                for (i, r) in refs.iter().enumerate() {
                    if used[i] || r.role != "contains" || to(r) != Some(Global::Res) {
                        continue;
                    }
                    if let Some(child) = self.resolve(site, r) {
                        used[i] = true;
                        seq += 1;
                        self.groups.entry(child.ien).or_default().push((ien, seq, t == "Observation"));
                    }
                }
            }
            Global::Ord => {
                take(self, p, "parent", false, &role("fulfils", Global::Ord));
                take(self, p, "requester", false, &role("requester", Global::Party));
                take(self, p, "reasons", true, &role("reason", Global::Chart));
            }
            Global::Med => {
                take(self, p, "drug", false, &role("drug", Global::Med));
                let of_order = |r: &Ref| {
                    to(r) == Some(Global::Med)
                        && (r.role == "fulfils"
                            || matches!(r.source_field.as_str(), "MedicationDispense.authorizingPrescription" | "MedicationAdministration.request"))
                };
                take(self, p, "order", false, &of_order);
                take(self, p, "requester", false, &role("requester", Global::Party));
                take(self, p, "reasons", true, &role("reason", Global::Chart));
            }
            Global::Chart => {
                take(self, p, "asserter", false, &role("author", Global::Party));
                take(self, p, "performer", false, &role("performer", Global::Party));
            }
            Global::Doc => {
                take(self, p, "author", false, &role("author", Global::Party));
                take(self, p, "order", false, &role("fulfils", Global::Ord));
            }
            Global::Party | Global::Link => {}
        }
        for (i, r) in refs.iter().enumerate() {
            if used[i] {
                continue;
            }
            let Some(target) = self.resolve(site, r) else { continue };
            let link = self.alloc(Global::Link);
            let fields = vec![
                ("from_global", sh(g.name())),
                ("from_ien", sh(ien.to_string())),
                ("to_global", sh(target.global.name())),
                ("to_ien", sh(target.ien.to_string())),
                ("role", sh(r.role)),
                ("field", sh(r.source_field.clone())),
            ];
            self.plans[Global::Link as usize].insert(link, Plan { fields, notes: Vec::new() });
        }
    }

    /// Step 3.
    pub(super) fn finish(&mut self) {
        let res = Global::Res as usize;
        // A result's group: an Observation group (a panel's `hasMember`) before a report, since
        // the report holds that Observation in turn.
        let groups = std::mem::take(&mut self.groups);
        for (child, mut found) in groups {
            found.sort_by_key(|(_, _, obs)| !*obs);
            if let Some(plan) = self.plans[res].get_mut(&child) {
                if !plan.has("parent") {
                    let (parent, seq, _) = found[0];
                    plan.fields.push(("parent", sh(parent.to_string())));
                    plan.fields.push(("seq", sh(seq.to_string())));
                }
            }
        }
        let parent_of: BTreeMap<u64, u64> = self.plans[res]
            .iter()
            .filter_map(|(ien, p)| Some((*ien, p.get("parent")?.reveal(self.pass).parse().ok()?)))
            .collect();
        for (ien, plan) in self.plans[res].iter_mut() {
            let mut root = *ien;
            for _ in 0..MAX_DEPTH {
                match parent_of.get(&root) {
                    Some(up) if *up != *ien => root = *up,
                    _ => break,
                }
            }
            plan.fields.push(("root", sh(root.to_string())));
        }
        // A medicine named by reference takes its drug's concept, so `by-medicine` groups every
        // order of one drug (GLOBALS.md §5.2).
        let med = Global::Med as usize;
        let from_drug: Vec<(u64, Text, Text)> = self.plans[med]
            .iter()
            .filter(|(_, p)| p.get("concept").is_none_or(|c| c.reveal(self.pass).is_empty()))
            .filter_map(|(ien, p)| {
                let drug: u64 = p.get("drug")?.reveal(self.pass).parse().ok()?;
                let d = self.plans[med].get(&drug)?;
                Some((*ien, d.get("concept")?.clone(), d.get("concept_text").cloned().unwrap_or_else(|| sh(""))))
            })
            .collect();
        for (ien, concept, text) in from_drug {
            if let Some(p) = self.plans[med].get_mut(&ien) {
                p.fields.retain(|(n, _)| !matches!(*n, "concept" | "concept_text"));
                p.fields.push(("concept", concept));
                p.fields.push(("concept_text", text));
            }
        }
    }

    /// Step 4.
    pub(super) fn write(self, db: &mut Globals<MemoryStore>) -> Result<Vec<Count>, CoreError> {
        let mut counts = Vec::new();
        for g in Global::ALL {
            let plans = &self.plans[g as usize];
            let mut packed = Vec::with_capacity(plans.len());
            for (ien, p) in plans {
                packed.push((*ien, self.pack(g.format(), &p.fields)?, &p.notes));
            }
            for batch in packed.chunks(BATCH) {
                db.transaction(|db| {
                    for (ien, value, notes) in batch {
                        let rec = Sub::from(*ien as i64);
                        db.set(g.name(), std::slice::from_ref(&rec), value.reveal(self.pass))?;
                        for (n, note) in notes.iter().enumerate() {
                            let subs = [rec.clone(), Sub::Str("note".into()), Sub::from(n as i64 + 1)];
                            db.set(g.name(), &subs, note.reveal(self.pass))?;
                        }
                    }
                    Ok(())
                })
                .map_err(engine)?;
            }
            counts.push(Count {
                global: g.name(),
                records: plans.len() as u64,
                stubs: self.stubs[g as usize],
                components: if g == Global::Res { self.components } else { 0 },
            });
        }
        db.transaction(|db| {
            for n in &self.nodes {
                db.set(n.global, &n.subs, n.value.reveal(self.pass))?;
            }
            Ok(())
        })
        .map_err(engine)?;
        Ok(counts)
    }

    /// One packed value. A single field the record does not have is written empty.
    fn pack(&self, format: &Format, fields: &Fields) -> Result<Text, CoreError> {
        let mut values: Vec<(&str, &str)> = fields.iter().map(|(n, v)| (*n, v.reveal(self.pass).as_str())).collect();
        for spec in format.versions[format.versions.len() - 1] {
            let single = !spec.ends_with("...") && !spec.contains('[');
            if single && !values.iter().any(|(n, _)| n == spec) {
                values.push((spec, ""));
            }
        }
        format.pack(&values).map(sh).map_err(engine)
    }
}

/// A stub's kind: the word a stored row of that type would have (INDEX-AND-LINKS.md §4), for
/// the types that have one; else the type in lower case.
fn stub_kind(t: &str) -> String {
    match t {
        "Encounter" => "visit".into(),
        "Appointment" => "planned".into(),
        "MedicationRequest" => "ordered".into(),
        "DocumentReference" => "note".into(),
        "AllergyIntolerance" => "allergy".into(),
        other => other.to_lowercase(),
    }
}
