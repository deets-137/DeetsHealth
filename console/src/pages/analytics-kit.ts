// The parts the Analytics views share (ANALYTICS.md §3.2): a view's frame on the sums (the title,
// the filter bar, a body drawn from the `stats` store), the headline tiles with their change, and
// the charts. Every chart is drawn by this code in HTML, in the theme's roles (styles/analytics
// .css): no chart library. A chart names counts, times and dollars only; its hints too.
import type { Stats } from "../agent/api";
import { stats, statsKey } from "../agent/stores";
import { filterBar, filters, onFilters, setFilters, TURN_TARGET_MS, type Filters } from "./analytics-filter";
import { el } from "./page";
import { goView, type View } from "./subnav";

// ---- words and numbers

/** The outcomes in the store's order (stats.rs › OUTCOMES), with their words and chart class. */
export const OUTCOMES: { key: string; word: string; cls: string }[] = [
  { key: "booked", word: "Booked", cls: "ch-booked" },
  { key: "handed_off", word: "Handed off", cls: "ch-handed" },
  { key: "abandoned", word: "Abandoned", cls: "ch-abandoned" },
  { key: "crashed", word: "Crashed", cls: "ch-crashed" },
  { key: "locked", word: "Locked", cls: "ch-locked" },
  { key: "ended", word: "Ended", cls: "ch-ended" },
];

export const span = (ms: number) => (ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(2)} s`);
export const dollars = (n: number) => (n === 0 ? "$0" : n < 0.1 ? `$${n.toFixed(3)}` : n < 100 ? `$${n.toFixed(2)}` : `$${Math.round(n).toLocaleString()}`);
export const pct = (part: number, whole: number) => (whole ? `${Math.round((part / whole) * 100)}%` : "—");
export const count = (n: number, one: string, many = `${one}s`) => `${n.toLocaleString()} ${n === 1 ? one : many}`;
export const words = (key: string) => {
  const w = key.replace(/_/g, " ");
  return w.charAt(0).toUpperCase() + w.slice(1);
};

/** What the range before is called: "the 7 days before". */
const PREV_WORDS: Record<string, string> = {
  today: "yesterday",
  "7d": "the 7 days before",
  "30d": "the 30 days before",
  "90d": "the 90 days before",
};

/** A call's Cartesia cost in dollars, or null when the plan's price is not set. */
export const cartesiaUsd = (s: Stats, credits: number) => (s.usd_per_credit === null ? null : credits * s.usd_per_credit);
/** Claude's dollars and, when the price is set, Cartesia's. */
export const allUsd = (s: Stats, t: { cost_usd: number; cartesia_credits: number }) => t.cost_usd + (cartesiaUsd(s, t.cartesia_credits) ?? 0);

// ---- the frame

export type Drawn = { stats: Stats; f: Filters; view: HTMLElement };

/** The filters as the route takes them (dev_server.py › stats). */
function routeFilters(f: Filters): Record<string, string> {
  const out: Record<string, string> = {};
  if (f.origin) out.origin = f.origin;
  if (f.outcome) out.outcome = f.outcome;
  if (f.ehr) out.ehr = f.ehr;
  if (f.seat) out.seat = f.seat;
  if (f.version) out.version = f.version;
  if (f.flagged) out.flagged = "true";
  if (f.slow) out.slow = "true";
  return out;
}

/** One view on the sums: its title, the shared filter bar, a count line, and a body that
 * `draw` fills from each answer of the store. It reads again each time the view opens. */
export function statsView(id: string, title: string, draw: (d: Drawn) => HTMLElement[]): View {
  return {
    id,
    title,
    build() {
      const bar = filterBar();
      const root = el(`
        <div class="auth an-view">
          <div class="auth-top">
            <h2 class="auth-title"></h2>
            <p class="cfg-status an-count" role="status"></p>
          </div>
          <div class="auth-scroll own-scrollbars an-body"></div>
        </div>`);
      root.querySelector("h2")!.textContent = title;
      root.insertBefore(bar.el, root.querySelector(".an-body"));
      const line = root.querySelector<HTMLElement>(".an-count")!;
      const body = root.querySelector<HTMLElement>(".an-body")!;
      body.replaceChildren(el(`<p class="empty">Loading</p>`));
      let key = "";
      let unsubscribe: (() => void) | null = null;
      const watch = () => {
        const f = filters();
        const next = statsKey(f.range, routeFilters(f));
        if (next === key) return;
        key = next;
        unsubscribe?.();
        unsubscribe = stats(key).subscribe(body, (s) => {
          if (s.value) {
            const t = s.value.totals;
            line.textContent = `${count(t.calls, "call")}${t.replayed_calls ? ` · ${t.replayed_calls} replayed, their waits left out` : ""}`;
            body.replaceChildren(...(t.calls ? draw({ stats: s.value, f: filters(), view: root }) : [el(`<p class="empty">No calls fit these dates and filters</p>`)]));
            return;
          }
          if (s.status === "down") body.replaceChildren(el(`<p class="empty">The agent's dev server is not running</p>`));
          else if (s.status === "refused") {
            const p = el(`<p class="empty"></p>`);
            p.textContent = s.reason ?? "The agent's dev server refused";
            body.replaceChildren(p);
          }
        });
      };
      watch();
      onFilters(() => {
        if (root.isConnected) watch();
      });
      // Each time the view opens again, the sums are read again: a call may have ended since,
      // or the filters changed while it was hidden (a hidden view never reads).
      let open: boolean | null = null;
      new ResizeObserver(() => {
        const now = root.isConnected && root.offsetParent !== null;
        if (now && open === false) {
          const was = key;
          watch();
          if (key === was) void stats(key).refresh("user");
        }
        open = now;
      }).observe(root);
      return root;
    },
  };
}

