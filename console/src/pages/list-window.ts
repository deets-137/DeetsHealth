// Long lists, drawn by the window (docs/architecture/TABLES.md §6; the owner, 2026-11-15:
// windowing, as DeetsMusic's library: ../DeetsMusic/src/collection-window.ts). A list longer
// than WINDOW_MIN draws only the rows near the view, plus one screen above and one below. A
// spacer before and a spacer after hold the height of the rows not drawn, so the scroll bar is
// true. A shorter list draws whole, as before.
//
// - Rows of any height: a row is measured once drawn; a row not drawn yet counts as the median
//   of the ones measured. The heights are kept by the draft's own object, so a redraw of the
//   page starts from them.
// - A drawn row never moves in the page while it stays drawn: rows come and go at the ends, so
//   the row that holds the focus keeps it. That row is never taken out.
// - A grid (the facility cards) windows by its lines: a line is as tall as its tallest card.
// - No value is read or logged here: the rows are the page's own.
import type { DataTable } from "./data-table";

/** Longer than this, a list windows (DeetsMusic's WINDOW_MIN). */
export const WINDOW_MIN = 200;

/** Lines drawn when no row of a list has been measured yet: they give the first heights. */
const FIRST_LINES = 20;

/** One row of a list (or one card of a grid): its draft object and how to draw it. */
export type Unit = { item: object; draw: () => HTMLElement[] };

type Options = {
  /** The heights of this list's rows, by item, kept across the page's redraws (`heightsOf`). */
  heights: WeakMap<object, number>;
  /** After rows are put in the page, before they are measured: the page's mode, the boxes'
   * heights, the marks of a search. */
  drawn?: (nodes: HTMLElement[]) => void;
};

const lists = new Map<string, WeakMap<object, number>>();

/** The kept heights of one list (by a fixed word: "departments", "places-table"). */
export function heightsOf(list: string): WeakMap<object, number> {
  let m = lists.get(list);
  if (!m) lists.set(list, (m = new WeakMap()));
  return m;
}

/** Every windower in the page, for `flushWindows` and `revealItem`. */
const live = new Set<Windower>();
/** Draws every windower whose list is in the page now: call after a redraw, before reading
 * where rows are (the row motion, a reveal), so no frame shows a list without its rows. */
export function flushWindows(): void {
  for (const w of [...live]) w.update();
}

/** Scrolls a windowed list so the row of `item` is drawn and in view. False when no windowed
 * list holds it (a short list draws every row). */
export function revealItem(item: object): boolean {
  for (const w of live) if (w.reveal(item)) return true;
  return false;
}

/** A table's rows, windowed when the list is long; else every row, as before. */
export function fillTable(t: DataTable, units: Unit[], o: Options): void {
  if (units.length <= WINDOW_MIN) {
    const nodes = units.flatMap((u) => u.draw());
    t.body.append(...nodes);
    return;
  }
  const spacer = () => {
    const tr = document.createElement("tr");
    tr.className = "lw-spacer";
    tr.setAttribute("aria-hidden", "true");
    const td = document.createElement("td");
    td.colSpan = t.span;
    tr.append(td);
    return tr;
  };
  t.table.setAttribute("aria-rowcount", String(units.length + 1));
  new Windower(t.body, units, spacer, () => 1, o);
}

/** A grid's cards, windowed by line when the list is long; else every card. */
export function fillGrid(grid: HTMLElement, units: Unit[], o: Options): void {
  if (units.length <= WINDOW_MIN) {
    grid.append(...units.flatMap((u) => u.draw()));
    return;
  }
  const spacer = () => {
    const d = document.createElement("div");
    d.className = "lw-spacer lw-spacer-grid";
    d.setAttribute("aria-hidden", "true");
    return d;
  };
  // The cards to a line: the grid's own columns, as the browser laid them out.
  const columns = () => {
    const cols = getComputedStyle(grid).gridTemplateColumns.trim();
    return cols && cols !== "none" ? Math.max(1, cols.split(/\s+/).length) : 1;
  };
  new Windower(grid, units, spacer, columns, o);
}

/** The nearest ancestor that scrolls up and down. A box that scrolls only sideways (a table's
 * `.dt-wrap`) also reads `overflow-y: auto`, but it is as tall as its rows: it is passed over. */
function scrollerOf(e: HTMLElement): HTMLElement {
  for (let p = e.parentElement; p; p = p.parentElement) {
    const y = getComputedStyle(p).overflowY;
    if ((y === "auto" || y === "scroll") && p.scrollHeight > p.clientHeight + 1) return p;
  }
  return document.scrollingElement as HTMLElement;
}

type Line = { units: Unit[]; nodes: HTMLElement[] | null };

