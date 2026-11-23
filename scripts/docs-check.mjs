// `npm run docs:check` — the docs checker (DOCS-ORG.md §4). Deterministic, fast, no model.
// DeetsMusic's scripts/docs-check.mjs, cut to the checks this repo has something for:
//   1. every link resolves            [x](path) in docs/, CLAUDE.md, README.md
//   2. every mention names a real file  "HANDOFF.md" in prose or `code`, by basename
//   3. every section pointer resolves   "SECURITY.md §4.3" → a heading numbered 4.3 in SECURITY.md
//   4. front matter present and valid   the fields and values of DOCS-ORG.md §2
//  11. nothing in ideas/ is above `idea`; docs/archive/ holds the archived docs and only them
// The drift checks (DOCS-ORG.md §4, DEV-API.md §7; 2026-11-12): a doc's ledger against the code.
//  12. the route ledger (DEV-API.md §2) equals the routes in dev_server.py, and each handler exists
//  13. the RPC ledger (EHR-SERVICE.md §3) equals the rpcs in ehr.proto, and each has both ends
//  14. the console and the agent agree: annotation fields, watch message types, the clients
// And one suspicion, listed but never failing the run:
//   6. a built doc whose sources changed in git after its `updated` date; a legacy doc whose
//      sources are all archived
// DeetsMusic's checks 5 (versions), 18 (generated copies) and 19 (shipped links) come when
// this repo has releases and generated copies.
//
//   node scripts/docs-check.mjs          report; exit 1 on any fact
//   node scripts/docs-check.mjs --warn   report; always exit 0
//
// Skipped on purpose: fenced code blocks (examples, not claims); files whose name starts with
// "_" (templates). A mention is NOT a fault when:
//  - its path, its paragraph or its section heading names another repo ("DeetsMusic's
//    `docs/ops/DEBUGGING.md`") — that file lives there;
//  - it sits inside a URL;
//  - the file exists but git ignores it;
//  - the doc is a plan (status project, designed or idea): a plan names files not made yet;
//  - the doc is generated (front matter `generated`): it quotes its source's comments.
// A link to a code line ("../src/layout.ts:344") resolves to the file.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { basename, dirname, join, posix } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");

const STATUS = ["shipped", "built", "designed", "project", "parked", "idea", "foundation", "sop", "guide", "archived"];
const KEYS = ["status", "shipped_in", "desk_test", "sources", "updated", "generated", "legacy"];
const LEGACY = ["patient-app"];
// A path that starts with one of these points into another repo, and is not ours to check.
const OTHER_REPO = /^(\.\.\/(?!src|docs|scripts|README|CLAUDE)|Deets[A-Za-z]+\/)/;
// Another repo named in prose: any Deets* name but this app's own.
const NAMES_REPO = /\bDeets(?!Health\b)[A-Z][A-Za-z]+|\bdeets\.solutions\b|\bdeets-137\//;
// An archived doc (DOCS-ORG.md §5) is a record: like a plan, it may name files that are gone.
const PLANS = ["project", "designed", "idea", "archived"];

const read = (p) => readFileSync(join(ROOT, p), "utf8").replace(/\r\n/g, "\n");
const tracked = () => execFileSync("git", ["ls-files", "-co", "--exclude-standard"], { cwd: ROOT, encoding: "utf8" }).split("\n").filter(Boolean);

