// Live reload in the demo build (docs/ops/DEMO.md §3.2). The dev server watches the build and
// sends the page's entry script each time it changes (`GET /agent/build`). When it differs from
// this page's own, the page reloads itself, but never during a call in this tab or over an edit
// not saved on a Config page: those hold the reload until they end.
//
// On this PC (`npm run dev`) the page is at `/`, Vite's own hot reload runs, and this does
// nothing.
import { at } from "./agent/api";

const holds = new Set<string>();
let newer = false;

/** Hold the reload while `on` (a call runs, an edit is not saved); release it when not. */
export function holdReload(key: string, on: boolean): void {
  if (on) holds.add(key);
  else holds.delete(key);
  maybeReload();
}

function maybeReload(): void {
  if (newer && holds.size === 0) location.reload();
}

/** This page's own entry script, as the build wrote it into index.html. */
function ownEntry(): string | null {
  const s = document.querySelector<HTMLScriptElement>('script[type="module"][src]');
  return s ? new URL(s.src, location.href).pathname : null;
}

export function startLiveReload(): void {
  if (import.meta.env.BASE_URL === "/") return;
  const mine = ownEntry();
  if (!mine) return;
  // EventSource reconnects on its own after a drop (a restart of npm run demo).
  const source = new EventSource(at("/agent/build"));
  source.addEventListener("message", (e) => {
    const entry = new URL(String(e.data), location.href).pathname;
    if (entry === mine) return;
    newer = true;
    maybeReload();
  });
}