/** Opens Logs with these filters set: every drill-down ends there. */
export function toLogs(from: HTMLElement, change: Partial<Filters>): void {
  setFilters(change);
  goView(from, "logs");
}

/** A section: its title, its note, then its parts. */
export function section(title: string, note: string | null, ...parts: HTMLElement[]): HTMLElement {
  const s = el(`<section class="an-section"><h3 class="an-section-title"></h3></section>`);
  s.querySelector("h3")!.textContent = title;
  if (note) {
    const p = el(`<p class="an-note"></p>`);
    p.textContent = note;
    s.append(p);
  }
  s.append(...parts);
  return s;
}

// ---- the headline tiles

export type Tile = {
  label: string;
  value: string;
  /** What the number rests on: "of 8 calls". */
  sub: string;
  /** The change against the range before, worked out by `change`. */
  delta?: Delta | null;
  /** A click opens this. */
  open?: () => void;
};

export type Delta = { text: string; better: boolean | null };

/** The change of a number against the range before: "▲ 4 pts vs the 7 days before". `up` says
 * whether a rise is better (true), worse (false) or neither (null). Null with no range before. */
export function change(
  f: Filters,
  now: number,
  before: number | null | undefined,
  up: boolean | null,
  as: "count" | "points" | "dollars" | "ms",
): Delta | null {
  const prev = PREV_WORDS[f.range];
  if (before === null || before === undefined || !prev) return null;
  const d = now - before;
  if (Math.abs(d) < (as === "dollars" ? 0.0005 : as === "ms" ? 1 : as === "points" ? 0.5 : 0.5)) return { text: `same as ${prev}`, better: null };
  const size =
    as === "points" ? `${Math.round(Math.abs(d))} pts` : as === "dollars" ? dollars(Math.abs(d)) : as === "ms" ? span(Math.abs(d)) : Math.abs(d).toLocaleString();
  return { text: `${d > 0 ? "▲" : "▼"} ${size} vs ${prev}`, better: up === null ? null : (d > 0) === up };
}

/** The share of `part` in `whole`, in points (0 to 100); null when there is no whole. */
export const share = (part: number, whole: number) => (whole ? (part / whole) * 100 : null);

export function tiles(items: Tile[]): HTMLElement {
  const row = el(`<div class="ch-tiles"></div>`);
  for (const t of items) {
    const tile = el(t.open ? `<button class="ch-tile" type="button"></button>` : `<div class="ch-tile"></div>`);
    const label = el(`<span class="ch-tile-label"></span>`);
    label.textContent = t.label;
    const value = el(`<span class="ch-tile-value"></span>`);
    value.textContent = t.value;
    const sub = el(`<span class="ch-tile-sub"></span>`);
    sub.textContent = t.sub;
    tile.append(label, value, sub);
    if (t.delta) {
      const d = el(`<span class="ch-tile-delta"></span>`);
      d.textContent = t.delta.text;
      if (t.delta.better !== null) d.classList.add(t.delta.better ? "is-better" : "is-worse");
      tile.append(d);
    }
    if (t.open) {
      tile.addEventListener("click", t.open);
      tile.setAttribute("aria-label", `${t.label}: ${t.value}. Open`);
    }
    row.append(tile);
  }
  return row;
}

