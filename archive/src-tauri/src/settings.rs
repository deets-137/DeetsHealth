//! Back-end settings: the few choices the Rust side must know BEFORE the webview is up.
//! Front-end preferences (theme, skin, surface, the Settings card's rows) stay in the
//! page's own store (src/settings-store.ts); this file is deliberately tiny.
//!
//! Persisted as JSON at `<app_data>/settings.json`. Every field has a default, so a missing
//! or stale file never blocks startup. **Nothing protected goes here** (SECURITY.md §4.1):
//! the file is plain JSON. A value that names a person or a site belongs in the sealed store.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsData {
    /// Where the window last sat, as [x, y] outer position, so the next launch opens it
    /// there (window.rs). A screen position, not an identifier.
    pub window_pos: Option<[i32; 2]>,
}

pub struct Settings {
    path: PathBuf,
    data: Mutex<SettingsData>,
}

impl Settings {
    /// Read `<dir>/settings.json`; a missing or unreadable file gives the defaults.
    pub fn load(dir: PathBuf) -> Self {
        let path = dir.join("settings.json");
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<SettingsData>(&t).ok())
            .unwrap_or_default();
        Self { path, data: Mutex::new(data) }
    }

    pub fn get(&self) -> SettingsData {
        self.data.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Change the data and write the file. The write goes to a temporary file first, then
    /// replaces the old one, so a crash mid-write never leaves half a file.
    pub fn update(&self, f: impl FnOnce(&mut SettingsData)) -> Result<(), String> {
        let snapshot = {
            let mut d = self.data.lock().unwrap_or_else(|p| p.into_inner());
            f(&mut d);
            d.clone()
        };
        let text = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}
