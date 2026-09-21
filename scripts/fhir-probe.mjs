// Read a FHIR server and print the SHAPE of each resource type, never a value
// (FHIR-SHAPES.md, SECURITY.md §2: sandbox data is handled as protected data).
//
//   node scripts/fhir-probe.mjs [--site smart-open] [--max 300] [--type Observation-lab] [--write]
//
// --write puts the output in docs/architecture/probes/<site>.md, a generated doc: edit this
// script, never that file. Without it, the output goes to stdout.
//
// For each probed type it prints, as markdown:
//   - how many resources it read, and from how many distinct patients (a count, no ids);
//   - every field path seen ("valueQuantity.unit", "code.coding[].system") and in what share
//     of the resources it appears;
//   - the value[x] kinds, and the distinct code systems (URLs, which are public vocabulary).
// It never prints a string, a number, a date or an id from a resource.
//
// Sites needing sign-in (Epic, the SMART launcher) are not here yet: they need the app's
// sign-in, and the probe does not hold tokens. Run once per site, and again when a site or a
// resource type is added.

const SITES = {
  // SMART Health IT's open R4 server: synthetic (Synthea) patients, no sign-in.
  "smart-open": "https://r4.smarthealthit.org",
};

const TYPES = {
  "Observation-lab": "Observation?category=laboratory",
  "Observation-vitals": "Observation?category=vital-signs",
  "DiagnosticReport": "DiagnosticReport",
  "MedicationRequest": "MedicationRequest",
  "AllergyIntolerance": "AllergyIntolerance",
  "Encounter": "Encounter",
  "Condition": "Condition",
  "Immunization": "Immunization",
  "Patient": "Patient",
};

const arg = (name, dflt) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : dflt;
};
const siteKey = arg("site", "smart-open");
const base = SITES[siteKey];
const max = Number(arg("max", "300"));
const only = arg("type", null);
if (!base) throw new Error(`unknown site ${siteKey}; known: ${Object.keys(SITES).join(", ")}`);

async function get(url) {
  const r = await fetch(url, { headers: { Accept: "application/fhir+json" } });
  if (!r.ok) throw new Error(`${r.status} on ${new URL(url).pathname}`);
  return r.json();
}

// Walk one resource and add every path it holds to `paths` (once per resource).
function walk(node, path, seen) {
  if (Array.isArray(node)) {
    for (const item of node) walk(item, `${path}[]`, seen);
  } else if (node && typeof node === "object") {
    for (const [k, v] of Object.entries(node)) {
      if (k === "text" && path === "" && v && v.div) { seen.add("text (narrative HTML)"); continue; }
      walk(v, path ? `${path}.${k}` : k, seen);
    }
  } else {
    seen.add(path);
  }
}

function systems(node, out) {
  if (Array.isArray(node)) node.forEach((n) => systems(n, out));
  else if (node && typeof node === "object") {
    if (typeof node.system === "string" && node.code !== undefined) out.add(node.system);
    Object.values(node).forEach((n) => systems(n, out));
  }
}

async function probe(label, query) {
  const counts = new Map();
  const valueKinds = new Map();
  const codeSystems = new Set();
  const patients = new Set();
  let n = 0;
  let pages = 0;
  let url = `${base}/${query}${query.includes("?") ? "&" : "?"}_count=100`;
  while (url && n < max) {
    const bundle = await get(url);
    pages++;
    for (const e of bundle.entry ?? []) {
      const res = e.resource;
      if (!res || n >= max) continue;
      n++;
      const seen = new Set();
      walk(res, "", seen);
      for (const p of seen) counts.set(p, (counts.get(p) ?? 0) + 1);
      const vk = Object.keys(res).find((k) => /^value[A-Z]/.test(k)) ??
        (res.component ? "(component[])" : "(none)");
      valueKinds.set(vk, (valueKinds.get(vk) ?? 0) + 1);
      systems(res, codeSystems);
      const subj = res.subject?.reference ?? res.patient?.reference ??
        (res.resourceType === "Patient" ? `Patient/${res.id}` : null);
      if (subj) patients.add(subj); // counted, never printed
    }
    url = bundle.link?.find((l) => l.relation === "next")?.url ?? null;
  }
  const pct = (c) => (c * 100 < n ? "<1%" : `${Math.round((100 * c) / n)}%`);
  const lines = [];
  lines.push(`### ${label}`, "");
  lines.push(`Read ${n} resources over ${pages} page(s), from ${patients.size} patient(s).`, "");
  lines.push("| value[x] kind | share |", "|---|---|");
  for (const [k, c] of [...valueKinds].sort((a, b) => b[1] - a[1])) lines.push(`| ${k} | ${pct(c)} |`);
  lines.push("", "Code systems: " + ([...codeSystems].sort().map((s) => `\`${s}\``).join(", ") || "none"), "");
  lines.push("| field path | present in |", "|---|---|");
  for (const [p, c] of [...counts].sort((a, b) => a[0].localeCompare(b[0]))) lines.push(`| \`${p}\` | ${pct(c)} |`);
  return lines.join("\n");
}

const today = new Date().toISOString().slice(0, 10);
const out = [`## Site: ${siteKey} (${base}) — probed ${today}`, ""];
for (const [label, query] of Object.entries(TYPES)) {
  if (only && only !== label) continue;
  try {
    out.push(await probe(label, query), "");
  } catch (e) {
    out.push(`### ${label}`, "", `Failed: ${e.message}`, "");
  }
}
if (process.argv.includes("--write")) {
  const { mkdirSync, writeFileSync } = await import("node:fs");
  const { dirname, join } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const dir = join(dirname(fileURLToPath(import.meta.url)), "..", "docs", "architecture", "probes");
  mkdirSync(dir, { recursive: true });
  const head = [
    "---",
    "status: foundation",
    "desk_test: none",
    "sources: [scripts/fhir-probe.mjs]",
    `updated: ${today}`,
    "---",
    `# FHIR probe — ${siteKey}`,
    "",
    "> Generated by `node scripts/fhir-probe.mjs --site " + siteKey + " --write`. Do not edit.",
    "> Shapes only: no value from any resource is printed. Findings: [FHIR-SHAPES.md](../FHIR-SHAPES.md).",
    "",
  ];
  writeFileSync(join(dir, `${siteKey}.md`), head.concat(out).join("\n"));
  console.log(`wrote docs/architecture/probes/${siteKey}.md`);
} else {
  console.log(out.join("\n"));
}
