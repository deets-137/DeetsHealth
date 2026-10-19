import { defineConfig } from "vite";

// https://vite.dev/config/
export default defineConfig({
  // The default port, or the next free one: never a fixed port (CLAUDE.md › Conventions).
  server: { port: 1420, strictPort: false },
});
