// The lists that open while an analyst types the agent's words (SYSTEM-PAGE.md §3.6; the owner,
// 2026-11-13): `{` opens the variables a section may hold, and " ." (a space, then a period,
// with no space after it) opens the tools it may name, as Epic's SmartPhrases start with a
// period. Typing on filters the list. The arrow keys move, Enter or Tab chooses, Escape closes;
// Ctrl+Space opens the list for the text before the cursor (the variables when nothing is
// typed yet). A variable goes in as `{name}`; a tool as `` `name` ``, the form the check reads.
//
// One list for the page, on the document's body, so no card or scroll box clips it. Its rows
// carry names, meanings and the clinic's own values only: never protected data.
import { el } from "./page";

export type PickItem = {
  /** What a choice puts in the words. */
  insert: string;
  /** What the row shows first, and what typing filters on. */
  name: string;
  /** A line under the name: what it is. */
  detail: string;
  /** A short word at the right: the value now, or where it comes from. */
  note?: string;
  /** A heading above the first row of its group. */
  group?: string;
};

export type PickSource = {
  /** The variables this box may hold. */
  variables: () => PickItem[];
  /** The tools this box may name, or why it may name none. */
  tools: () => PickItem[] | string;
};

type Kind = "variable" | "tool";
type Context = { kind: Kind; start: number; query: string };

let menu: HTMLElement | null = null;
let open: { box: HTMLTextAreaElement; ctx: Context; items: PickItem[]; active: number } | null = null;
let seq = 0;

function theMenu(): HTMLElement {
  if (menu) return menu;
  menu = el(`<div class="menu word-menu" role="listbox" hidden></div>`);
  menu.id = "word-menu";
  // A row keeps the box's focus: the choice is made on click.
  menu.addEventListener("mousedown", (e) => e.preventDefault());
  document.body.append(menu);
  // Anything that moves the box under the list closes it.
  window.addEventListener("scroll", (e) => open && !menu!.contains(e.target as Node) && close(), true);
  window.addEventListener("resize", () => close());
  return menu;
}

