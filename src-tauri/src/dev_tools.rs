// Dev tools (feature `dev`, never in a release; docs/ops/DEBUGGING.md). The inspector opens
// each site's sealed store with this PC's keys and prints the core's report
// (core/src/dev.rs): counts, links, warnings and shapes for every site; texts, the patient
// and sample resources only for a sandbox site.
//
//   npm run inspect -- [--dir <app data>] [--site <key>] [--shapes] [--sample N] [--type T]... [--out <file>]
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
    let mut opts = Options::default();
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dir" => dir = it.next().map(PathBuf::from),
            "--site" => only = it.next(),
            "--shapes" => opts.shapes = true,
            "--sample" => opts.sample = it.next().and_then(|n| n.parse().ok()).unwrap_or(1),
            "--type" => opts.sample_types.extend(it.next()),
            "--out" => out = it.next().map(PathBuf::from),
            "--help" | "-h" => {
                println!("npm run inspect -- [--dir <app data>] [--site <key>] [--shapes] [--sample N] [--type T]... [--out <file>]");
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
            Ok(Some(store)) => match dev::report(&store, &opts) {
                Ok(r) => text.push_str(&r),
                Err(e) => text.push_str(&format!("could not read: {e}\n")),
            },
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

/// The site's app key, shortened: it is our own random name, not record data.
fn short(site: &SiteKey) -> &str {
    &site.as_str()[..8.min(site.as_str().len())]
}
