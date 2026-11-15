// The dev database: native Postgres, one cluster per repo (the owner, 2026-11-12; DATABASE.md §3).
// `npm run dev` calls ensurePostgres() and ingest(); `npm run db:start | db:stop | db:status |
// db:ingest | db:test` run them by hand.
//
// - The cluster lives in `.pg/` (git-ignored): `.pg/data` (the cluster), `.pg/password` (the dev
//   role's password, made at first start), `.pg/log` (the server's log).
// - It runs as the signed-in user through pg_ctl: no admin rights, no Windows service, and the
//   same on the Mac. It listens on 127.0.0.1 only.
// - Never a fixed port (CLAUDE.md): a running cluster keeps its port (read from postmaster.pid);
//   a new start takes the first free port from 5433 up, so it never meets another Postgres.
// - It stays running after `npm run dev` stops, like a service. `npm run db:stop` stops it.
// - The major version is pinned (PG_MAJOR) to what the cloud's managed Postgres serves.
import { spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// AWS RDS has served 18 since November 2025, standard support to February 2031 (DATABASE.md §3).
export const PG_MAJOR = 18;
export const DATABASE = "deetshealth_dev";
export const ROLE = "deets";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const home = join(root, ".pg");
const data = join(home, "data");
const passwordFile = join(home, "password");
// The dev sealing key: 32 random bytes, made once (ehr/src/seal_key.rs). The cloud uses a KMS key.
const sealKeyFile = join(home, "seal.key");
const log = join(home, "log");
// DEETS_EHR_BIN points at another build, for when a running dev server holds this one.
const ehrBin =
  process.env.DEETS_EHR_BIN || join(root, "target", "debug", `deetshealth-ehr${process.platform === "win32" ? ".exe" : ""}`);
const exe = process.platform === "win32" ? ".exe" : "";

/** The folder that holds pg_ctl for PG_MAJOR: DEETS_PG_BIN, the standard install places, or PATH. */
export function pgBin() {
  const places = [
    process.env.DEETS_PG_BIN,
    `C:\\Program Files\\PostgreSQL\\${PG_MAJOR}\\bin`,
    `/opt/homebrew/opt/postgresql@${PG_MAJOR}/bin`,
    `/usr/local/opt/postgresql@${PG_MAJOR}/bin`,
    `/usr/lib/postgresql/${PG_MAJOR}/bin`,
    // The EDB installer on the Mac (postgresql.org's download).
    `/Library/PostgreSQL/${PG_MAJOR}/bin`,
  ].filter(Boolean);
  // A folder counts only when it holds the server too: Homebrew's libpq ships pg_ctl and initdb
  // without `postgres`, and it is often first on PATH (seen on the Mac, 2026-11-12).
  const server = (p) => existsSync(join(p, `pg_ctl${exe}`)) && existsSync(join(p, `postgres${exe}`));
  for (const p of places) if (server(p)) return p;
  const which = spawnSync(process.platform === "win32" ? "where" : "which", ["-a", "pg_ctl"].slice(process.platform === "win32" ? 1 : 0), { encoding: "utf8" });
  const found = which.status === 0 ? which.stdout.split(/\r?\n/).map((l) => dirname(l.trim())).find((d) => d && server(d)) : undefined;
  return found ?? null;
}

function run(bin, tool, args, env = {}) {
  return spawnSync(join(bin, `${tool}${exe}`), args, {
    encoding: "utf8",
    env: { ...process.env, ...env },
  });
}

function installHint() {
  return process.platform === "win32"
    ? `install it with: winget install --id PostgreSQL.PostgreSQL.${PG_MAJOR} (DATABASE.md §3)`
    : `install it with: brew install postgresql@${PG_MAJOR} (DATABASE.md §3)`;
}

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

/** The port of the running cluster, from postmaster.pid (its fourth line), or null. */
function runningPort(bin) {
  const pid = join(data, "postmaster.pid");
  if (!existsSync(pid)) return null;
  const port = Number(readFileSync(pid, "utf8").split(/\r?\n/)[3]);
  if (!port) return null;
  const ready = run(bin, "pg_isready", ["-h", "127.0.0.1", "-p", String(port), "-q"]);
  return ready.status === 0 ? port : null;
}

/** Make the cluster on first use: the dev role with a random password, scram only, UTF-8. */
function init(bin) {
  mkdirSync(home, { recursive: true });
  const password = randomBytes(24).toString("hex");
  writeFileSync(passwordFile, password, { mode: 0o600 });
  const pwfile = join(home, ".initpw");
  writeFileSync(pwfile, password, { mode: 0o600 });
  try {
    const made = run(bin, "initdb", [
      "-D", data, "-U", ROLE, "--pwfile", pwfile, "--auth=scram-sha-256", "-E", "UTF8", "--locale=C",
    ]);
    if (made.status !== 0) throw new Error(`initdb failed: ${(made.stderr || made.stdout).trim()}`);
  } finally {
    rmSync(pwfile, { force: true });
  }
}

/**
 * Start the dev cluster if it is not up, make the database if it is missing, and return how to
 * reach it. Returns null (with the reason) when Postgres is not installed: nothing reads the
 * database yet, so `npm run dev` goes on without it.
 */
export async function ensurePostgres() {
  const bin = pgBin();
  if (!bin) return { error: `Postgres ${PG_MAJOR} is not installed; ${installHint()}` };
  const version = run(bin, "pg_ctl", ["--version"]).stdout || "";
  if (!version.includes(` ${PG_MAJOR}.`)) {
    return { error: `found ${version.trim() || "pg_ctl"} at ${bin}, but DeetsHealth pins Postgres ${PG_MAJOR}; ${installHint()}` };
  }
  if (!existsSync(data) || readdirSync(data).length === 0) init(bin);
  if (!existsSync(sealKeyFile)) writeFileSync(sealKeyFile, randomBytes(32), { mode: 0o600 });
  const password = readFileSync(passwordFile, "utf8").trim();

  let port = runningPort(bin);
  let started = false;
  if (port === null) {
    port = await freePort(5433);
    // No pipes: the server pg_ctl starts would inherit them and hold them open, so spawnSync
    // would never return (seen on Windows, 2026-11-12). Its output goes to the log file.
    const up = spawnSync(
      join(bin, `pg_ctl${exe}`),
      ["start", "-w", "-D", data, "-l", log, "-o", `-p ${port} -c listen_addresses=127.0.0.1`],
      { stdio: "ignore" },
    );
    if (up.status !== 0) return { error: `Postgres did not start; see ${log}` };
    started = true;
  }

  const env = { PGPASSWORD: password };
  const conn = ["-h", "127.0.0.1", "-p", String(port), "-U", ROLE];
  const has = run(bin, "psql", [...conn, "-d", "postgres", "-tAc", `SELECT 1 FROM pg_database WHERE datname = '${DATABASE}'`], env);
  if (has.status !== 0) return { error: `could not reach Postgres on port ${port}: ${has.stderr.trim()}` };
  if (has.stdout.trim() !== "1") {
    const made = run(bin, "createdb", [...conn, DATABASE], env);
    if (made.status !== 0) return { error: `could not make the database: ${made.stderr.trim()}` };
  }
  const url = `postgresql://${ROLE}:${password}@127.0.0.1:${port}/${DATABASE}`;
  const storeEnv = { DEETS_DATABASE_URL: url, DEETS_SEAL_KEY_FILE: sealKeyFile };
  // For ehr when it is not started by `npm run dev` (a typed call run by hand): the agent reads
  // this git-ignored file (agent/deets_agent/ehr_process.py `store_env`). Rewritten at each start,
  // so a new port is always in it.
  writeFileSync(
    join(home, "ehr.env"),
    Object.entries(storeEnv).map(([k, v]) => `${k}=${v}`).join("\n") + "\n",
    { mode: 0o600 },
  );
  return { port, started, url, env: storeEnv };
}

/** File the dev call records into the store with the built ehr (store.rs). Idempotent. */
export function ingest(db) {
  if (!existsSync(ehrBin)) return { error: "ehr is not built: cargo build -p deetshealth-ehr" };
  const done = spawnSync(ehrBin, ["ingest", "--calls", join(root, "agent", "calls")], {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, ...db.env },
  });
  if (done.status !== 0) return { error: (done.stderr || done.stdout).trim() || "ehr ingest failed" };
  // ehr's own notes on stderr (counts only, such as the re-seal pass's), one per line.
  const notes = done.stderr.split(/\r?\n/).map((l) => l.trim()).filter(Boolean);
  return { line: done.stdout.trim(), notes };
}

