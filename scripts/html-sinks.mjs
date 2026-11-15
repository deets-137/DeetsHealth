#!/usr/bin/env node
// The console's HTML sinks live in one file (CONSOLE.md §5.1). The page's policy turns on
// Trusted Types, and `console/src/pages/page.ts` holds the one policy (`setHtml`, `el`). A sink
// anywhere else would throw in Edge and Chrome on the demo, and would skip the policy's one
// door, so this check fails on it first. Run by `npm run check` (the web scope).
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(ROOT, "console", "src");
const ALLOWED = join(SRC, "pages", "page.ts");
// The sinks Trusted Types guards that take HTML, and the ones that run text as code.
const SINKS = /\.(innerHTML|outerHTML)\s*=(?!=)|\binsertAdjacentHTML\s*\(|\bDOMParser\b|\bcreateContextualFragment\s*\(|\bdocument\.write(ln)?\s*\(|\bsrcdoc\b|\beval\s*\(|\bnew\s+Function\s*\(/;

function* files(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) yield* files(path);
    else if (/\.(ts|js|mjs)$/.test(entry.name)) yield path;
  }
}

const found = [];
for (const file of files(SRC)) {
  if (file === ALLOWED) continue;
  readFileSync(file, "utf8")
    .split(/\r?\n/)
    .forEach((line, i) => {
      const code = line.replace(/\/\/.*$/, "");
      if (SINKS.test(code)) found.push(`${relative(ROOT, file).replaceAll("\\", "/")}:${i + 1}`);
    });
}
if (found.length) {
  console.error("An HTML sink outside console/src/pages/page.ts (use setHtml or el; CONSOLE.md §5.1):");
  for (const at of found) console.error(`  ${at}`);
  process.exit(1);
}
console.log("html sinks: only in page.ts");