/** Text with fenced code blocks blanked (line count kept, so line numbers stay true). */
const unfence = (t) => t.replace(/^(```|~~~)[^\n]*\n[\s\S]*?^\1[^\n]*$/gm, (m) => m.replace(/[^\n]/g, ""));
const lineOf = (t, i) => t.slice(0, i).split("\n").length;

function frontMatter(text) {
  if (!text.startsWith("---\n")) return null;
  const end = text.indexOf("\n---\n", 4);
  if (end < 0) return null;
  const fm = {};
  for (const line of text.slice(4, end).split("\n")) {
    const m = /^([a-z_]+):\s*(.*)$/.exec(line);
    if (m) fm[m[1]] = m[2].trim();
  }
  return fm;
}

/** The numbers headings carry: "## 5.1 Part markers" → "5.1"; "### §4a. …" → "4a". */
function headingNumbers(text) {
  const out = new Set();
  for (const m of text.matchAll(/^#{1,6}\s+§?\s?([0-9]+[a-z]?(?:\.[0-9]+[a-z]?)*)\.?[\s)—:-]/gm)) out.add(m[1]);
  return out;
}

export function check() {
  const files = tracked();
  const docs = files.filter((f) => /^docs\/.*\.md$/.test(f) && !basename(f).startsWith("_"));
  const roots = ["CLAUDE.md", "README.md"].filter((f) => existsSync(join(ROOT, f)));
  const byName = new Map();
  for (const f of files.filter((f) => f.endsWith(".md"))) {
    if (!byName.has(basename(f))) byName.set(basename(f), []);
    byName.get(basename(f)).push(f);
  }
  const facts = [];
  const fail = (n, file, line, msg) => facts.push({ n, at: `${file}${line ? `:${line}` : ""}`, msg });
  const texts = new Map([...docs, ...roots].map((f) => [f, read(f)]));

  for (const [file, raw] of texts) {
    const text = unfence(raw);

    // 1. Links.
    for (const m of text.matchAll(/\[[^\]\n]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g)) {
      let target = m[1];
      if (/^([a-z]+:|#|\/\/)/i.test(target) || target.startsWith("<")) continue;
      target = decodeURI(target.replace(/[#?].*$/, "")).replace(/(\.\w+):\d+(?::\d+)?$/, "$1");
      if (!target) continue;
      const at = posix.normalize(posix.join(posix.dirname(file), target));
      if (!existsSync(join(ROOT, at))) fail(1, file, lineOf(text, m.index), `link to ${m[1]} — no such file (${at})`);
    }

    // 2. Mentions, and 3. section pointers. A generated doc (TOKENS.md) quotes its source's
    // comments, and the token sheets came from DeetsMusic with DeetsMusic's doc names in them.
    const fmHere = frontMatter(raw);
    if (fmHere?.generated) continue;
    const plan = PLANS.includes(fmHere?.status);
    for (const m of text.matchAll(/((?:\.\.\/|[\w.-]+\/)*)\b([A-Za-z][\w-]*\.md)\b(\s+§\s?([0-9]+[a-z]?(?:\.[0-9]+[a-z]?)*))?/g)) {
      const [, path, name, , sec] = m;
      if (OTHER_REPO.test(path)) continue;
      const before = text.slice(0, m.index);
      if (/https?:\/\/\S*$/.test(before)) continue;
      const line = lineOf(text, m.index);
      const hits = byName.get(name);
      const para = before.slice(before.lastIndexOf("\n\n") + 1) + text.slice(m.index, text.indexOf("\n\n", m.index) >>> 0);
      const heading = [...before.matchAll(/^#{1,6} .*$/gm)].pop()?.[0] ?? "";
      const elsewhere = NAMES_REPO.test(para) || NAMES_REPO.test(heading);
      if (!hits) {
        if (!elsewhere && !plan) fail(2, file, line, `mentions ${path}${name} — no such file in the repo`);
        continue;
      }
      // A pointer into another repo's doc of the same name is that repo's to check.
      if (!sec || elsewhere) continue;
      const target = hits.find((h) => h.startsWith("docs/")) ?? hits[0];
      const nums = headingNumbers(read(target));
      const ok = nums.has(sec) || [...nums].some((n) => n.startsWith(sec + "."));
      if (!ok) fail(3, file, line, `${name} §${sec} — no heading numbered ${sec} in ${target}`);
    }
  }

  // 4. Front matter, and 11. ideas/.
  for (const file of docs) {
    const fm = frontMatter(texts.get(file));
    if (!fm) { fail(4, file, 1, "no front matter"); continue; }
    for (const k of Object.keys(fm)) if (!KEYS.includes(k)) fail(4, file, 1, `unknown key "${k}"`);
    if (!STATUS.includes(fm.status)) fail(4, file, 1, `status "${fm.status ?? ""}" is not one of ${STATUS.join(" ")}`);
    if (fm.status === "shipped" && !fm.shipped_in) fail(4, file, 1, "status shipped needs shipped_in");
    if (fm.shipped_in && fm.status !== "shipped" && fm.status !== "archived") fail(4, file, 1, `shipped_in with status ${fm.status}`);
    // closed <date>: closed on the owner's word, not run (docs/PRE-LIVE-VERIFICATIONS.md).
    if (!/^(none|open|(passed|closed) \d{4}-\d{2}-\d{2})$/.test(fm.desk_test ?? "")) fail(4, file, 1, `desk_test "${fm.desk_test ?? ""}" — none, open, passed <date> or closed <date>`);
    if (!fm.generated && !/^\d{4}-\d{2}-\d{2}$/.test(fm.updated ?? "")) fail(4, file, 1, `updated "${fm.updated ?? ""}" is not a date`);
    const src = /^\[(.*)\]$/.exec(fm.sources ?? "");
    if (!src) fail(4, file, 1, "sources is not a [list]");
    else if (fm.status !== "archived") for (const s of src[1].split(",").map((x) => x.trim()).filter(Boolean)) {
      if (!existsSync(join(ROOT, s))) fail(4, file, 1, `source ${s} does not exist`);
    }
    if (fm.legacy !== undefined && !LEGACY.includes(fm.legacy)) fail(4, file, 1, `legacy "${fm.legacy}" is not one of ${LEGACY.join(" ")}`);
    if (file.startsWith("docs/ideas/") && fm.status !== "idea") fail(11, file, 1, `in ideas/ with status ${fm.status} — move it out of ideas/`);
    if (file.startsWith("docs/archive/") !== (fm.status === "archived")) fail(11, file, 1, `status ${fm.status} — archived docs, and only they, live in docs/archive/`);
  }

  drift(fail);
  const suspicions = stale(docs, texts);
  return { facts, suspicions, docs: docs.length };
}

// ---- The drift checks (12–14): a doc's ledger against the code --------------------------------

const DEV_API = "docs/architecture/DEV-API.md";
const DEV_SERVER = "agent/deets_agent/dev_server.py";
const CATALOG = "agent/deets_agent/catalog.py";
const API_TS = "console/src/agent/api.ts";
const TRANSPORT_TS = "console/src/agent/transport.ts";
const EHR_DOC = "docs/architecture/EHR-SERVICE.md";
const PROTO = "ehr/proto/deets/ehr/v1/ehr.proto";
const EHR_SERVICE = "ehr/src/service.rs";
const EHR_CLIENT = "agent/deets_agent/ehr_client.py";

/** The body of a numbered `##` section: "## 2. …" up to the next `##`. */
function section(text, num) {
  const start = text.search(new RegExp(`^## ${num}\\.? `, "m"));
  if (start < 0) return null;
  const rest = text.slice(start + 3);
  const end = rest.search(/^## /m);
  return end < 0 ? rest : rest.slice(0, end);
}

/** The table rows of a text whose first cells match `first`; each row as its cells. */
function rows(text, first) {
  const out = [];
  for (const line of text.split("\n")) {
    if (!line.startsWith("|")) continue;
    const cells = line.split("|").slice(1, -1).map((c) => c.trim());
    if (first.test(cells[0] ?? "")) out.push({ cells, line: line });
  }
  return out;
}

const ticked = (cell) => /^`([^`]+)`$/.exec(cell)?.[1];
/** A difference both ways, as two lists. */
const diff = (a, b) => [[...a].filter((x) => !b.has(x)), [...b].filter((x) => !a.has(x))];

/** The type members of a TS union `export type NAME =` up to its closing `;`: the first one at
 * a line's end whose next line starts at the margin (a member's own `;` lines are indented). */
function union(ts, name) {
  const m = new RegExp(`export type ${name} =([\\s\\S]*?);[ \\t]*\\n(?=\\S|$)`).exec(ts);
  if (!m) return null;
  return m[1].split(/\n\s*\|\s*/).map((s) => s.trim()).filter(Boolean);
}

function drift(fail) {
  const need = [DEV_API, DEV_SERVER, CATALOG, API_TS, TRANSPORT_TS, EHR_DOC, PROTO, EHR_SERVICE, EHR_CLIENT];
  for (const p of need) if (!existsSync(join(ROOT, p))) { fail(12, p, 0, "missing: the drift checks read it"); return; }
  const server = read(DEV_SERVER);
  const devApi = read(DEV_API);
  const apiTs = read(API_TS);
  const clients = apiTs + "\n" + read(TRANSPORT_TS);

  // 12. The route ledger.
  const ledger = section(devApi, 2);
  if (!ledger) fail(12, DEV_API, 0, "no section 2 (the route ledger)");
  else {
    const inCode = new Set(
      [...server.matchAll(/@app\.(get|post|put|patch|delete|websocket)\("([^"]+)"\)/g)].map(
        ([, m, p]) => `${m === "websocket" ? "WS" : m.toUpperCase()} ${p}`,
      ),
    );
    const routeRows = rows(ledger, /^`(GET|POST|PUT|PATCH|DELETE|WS) \//);
    const inDoc = new Set(routeRows.map((r) => ticked(r.cells[0])));
    const [noRow, noRoute] = diff(inCode, inDoc);
    for (const r of noRow) fail(12, DEV_API, 0, `${r} is a route in ${DEV_SERVER} with no row in §2`);
    for (const r of noRoute) fail(12, DEV_API, 0, `§2 has a row for ${r}, which ${DEV_SERVER} does not serve`);
    for (const { cells } of routeRows) {
      const handler = ticked(cells[1] ?? "");
      if (!handler || !new RegExp(`async def ${handler}\\(`).test(server))
        fail(12, DEV_API, 0, `${ticked(cells[0])}: handler ${cells[1] ?? "(none)"} is not an async def in ${DEV_SERVER}`);
      // 14. Each client it names exists.
      const client = ticked(cells[2] ?? "")?.replace(/\(\)$/, "");
      if (client && !new RegExp(`export\\s+(?:async\\s+)?(?:function|const|class)\\s+${client}\\b`).test(clients))
        fail(14, DEV_API, 0, `${ticked(cells[0])}: client ${client} is not exported from api.ts or transport.ts`);
    }
  }

  // 13. The RPC ledger.
  const contract = section(read(EHR_DOC), 3);
  if (!contract) fail(13, EHR_DOC, 0, "no section 3 (the RPC ledger)");
  else {
    const rpcs = new Set([...read(PROTO).matchAll(/^\s*rpc (\w+)\(/gm)].map((m) => m[1]));
    const rpcRows = rows(contract, /^`[A-Z]\w*`$/);
    const [noRow, noRpc] = diff(rpcs, new Set(rpcRows.map((r) => ticked(r.cells[0]))));
    for (const r of noRow) fail(13, EHR_DOC, 0, `rpc ${r} in ehr.proto has no row in §3`);
    for (const r of noRpc) fail(13, EHR_DOC, 0, `§3 has a row for ${r}, which ehr.proto does not declare`);
    const service = read(EHR_SERVICE);
    const client = read(EHR_CLIENT);
    for (const { cells } of rpcRows) {
      const [rpc, handler, method] = cells.slice(0, 3).map(ticked);
      if (!handler || !new RegExp(`async fn ${handler}\\(`).test(service)) fail(13, EHR_DOC, 0, `${rpc}: no async fn ${handler} in ${EHR_SERVICE}`);
      if (!method || !new RegExp(`async def ${method}\\(`).test(client)) fail(13, EHR_DOC, 0, `${rpc}: no async def ${method} in ${EHR_CLIENT}`);
    }
  }

  // 14. The annotation fields: catalog.py › FIELDS against api.ts › Annotation.
  const py = /^FIELDS = \{([\s\S]*?)^\}/m.exec(read(CATALOG));
  const ts = union(apiTs, "Annotation");
  if (!py || !ts) fail(14, !py ? CATALOG : API_TS, 0, !py ? "no FIELDS = { … }" : "no export type Annotation");
  else {
    const want = new Map([...py[1].matchAll(/"(\w+)":\s*\{([^}]*)\}/g)].map(([, k, f]) => [k, new Set([...f.matchAll(/"(\w+)"/g)].map((x) => x[1]))]));
    const have = new Map();
    for (const member of ts) {
      const kinds = /kind:\s*((?:"\w+"\s*\|?\s*)+)/.exec(member)?.[1].match(/\w+/g) ?? [];
      const fields = new Set([...member.matchAll(/(\w+)\??:/g)].map((x) => x[1]).filter((f) => f !== "kind" && f !== "call"));
      for (const k of kinds) have.set(k, fields);
    }
    const [noTs, noPy] = diff(new Set(want.keys()), new Set(have.keys()));
    for (const k of noTs) fail(14, API_TS, 0, `annotation kind ${k} is in catalog.py › FIELDS but not in Annotation`);
    for (const k of noPy) fail(14, API_TS, 0, `annotation kind ${k} is in Annotation but not in catalog.py › FIELDS`);
    for (const [k, fields] of want) {
      if (!have.has(k)) continue;
      const [noField, extra] = diff(fields, have.get(k));
      if (noField.length || extra.length)
        fail(14, API_TS, 0, `annotation ${k}: catalog.py takes {${[...fields].join(", ")}}, Annotation has {${[...have.get(k)].join(", ")}}`);
    }
  }

  // 14. The watch messages: the "type" values the server sends against api.ts › WatchMessage.
  const watchTs = union(apiTs, "WatchMessage");
  if (!watchTs) fail(14, API_TS, 0, "no export type WatchMessage");
  else {
    const sent = new Set([...server.matchAll(/"type":\s*"(\w+)"/g)].map((m) => m[1]));
    const typed = new Set(watchTs.map((m) => /type:\s*"(\w+)"/.exec(m)?.[1]).filter(Boolean));
    const [noType, noSend] = diff(sent, typed);
    for (const t of noType) fail(14, API_TS, 0, `the dev server sends type "${t}", which WatchMessage does not have`);
    for (const t of noSend) fail(14, API_TS, 0, `WatchMessage has type "${t}", which the dev server never sends`);
  }
}

