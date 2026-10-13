//! Reads `epic-client.json` at the repo root (git-ignored; integrations/EPIC.md §2) and gives
//! its values to the core as build-time constants. With no file, the Epic entry is left out of
//! the Connect menu. The client ID is never printed: cargo shows a build script's output only
//! when the build fails, and this script writes only `cargo:` lines.

use std::path::Path;

fn main() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../epic-client.json");
    println!("cargo:rerun-if-changed={}", file.display());
    let Ok(text) = std::fs::read_to_string(&file) else { return };
    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => panic!("epic-client.json is not valid JSON ({e}); see integrations/EPIC.md §2"),
    };
    let field = |name: &str| json.get(name).and_then(|v| v.as_str()).unwrap_or("").trim().to_owned();

    let id = field("nonprod_client_id");
    if id.is_empty() {
        return;
    }
    let scopes = field("scope_version");
    if scopes != "v1" && scopes != "v2" {
        panic!("epic-client.json: scope_version must be \"v1\" or \"v2\" (the registration's SMART scope version)");
    }
    println!("cargo:rustc-env=DEETSHEALTH_EPIC_CLIENT_ID={id}");
    println!("cargo:rustc-env=DEETSHEALTH_EPIC_SCOPES={scopes}");
}