class Windower {
  private lines: Line[] = [];
  private cols = 0;
  private start = 0;
  private end = 0;
  private top: HTMLElement;
  private bottom: HTMLElement;
  private scroller: HTMLElement | null = null;
  private queued = false;
  private gap = 0;
  /** Each drawn node's line, for the resize observer. */
  private lineOf = new Map<Element, number>();
  private watch: ResizeObserver;
  private onScroll = () => this.queue();

  constructor(
    private host: HTMLElement,
    private units: Unit[],
    spacer: () => HTMLElement,
    private columns: () => number,
    private o: Options,
  ) {
    host.classList.add("lw-host");
    this.top = spacer();
    this.bottom = spacer();
    this.chunk(1);
    host.replaceChildren(this.top, this.bottom);
    // Before the first draw, the whole list's height is in the spacer: a redraw that keeps the
    // page's scroll place finds the same height there.
    this.setSpacer(this.bottom, this.span(0, this.lines.length));
    this.setSpacer(this.top, 0);
    this.watch = new ResizeObserver((entries) => {
      let changed = false;
      for (const e of entries) {
        const i = this.lineOf.get(e.target);
        if (i !== undefined && this.measure(i)) changed = true;
        if (e.target === this.host && this.cols && this.columns() !== this.cols) changed = true;
        if (e.target === this.scroller) changed = true; // the view grew or shrank
      }
      if (changed) this.queue();
    });
    this.watch.observe(host);
    live.add(this);
    this.queue();
  }

  /** Cuts the units into lines of `cols`. */
  private chunk(cols: number) {
    this.cols = cols;
    this.lines = [];
    for (let i = 0; i < this.units.length; i += cols) this.lines.push({ units: this.units.slice(i, i + cols), nodes: null });
  }

  /** The median of the measured heights in this list, or 0 when none is measured. */
  private estimate(): number {
    const known: number[] = [];
    for (const u of this.units) {
      const h = this.o.heights.get(u.item);
      if (h !== undefined) known.push(h);
    }
    if (!known.length) return 0;
    known.sort((a, b) => a - b);
    return known[Math.floor(known.length / 2)];
  }

  /** A line's height, its gap after it included. */
  private pitch(i: number, est: number): number {
    let h = 0;
    for (const u of this.lines[i].units) h = Math.max(h, this.o.heights.get(u.item) ?? est);
    return h + this.gap;
  }

  /** The height of lines [a, b). */
  private span(a: number, b: number): number {
    const est = this.estimate();
    let y = 0;
    for (let i = a; i < b; i++) y += this.pitch(i, est);
    return y;
  }

  private setSpacer(s: HTMLElement, px: number) {
    // In a grid, the spacer is a line of its own, and the grid adds one gap after it.
    const h = Math.max(0, px - this.gap);
    s.style.display = px > 0 ? "" : "none";
    const box = (s.firstElementChild as HTMLElement | null) ?? s;
    box.style.height = `${h}px`;
  }

  /** Measures line `i`'s units from its drawn nodes; true when a height changed. */
  private measure(i: number): boolean {
    const line = this.lines[i];
    if (!line?.nodes) return false;
    let changed = false;
    if (this.cols === 1) {
      // A table row and its help row: one unit.
      let h = 0;
      for (const n of line.nodes) h += n.getBoundingClientRect().height;
      const item = line.units[0].item;
      if (Math.abs((this.o.heights.get(item) ?? -1) - h) > 0.5) {
        this.o.heights.set(item, h);
        changed = true;
      }
    } else {
      line.nodes.forEach((n, k) => {
        const item = line.units[k]?.item;
        if (!item) return;
        const h = n.getBoundingClientRect().height;
        if (Math.abs((this.o.heights.get(item) ?? -1) - h) > 0.5) {
          this.o.heights.set(item, h);
          changed = true;
        }
      });
    }
    return changed;
  }

  private queue() {
    if (this.queued) return;
    this.queued = true;
    requestAnimationFrame(() => {
      this.queued = false;
      this.update();
    });
  }

  private dispose() {
    live.delete(this);
    this.watch.disconnect();
    this.scroller?.removeEventListener("scroll", this.onScroll);
  }

  /** The line that holds the focus, or -1. */
  private focusLine(): number {
    const f = document.activeElement;
    if (!f || !this.host.contains(f)) return -1;
    for (let i = this.start; i < this.end; i++) if (this.lines[i].nodes?.some((n) => n.contains(f))) return i;
    return -1;
  }