// ---- Check 6, the suspicion: a doc older than its sources ------------------------------------

/** Each tracked file's last change: the newest commit date, or today when the tree changes it. */
function lastChanged() {
  const out = new Map();
  const log = execFileSync("git", ["log", "--format=%x01%cs", "--name-only", "--no-renames"], { cwd: ROOT, encoding: "utf8", maxBuffer: 64 << 20 });
  let date = "";
  for (const line of log.split("\n")) {
    if (line.startsWith("\x01")) date = line.slice(1);
    else if (line && !out.has(line)) out.set(line, date);
  }
  const today = new Date().toISOString().slice(0, 10);
  const status = execFileSync("git", ["status", "--porcelain", "--untracked-files=all"], { cwd: ROOT, encoding: "utf8" });
  for (const line of status.split("\n")) if (line.length > 3) out.set(line.slice(3).replace(/^.* -> /, ""), today);
  return out;
}

function stale(docs, texts) {
  const out = [];
  let changed;
  try {
    changed = lastChanged();
  } catch {
    return [{ at: "git", msg: "could not read the history; check 6 skipped" }];
  }
  for (const file of docs) {
    const fm = frontMatter(texts.get(file));
    const src = /^\[(.*)\]$/.exec(fm?.sources ?? "")?.[1].split(",").map((s) => s.trim()).filter(Boolean) ?? [];
    if (!fm || fm.generated || !src.length) continue;
    if (fm.legacy) {
      if (src.every((s) => s.startsWith("archive/"))) out.push({ at: file, msg: "legacy, and every source is archived: ready for docs/archive/ (DOCS-ORG.md §5)" });
      continue;
    }
    if (fm.status !== "built" && fm.status !== "foundation") continue;
    const newer = src.filter((s) => (changed.get(s) ?? "") > fm.updated);
    if (newer.length) out.push({ at: file, msg: `updated ${fm.updated}; changed since: ${newer.map((s) => `${s} (${changed.get(s)})`).join(", ")}` });
  }
  return out;
}

export function report({ facts, suspicions = [], docs }) {
  const lines = [];
  const names = {
    1: "links", 2: "mentions", 3: "section pointers", 4: "front matter", 11: "ideas/",
    12: "route ledger", 13: "RPC ledger", 14: "console and agent agree",
  };
  for (const n of Object.keys(names).map(Number)) {
    const mine = facts.filter((f) => f.n === n);
    lines.push(`check ${n} — ${names[n]}: ${mine.length ? `${mine.length} FAIL` : "ok"}`);
    for (const f of mine) lines.push(`    ${f.at}  ${f.msg}`);
  }
  lines.push(`check 6 — docs older than their sources (a suspicion, never a failure): ${suspicions.length ? `${suspicions.length} to read` : "none"}`);
  for (const s of suspicions) lines.push(`    ${s.at}  ${s.msg}`);
  lines.push(`[docs-check] ${docs} docs · ${facts.length} fact${facts.length === 1 ? "" : "s"} failing`);
  return lines.join("\n");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const r = check();
  console.log(report(r));
  if (r.facts.length && !process.argv.includes("--warn")) process.exit(1);
}
