import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const agentPort = process.env.DEETS_AGENT_PORT;
const agentToken = process.env.DEETS_AGENT_TOKEN ?? "";

// https://vite.dev/config/
export default defineConfig({
  // This folder is the site's root, however Vite is started (npm run dev passes it too).
  root: fileURLToPath(new URL(".", import.meta.url)),
  // The default port, or the next free one: never a fixed port (CLAUDE.md › Conventions).
  // Vite's hot reload rides the same port, so nothing else needs to know it.
  // `npm run dev` (scripts/dev.mjs) picks the port itself and passes --port with --strictPort.
  server: {
    port: 1420,
    strictPort: false,
    // The agent's dev server (CHAT.md §5), when scripts/dev.mjs started one: /agent/* goes
    // there, with the launch token added here so the page never holds it. The Origin header
    // passes through unchanged; the agent checks it.
    proxy: agentPort
      ? {
          "/agent": {
            target: `http://127.0.0.1:${agentPort}`,
            ws: true,
            headers: { "x-deets-token": agentToken },
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
