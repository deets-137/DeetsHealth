import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// https://vite.dev/config/
export default defineConfig({
  // This folder is the site's root, however Vite is started (npm run dev passes it too).
  root: fileURLToPath(new URL(".", import.meta.url)),
  // The default port, or the next free one: never a fixed port (CLAUDE.md › Conventions).
  // Vite's hot reload rides the same port, so nothing else needs to know it.
  server: { port: 1420, strictPort: false },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // The browsers the console supports (CONSOLE.md §2). Chrome and Edge 109 are the last on
    // Windows 7 and 8.1, which some hospital PCs still run; Firefox 115 is its long-term
    // release; Safari 16 is macOS Monterey's.
    target: ["chrome109", "edge109", "firefox115", "safari16"],
  },
});
