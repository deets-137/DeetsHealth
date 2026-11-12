// npm run types — writes the TypeScript type of every card payload and event into archive/src/model/
// (CORE-CRATE.md §9, fork 8a). The Rust structs in core/src/payload.rs are the source; never
// edit archive/src/model/ by hand. The payloads are the patient app's cards, so the output
// lives with the archived front end until payload.rs is redesigned (CONVERSION.md §5.1).
//
// ts-rs exports through tests: `cargo test --features ts export_bindings` runs one small test
// per type, and each writes its file. The folder is emptied first, so a type removed from
// Rust leaves no stale file behind.

import { spawnSync } from "node:child_process";
import { mkdirSync, rmSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "archive", "src", "model");

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

// With `sqlite` too, so this is the same build `cargo test` already made (and no dead-code noise).
const r = spawnSync("cargo", ["test", "-q", "-p", "deetshealth-core", "--features", "sqlite,ts", "export_bindings"], {
  cwd: root,
  stdio: "inherit",
  // A u64 is a plain number on the page: every count and time here is far below 2^53.
  env: { ...process.env, TS_RS_EXPORT_DIR: out, TS_RS_LARGE_INT: "number" },
});
if (r.status !== 0) process.exit(r.status ?? 1);

const files = readdirSync(out).filter((f) => f.endsWith(".ts"));
console.log(`types: ${files.length} files in archive/src/model/`);
