// `npm run dev:app` — tauri dev, isolated from the installed app AND from any other dev
// server on the machine (DeetsMusic's scripts/dev-app.mjs, cut down):
//   1. find a free port from 1420 up (another Deets* project may hold 1420);
//   2. merge src-tauri/tauri.dev.conf.json (the dev identifier) with a build.devUrl on that
//      port and "(dev)" window titles into a generated, gitignored config. The titles are
//      stamped onto the FULL window objects from tauri.conf.json: Tauri merges an overlay as
//      a JSON merge patch, so an array replaces the whole array;
//   3. run `tauri dev --config <generated>` with VITE_PORT set so vite.config.ts binds the
//      same port;
//   4. open a WebView2 remote-debugging (CDP) port on the main window, from 9222 up, for
//      console calls from outside the app. It is appended to the window's own
//      additionalBrowserArgs — the env var would replace them.
//   5. flags for measuring: --perf (hold DevTools shut), --built (serve a release-shaped
//      bundle), --gpu=off|slow (pretend to be a weaker machine).
//   6. --fresh : be a first-time user (wipes the DEV profile only).
import { createServer } from "node:net";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const base = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.dev.conf.json"), "utf8"));
const conf = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));

// Vite binds `localhost`, which on this Windows box resolves to ::1 first — so probe both
// families, or an orphaned vite on ::1 reads as free and the launch fails.
const freeOn = (port, host) =>
  new Promise((resolve) => {
    const s = createServer();
    s.once("error", () => resolve(false));
    s.listen(port, host, () => s.close(() => resolve(true)));
  });
const free = async (port) => (await freeOn(port, "127.0.0.1")) && (await freeOn(port, "::1"));

let port = 1420;
while (!(await free(port))) port += 1;
let cdp = 9222;
while (!(await free(cdp))) cdp += 1;

// --gpu=off | slow — pretend this is a weaker machine.
//   off  : no GPU at all (software drawing) — a hard floor.
//   slow : the GPU stays, but raster moves to the CPU and its memory is squeezed to 64 MB.
const GPU_FLAGS = {
  off: "--disable-gpu --disable-gpu-compositing",
  slow: "--disable-gpu-rasterization --disable-accelerated-2d-canvas --force-gpu-mem-available-mb=64",
};
const gpuMode = (process.argv.find((a) => a.startsWith("--gpu=")) ?? "").slice(6);
if (gpuMode && !GPU_FLAGS[gpuMode]) {
  console.error(`[dev:app] unknown --gpu=${gpuMode} — use ${Object.keys(GPU_FLAGS).join(" or ")}`);
  process.exit(2);
}

// --fresh — launch as a FIRST-TIME USER. Everything a first run does not have lives in two
// folders, both named after the identifier: %APPDATA%\<id> (settings.json, the logs, later
// the sealed cache) and %LOCALAPPDATA%\<id>\EBWebView (localStorage — theme, skin, surface,
// the layout keys). Delete both and the next launch is a true first run.
// The guard is the whole safety story: the identifier comes from tauri.dev.conf.json and
// must end in ".dev", so this can never delete the installed app's data.
if (process.argv.includes("--fresh")) {
  const id = base.identifier;
  if (!id.endsWith(".dev")) {
    console.error(`[dev:app] --fresh refuses identifier "${id}": it is not a dev profile. Nothing was deleted.`);
    process.exit(2);
  }
  const roaming = join(process.env.APPDATA, id);
  const local = join(process.env.LOCALAPPDATA, id, "EBWebView");
  for (const dir of [roaming, local]) {
    try {
      rmSync(dir, { recursive: true, force: true });
    } catch (e) {
      // Windows will not delete a file a process holds open — the dev app is still running.
      console.error(`[dev:app] --fresh could not clear ${dir}: ${e.message}`);
      console.error(`[dev:app] Close the dev app first, then try again.`);
      process.exit(2);
    }
    // Look at the target afterwards. From inside an MSIX package (a shell in the Claude
    // desktop app) the delete lands in the package's private copy of %APPDATA%, throws
    // nothing, and leaves the real folder untouched. Run it from an ordinary terminal.
    if (existsSync(dir)) {
      console.error(`[dev:app] --fresh deleted ${dir} and it is STILL THERE. Nothing was cleared.`);
      console.error(`[dev:app] A sandboxed or redirected shell (an MSIX package) does this. Run it from a normal terminal.`);
      process.exit(2);
    }
  }
  console.log(`[dev:app] --fresh: cleared ${id} — first-time user`);
}

const windows = conf.app.windows.map((w) => ({
  ...w,
  title: `${w.title} (dev)`,
  ...(w.label === "main"
    ? {
        additionalBrowserArgs:
          `${w.additionalBrowserArgs ?? ""} --remote-debugging-port=${cdp}${gpuMode ? " " + GPU_FLAGS[gpuMode] : ""}`.trim(),
      }
    : {}),
}));

// --perf  : hold DevTools shut (it renders in the same GPU process and distorts every
//           graphics measurement).
// --built : also serve a RELEASE-SHAPED bundle — `vite build` with VITE_PERF=1, served by
//           `vite preview` — so the page is minified and bundled like the installed app,
//           while the telemetry stays compiled in.
const BUILT = process.argv.includes("--built");
const PERF = BUILT || process.argv.includes("--perf");
const passThrough = process.argv
  .slice(2)
  .filter((a) => a !== "--perf" && a !== "--built" && !a.startsWith("--gpu=") && a !== "--fresh");

let preview;
if (BUILT) {
  console.log(`[dev:app] --built: building a release-shaped bundle with the telemetry kept…`);
  const build = spawn(process.execPath, [join(root, "node_modules", "vite", "bin", "vite.js"), "build"], {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, VITE_PERF: "1" },
  });
  const code = await new Promise((r) => build.on("exit", r));
  if (code) process.exit(code);
  preview = spawn(
    process.execPath,
    [join(root, "node_modules", "vite", "bin", "vite.js"), "preview", "--port", String(port), "--strictPort"],
    { cwd: root, stdio: "inherit", env: { ...process.env } },
  );
  await new Promise((r) => setTimeout(r, 1200)); // let preview bind before tauri loads the URL
}

const gen = join(root, "src-tauri", ".tauri.dev.gen.json");
writeFileSync(
  gen,
  JSON.stringify(
    {
      ...base,
      app: { windows },
      // With --built the bundle is already made and served by preview, so tauri must NOT
      // start the dev server on top of it.
      build: { devUrl: `http://localhost:${port}`, ...(BUILT ? { beforeDevCommand: "" } : {}) },
    },
    null,
    2,
  ),
);
console.log(
  `[dev:app] ${BUILT ? "release-shaped bundle (vite preview)" : "vite"} on ${port} · webview CDP on ${cdp} · identifier ${base.identifier}` +
    (PERF ? " · DevTools held shut (--perf)" : "") +
    (gpuMode ? ` · GPU ${gpuMode.toUpperCase()} (pretending to be a weaker machine)` : ""),
);

// Run the local tauri CLI directly (no npx/shell) so a space in the path — this user dir
// has one — can't split an argument; the config is passed relative to the repo root.
const tauriBin = join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
const child = spawn(
  process.execPath,
  [tauriBin, "dev", "--config", "src-tauri/.tauri.dev.gen.json", ...passThrough],
  {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, VITE_PORT: String(port), ...(PERF ? { DEETS_NO_DEVTOOLS: "1" } : {}) },
  },
);
child.on("exit", (code) => {
  preview?.kill(); // the preview server outlives tauri otherwise and holds the port
  process.exit(code ?? 0);
});
