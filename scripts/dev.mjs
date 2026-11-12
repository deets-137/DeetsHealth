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
import { spawn, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { createServer } from "node:net";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { DATABASE, PG_MAJOR, ensurePostgres, ingest } from "./postgres.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

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
  ["run", "python", "-m", "deets_agent.dev_server", "--port", String(agentPort), ...origins.flatMap((o) => ["--origin", o])],
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