  /** Draws the lines near the view (on a frame, or at once from `flushWindows`). */
  update(): void {
    if (!this.host.isConnected) {
      // Its view is hidden (kept for later): wait. Its page was drawn again without it: done.
      if (this.scroller?.isConnected && this.scroller !== document.scrollingElement) this.dispose();
      return;
    }
    // Looked up on each update: on a first load the page may not have its height yet, and the
    // box that scrolls is found only once the list overflows it.
    const scroller = scrollerOf(this.host);
    if (scroller !== this.scroller) {
      if (this.scroller) {
        this.scroller.removeEventListener("scroll", this.onScroll);
        this.watch.unobserve(this.scroller);
      }
      this.scroller = scroller;
      scroller.addEventListener("scroll", this.onScroll, { passive: true });
      this.watch.observe(scroller);
    }
    const cols = this.columns();
    if (cols !== this.cols) {
      // The grid's lines changed width: draw again from the same scroll place.
      for (const l of this.lines) l.nodes?.forEach((n) => n.remove());
      this.lineOf.clear();
      this.chunk(cols);
      this.start = this.end = 0;
    }
    this.gap = this.cols > 1 ? parseFloat(getComputedStyle(this.host).rowGap) || 0 : 0;
    const est = this.estimate();
    const view = this.scroller.getBoundingClientRect();
    const box = this.host.getBoundingClientRect();
    let s: number;
    let e: number;
    if (!est) {
      s = 0;
      e = Math.min(this.lines.length, FIRST_LINES);
    } else {
      const from = view.top - box.top - view.height;
      const to = view.bottom - box.top + view.height;
      s = -1;
      e = this.lines.length;
      let y = 0;
      for (let i = 0; i < this.lines.length; i++) {
        const h = this.pitch(i, est);
        if (s < 0 && y + h > from) s = i;
        if (y > to) {
          e = i;
          break;
        }
        y += h;
      }
      if (s < 0) s = Math.max(0, this.lines.length - 1);
      e = Math.max(e, s + 1);
    }
    const f = this.focusLine();
    if (f >= 0) {
      s = Math.min(s, f);
      e = Math.max(e, f + 1);
    }
    if (s === this.start && e === this.end) return;

    // The first line in view, and where it is now: after the draw, it is put back there.
    let anchor = -1;
    let anchorTop = 0;
    for (let i = this.start; i < this.end; i++) {
      const top = this.lines[i].nodes?.[0]?.getBoundingClientRect().top;
      if (top !== undefined && top >= view.top) {
        anchor = i;
        anchorTop = top;
        break;
      }
    }

    const fresh: HTMLElement[] = [];
    const draw = (i: number) => {
      const line = this.lines[i];
      line.nodes = line.units.flatMap((u) => u.draw());
      line.nodes.forEach((n, k) => {
        this.lineOf.set(n, i);
        this.watch.observe(n);
        // Screen readers: each row's place in the whole list (the header is row 1).
        if (this.cols === 1 && k === 0) n.setAttribute("aria-rowindex", String(i + 2));
      });
      fresh.push(...line.nodes);
      return line.nodes;
    };
    const drop = (i: number) => {
      const line = this.lines[i];
      line.nodes?.forEach((n) => {
        this.watch.unobserve(n);
        this.lineOf.delete(n);
        n.remove();
      });
      line.nodes = null;
    };
    const overlap = s < this.end && e > this.start;
    if (!overlap) {
      for (let i = this.start; i < this.end; i++) drop(i);
      const nodes: HTMLElement[] = [];
      for (let i = s; i < e; i++) nodes.push(...draw(i));
      this.bottom.before(...nodes);
    } else {
      for (let i = this.start; i < Math.min(s, this.end); i++) drop(i);
      for (let i = Math.max(e, this.start); i < this.end; i++) drop(i);
      const before: HTMLElement[] = [];
      for (let i = s; i < this.start; i++) before.push(...draw(i));
      this.top.after(...before);
      const after: HTMLElement[] = [];
      for (let i = Math.max(this.end, s); i < e; i++) after.push(...draw(i));
      this.bottom.before(...after);
    }
    this.start = s;
    this.end = e;
    if (fresh.length) this.o.drawn?.(fresh);
    for (let i = s; i < e; i++) this.measure(i);
    this.setSpacer(this.top, this.span(0, s));
    this.setSpacer(this.bottom, this.span(e, this.lines.length));
    // Put the line that was in view back where it was: the measured rows replace the estimates.
    const now = anchor >= 0 ? this.lines[anchor].nodes?.[0]?.getBoundingClientRect().top : undefined;
    if (now !== undefined && Math.abs(now - anchorTop) > 0.5) this.scroller.scrollTop += now - anchorTop;
    // The measured rows may not fill the view: look again on the next frame.
    if (!est || fresh.length) this.queue();
  }

  /** Scrolls so the line of `item` is in view and drawn; false when this list does not hold it. */
  reveal(item: object): boolean {
    if (!this.host.isConnected) return false;
    const i = this.lines.findIndex((l) => l.units.some((u) => u.item === item));
    if (i < 0) return false;
    this.update();
    if (!this.scroller) return false;
    if (i < this.start || i >= this.end) {
      const view = this.scroller.getBoundingClientRect();
      const box = this.host.getBoundingClientRect();
      const y = box.top - view.top + this.span(0, i);
      this.scroller.scrollTop += y - view.height / 2;
      this.update();
    }
    return true;
  }
}
