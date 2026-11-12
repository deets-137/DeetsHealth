// `npm run dev`: the console and the agent's dev server together (CHAT.md §5; the owner,
// 2026-11-12, fork 2A). Never a fixed port: several Deets apps run on one machine at once.
// 1. Pick a free port for Vite (from 1420 up) and one for the agent (from 8790 up).
// 2. Make a fresh token for this launch. The agent refuses every request without it.
// 3. Start the agent with the token and the console's origin; wait for its first line.
// 4. Start Vite on its port. Its proxy sends /agent/* to the agent and adds the token, so the
//    page never holds it (console/vite.config.ts).
// Ctrl+C stops both. `npm run dev:web` starts the console alone (no agent).
import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { createServer } from "node:net";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

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

const webPort = await freePort(1420);
const agentPort = await freePort(8790);
const token = randomBytes(32).toString("hex");
const origins = [`http://localhost:${webPort}`, `http://127.0.0.1:${webPort}`];

const children = [];
const stop = (code = 0) => {
  for (const c of children) if (c.exitCode === null) c.kill("SIGTERM");
  process.exit(code);
};
process.on("SIGINT", () => stop(0));
process.on("SIGTERM", () => stop(0));

const agent = spawn(
  "uv",
  ["run", "python", "-m", "deets_agent.dev_server", "--port", String(agentPort), ...origins.flatMap((o) => ["--origin", o])],
  { cwd: join(root, "agent"), env: { ...process.env, DEETS_DEV_TOKEN: token }, stdio: ["ignore", "pipe", "inherit"] },
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

const vite = spawn("npx", ["vite", "console", "--port", String(webPort), "--strictPort"], {
  cwd: root,
  env: { ...process.env, DEETS_AGENT_PORT: String(agentPort), DEETS_AGENT_TOKEN: token },
  stdio: "inherit",
});
children.push(vite);
vite.on("exit", (code) => stop(code ?? 0));
