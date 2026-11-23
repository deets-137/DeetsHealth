// The console's one table (docs/architecture/TABLES.md; the owner, 2026-11-15: "a centralized
// table UI piece like we did for the tabbar"). Every table in the console is built here or wears
// its classes: Config › Hospital, Config › System, Config › Cartesia, Teams › Auth, Teams › Roles.
//
// - Fixed columns: each column names a kind, and its width is a skin token on a <col>, so no
//   column moves while the analyst types (TABLES.md §3.1).
// - One cell box: every control fills its cell from one top edge; a header's word sits over its
//   box's words (§3.2, §3.3). Read shows words in every cell (§3.5).
// - The row tools at the right edge: ↑ ↓ ✕ in the "i"'s square, dimmed when they cannot act
//   (the owner's 1A).
// - The title line: the title and its count, then Add in the tab bar's frame (the owner's 2).
// - The motion: ↑ ↓ slide, ✕ fades then the rows close up, Add slides in (§3.8).
import { reducedMotion, tokenMs, tokenString } from "../tokens";
import { flushWindows, revealItem } from "./list-window";
import { el } from "./page";

/** A column's kind: its width (TABLES.md §3.1). `text` shares what the others leave. */
export type Kind = "text" | "id" | "short" | "choice" | "flag" | "num";
export type Column = { head: string; kind?: Kind };

const WIDTH: Record<Exclude<Kind, "text">, string> = {
  id: "var(--col-id)",
  short: "var(--col-short)",
  choice: "var(--col-choice)",
  flag: "var(--col-flag)",
  num: "var(--col-num)",
};

/** Which tools a table's rows carry; `wide` holds Auth's "Confirm?". No tools: no column. */
export type ToolSet = { move?: boolean; remove?: boolean; wide?: boolean };

export type DataTable = { wrap: HTMLElement; table: HTMLTableElement; body: HTMLTableSectionElement; span: number };

const INFO = `<span class="i-glyph" aria-hidden="true">i</span>`;
// The rail's chevron (chat.ts, panel-grow.ts), turned for ↑; ✕ in the same stroke.
const CHEVRON = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 9l6 6 6-6"/></svg>`;
const CROSS = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17"/></svg>`;

/** A table: its box (which scrolls sideways when narrower than the columns), the widths, the
 * header. The first column is the rows' "i" and has no header; the tools' column is last. */
export function table(label: string, columns: (Column | string)[], tools?: ToolSet): DataTable {
  const wrap = el(`<div class="dt-wrap" data-empty="None yet"><table class="dt"><colgroup></colgroup><thead><tr></tr></thead><tbody></tbody></table></div>`);
  const t = wrap.querySelector("table")!;
  t.setAttribute("aria-label", label);
  const cols = t.querySelector("colgroup")!;
  const head = t.querySelector("thead tr")!;
  const fixed: string[] = [];
  let shared = 0;
  const column = (width: string | null, text: string, cls?: string) => {
    const c = document.createElement("col");
    if (width) {
      c.style.width = width;
      fixed.push(width);
    } else shared++;
    cols.append(c);
    const th = document.createElement("th");
    th.scope = "col";
    th.textContent = text;
    if (cls) th.className = cls;
    head.append(th);
  };
  column("calc(var(--info-size) + var(--space-2))", "", "dt-i");
  for (const c of columns) {
    const col = typeof c === "string" ? { head: c } : c;
    const kind = col.kind ?? "text";
    column(kind === "text" ? null : WIDTH[kind], col.head);
  }
  const n = tools ? (tools.move ? 2 : 0) + (tools.remove ? 1 : 0) : 0;
  if (n) {
    const inner = tools!.wide ? "var(--split-half)" : `${n} * var(--info-size) + ${n - 1} * var(--space-1)`;
    column(`calc(${inner} + 2 * var(--space-2))`, "", "dt-tools-head");
  }
  // Narrower than this, the table scrolls sideways inside its box; the page never does.
  t.style.minWidth = `calc(${[...fixed, `${shared} * var(--col-min)`].join(" + ")})`;
  return { wrap, table: t, body: t.querySelector("tbody")!, span: cols.childElementCount };
}

/** Each draft row's object and its rows on screen, so the motion finds a row after a redraw. */
const shown = new Map<object, HTMLTableRowElement[]>();

const forget = () => {
  for (const [item, trs] of shown) if (!trs[0].isConnected) shown.delete(item);
};

/** The rows whose help is open, by the draft's own object: a row drawn again (a redraw, a long
 * list's window, TABLES.md §6) opens as it was. */
const opened = new WeakSet<object>();

/** Puts an element that is not a row (a facility's card) in the motion, by its draft object, so
 * a filter or an edit slides it as it does a row. */