// ---- the charts

export type Series = { name: string; cls: string };

/** Every day from the first to the last, so a day with no call is a gap, not a missing column.
 * `from`: the range's first day when it has one. */
export function allDays<T extends { day: string }>(days: T[], from: string | null, empty: (day: string) => T): T[] {
  const by = new Map(days.filter((d) => d.day).map((d) => [d.day, d]));
  const first = from ?? days.find((d) => d.day)?.day;
  const today = localDay(new Date());
  const last = [...by.keys()].sort().pop() ?? today;
  if (!first) return days;
  const out: T[] = [];
  const end = last > today ? last : today;
  for (let d = new Date(`${first}T12:00:00`); localDay(d) <= end && out.length < 400; d.setDate(d.getDate() + 1)) {
    const k = localDay(d);
    out.push(by.get(k) ?? empty(k));
  }
  return out;
}

export const localDay = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

/** The range's first day, on this browser's clock (the axis only; the sums use the server's). */
export function rangeStart(f: Filters): string | null {
  const back: Record<string, number> = { today: 0, "7d": 6, "30d": 29, "90d": 89 };
  if (!(f.range in back)) return null;
  const d = new Date();
  d.setDate(d.getDate() - back[f.range]);
  return localDay(d);
}

/** Columns over days, each stacked by series: the parts of each day, a hint per column, and a
 * click per series part when `pick` is given. The axis names the tallest column. */
export function columns(
  label: string,
  days: { day: string; parts: number[] }[],
  series: Series[],
  format: (n: number) => string,
  pick?: (series: number) => void,
): HTMLElement {
  const wrap = el(`<div class="ch-chart"></div>`);
  const top = Math.max(0, ...days.map((d) => d.parts.reduce((a, b) => a + b, 0)));
  const plot = el(`<div class="ch-cols" role="img"></div>`);
  plot.setAttribute("aria-label", `${label}: ${days.length} days, the most in one day ${format(top)}`);
  for (const d of days) {
    const total = d.parts.reduce((a, b) => a + b, 0);
    const col = el(`<div class="ch-col"></div>`);
    col.title = `${d.day.slice(5)}: ${format(total)}${series.length > 1 ? ` (${series.map((s, i) => `${s.name} ${format(d.parts[i] ?? 0)}`).join(", ")})` : ""}`;
    d.parts.forEach((v, i) => {
      if (!v) return;
      const seg = el(`<span class="ch-seg"></span>`);
      seg.classList.add(series[i].cls);
      seg.style.height = `${top ? (v / top) * 100 : 0}%`;
      if (pick) {
        seg.classList.add("is-pickable");
        seg.addEventListener("click", () => pick(i));
      }
      col.append(seg);
    });
    plot.append(col);
  }
  const axis = el(`<div class="ch-axis"><span></span><span>0</span></div>`);
  axis.firstElementChild!.textContent = format(top);
  const x = el(`<div class="ch-xlabels"><span></span><span></span></div>`);
  x.children[0].textContent = days[0]?.day.slice(5) ?? "";
  x.children[1].textContent = days[days.length - 1]?.day.slice(5) ?? "";
  const frame = el(`<div class="ch-frame"></div>`);
  frame.append(axis, plot);
  wrap.append(frame, x);
  if (series.length > 1) wrap.append(legend(series));
  return wrap;
}

export function legend(series: Series[], values?: string[]): HTMLElement {
  const l = el(`<ul class="ch-legend"></ul>`);
  series.forEach((s, i) => {
    const li = el(`<li><span class="ch-key" aria-hidden="true"></span><span></span></li>`);
    li.firstElementChild!.classList.add(s.cls);
    li.lastElementChild!.textContent = values ? `${s.name} ${values[i]}` : s.name;
    l.append(li);
  });
  return l;
}

/** Bars across, one per row: its label, its bar (as long as its share of the longest), its
 * value. A row with `open` is a button. */
export function bars(rows: { label: string; value: number; text: string; cls?: string; open?: () => void }[]): HTMLElement {
  const top = Math.max(0, ...rows.map((r) => r.value));
  const list = el(`<div class="ch-bars"></div>`);
  for (const r of rows) {
    const row = el(r.open ? `<button class="ch-bar-row" type="button"></button>` : `<div class="ch-bar-row"></div>`);
    const label = el(`<span class="ch-bar-label"></span>`);
    label.textContent = r.label;
    label.title = r.label;
    const track = el(`<span class="ch-bar-track" aria-hidden="true"><span class="ch-bar"></span></span>`);
    const fill = track.firstElementChild as HTMLElement;
    fill.style.width = `${top ? Math.max(1, (r.value / top) * 100) : 0}%`;
    fill.classList.add(r.cls ?? "ch-accent");
    const value = el(`<span class="ch-bar-value"></span>`);
    value.textContent = r.text;
    row.append(label, track, value);
    if (r.open) row.addEventListener("click", r.open);
    list.append(row);
  }
  return list;
}

