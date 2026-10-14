// Dev tools (feature `dev`, never in a release; docs/ops/DEBUGGING.md). The inspector opens
// each site's sealed store with this PC's keys and prints the core's report
// (core/src/dev.rs): counts, links, warnings and shapes for every site; texts, the patient
// and sample resources only for a sandbox site.
//
//   npm run inspect -- [--dir <app data>] [--site <key>] [--shapes] [--sample N] [--type T]... [--out <file>] [--export <folder>]
//
// `--export` (or `--csv`): a sandbox site's whole record as CSV files and one workbook,
// `record.xlsx` (xlsx.rs), in `<folder>\<site>\`. The core refuses a real site.
//
// The default folder is the dev app's, `%APPDATA%\com.deetshealth.dev`. A process started
// inside the Claude desktop app sees a private copy of AppData (CLAUDE.md › How to verify), so
// start it from your own terminal, or through WMI with `--out`.

use crate::sites::SiteFiles;
use deetshealth_core::dev::{self, Options};
use deetshealth_core::platform::SiteKey;
use std::path::PathBuf;

/// Run the inspector with these arguments (the program name already removed). Returns the
/// process exit code.
pub fn inspect(args: Vec<String>) -> i32 {
    let mut dir = std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("com.deetshealth.dev"));
    let mut only: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut csv: Option<PathBuf> = None;
    let mut opts = Options::default();
    let mut tree = false;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dir" => dir = it.next().map(PathBuf::from),
            "--site" => only = it.next(),
            "--shapes" => opts.shapes = true,
            "--tree" => tree = true,
            "--sample" => opts.sample = it.next().and_then(|n| n.parse().ok()).unwrap_or(1),
            "--type" => opts.sample_types.extend(it.next()),
            "--out" => out = it.next().map(PathBuf::from),
            "--export" | "--csv" => csv = it.next().map(PathBuf::from),
            "--help" | "-h" => {
                println!("npm run inspect -- [--dir <app data>] [--site <key>] [--shapes] [--tree] [--sample N] [--type T]... [--out <file>] [--export <folder>]");
                println!("  --tree: build the record view (GLOBALS.md) and print its counts and index sizes (any site)");
                println!("  --export (or --csv): a sandbox site's whole record as CSV files and record.xlsx, one folder per site (a real site is refused)");
                return 0;
            }
            other => {
                eprintln!("unknown argument: {other} (try --help)");
                return 2;
            }
        }
    }
    let Some(dir) = dir else {
        eprintln!("no app data folder: pass --dir");
        return 2;
    };
    let files = SiteFiles::new(&dir);
    let mut sites = files.list();
    if let Some(k) = &only {
        sites.retain(|s| s.as_str() == k);
    }
    let mut text = format!("DeetsHealth inspector · {} · {} site(s)\n", dir.display(), sites.len());
    if sites.is_empty() {
        text.push_str("no sites (sign in first, or check --dir)\n");
    }
    for site in &sites {
        text.push_str(&format!("\n════ site {} ════\n", short(site)));
        match files.open(site) {
            Ok(Some(store)) => {
                match dev::report(&store, &opts) {
                    Ok(r) => text.push_str(&r),
                    Err(e) => text.push_str(&format!("could not read: {e}\n")),
                }
                if tree {
                    match dev::tree_report(&store) {
                        Ok(r) => text.push_str(&r),
                        Err(e) => text.push_str(&format!("\nrecord view: could not build: {e}\n")),
                    }
                }
                if let Some(root) = &csv {
                    text.push_str(&write_csv(&store, &root.join(short(site))));
                }
            }
            Ok(None) => text.push_str("no key for this site (erased?)\n"),
            Err(e) => text.push_str(&format!("could not open: {e}\n")),
        }
    }
    match out {
        Some(path) => match std::fs::write(&path, &text) {
            Ok(()) => {
                println!("written: {}", path.display());
                0
            }
            Err(e) => {
                eprintln!("could not write {}: {e}", path.display());
                1
            }
        },
        None => {
            print!("{text}");
            0
        }
    }
}