export function track(item: object, ...els: HTMLElement[]): void {
  if (shown.size > 500) forget();
  shown.set(item, els as HTMLTableRowElement[]);
}

/** A cell's content: a control as it is; words in the box's padding, so they line up with the
 * header and with the boxes' words. */
function cell(c: string | Node): HTMLTableCellElement {
  const td = document.createElement("td");
  const control = c instanceof HTMLElement && (c.classList.contains("cfg-control") || c.classList.contains("dt-cell"));
  if (control) td.append(c);
  else {
    const words = el(`<div class="dt-text"></div>`);
    words.append(c);
    td.append(words);
  }
  return td;
}

/** One row with its "i", then a help row under it that rolls down (config-kit.ts › row).
 * `item`: the draft's own object, for the motion (null for a row no edit moves). `facts`: each a
 * label and its text or links. `tools`: the row's tools (`rowTools`), in a table that has them. */
export function row(
  item: object | null,
  title: string,
  cells: (string | Node)[],
  facts: [string, string | Node][],
  tools?: HTMLElement,
): HTMLTableRowElement[] {
  const tr = el(`<table><tbody><tr class="dt-row"><td class="dt-i"><button class="cfg-info" type="button" aria-expanded="false">${INFO}</button></td></tr></tbody></table>`).querySelector("tr")!;
  for (const c of cells) tr.append(cell(c));
  if (tools) {
    const td = document.createElement("td");
    td.className = "dt-tools-cell";
    td.append(tools);
    tr.append(td);
  }
  const help = el(`<table><tbody><tr class="dt-help"><td></td><td><div class="cfg-help"><div class="cfg-help-text prompt-help"></div></div></td></tr></tbody></table>`).querySelector("tr")!;
  help.lastElementChild!.setAttribute("colspan", String(cells.length + (tools ? 1 : 0)));
  const text = help.querySelector<HTMLElement>(".prompt-help")!;
  for (const [label, value] of facts) {
    const p = el(`<p><span class="prompt-help-key"></span> </p>`);
    p.firstElementChild!.textContent = label;
    p.append(value);
    text.append(p);
  }
  const info = tr.querySelector<HTMLButtonElement>(".cfg-info")!;
  const box = help.querySelector<HTMLElement>(".cfg-help")!;
  // No facts: no "i" (its column stays, so the words line up with other tables').
  if (!facts.length) info.remove();
  info.setAttribute("aria-label", `About ${title}`);
  const setOpen = (open: boolean) => {
    box.classList.toggle("open", open);
    info.setAttribute("aria-expanded", String(open));
  };
  if (item && opened.has(item)) setOpen(true);
  info.addEventListener("click", () => {
    const open = !box.classList.contains("open");
    setOpen(open);
    if (item) {
      if (open) opened.add(item);
      else opened.delete(item);
    }
  });
  if (item) {
    if (shown.size > 500) forget();
    shown.set(item, [tr, help]);
  }
  return [tr, help];
}

/** A heading row across the table: Places' facility before its departments. */
export function groupRow(t: DataTable, ...parts: (string | Node)[]): HTMLTableRowElement {
  const tr = el(`<table><tbody><tr class="dt-group"><td class="dt-i"></td><td></td></tr></tbody></table>`).querySelector("tr")!;
  const td = tr.lastElementChild as HTMLTableCellElement;
  td.colSpan = t.span - 1;
  td.append(...parts);
  return tr;
}

/** One tool's square: ↑, ↓ or ✕. */
export function toolButton(kind: "up" | "down" | "remove", label: string): HTMLButtonElement {
  const b = el(`<button class="dt-tool" type="button">${kind === "remove" ? CROSS : CHEVRON}</button>`) as HTMLButtonElement;
  b.dataset.tool = kind;
  b.setAttribute("aria-label", label);
  return b;
}

/** A row's tools. A key left out: no such tool; a key set to null: the tool shows, dimmed (the
 * first row's ↑, a built-in row's ✕). Disabled in Read with every Config control. */
export function rowTools(what: string, o: { up?: (() => void) | null; down?: (() => void) | null; remove?: (() => void) | null; removeTitle?: string }): HTMLElement {
  const wrap = el(`<div class="cfg-control dt-tools"></div>`);
  const add = (kind: "up" | "down" | "remove", label: string, act: (() => void) | null | undefined) => {
    if (act === undefined) return;
    const b = toolButton(kind, label);
    if (act) b.addEventListener("click", act);
    else b.dataset.off = "";
    wrap.append(b);
  };
  add("up", `Move ${what} up`, o.up);
  add("down", `Move ${what} down`, o.down);
  add("remove", `Remove ${what}`, o.remove);
  if (o.removeTitle) wrap.querySelector<HTMLElement>('[data-tool="remove"]')!.title = o.removeTitle;
  return wrap;
}

