// The Analytics filter bar (ANALYTICS.md §3: one filter bar, kept across the views; built with
// Logs, §3.1). One state for every view, kept for this viewer in localStorage; each view draws
// its own bar on it. The data class comes first: the list shows one class at a time, and on a
// dev machine the only class is Synthetic (DATABASE.md §7.6; the owner's A).
//
// The range is read by the store (stores.ts › logs, a fixed word); the rest filter the rows the
// store holds, in the page. No value from a call is kept here: the filters are fixed words and
// version labels.
import type { LoggedCall } from "../agent/api";
import { RANGES, type Range } from "../agent/stores";
import { el } from "./page";

export type Filters = {
  range: Range;
  origin: string;
  outcome: string;
  ehr: string;
  seat: string;
  /** `kind:label`: `prompt:11-21 | 3`, `system:…`, `hospital:…`, `model:…`; "" for any. */
  version: string;
  flagged: boolean;
  slow: boolean;
};

/** The target for one turn (DEBUGGING.md §1; timeline.ts). */
export const TURN_TARGET_MS = 800;

const KEY = "deets.console.analytics.filters";
const ANY: Filters = { range: "30d", origin: "", outcome: "", ehr: "", seat: "", version: "", flagged: false, slow: false };

const RANGE_WORDS: Record<Range, string> = {
  today: "Today",
  "7d": "Last 7 days",
  "30d": "Last 30 days",
  "90d": "Last 90 days",
  all: "All time",
};
const ORIGINS: [string, string][] = [["", "Any origin"], ["test", "Test"], ["kept", "Kept"], ["typed", "Typed"]];
const OUTCOMES: [string, string][] = [
  ["", "Any outcome"],
  ["booked", "Booked"],
  ["handed_off", "Handed off"],
  ["abandoned", "Abandoned"],
  ["crashed", "Crashed"],
  ["locked", "Locked"],
  ["ended", "Ended"],
];
const EHRS: [string, string][] = [["", "Any EHR"], ["local", "Local"], ["epic", "Epic"]];
const SEATS: [string, string][] = [["", "Any seat"], ["text", "Text"], ["voice", "Voice"]];
const VERSION_KINDS: [keyof LoggedCall, string, string][] = [
  ["prompt_version", "prompt", "Words"],
  ["system_version", "system", "System"],
  ["hospital_version", "hospital", "Hospital"],
  ["model", "model", "Model"],
];

function load(): Filters {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null") as Partial<Filters> | null;
    if (!v || typeof v !== "object") return { ...ANY };
    const f = { ...ANY, ...v };
    if (!RANGES.includes(f.range)) f.range = ANY.range;
    return f;
  } catch {
    return { ...ANY };
  }
}

let state: Filters = load();
const listeners = new Set<() => void>();

export const filters = (): Filters => state;

/** Changes the filters for every view: a chart's drill-down sets one before it opens Logs. */
export function setFilters(change: Partial<Filters>): void {
  set(change);
}

function set(change: Partial<Filters>): void {
  state = { ...state, ...change };
  try {
    localStorage.setItem(KEY, JSON.stringify(state));
  } catch {
    /* storage blocked: the filters hold for this visit */
  }
  for (const fn of listeners) fn();
}

/** Runs `fn` after each change of the filters. */
export function onFilters(fn: () => void): void {
  listeners.add(fn);
}

/** A call whose requests the replay cache answered (a kept-script draw): its waits are the
 * cache's, not a caller's (latency.py leaves such rows out too). */
export const replayed = (c: LoggedCall): boolean => c.requests_replayed > 0;

/** The slowest turn of a call, or null when no turn was answered or the call was replayed. */
export const slowest = (c: LoggedCall): number | null =>
  c.turn_waits_ms.length && !replayed(c) ? Math.max(...c.turn_waits_ms) : null;

/** A call is flagged when it has a staff flag or a review flag. */
export const flagged = (c: LoggedCall): boolean => c.staff_flags.length > 0 || c.review_flags.length > 0;

/** Whether a call passes every filter but the range (the store read it). */
export function passes(c: LoggedCall, f: Filters = state): boolean {
  if (f.origin && c.origin !== f.origin) return false;
  if (f.outcome && c.outcome !== f.outcome) return false;
  if (f.ehr && (c.ehr ?? "local") !== f.ehr) return false;
  if (f.seat && (c.seat ?? "text") !== f.seat) return false;
  if (f.flagged && !flagged(c)) return false;
  if (f.slow && !((slowest(c) ?? 0) > TURN_TARGET_MS)) return false;
  if (f.version) {
    const at = f.version.indexOf(":");
    const kind = f.version.slice(0, at);
    const label = f.version.slice(at + 1);
    const field = VERSION_KINDS.find(([, k]) => k === kind)?.[0];
    if (!field || c[field] !== label) return false;
  }
  return true;
}