/** One bar split into parts (Speed's stages), with each part's value in the legend. */
export function split(label: string, parts: { series: Series; value: number; text: string }[]): HTMLElement {
  const total = parts.reduce((a, p) => a + p.value, 0);
  const wrap = el(`<div class="ch-chart"></div>`);
  const barEl = el(`<div class="ch-split" role="img"></div>`);
  barEl.setAttribute("aria-label", `${label}: ${parts.map((p) => `${p.series.name} ${p.text}`).join(", ")}`);
  for (const p of parts) {
    if (!p.value) continue;
    const seg = el(`<span class="ch-split-part"></span>`);
    seg.classList.add(p.series.cls);
    seg.style.width = `${total ? (p.value / total) * 100 : 0}%`;
    seg.title = `${p.series.name}: ${p.text}`;
    barEl.append(seg);
  }
  wrap.append(barEl, legend(parts.map((p) => p.series), parts.map((p) => p.text)));
  return wrap;
}

/** Turns by wait: a column per bucket, a line at the target. */
export function histogram(hist: number[], bucketMs: number): HTMLElement {
  const top = Math.max(0, ...hist);
  const wrap = el(`<div class="ch-chart"></div>`);
  const plot = el(`<div class="ch-cols ch-hist" role="img"></div>`);
  const turns = hist.reduce((a, b) => a + b, 0);
  plot.setAttribute("aria-label", `The spread of ${turns} turns' waits, in ${span(bucketMs)} steps; a line at ${span(TURN_TARGET_MS)}`);
  hist.forEach((n, i) => {
    const col = el(`<div class="ch-col"></div>`);
    const from = i * bucketMs;
    col.title = `${i === hist.length - 1 ? `${span(from)} and over` : `${span(from)} to ${span(from + bucketMs)}`}: ${count(n, "turn")}`;
    if (n) {
      const seg = el(`<span class="ch-seg"></span>`);
      seg.classList.add(from >= TURN_TARGET_MS ? "ch-worse" : "ch-accent");
      seg.style.height = `${top ? (n / top) * 100 : 0}%`;
      col.append(seg);
    }
    plot.append(col);
  });
  const line = el(`<span class="ch-target" aria-hidden="true"></span>`);
  line.style.left = `${(TURN_TARGET_MS / (bucketMs * hist.length)) * 100}%`;
  plot.append(line);
  const axis = el(`<div class="ch-axis"><span></span><span>0</span></div>`);
  axis.firstElementChild!.textContent = String(top);
  const frame = el(`<div class="ch-frame"></div>`);
  frame.append(axis, plot);
  const x = el(`<div class="ch-xlabels"><span>0</span><span></span></div>`);
  x.children[1].textContent = `${span(bucketMs * (hist.length - 1))}+`;
  wrap.append(frame, x);
  return wrap;
}

/** A short list as rows of words: a header, then each row's cells. A row with `open` is a
 * button. */
export function list(heads: string[], rows: { cells: string[]; open?: () => void; cls?: string }[]): HTMLElement {
  const t = el(`<div class="ch-list" role="table"></div>`);
  t.style.setProperty("--ch-grid", `minmax(0, 2fr) repeat(${Math.max(1, heads.length - 1)}, minmax(0, 1fr))`);
  const head = el(`<div class="ch-list-row ch-list-head" role="row"></div>`);
  for (const h of heads) {
    const c = el(`<span role="columnheader"></span>`);
    c.textContent = h;
    head.append(c);
  }
  t.append(head);
  for (const r of rows) {
    const row = el(r.open ? `<button class="ch-list-row" type="button" role="row"></button>` : `<div class="ch-list-row" role="row"></div>`);
    if (r.cls) row.classList.add(r.cls);
    for (const cell of r.cells) {
      const c = el(`<span role="cell"></span>`);
      c.textContent = cell;
      c.title = cell;
      row.append(c);
    }
    if (r.open) row.addEventListener("click", r.open);
    t.append(row);
  }
  if (!rows.length) t.append(el(`<p class="empty">None in these dates</p>`));
  return t;
}
