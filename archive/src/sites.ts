// The Sites state: the connected sites and what each one is doing now (CORE-CRATE.md §2,
// S1–S6). The Settings card draws it (settings-card.ts › Sites); this module owns the Rust
// calls and events, so the rows read one state wherever they are drawn.
//
//   sites_list      → the Connect menu and one row per saved site (SitesPayload)
//   sign_in_start   Connect › a site           site_sign_in   a row's Sign in
//   sign_in_cancel  Cancel while waiting       site_refresh   Refresh
//   site_sign_out   Sign out, after its confirm
//   events: sign-in, sites-changed, fetch-progress, fetch-done (errors toast in app-errors.ts)
//
// **The PHI wall (SECURITY.md §4.3).** A site's `name` is protected for a real site. It goes
// into the page by `textContent` only (settings-card.ts), never into markup, a hover hint, a
// toast, diag or the log. Everything else here is a count, a fixed word or the app's own
// random site key.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import * as diag from "./diag";
import { toast } from "./toast";
import type { CatalogSite } from "./model/CatalogSite";
import type { FetchDone } from "./model/FetchDone";
import type { FetchProgress } from "./model/FetchProgress";
import type { SignInStatus } from "./model/SignInStatus";
import type { SiteCounts } from "./model/SiteCounts";
import type { SitesPayload } from "./model/SitesPayload";
import type { SiteView } from "./model/SiteView";

/** What a row shows beyond its saved data: a fetch in flight, a failed fetch, a dead token. */
interface Live {
  /** Counts of the fetch now running; absent when none runs. */
  fetching?: SiteCounts;
  failed?: boolean;
  signInAgain?: boolean;
}

let catalog: CatalogSite[] = [];
let sites: SiteView[] = [];
const live = new Map<string, Live>();
/** The one waiting sign-in: "new" for Connect, a site key for a row's Sign in. */
let waiting: string | null = null;

/** "list": rows or halves change (a full re-render). "live": only the status lines. */
export type SitesChange = "list" | "live";
const listeners = new Set<(c: SitesChange) => void>();
const notify = (c: SitesChange) => listeners.forEach((f) => f(c));
export const onSitesChange = (f: (c: SitesChange) => void): (() => void) => {
  listeners.add(f);
  return () => listeners.delete(f);
};

export const sitesCatalog = (): CatalogSite[] => catalog;
export const sitesList = (): SiteView[] => sites;
export const siteLive = (key: string): Live => live.get(key) ?? {};
export const signInWaiting = (): string | null => waiting;

const liveOf = (key: string): Live => {
  let l = live.get(key);
  if (!l) live.set(key, (l = {}));
  return l;
};

async function load(): Promise<void> {
  try {
    const p = await invoke<SitesPayload>("sites_list");
    catalog = p.catalog;
    sites = p.sites;
    for (const k of [...live.keys()]) if (!sites.some((s) => s.key === k)) live.delete(k);
  } catch (err) {
    diag.error("sites: list", { kind: typeof err === "string" ? err : "unknown" });
  }
  notify("list");
}

// ── actions: each Rust failure already toasts through `app-error`; here only the row ──

export async function connect(index: number): Promise<void> {
  waiting = "new";
  notify("list");
  try {
    await invoke("sign_in_start", { site: index });
  } catch {
    waiting = null;
    notify("list");
  }
}

export async function signInAgain(key: string): Promise<void> {
  waiting = key;
  notify("list");
  try {
    await invoke("site_sign_in", { site: key });
  } catch {
    waiting = null;
    notify("list");
  }
}

export function cancelSignIn(): void {
  waiting = null;
  notify("list");
  invoke("sign_in_cancel").catch(() => {});
}

export async function refresh(key: string): Promise<void> {
  const l = liveOf(key);
  if (l.fetching) return; // the row already counts up; Rust would say "already updating"
  l.fetching = emptyCounts();
  l.failed = false;
  notify("list");
  try {
    await invoke("site_refresh", { site: key });
  } catch {
    l.fetching = undefined;
    notify("list");
  }
}

export async function signOut(key: string): Promise<void> {
  try {
    await invoke("site_sign_out", { site: key });
    toast({ kind: "success", text: "Signed out. This site's data is erased from this PC." });
  } catch {
    /* toasted by app-error; `sites-changed` redraws the row */
  }
}

const emptyCounts = (): SiteCounts => ({ results: 0, medicines: 0, allergies: 0, conditions: 0, visits: 0 });

/** Listen for the four events, then load the rows. Once, at boot. */
export async function initSites(): Promise<void> {
  try {
    await listen<SignInStatus>("sign-in", (ev) => {
      if (ev.payload === "waiting") return; // set by the click that started it
      waiting = null;
      notify("list");
    });
    await listen("sites-changed", () => void load());
    await listen<FetchProgress>("fetch-progress", (ev) => {
      const l = liveOf(ev.payload.site);
      const first = !l.fetching;
      l.fetching = ev.payload.counts;
      l.failed = false;
      l.signInAgain = false; // a page came back: the token works
      notify(first ? "list" : "live");
    });
    await listen<FetchDone>("fetch-done", (ev) => {
      const l = liveOf(ev.payload.site);
      l.fetching = undefined;
      l.failed = ev.payload.end === "failed";
      l.signInAgain = ev.payload.end === "sign-in-again";
      void load();
    });
  } catch (err) {
    diag.error("sites: init", { kind: err instanceof Error ? err.name : typeof err });
  }
  await load();
}

// ── the status line under a row's label: fixed words and counts only ──

const COUNT_WORDS: [keyof SiteCounts, string, string][] = [
  ["results", "result", "results"],
  ["medicines", "medicine", "medicines"],
  ["allergies", "allergy", "allergies"],
  ["conditions", "condition", "conditions"],
  ["visits", "visit", "visits"],
];

/** "1,240 results · 38 medicines": the kinds with a count above 0, in a fixed order. */
export function countsText(c: SiteCounts): string {
  return COUNT_WORDS.filter(([k]) => c[k] > 0)
    .map(([k, one, many]) => `${c[k].toLocaleString()} ${c[k] === 1 ? one : many}`)
    .join(" · ");
}

/** "Updated just now", "Updated 2 min ago", … */
export function updatedText(at: number | null): string {
  if (at === null) return "Not updated yet";
  const s = Math.max(0, Date.now() / 1000 - at);
  if (s < 60) return "Updated just now";
  if (s < 3600) return `Updated ${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `Updated ${Math.floor(s / 3600)} h ago`;
  const d = Math.floor(s / 86400);
  return `Updated ${d} ${d === 1 ? "day" : "days"} ago`;
}

export const WAITING_TEXT = "Finish signing in in your browser";