function select(label: string, parts: [string, string][], value: string, on: (v: string) => void): HTMLSelectElement {
  const s = document.createElement("select");
  s.className = "cfg-field an-select";
  s.setAttribute("aria-label", label);
  for (const [v, words] of parts) s.add(new Option(words, v, false, v === value));
  s.addEventListener("change", () => on(s.value));
  return s;
}

function toggle(words: string, value: boolean, on: (v: boolean) => void): HTMLElement {
  const wrap = el(`<div class="tabs an-toggle" role="group"><button class="tab" type="button"></button></div>`);
  const b = wrap.querySelector<HTMLButtonElement>("button")!;
  b.textContent = words;
  b.setAttribute("aria-pressed", String(value));
  b.addEventListener("click", () => {
    const now = b.getAttribute("aria-pressed") !== "true";
    b.setAttribute("aria-pressed", String(now));
    on(now);
  });
  return wrap;
}

export type FilterBar = {
  el: HTMLElement;
  /** The version labels the rows hold, so the Version list offers only those. */
  setVersions(calls: LoggedCall[]): void;
};

/** One view's filter bar on the shared state. `search`: the view's own search box, at its end. */
export function filterBar(search?: HTMLElement): FilterBar {
  const bar = el(`<div class="an-filters" role="group" aria-label="Filters"></div>`);
  const f = state;
  const klass = select("Data class", [["synthetic", "Synthetic"]], "synthetic", () => {});
  klass.title = "The class of calls listed. A dev machine holds synthetic calls only (DATABASE.md §7)";
  klass.classList.add("an-class");
  const version = select("Version", [["", "Any version"]], f.version, (v) => set({ version: v }));
  const selects: [keyof Filters, HTMLSelectElement][] = [
    ["range", select("Dates", RANGES.map((r) => [r, RANGE_WORDS[r]]), f.range, (v) => set({ range: v as Range }))],
    ["origin", select("Origin", ORIGINS, f.origin, (v) => set({ origin: v }))],
    ["outcome", select("Outcome", OUTCOMES, f.outcome, (v) => set({ outcome: v }))],
    ["ehr", select("EHR", EHRS, f.ehr, (v) => set({ ehr: v }))],
    ["seat", select("Seat", SEATS, f.seat, (v) => set({ seat: v }))],
  ];
  const toggles: [keyof Filters, HTMLElement][] = [
    ["flagged", toggle("Flagged only", f.flagged, (v) => set({ flagged: v }))],
    ["slow", toggle(`Over ${TURN_TARGET_MS} ms only`, f.slow, (v) => set({ slow: v }))],
  ];
  bar.append(klass, ...selects.map(([, s]) => s), version, ...toggles.map(([, t]) => t));
  if (search) bar.append(search);
  // Another view's bar, or a drill-down, changed the filters: this bar shows them too.
  onFilters(() => {
    for (const [k, s] of selects) if (s.value !== state[k]) s.value = String(state[k]);
    for (const [k, t] of toggles) t.querySelector("button")!.setAttribute("aria-pressed", String(state[k]));
    if (version.value !== state.version) {
      if (![...version.options].some((o) => o.value === state.version)) version.add(new Option(state.version.replace(":", " "), state.version));
      version.value = state.version;
    }
  });
  return {
    el: bar,
    setVersions(calls) {
      const chosen = state.version;
      version.replaceChildren(new Option("Any version", "", false, chosen === ""));
      let found = chosen === "";
      for (const [field, kind, words] of VERSION_KINDS) {
        const labels = [...new Set(calls.map((c) => c[field]).filter((x): x is string => typeof x === "string" && x !== ""))].sort().reverse();
        if (!labels.length) continue;
        const group = document.createElement("optgroup");
        group.label = words;
        for (const l of labels) {
          const v = `${kind}:${l}`;
          if (v === chosen) found = true;
          group.append(new Option(`${words} ${l}`, v, false, v === chosen));
        }
        version.append(group);
      }
      // A version no row holds now stays chosen, so the filter never changes by itself.
      if (!found) version.add(new Option(chosen.replace(":", " "), chosen, false, true));
    },
  };
}