/** The list's context at the cursor, or null. `{{` is a plain brace, so it opens nothing. */
function contextAt(box: HTMLTextAreaElement): Context | null {
  if (box.selectionStart !== box.selectionEnd) return null;
  const before = box.value.slice(0, box.selectionStart);
  const v = /(^|[^{])\{([a-z0-9_]*)$/.exec(before);
  if (v) return { kind: "variable", start: before.length - v[2].length - 1, query: v[2] };
  const t = /(^|\s)\.([a-z0-9_]*)$/.exec(before);
  if (t) return { kind: "tool", start: before.length - t[2].length - 1, query: t[2] };
  return null;
}

/** The items for a context: those whose name holds what was typed, the ones that start with it
 * first; the groups keep their order. */
function itemsFor(source: PickSource, ctx: Context): PickItem[] | string {
  const all = ctx.kind === "variable" ? source.variables() : source.tools();
  if (typeof all === "string") return all;
  const q = ctx.query;
  const hit = all.filter((i) => i.name.includes(q));
  const rank = (i: PickItem) => (i.name.startsWith(q) || i.name.startsWith(`{${q}`) ? 0 : 1);
  return hit.sort((a, b) => rank(a) - rank(b));
}

/** The cursor's place on screen: a copy of the box's text up to the cursor, in a hidden box with
 * the same type, padding and width. */
function caretRect(box: HTMLTextAreaElement): DOMRect {
  const style = getComputedStyle(box);
  const mirror = document.createElement("div");
  for (const p of [
    "boxSizing", "width", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft",
    "borderTopWidth", "borderRightWidth", "borderBottomWidth", "borderLeftWidth",
    "fontFamily", "fontSize", "fontWeight", "lineHeight", "letterSpacing", "tabSize",
  ] as const) {
    mirror.style[p] = style[p];
  }
  mirror.style.position = "fixed";
  mirror.style.visibility = "hidden";
  mirror.style.whiteSpace = "pre-wrap";
  mirror.style.overflowWrap = "break-word";
  mirror.style.borderStyle = "solid";
  const r = box.getBoundingClientRect();
  mirror.style.left = `${r.left}px`;
  mirror.style.top = `${r.top}px`;
  mirror.textContent = box.value.slice(0, box.selectionStart);
  const mark = document.createElement("span");
  mark.textContent = "​";
  mirror.append(mark);
  document.body.append(mirror);
  const at = mark.getBoundingClientRect();
  mirror.remove();
  return new DOMRect(at.left, at.top - box.scrollTop, at.width, at.height);
}

function draw() {
  if (!open) return;
  const m = theMenu();
  const { items, active, ctx } = open;
  m.replaceChildren();
  if (!items.length) {
    const why = ctx.kind === "variable" ? "No variable holds that name" : "No tool holds that name";
    m.append(el(`<p class="word-empty"></p>`));
    m.lastElementChild!.textContent = why;
  }
  let group: string | undefined;
  items.forEach((item, i) => {
    if (item.group && item.group !== group) {
      group = item.group;
      const h = el(`<p class="word-group" role="presentation"></p>`);
      h.textContent = group;
      m.append(h);
    }
    const row = el(`<div class="menu-row word-row" role="option"><span class="word-main"><span class="word-name"></span><span class="word-note"></span></span><span class="word-detail"></span></div>`);
    row.id = `word-${seq}-${i}`;
    row.setAttribute("aria-selected", String(i === active));
    row.querySelector(".word-name")!.textContent = item.name;
    const note = row.querySelector<HTMLElement>(".word-note")!;
    note.textContent = item.note ?? "";
    note.hidden = !item.note;
    row.querySelector(".word-detail")!.textContent = item.detail;
    row.addEventListener("mousemove", () => {
      if (open && open.active !== i) {
        open.active = i;
        draw();
      }
    });
    row.addEventListener("click", () => choose(i));
    m.append(row);
  });
  const current = m.querySelector<HTMLElement>('[aria-selected="true"]');
  current?.scrollIntoView({ block: "nearest" });
  open.box.setAttribute("aria-activedescendant", current?.id ?? "");
}

function place() {
  if (!open) return;
  const m = theMenu();
  const at = caretRect(open.box);
  m.hidden = false;
  // Under the cursor's line; above it when there is no room below. Inside the window at the side.
  const h = m.offsetHeight;
  const w = m.offsetWidth;
  const below = at.bottom + h <= window.innerHeight;
  m.style.top = `${below ? at.bottom : Math.max(0, at.top - h)}px`;
  m.style.left = `${Math.max(0, Math.min(at.left, window.innerWidth - w))}px`;
}

function show(box: HTMLTextAreaElement, ctx: Context, found: PickItem[] | string) {
  if (typeof found === "string") {
    // A section where no tool belongs: say so in the list, once, with nothing to choose.
    open = { box, ctx, items: [], active: 0 };
    const m = theMenu();
    m.replaceChildren(el(`<p class="word-empty"></p>`));
    m.firstElementChild!.textContent = found;
  } else {
    const same = open?.box === box && open.ctx.kind === ctx.kind;
    open = { box, ctx, items: found, active: same ? Math.min(open!.active, Math.max(0, found.length - 1)) : 0 };
    seq++;
    draw();
  }
  box.setAttribute("aria-expanded", "true");
  place();
}

function close() {
  if (!open) return;
  open.box.setAttribute("aria-expanded", "false");
  open.box.removeAttribute("aria-activedescendant");
  open = null;
  if (menu) menu.hidden = true;
}

function choose(i: number) {
  if (!open) return;
  const { box, ctx, items } = open;
  const item = items[i];
  if (!item) return;
  close();
  const end = box.selectionStart;
  box.focus();
  box.setSelectionRange(ctx.start, end);
  // insertText keeps the box's own undo; setRangeText where a browser lacks it.
  if (!document.execCommand("insertText", false, item.insert)) {
    box.setRangeText(item.insert, ctx.start, end, "end");
    box.dispatchEvent(new Event("input", { bubbles: true }));
  }
}

/** Give a text box the two lists. */
export function attachPicker(box: HTMLTextAreaElement, source: PickSource) {
  theMenu();
  box.setAttribute("aria-autocomplete", "list");
  box.setAttribute("aria-controls", "word-menu");
  box.setAttribute("aria-expanded", "false");
  const update = () => {
    const ctx = contextAt(box);
    if (!ctx) return close();
    show(box, ctx, itemsFor(source, ctx));
  };
  box.addEventListener("input", update);
  box.addEventListener("blur", () => close());
  box.addEventListener("keydown", (e) => {
    if (e.key === " " && e.ctrlKey) {
      e.preventDefault();
      // Nothing typed yet: the variables; a choice puts in the whole `{name}` at the cursor.
      const ctx = contextAt(box) ?? { kind: "variable" as const, start: box.selectionStart, query: "" };
      return show(box, ctx, itemsFor(source, ctx));
    }
    if (!open || open.box !== box) return;
    const n = open.items.length;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!n) return;
      open.active = (open.active + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      draw();
    } else if (e.key === "Enter" || e.key === "Tab") {
      if (!n) return close();
      e.preventDefault();
      choose(open.active);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  });
  // A click elsewhere in the box moves the cursor out of the context.
  box.addEventListener("click", () => open?.box === box && !contextAt(box) && close());
}
