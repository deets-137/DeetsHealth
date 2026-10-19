# archive/ — the patient app

The patient health-record viewer for Windows (Tauri v2, WebView2) that DeetsHealth was before
the conversion to the hospital phone agent (`docs/CONVERSION.md`, 2026-10-19).

- Each file sits at its old path under `archive/` (`src-tauri/x.rs` → `archive/src-tauri/x.rs`),
  so links between archived files still work.
- **Nothing here is built or checked.** The Cargo workspace, `package.json` and
  `npm run docs:check` leave this folder out.
- **To run the patient app again**, check out the commit before the conversion: branch
  `archive/patient-app` (commit `073b9b3`), or the tag `patient-app-final` once it is pushed.
- To bring one piece back, copy it out of here and read its doc first.