export function stopPostgres() {
  const bin = pgBin();
  if (!bin || !existsSync(join(data, "postmaster.pid"))) return "not running";
  const down = run(bin, "pg_ctl", ["stop", "-D", data, "-m", "fast"]);
  return down.status === 0 ? "stopped" : `did not stop: ${(down.stderr || down.stdout).trim()}`;
}

// `node scripts/postgres.mjs start | stop | status`
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const what = process.argv[2] || "status";
  if (what === "stop") {
    console.log(`[db] ${stopPostgres()}`);
  } else if (what === "start") {
    const db = await ensurePostgres();
    console.log(db.error ? `[db] ${db.error}` : `[db] Postgres ${PG_MAJOR} on 127.0.0.1:${db.port}, database ${DATABASE}`);
    process.exit(db.error ? 1 : 0);
  } else if (what === "ingest") {
    const db = await ensurePostgres();
    const done = db.error ? db : ingest(db);
    for (const note of done.notes ?? []) console.log(`[db] ${note}`);
    console.log(`[db] ${done.error ?? done.line}`);
    process.exit(done.error ? 1 : 0);
  } else if (what === "test") {
    // ehr's store test against this cluster; it makes and drops its own database.
    const db = await ensurePostgres();
    if (db.error) {
      console.log(`[db] ${db.error}`);
      process.exit(1);
    }
    // Its own target folder: a running dev server holds target/debug's ehr binary, which an
    // integration test build would have to replace (Windows locks a running .exe).
    const t = spawnSync("cargo", ["test", "-p", "deetshealth-ehr", "--test", "store"], {
      cwd: root,
      stdio: "inherit",
      env: { ...process.env, ...db.env, CARGO_TARGET_DIR: join(root, "target", "store-test") },
    });
    process.exit(t.status ?? 1);
  } else {
    const bin = pgBin();
    const port = bin ? runningPort(bin) : null;
    console.log(
      !bin ? `[db] Postgres ${PG_MAJOR} is not installed; ${installHint()}`
        : port ? `[db] running on 127.0.0.1:${port}` : "[db] not running",
    );
  }
}
