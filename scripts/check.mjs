#!/usr/bin/env node
// The single check command (HANDOFF.md › Run it). It runs only the checks that the changed
// files need, so it stays fast; name scopes or pass --all to run more.
//
//   npm run check                       the scopes the changes need (the default)
//   npm run check -- ehr agent          the named scopes only
//   npm run check -- --all              every scope
//   npm run check -- --all --full       every scope and the slow extras (vite build, clippy)
//   npm run check -- --list             say what would run and why; run nothing
//   npm run check -- --base origin/main compare against this ref, not the upstream branch
//
// "Changed" means the commits since the base (the upstream branch's merge-base, else
// origin/main's) plus the working tree: staged, unstaged and untracked files. --no-worktree
// counts commits only (the pre-push hook uses it). It stops at the first failure and prints
// the tail of that step's output; --keep-going runs the rest anyway.
//
// The output of a passing step is not shown. The dev records in agent/calls/ hold synthetic
// data only, and no step prints a value from a record (CLAUDE.md › Privacy rules).
import { spawnSync, execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { ensureProtoc } from "./protoc.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const AGENT = join(ROOT, "agent");
const WORKER = join(ROOT, "ehr", "jwks-worker");
// The ehr that the agent's tests run: its own build, apart from the dev server's (see `agent`).
const CHECK_TARGET = join(ROOT, "target", "check");
const CHECK_EHR = join(CHECK_TARGET, "debug", `deetshealth-ehr${process.platform === "win32" ? ".exe" : ""}`);
const TTY = Boolean(process.stdout.isTTY);

// Each scope: what it checks, the files that make it run, and its steps.
// A scope runs when any changed path matches one of its patterns. `needs` pulls in another
// scope's reason: a change to core rebuilds ehr, and the agent's tests run against ehr.
const SCOPES = {
	docs: {
		about: "the docs check: links, mentions, § pointers, front matter",
		always: true,
		steps: [["docs check", "node", ["scripts/docs-check.mjs"]]],
	},
	web: {
		about: "the console's front end: type check, HTML sinks in page.ts only",
		match: [/^console\//, /^package(-lock)?\.json$/, /^scripts\/(tokens|html-sinks)\.mjs$/],
		steps: [
			["front-end type check", "node", ["node_modules/typescript/bin/tsc", "-p", "console", "--noEmit"]],
			["HTML sinks", "node", ["scripts/html-sinks.mjs"]],
		],
		full: [["front-end bundle", "node", ["node_modules/vite/bin/vite.js", "build", "console"]]],
	},
	core: {
		about: "the core crate: tests and the WebAssembly build",
		match: [/^core\//, /^Cargo\.(toml|lock)$/, /^\.cargo\//],
		steps: [
			["core tests", "cargo", ["test", "-q", "-p", "deetshealth-core", "--features", "sqlite"]],
			["core for WebAssembly", "cargo", ["check", "-q", "-p", "deetshealth-core", "--target", "wasm32-unknown-unknown"]],
		],
	},
	ehr: {
		about: "the ehr service: its tests, its banned crates, the advisories",
		match: [/^ehr\/(?!jwks-worker\/)/, /^Cargo\.(toml|lock)$/, /^\.cargo\//, /^scripts\/protoc\.mjs$/, /^deny\.toml$/],
		needs: ["core"],
		steps: [
			["ehr tests", "cargo", ["test", "-q", "-p", "deetshealth-ehr"]],
			// cargo-deny (deny.toml; SECURITY.md §7.1, items 2 and 5): no second TLS stack or crypto
			// provider in ehr on any platform, then RustSec's advisories and crates.io-only sources
			// for the whole workspace. The advisories need the network (RustSec's database).
			["ehr banned crates", "cargo", ["deny", "--locked", "--manifest-path", "ehr/Cargo.toml", "check", "bans"]],
			["Rust advisories and sources", "cargo", ["deny", "--locked", "--workspace", "check", "advisories", "sources"]],
		],
		full: [["ehr lints", "cargo", ["clippy", "-q", "-p", "deetshealth-ehr", "--all-targets", "--", "-D", "warnings"]]],
	},
	agent: {
		about: "the agent: lint and its tests (replay only, no tokens), against a fresh ehr build",
		match: [/^agent\/(?!calls\/)/], // agent/calls/ holds dev call records, not code
		needs: ["ehr"],
		steps: [
			// Its own target folder: a running dev server holds target/debug's ehr, and Windows
			// will not replace a running .exe (2026-11-12). The tests run this build.
			["ehr build for the agent", "cargo", ["build", "-q", "-p", "deetshealth-ehr", "--target-dir", CHECK_TARGET]],
			["agent proto code", "uv", ["run", "python", "scripts/gen_proto.py"], AGENT],
			["agent lint", "uv", ["run", "ruff", "check", "."], AGENT],
			["agent tests", "uv", ["run", "pytest", "-q"], AGENT, { DEETS_EHR_BIN: CHECK_EHR }],
		],
	},
	keys: {
		about: "the Epic key-set Worker: its build",
		match: [/^ehr\/jwks-worker\//],
		steps: [["key-set Worker build", "node", ["node_modules/cf/bin/cf", "build"], WORKER]],
	},
};

// --- arguments ---
const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const opt = (name) => {
	const i = args.indexOf(name);
	return i >= 0 ? args[i + 1] : undefined;
};
const named = args.filter((a, i) => !a.startsWith("--") && args[i - 1] !== "--base");
for (const n of named) {
	if (!SCOPES[n]) {
		console.error(`check: no scope "${n}". Scopes: ${Object.keys(SCOPES).join(", ")}.`);
		process.exit(2);
	}
}

// --- what changed ---
function git(...a) {
	try {
		return execFileSync("git", a, { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
	} catch {
		return null;
	}
}
function findBase() {
	const given = opt("--base");
	if (given) return { ref: given, base: git("merge-base", "HEAD", given) ?? given };
	for (const ref of ["@{upstream}", "origin/main"]) {
		const base = git("merge-base", "HEAD", ref);
		if (base) return { ref, base };
	}
	return null;
}
function changedFiles(base) {
	const lines = [git("diff", "--name-only", `${base}...HEAD`)];
	if (!flag("--no-worktree")) {
		lines.push(git("diff", "--name-only", "HEAD"), git("ls-files", "--others", "--exclude-standard"));
	}
	return [...new Set(lines.filter(Boolean).join("\n").split("\n").filter(Boolean))];
}

// --- pick the scopes ---
const reasons = new Map(); // scope -> why it runs
let baseInfo = null;
if (flag("--all")) {
	for (const s of Object.keys(SCOPES)) reasons.set(s, "--all");
} else if (named.length) {
	for (const s of named) reasons.set(s, "named");
	reasons.set("docs", reasons.get("docs") ?? "always");
} else {
	baseInfo = findBase();
	if (!baseInfo) {
		for (const s of Object.keys(SCOPES)) reasons.set(s, "no base branch found, so everything");
	} else {
		const files = changedFiles(baseInfo.base);
		for (const [s, def] of Object.entries(SCOPES)) {
			if (def.always) {
				reasons.set(s, "always");
				continue;
			}
			const hits = files.filter((f) => def.match.some((re) => re.test(f)));
			if (hits.length) reasons.set(s, `${hits.length} changed file${hits.length > 1 ? "s" : ""}, e.g. ${hits[0]}`);
		}
		// A scope also runs when a scope it builds on runs: core → ehr → agent.
		let grew = true;
		while (grew) {
			grew = false;
			for (const [s, def] of Object.entries(SCOPES)) {
				if (reasons.has(s)) continue;
				const up = (def.needs ?? []).find((n) => reasons.has(n) && reasons.get(n) !== "always");
				if (up) {
					reasons.set(s, `it builds on ${up}`);
					grew = true;
				}
			}
		}
	}
}

// --- plan ---
const order = Object.keys(SCOPES).filter((s) => reasons.has(s));
const plan = [];
for (const s of order) {
	const def = SCOPES[s];
	for (const step of [...def.steps, ...(flag("--full") ? def.full ?? [] : [])]) plan.push({ scope: s, step });
}

console.log(baseInfo ? `check: changes since ${baseInfo.ref}${flag("--no-worktree") ? " (commits only)" : " and the working tree"}` : "check:");
for (const s of order) console.log(`  ${s.padEnd(6)} ${SCOPES[s].about} — ${reasons.get(s)}`);
const skipped = Object.keys(SCOPES).filter((s) => !reasons.has(s));
if (skipped.length) console.log(`  skipped: ${skipped.join(", ")} (no changes there; --all runs them)`);
if (flag("--list")) process.exit(0);

// --- preconditions ---
if (reasons.has("agent") && !existsSync(join(ROOT, "dev-certs"))) {
	console.error("check: the agent's tests need the dev certificates. Run once: cargo run -p deetshealth-ehr --example dev_certs");
	process.exit(1);
}
if (reasons.has("keys") && !existsSync(join(WORKER, "node_modules"))) {
	console.error("check: the key-set Worker is not installed. Run once: cd ehr/jwks-worker && npm install");
	process.exit(1);
}
// cargo-deny, pinned: a newer version can read deny.toml differently, so it moves by a commit
// that changes this line and DEPENDENCIES.md's row.
const CARGO_DENY = "0.20.2";
if (plan.some(({ step }) => step[2]?.[0] === "deny")) {
	const v = spawnSync("cargo", ["deny", "--version"], { encoding: "utf8" });
	const have = v.status === 0 ? v.stdout.trim().split(/\s+/).pop() : null;
	if (have !== CARGO_DENY) {
		console.error(
			`check: cargo-deny ${CARGO_DENY} is needed (${have ? `found ${have}` : "not installed"}). ` +
				`Run once: cargo install cargo-deny --version ${CARGO_DENY} --locked`,
		);
		process.exit(1);
	}
}
// ehr's build runs Google's pinned protoc (DEPENDENCIES.md §5): fetch it before any cargo step.
if (plan.some(({ step }) => step[1] === "cargo")) {
	try {
		await ensureProtoc({ quiet: true });
	} catch (e) {
		console.error(`check: ${e.message}`);
		process.exit(1);
	}
}

// --- run ---
const started = Date.now();
const failed = [];
for (const { scope, step } of plan) {
	const [label, cmd, cmdArgs, cwd = ROOT, env = {}] = step;
	const t0 = Date.now();
	// On a terminal, show the step while it runs, then overwrite the line with its result.
	if (TTY) process.stdout.write(`  … ${label}`);
	// No shell: every command here is a real executable (node, cargo, uv), so nothing is
	// re-parsed by cmd.exe or sh.
	const r = spawnSync(cmd, cmdArgs, {
		cwd,
		env: { ...process.env, ...env },
		encoding: "utf8",
		maxBuffer: 64 * 1024 * 1024,
	});
	const secs = ((Date.now() - t0) / 1000).toFixed(1);
	const lead = TTY ? "\r\x1b[2K" : "";
	if (r.status === 0) {
		process.stdout.write(`${lead}  ok ${label} (${secs} s)\n`);
		continue;
	}
	process.stdout.write(`${lead}  FAILED ${label} (${secs} s) [${scope}]\n`);
	const out = `${r.stdout ?? ""}${r.stderr ?? ""}${r.error ? String(r.error) : ""}`.trimEnd().split("\n");
	console.log(out.slice(-60).map((l) => `    ${l}`).join("\n"));
	failed.push(label);
	if (!flag("--keep-going")) break;
}
const total = ((Date.now() - started) / 1000).toFixed(1);
if (failed.length) {
	console.log(`check: failed (${failed.join(", ")}) after ${total} s`);
	process.exit(1);
}
console.log(`check: all ${plan.length} steps passed in ${total} s`);
