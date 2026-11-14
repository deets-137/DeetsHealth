import type { ServerResponse } from "node:http";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";

const agentPort = process.env.DEETS_AGENT_PORT;
const agentToken = process.env.DEETS_AGENT_TOKEN ?? "";

// When the agent cannot be reached, the page is told so in a way the agent's own answers never
// use: this header (the agent answers 502 and 503 itself). api.ts reads it as "the agent did
// not answer", so the read layer shows "not running" and tries again (LOADING.md §2, rule 7).
const UNREACHABLE = { "x-deets-agent": "unreachable", "content-type": "text/plain" };
const unreachable = (res: ServerResponse) => {
  if (!res.headersSent && !res.writableEnded) res.writeHead(503, UNREACHABLE).end();
};

/** `npm run dev:web` (no agent): every /agent request is answered "unreachable". */
const noAgent: Plugin = {
  name: "deets-no-agent",
  configureServer(server) {
    if (agentPort) return;
    server.middlewares.use("/agent", (_req, res) => unreachable(res));
  },
};

// https://vite.dev/config/
export default defineConfig({
  plugins: [noAgent],
  // This folder is the site's root, however Vite is started (npm run dev passes it too).
  root: fileURLToPath(new URL(".", import.meta.url)),
  // The default port, or the next free one: never a fixed port (CLAUDE.md › Conventions).
  // Vite's hot reload rides the same port, so nothing else needs to know it.
  // `npm run dev` (scripts/dev.mjs) picks the port itself and passes --port with --strictPort.
  server: {
    port: 1420,
    strictPort: false,
    // The demo's build (docs/ops/DEMO.md §3.2) is written inside this folder on each save. It
    // is not this page's source: watching it reloaded the open page on every rebuild, which
    // threw away an edit not saved (the owner, 2026-11-14).
    watch: { ignored: ["**/dist-demo/**", "**/dist/**"] },
    // The agent's dev server (CHAT.md §5), when scripts/dev.mjs started one: /agent/* goes
    // there, with the launch token added here so the page never holds it. The Origin header
    // passes through unchanged; the agent checks it.
    proxy: agentPort
      ? {
          "/agent": {
            target: `http://127.0.0.1:${agentPort}`,
            ws: true,
            headers: { "x-deets-token": agentToken },
            // Runs before Vite's own handler, which then sees the answer sent and writes none.
            configure: (proxy) =>
              proxy.on("error", (_err, _req, res) => {
                if (res && "writeHead" in res) unreachable(res as ServerResponse);
              }),
          },
        }
      : undefined,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // The browsers the console supports (CONSOLE.md §2). Chrome and Edge 109 are the last on
    // Windows 7 and 8.1, which some hospital PCs still run; Firefox 115 is its long-term
    // release; Safari 16 is macOS Monterey's.
    target: ["chrome109", "edge109", "firefox115", "safari16"],
  },
});