/// Write one site's export into `dir`: a CSV file per table and `record.xlsx` with every table
/// as a sheet. The core refuses a real site.
fn write_csv(store: &deetshealth_core::store::SiteStore<crate::cng::Cng>, dir: &std::path::Path) -> String {
    let mut tables = match dev::export_tables(store) {
        Ok(None) => return "\nexport: refused, a real site (sandbox sites only)\n".to_owned(),
        Err(e) => return format!("\nexport: could not read: {e}\n"),
        Ok(Some(t)) => t,
    };
    // The record view's globals after the store's tables (GLOBALS.md).
    let tree_note = match dev::export_tree(store) {
        Ok(Some(t)) => {
            tables.extend(t);
            String::new()
        }
        Ok(None) => String::new(),
        Err(e) => format!(" (the record view could not be built: {e})"),
    };
    if let Err(e) = std::fs::create_dir_all(dir) {
        return format!("\nexport: could not make {}: {e}\n", dir.display());
    }
    let mut written = 0;
    for (name, rows) in &tables {
        // A UTF-8 mark first, so Excel reads accents right.
        if std::fs::write(dir.join(format!("{name}.csv")), format!("\u{feff}{}", dev::to_csv(rows))).is_ok() {
            written += 1;
        }
    }
    let sheets: Vec<crate::xlsx::SheetData> = tables.iter().map(|(n, rows)| crate::xlsx::SheetData { name: n, rows }).collect();
    let about = [
        "DeetsHealth: a sandbox patient's record (public test data)",
        "Made by `npm run inspect -- --export` (docs/ops/DEBUGGING.md §5). Sandbox sites only: the tool refuses a real record.",
        "The `row` column joins every sheet. Several values in one field are joined with ' | '.",
        "The ^ sheets are the record view (docs/architecture/GLOBALS.md): `ien` is the record's number, and a pointer field holds an IEN in its own global.",
    ];
    let book = crate::xlsx::workbook(&about, &sheets, describe);
    let xlsx = match std::fs::write(dir.join("record.xlsx"), book) {
        Ok(()) => "record.xlsx",
        Err(_) => "no workbook (could not write record.xlsx)",
    };
    format!("\nexport: {written} CSV files and {xlsx} in {}{tree_note}\n", dir.display())
}

/// The About sheet's line for a table.
fn describe(name: &str) -> String {
    match name {
        "records" => "One row per record: table, FHIR type, kind, code, display name, status, dates. The fields the cards sort and group by.".into(),
        "patient" => "Name, birth date, sex: all the app keeps from the Patient resource.".into(),
        "links" => "Which record points at which. resolved = no: the target was never fetched.".into(),
        "warnings" => "The site's notes on each search, in full text.".into(),
        "^ENC" => "The record view: visits (and stub visits the site did not send).".into(),
        "^RES" => "The record view: every result, one value per record; components and panel members point at their group (parent, seq, root).".into(),
        "^ORD" => "The record view: orders other than medicines.".into(),
        "^MED" => "The record view: medicine orders, fills, doses given, vaccines and drug records.".into(),
        "^CHART" => "The record view: problems, diagnoses, allergies, procedures, goals, care plans.".into(),
        "^DOC" => "The record view: notes and forms (the note's text is not loaded).".into(),
        "^PARTY" => "The record view: people, places and organizations; mostly stubs with the reference's display text.".into(),
        "^LINK" => "The record view: links with no pointer field.".into(),
        t => format!("One row per {t} resource, one column per JSON field path."),
    }
}

/// The site's app key, shortened: it is our own random name, not record data.
fn short(site: &SiteKey) -> &str {
    &site.as_str()[..8.min(site.as_str().len())]
}
