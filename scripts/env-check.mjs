// `npm run env:check`: what this machine's .env and secret files lack, by name.
//
// It prints names and states only, never a value (HANDOFF.md › On another machine). The groups
// come from .env.example, the one list of every name (its header says how it is laid out):
// - a group whose title says "(needed)": nothing works without it;
// - any other group: a feature, on when each of its `NAME=` lines has a value here;
// - a `# NAME=` line: a setting with a default in the code, reported as set or default.
// Then the secret files outside .env, also made per machine.
// It exits 1 only when a "(needed)" group is incomplete.
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

/** The groups of .env.example, in order: { title, needed, names, optional }. */
function groups() {
  const out = [];
  let group = null;
  for (const line of readFileSync(join(root, ".env.example"), "utf8").split(/\r?\n/)) {
    const head = /^#\s*==\s*(.+?)\s*==\s*$/.exec(line);
    if (head) {
      group = { title: head[1].replace(/\s*\(needed\)$/, ""), needed: /\(needed\)$/.test(head[1]), names: [], optional: [] };
      out.push(group);
      continue;
    }
    if (!group) continue;
    const set = /^([A-Z][A-Z0-9_]*)=/.exec(line);
    const opt = /^#\s*([A-Z][A-Z0-9_]*)=/.exec(line);
    if (set) group.names.push(set[1]);
    else if (opt) group.optional.push(opt[1]);
  }
  return out;
}

/** The names in this machine's .env that hold a value. The values are read to test them for
 * emptiness and dropped here; none leaves this function. */
function filledNames() {
  const file = join(root, ".env");
  if (!existsSync(file)) return null;
  const filled = new Set();
  for (const line of readFileSync(file, "utf8").split(/\r?\n/)) {
    const m = /^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*)$/.exec(line);
    if (!m) continue;
    const value = m[2].replace(/\s+#.*$/, "").trim().replace(/^(["'])(.*)\1$/, "$2").trim();
    if (value) filled.add(m[1]);
    else filled.delete(m[1]);
  }
  return filled;
}

/** The tunnel id is a public setting (DEMO.md §7), needed here only to find its credentials
 * file. It is not printed. */
function tunnelId() {
  const file = join(root, ".env");
  if (!existsSync(file)) return "";
  const m = /^\s*DEETS_TUNNEL_ID\s*=\s*"?([0-9a-f-]{36})"?\s*$/m.exec(readFileSync(file, "utf8"));
  return m ? m[1] : "";
}

const filled = filledNames();
const all = groups();
let failed = false;

console.log(".env (names only, never a value):");
if (!filled) {
  console.log("  no .env on this machine: copy .env.example to .env and fill it in by hand");
}
for (const g of all) {
  const have = filled ?? new Set();
  const missing = g.names.filter((n) => !have.has(n));
  const state = g.names.length === 0 ? "-" : missing.length === 0 ? "ok " : g.needed ? "MISSING" : "off";
  if (g.needed && missing.length) failed = true;
  const what = g.needed ? `${g.title} (needed)` : g.title;
  console.log(`  ${state.padEnd(7)} ${what}${missing.length ? ` — lacks ${missing.join(", ")}` : ""}`);
  const set = g.optional.filter((n) => have.has(n));
  if (set.length) console.log(`          set here, in place of the default: ${set.join(", ")}`);
}
if (filled) {
  const known = new Set(all.flatMap((g) => [...g.names, ...g.optional]));
  const unknown = [...filled].filter((n) => !known.has(n)).sort();
  if (unknown.length) console.log(`  not in .env.example (add it there, or remove it here): ${unknown.join(", ")}`);
}

console.log("Secret files (also per machine):");
const files = [
  ["dev-certs/ca.pem", "the dev certificates: cargo run -p deetshealth-ehr --example dev_certs", true],
  ["epic-client.json", "Epic's sandbox: the client IDs (EPIC.md §2)", false],
  ["dev-certs/epic-nonprod-es384.key", "Epic's sandbox: this machine's key (cd ehr/jwks-worker && npm run make-key)", false],
];
for (const [path, what, needed] of files) {
  const ok = existsSync(join(root, path));
  if (needed && !ok) failed = true;
  console.log(`  ${(ok ? "ok " : needed ? "MISSING" : "off").padEnd(7)} ${path} — ${what}`);
}
const id = tunnelId();
if (id) {
  const ok = existsSync(join(homedir(), ".cloudflared", `${id}.json`));
  console.log(`  ${(ok ? "ok " : "off").padEnd(7)} ~/.cloudflared/<tunnel id>.json — the tunnel's credentials (only on the demo machine)`);
}

process.exit(failed ? 1 : 0);