/** The tools of a row in a list the page edits in place: ↑ ↓ among its `peers` (every row
 * when none is given), ✕. `done` tells the kit and draws again. */
export function listTools<T extends object>(
  list: T[],
  item: T,
  what: string,
  done: () => void,
  o: { move?: boolean; remove?: boolean | null; peers?: (x: T) => boolean; still?: boolean } = {},
): HTMLElement {
  const peer = o.peers ?? (() => true);
  const near = (step: -1 | 1) => {
    for (let i = list.indexOf(item) + step; i >= 0 && i < list.length; i += step) if (peer(list[i])) return i;
    return -1;
  };
  const swap = (step: -1 | 1) => {
    const to = near(step);
    if (to < 0) return null;
    return () =>
      change(() => {
        const at = list.indexOf(item);
        const other = near(step);
        if (other >= 0) [list[other], list[at]] = [list[at], list[other]];
      }, done);
  };
  const move = o.move ?? true;
  // `still`: ↑ ↓ show dimmed (a search hides some rows: a move would swap with a row not shown).
  return rowTools(what, {
    up: move ? (o.still ? null : swap(-1)) : undefined,
    down: move ? (o.still ? null : swap(1)) : undefined,
    remove: o.remove === false ? undefined : o.remove === null ? null : () => change(() => void list.splice(list.indexOf(item), 1), done, { removed: item }),
  });
}

/** A section's title line: the title and its count at the left, its actions (Add, Import) at
 * the right; then its note. The title is a page heading's size, level with the buttons. */
export function titleLine(title: string, count: number | string | null, note: string | null, ...actions: HTMLElement[]): HTMLElement[] {
  const line = el(`<div class="dt-title-line"><h3 class="dt-title"><span></span><span class="dt-count"></span></h3><div class="dt-actions"></div></div>`);
  const [name, n] = [...line.querySelectorAll<HTMLElement>(".dt-title span")];
  name.textContent = title;
  if (count === null) n.remove();
  else n.textContent = String(count);
  line.querySelector(".dt-actions")!.append(...actions);
  if (!note) return [line];
  const p = el(`<p class="sys-note"></p>`);
  p.textContent = note;
  return [line, p];
}

/** Add, in the tab bar's frame (as Discard | Save). Hidden in Read, its space kept. */
export function addButton(label: string, on: () => void, word = "Add"): HTMLElement {
  const wrap = el(`<div class="cfg-control dt-add"><div class="tabs" role="group"><button class="tab" type="button"></button></div></div>`);
  const b = wrap.querySelector<HTMLButtonElement>("button")!;
  b.textContent = word;
  b.setAttribute("aria-label", label);
  b.addEventListener("click", on);
  return wrap;
}

/** Adds `item` at the foot of `list` (TABLES.md §3.7): it slides in, the table scrolls to it,
 * and the cursor goes to its first box. */
export function addTo<T extends object>(list: T[], item: T, done: () => void, max = Infinity) {
  if (list.length >= max) return;
  change(() => void list.push(item), done, { added: item });
}

/** An editable cell: the words (Read) and the box (Edit) in one place, as on Config › Agent, so
 * nothing moves when Edit fades the box in. One line: Enter does not break it. */
export function field(label: string, value: string, max: number, on: (v: string) => void): HTMLElement {
  const wrap = el(`
    <div class="cfg-control prompt-control">
      <div class="prompt-words"></div>
      <textarea class="cfg-field prompt-text" rows="1" spellcheck="true"></textarea>
    </div>`);
  const words = wrap.querySelector<HTMLElement>(".prompt-words")!;
  const box = wrap.querySelector<HTMLTextAreaElement>("textarea")!;
  box.value = value;
  box.maxLength = max;
  box.setAttribute("aria-label", label);
  words.textContent = value;
  box.addEventListener("keydown", (e) => e.key === "Enter" && e.preventDefault());
  box.addEventListener("input", () => {
    // A pasted line break becomes a space: a value is one line.
    if (/[\r\n]/.test(box.value)) box.value = box.value.replace(/[\r\n]+/g, " ");
    words.textContent = box.value;
    grow(box);
    on(box.value);
  });
  return wrap;
}

/** A text box as tall as its words. */
export function grow(box: HTMLTextAreaElement) {
  if (!box.isConnected) return;
  box.style.height = "auto";
  box.style.height = `${box.scrollHeight}px`;
}

/** Every text box under `root` as tall as its words, in two layouts for all of them: every
 * height set to auto, then every height read, then every height written (TABLES.md §6.1: one
 * box at a time forced one layout of the page per box). */
