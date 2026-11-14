// `npm run dev`: the console and the agent's dev server together (CHAT.md §5; the owner,
// 2026-11-12, fork 2A). Never a fixed port: several Deets apps run on one machine at once.
// 0. Build ehr (only what changed), so the agent never starts an old one, and make the agent's
//    gRPC code from the contract again.
// 1. Pick a free port for Vite (from 1420 up) and one for the agent (from 8790 up).
// 2. Make a fresh token for this launch. The agent refuses every request without it.
// 3. Start the agent with the token and the console's origin; wait for its first line.
// 4. Start Vite on its port. Its proxy sends /agent/* to the agent and adds the token, so the
//    page never holds it (console/vite.config.ts).
// 5. Before all of this, start the dev database if it is not up (scripts/postgres.mjs;
//    DATABASE.md §3). It stays up after Ctrl+C; `npm run db:stop` stops it.
// Ctrl+C stops both. `npm run dev:web` starts the console alone (no agent).
//
// The demo (docs/ops/DEMO.md §3.4; the owner, 2026-11-13: "tie it to npm run dev with the
// guards"): when this checkout is set up for it, `npm run dev` also
// 6. builds the console under /demo/ into console/dist-demo, and again on each save (the live
//    reload, DEMO.md §3.2). Vite's dev server never faces the tunnel (§2.2);
// 7. gives the agent that folder (`--static`), which opens its second door: Access tokens only;
// 8. runs the tunnel to the agent's port. Ctrl+C stops it with the rest.
// The guards, each of which turns the demo off with its reason and lets the rest run:
// - one connector only: when Cloudflare already lists a connection on the tunnel (another PC,
//   another run), this run opens none, so requests never split between two agents;
// - the main checkout only: a git worktree never opens the tunnel;
// - a tunnel that stops ends the demo, not the dev server.
// `npm run dev -- --no-demo` runs without it. The demo also needs its settings in .env and the
// tunnel's credentials file; without them it stays off and says which is missing.
import { spawn, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { createServer } from "node:net";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { DATABASE, PG_MAJOR, ensurePostgres, ingest } from "./postgres.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const demoDir = join(root, "console", "dist-demo");

/** One value from the repo's .env, read here only for the demo's tunnel id; the agent reads
 * its own settings from the file. Nothing else in .env is loaded into this process. */
function envValue(name) {
  if (process.env[name]) return process.env[name].trim();
  const file = join(root, ".env");
  if (!existsSync(file)) return "";
  const m = new RegExp(`^\\s*${name}\\s*=\\s*"?([^"\\r\\n]*)"?\\s*$`, "m").exec(readFileSync(file, "utf8"));
  return m ? m[1].trim() : "";
}

/** The tunnel's open connections, as Cloudflare lists them (`cf tunnels connections list`):
 * a number, or null when the list cannot be read. */
function openConnections(id) {
  const r = spawnSync("cf", ["tunnels", "connections", "list", "--tunnel-id", id], {
    encoding: "utf8",
    shell: process.platform === "win32", // cf is a .cmd file on Windows
    timeout: 20_000,
  });
  if (r.status !== 0) return null;
  try {
    const list = JSON.parse(r.stdout.slice(r.stdout.indexOf("[")));
    return list.reduce((n, c) => n + (Array.isArray(c.conns) ? c.conns.length : 1), 0);
  } catch {
    return null;
  }
}

/** Whether this checkout is a git worktree rather than the main one. */
function inWorktree() {
  const git = (arg) => spawnSync("git", ["rev-parse", arg], { cwd: root, encoding: "utf8" }).stdout?.trim();
  const own = git("--absolute-git-dir");
  const common = git("--git-common-dir");
  if (!own || !common) return false;
  // A worktree's own git dir is under the common one (.git/worktrees/<name>). Resolved, so
  // Windows' two kinds of slash and a relative ".git" compare as one path.
  const same = (a, b) => resolve(root, a).toLowerCase() === resolve(root, b).toLowerCase();
  return !same(own, common);
}

/** The demo's tunnel, or null with the reason it stays off. Every reason lets the rest run. */
async function demoTunnel() {
  if (process.argv.includes("--no-demo")) return { off: "--no-demo" };
  const id = envValue("DEETS_TUNNEL_ID");
  if (!id) return { off: null }; // not set up on this machine: nothing to say
  if (!/^[0-9a-f-]{36}$/.test(id)) return { off: "DEETS_TUNNEL_ID in .env is not a tunnel id" };
  for (const name of ["DEETS_ACCESS_TEAM", "DEETS_ACCESS_AUD", "DEETS_DEMO_ORIGIN"]) {
    if (!envValue(name)) return { off: `${name} is missing from .env (docs/ops/DEMO.md §7)` };
  }
  if (inWorktree()) return { off: "this is a git worktree; only the main checkout opens the tunnel" };
  const creds = join(homedir(), ".cloudflared", `${id}.json`);
  if (!existsSync(creds)) return { off: `the tunnel's credentials file is missing: ~/.cloudflared/${id}.json` };
  if (spawnSync("cloudflared", ["--version"], { stdio: "ignore" }).status !== 0) {
    return { off: "cloudflared is not installed or not on PATH" };
  }
  // One connector only. A run stopped a moment ago can still be listed while Cloudflare notices
  // it is gone, so a listed connection gets 20 s to clear before the demo stays off.
  for (let tries = 0; ; tries++) {
    const open = openConnections(id);
    if (open === null) return { off: "Cloudflare's list of tunnel connections could not be read (is `cf` signed in?)" };
    if (open === 0) break;
    if (tries === 4) return { off: "the tunnel already has a connector (another PC or run); this run opens none" };
    if (tries === 0) console.log("[demo] the tunnel has a connector open; waiting up to 20 s for it to clear");
    await new Promise((ok) => setTimeout(ok, 5_000));
  }
  return { id, creds, origin: envValue("DEETS_DEMO_ORIGIN") };
}

const demoCheck = await demoTunnel();
const tunnel = demoCheck.id ? demoCheck : null;
if (demoCheck.off) console.log(`[demo] off: ${demoCheck.off}`);

/** The first port from `start` up that nothing is listening on, on 127.0.0.1 and localhost. */
async function freePort(start) {
  const free = (port, host) =>
    new Promise((ok) => {
      const s = createServer();
      s.once("error", () => ok(false));
      s.listen({ port, host, exclusive: true }, () => s.close(() => ok(true)));
    });
  for (let port = start; port < start + 200; port++) {
    if ((await free(port, "127.0.0.1")) && (await free(port, "::1").catch(() => true))) return port;
  }
  throw new Error(`no free port from ${start}`);
}

// The dev database. Nothing reads it yet, so a machine without Postgres goes on without it.
const db = await ensurePostgres();
if (db.error) console.error(`[db] ${db.error}. Going on without the database.`);
else console.log(`[db] Postgres ${PG_MAJOR} on 127.0.0.1:${db.port}, database ${DATABASE}${db.started ? " (started)" : ""}`);

const webPort = await freePort(1420);
const agentPort = await freePort(8790);
const token = randomBytes(32).toString("hex");
const origins = [`http://localhost:${webPort}`, `http://127.0.0.1:${webPort}`];

// Build ehr first: the agent starts target/debug's binary, and an old one ignores the fake
// Twilio's settings, so a test call would send no text (CHAT.md §8). Cargo builds only what
// changed. It fails while an old ehr still runs, because Windows locks a running .exe.
const built = spawnSync("cargo", ["build", "-p", "deetshealth-ehr", "--quiet"], { cwd: root, stdio: "inherit" });
if (built.status !== 0) {
  console.error(
    "[dev] ehr did not build. If the error says the file is in use, an older dev server or an " +
      "ehr it left behind is still running: stop it (Task Manager › deetshealth-ehr.exe) and run npm run dev again.",
  );
  process.exit(1);
}
// File the call records into the store (DATABASE.md): idempotent, so only new or changed ones.
if (!db.error) {
  const filed = ingest(db);
  if (filed.error) console.error(`[db] ${filed.error}`);
  else console.log(`[db] ${filed.line}`);
}
// The agent's gRPC code for ehr's contract is generated, not committed: make it again, so a
// changed contract never meets old code (EHR-SERVICE.md §3).
const proto = spawnSync("uv", ["run", "python", "scripts/gen_proto.py"], { cwd: join(root, "agent"), stdio: "ignore" });
if (proto.status !== 0) {
  console.error("[dev] the agent's gRPC code did not generate: run `uv run python scripts/gen_proto.py` in agent/");
  process.exit(1);
}

const children = [];
const stop = (code = 0) => {
  for (const c of children) {
    if (c.exitCode !== null) continue;
    // On Windows a kill reaches the child only (here `uv`), not the Python under it or the ehr
    // under that; an ehr left behind holds the binary and blocks the next build. So end the
    // whole tree.
    if (process.platform === "win32") spawnSync("taskkill", ["/pid", String(c.pid), "/T", "/F"], { stdio: "ignore" });
    else c.kill("SIGTERM");
  }
  process.exit(code);
};
process.on("SIGINT", () => stop(0));
process.on("SIGTERM", () => stop(0));

const agent = spawn(
  "uv",
  [
    "run",
    "python",
    "-m",
    "deets_agent.dev_server",
    "--port",
    String(agentPort),
    ...origins.flatMap((o) => ["--origin", o]),
    ...(tunnel ? ["--static", demoDir] : []),
  ],
  {
    cwd: join(root, "agent"),
    // DEETS_DATABASE_URL and DEETS_SEAL_KEY_FILE: for ehr, the store's only writer (DATABASE.md
    // §2), which the agent starts; unset without Postgres.
    env: { ...process.env, DEETS_DEV_TOKEN: token, ...(db.env ?? {}) },
    stdio: ["ignore", "pipe", "inherit"],
  },
);
children.push(agent);
agent.on("exit", (code) => {
  console.error(`[dev] the agent dev server stopped (${code ?? "signal"})`);
  stop(code || 1);
});

// Wait for the agent's first line ("agent dev server on ..."), then pass its output through.
await new Promise((ok, fail) => {
  let seen = "";
  const timer = setTimeout(() => fail(new Error("the agent dev server did not start in 60 s")), 60_000);
  agent.stdout.on("data", (chunk) => {
    process.stdout.write(`[agent] ${chunk}`);
    seen += chunk;
    if (seen.includes("agent dev server on")) {
      clearTimeout(timer);
      ok();
    }
  });
}).catch((e) => {
  console.error(`[dev] ${e.message}`);
  stop(1);
});

// Run Vite's own script with this Node, not `npx`: on Windows `npx` is a .cmd file, which
// spawn() cannot start without a shell (ENOENT).
const viteBin = join(root, "node_modules", "vite", "bin", "vite.js");
const vite = spawn(process.execPath, [viteBin, "console", "--port", String(webPort), "--strictPort"], {
  cwd: root,
  env: { ...process.env, DEETS_AGENT_PORT: String(agentPort), DEETS_AGENT_TOKEN: token },
  stdio: "inherit",
});
children.push(vite);
vite.on("exit", (code) => stop(code ?? 0));

if (tunnel) {
  // 6. The demo build, under /demo/, again on each save. The agent serves it as it lands, and
  // its live reload tells each open page (DEMO.md §3.2). No launch token in its environment.
  const build = spawn(
    process.execPath,
    [viteBin, "build", "console", "--base", "/demo/", "--outDir", demoDir, "--emptyOutDir", "--watch", "--logLevel", "warn"],
    { cwd: root, env: { ...process.env }, stdio: "inherit" },
  );
  children.push(build);
  // A guard: the demo's parts stopping never stops the dev server.
  build.on("exit", (code) => {
    console.error(`[demo] the demo build stopped (${code ?? "signal"}); guests see the last build`);
  });

  // 8. The tunnel to the agent's port, at warn level: its info lines name each connection.
  const cf = spawn(
    "cloudflared",
    [
      "tunnel",
      "--no-autoupdate",
      "--loglevel",
      "warn",
      "run",
      "--credentials-file",
      tunnel.creds,
      "--url",
      `http://127.0.0.1:${agentPort}`,
      tunnel.id,
    ],
    { cwd: root, stdio: ["ignore", "inherit", "inherit"] },
  );
  children.push(cf);
  cf.on("exit", (code) => {
    console.error(`[demo] the tunnel stopped (${code ?? "signal"}); the demo is off, the rest runs on`);
  });
  console.log(`[demo] on: ${tunnel.origin}/demo/ (Access sign-in; npm run dev -- --no-demo to keep it off)`);
}