export function growAll(root: ParentNode | Iterable<ParentNode> = document) {
  const roots = Symbol.iterator in root ? [...(root as Iterable<ParentNode>)] : [root as ParentNode];
  const boxes = roots.flatMap((r) => [...r.querySelectorAll<HTMLTextAreaElement>("textarea.prompt-text")]).filter((b) => b.isConnected);
  for (const b of boxes) b.style.height = "auto";
  const heights = boxes.map((b) => b.scrollHeight);
  boxes.forEach((b, i) => (b.style.height = `${heights[i]}px`));
}

/** A control (a select, Off | On, a number) with its words for Read in the same place
 * (TABLES.md §3.5): Read shows `words()`, Edit the control. */
export function dual(control: HTMLElement, words: () => string): HTMLElement {
  const wrap = el(`<div class="cfg-control dt-dual"><div class="dt-text dt-read"></div><div class="dt-edit"></div></div>`);
  const read = wrap.querySelector<HTMLElement>(".dt-read")!;
  wrap.querySelector(".dt-edit")!.append(control);
  const show = () => (read.textContent = words());
  show();
  // After the control's own listener has changed the draft.
  for (const e of ["input", "change", "click"]) control.addEventListener(e, show);
  return wrap;
}

// ---- the motion (TABLES.md §3.8)

let moving = false;

const ease = () => tokenString("--ease", "ease");

/** One edit to a table's rows, with its motion: each row's place is read before the edit, and
 * after the redraw each row slides from there to its new place (FLIP). A removed row fades out
 * first; an added one rises in. With reduced motion, the edit happens at once. */
export function change(mutate: () => void, redraw: () => void, o: { added?: object; removed?: object } = {}) {
  if (moving) return;
  const motion = !reducedMotion();
  const run = () => {
    const before = motion ? places() : null;
    mutate();
    redraw();
    moving = false;
    // A long list's rows are drawn now, and a new row at its foot is scrolled to (TABLES.md §6).
    flushWindows();
    if (o.added) revealItem(o.added);
    // The boxes take their heights now, not on the next frame, so each row is measured where it
    // will stay (no jump after the slide).
    growAll();
    if (before) slide(before, o.added);
    if (o.added) arrive(o.added, motion);
  };
  const gone = motion && o.removed ? shown.get(o.removed)?.filter((tr) => tr.isConnected) : undefined;
  if (!gone?.length) return run();
  moving = true;
  const fades = gone.map((tr) => tr.animate([{ opacity: 1 }, { opacity: 0 }], { duration: tokenMs("--dur-collapse"), easing: ease(), fill: "forwards" }));
  void fades[0].finished.then(run, run);
}

/** A search's new rows (TABLES.md §6.3, W2): each row that stays slides from where it was to
 * where it is, and each row that comes in fades in. A row that leaves goes at once, so the
 * typing never waits. With reduced motion, the rows change at once. */
export function refilter(redraw: () => void) {
  if (moving) return redraw();
  const before = reducedMotion() ? null : places();
  redraw();
  flushWindows();
  growAll();
  if (!before) return;
  slide(before);
  const duration = tokenMs("--dur-collapse");
  for (const [item, els] of shown) {
    if (before.has(item) || !els[0].isConnected) continue;
    for (const e of els) e.animate([{ opacity: 0 }, { opacity: 1 }], { duration, easing: ease() });
  }
}

/** Where each shown row's lines are now (their tops), by its draft object. */
function places(): Map<object, number[]> {
  forget();
  const out = new Map<object, number[]>();
  for (const [item, trs] of shown) out.set(item, trs.map((tr) => tr.getBoundingClientRect().top));
  return out;
}

function slide(before: Map<object, number[]>, added?: object) {
  const duration = tokenMs("--dur-collapse");
  for (const [item, trs] of shown) {
    const was = before.get(item);
    if (!was || item === added || !trs[0].isConnected) continue;
    trs.forEach((tr, i) => {
      const dy = was[i] - tr.getBoundingClientRect().top;
      if (Math.abs(dy) < 0.5) return;
      tr.animate([{ transform: `translateY(${dy}px)` }, { transform: "none" }], { duration, easing: ease() });
    });
  }
}

/** The new row rises in, the table scrolls to it, and its first box takes the cursor. */
function arrive(item: object, motion: boolean) {
  const trs = shown.get(item);
  if (!trs?.[0].isConnected) return;
  const [tr] = trs;
  if (motion) {
    tr.animate([{ opacity: 0, transform: `translateY(${tokenString("--wave-rise", "6px")})` }, { opacity: 1, transform: "none" }], {
      duration: tokenMs("--dur-collapse"),
      easing: ease(),
    });
  }
  tr.querySelector<HTMLElement>("textarea, input, select")?.focus({ preventScroll: true });
  tr.scrollIntoView({ block: "nearest", behavior: motion ? "smooth" : "auto" });
}
