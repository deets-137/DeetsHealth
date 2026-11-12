// The quick panel (DeetsMusic's docs/features/QUICK-SETTINGS.md): the cog's panel. One row of
// squares: the app's groups (Look, Help, Window), then a right cluster (Reset, and the cog =
// All settings). A press on a square makes the panel taller and shows its settings; a press on
// the same square shuts them again, a press on another swaps them.
//
// The rows are the Settings card's own sections, drawn by the card's own code
// (`mountSettingsParts`), so a row here and the same row in Settings can never disagree.
// Motion: the panel's `.pop` arrival, its parts through `enterRows` (src/pop.ts), and the
// height moves to the new content.
//
// DeetsMusic's service squares (Apple Music, Discord), its other groups, the Tips line and
// the New badges are left out: the shell has none of those yet.

import { makeDropdown, type DropdownHandle } from "./dropdown";
import { enterRows } from "./pop";
import { mountSettingsParts, type SettingsPart } from "./settings-card";
import { requestCard } from "./layout-bus";
import { expandCard, isGrownCard, collapseGrow, onGrowChange } from "./card-grow";
import { tokenMs } from "./boot-cover";
import type { CardInstance } from "./cards";
import * as diag from "./diag";

type Part = "look" | "help" | "window" | "reset";

/** What each square shows. */
const PARTS: Record<Part, SettingsPart[]> = {
  look: [{ title: "Skin settings" }, { title: "Motion" }],
  help: [{ title: "Menus, hints and notices" }],
  window: [{ title: "Window" }],
  reset: [{ title: "Reset" }],
};

/** The app's own icons, from DeetsMusic (the owner's sketches). Stroked in the ink; the
 *  Window icon's back pane in --subtext. */
const WINDOW_GLYPH =
  `<svg class="quick__glyph" viewBox="0 0 24 24" aria-hidden="true">` +
  `<path class="quick__glyph-back" d="M18 5.5h2.5v15h-15V18"/>` +
  `<rect x="3" y="3" width="15" height="15"/><rect x="6" y="6" width="9" height="9"/><path d="M10.5 6v9M6 10.5h9"/></svg>`;
const BRUSH_GLYPH =
  `<svg class="quick__glyph" viewBox="0 0 24 24" aria-hidden="true">` +
  `<path d="M11.2 12.8C13.6 9.4 17 5.6 20.2 3.4c.4-.3.8.1.5.5-2.2 3.2-6 6.6-9.4 9z"/>` +
  `<path d="M10.4 12.3l1.3 1.3c.2 1.9-.6 3.6-2.3 4.6-1.6.9-3.6 1-5.4 1.8.6-1.8.8-3.7 1.8-5.3 1-1.7 2.8-2.5 4.6-2.4z"/></svg>`;
/** Help (menus, hints and notices): a helipad — an H in a ring. */
const HELIPAD_GLYPH =
  `<svg class="quick__glyph" viewBox="0 0 24 24" aria-hidden="true">` +
  `<circle cx="12" cy="12" r="9"/><path d="M9 7.5v9M15 7.5v9M9 12h6"/></svg>`;
/** Reset: a ring with two arrowheads chasing each other. */
const RESET_GLYPH =
  `<svg class="quick__glyph" viewBox="0 0 24 24" aria-hidden="true">` +
  `<circle cx="12" cy="12" r="7"/>` +
  `<path class="quick__glyph-fill" d="M10.6 2.6L15 5l-4.4 2.4zM13.4 16.6L9 19l4.4 2.4z"/></svg>`;

const reduced = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

let dropdown: DropdownHandle | null = null;

/** Open the panel from elsewhere (the Compass, when it comes). */
export function openQuickPanel(): void {
  dropdown?.open();
}

/** Wire the cog's panel. Call once, after the DOM exists. */
export function initQuickPanel(): void {
  const root = document.getElementById("quick");
  const cog = document.getElementById("cog-open");
  const panel = document.getElementById("quick-panel");
  const body = document.getElementById("quick-body");
  const rowsHost = document.getElementById("quick-rows");
  const all = document.getElementById("quick-all");
  if (!root || !cog || !panel || !body || !rowsHost || !all) return;

  panel.querySelector("[data-quick='look']")!.innerHTML = BRUSH_GLYPH;
  panel.querySelector("[data-quick='help']")!.innerHTML = HELIPAD_GLYPH;
  panel.querySelector("[data-quick='window']")!.innerHTML = WINDOW_GLYPH;
  panel.querySelector("[data-quick='reset']")!.innerHTML = RESET_GLYPH;
  all.innerHTML = cog.innerHTML; // the panel's cog is the title bar's own glyph

  // ── the cog keeps its turn: every press adds --cog-step, open or shut. A press on the
  //    panel's cog turns the title bar one too: the panel shuts as it acts. Six teeth, so
  //    each step ends on an identical tooth. ──
  let cogTurns = 0;
  const turn = () => {
    cogTurns += 1;
    cog.style.setProperty("--cog-angle", `calc(var(--cog-step) * ${cogTurns})`);
  };
  cog.addEventListener("click", turn);

  // ── the height follows the new content instead of jumping ──
  let grow: Animation | null = null;
  const animateHeight = (from: number) => {
    grow?.cancel();
    grow = null;
    panel.classList.remove("is-growing");
    if (panel.hidden || !from || reduced()) return;
    const to = panel.offsetHeight;
    if (Math.abs(to - from) < 1) return;
    panel.classList.add("is-growing");
    const a = panel.animate([{ height: `${from}px` }, { height: `${to}px` }], {
      duration: tokenMs("--pop-grow"),
      easing: getComputedStyle(panel).getPropertyValue("--pop-ease").trim() || "ease",
    });
    grow = a;
    a.onfinish = () => {
      if (grow !== a) return;
      grow = null;
      panel.classList.remove("is-growing");
    };
  };

  // ── the open part: one at a time, remounted on each press ──
  let open: Part | null = null;
  let rows: CardInstance | null = null;
  const show = (part: Part | null) => {
    const from = panel.offsetHeight;
    rows?.destroy();
    rows = null;
    open = part;
    panel.querySelectorAll<HTMLElement>("[data-quick]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.quick === part)));
    body.hidden = !part;
    if (part) {
      rows = mountSettingsParts(rowsHost, PARTS[part]);
      enterRows(rowsHost.querySelectorAll(".set__section > *"));
      diag.log("quick:part", { part });
    }
    animateHeight(from);
  };

  // ── The panel's cog: All settings, or Collapse while Settings is grown — then it wears the
  //    pressed fill, as a square whose rows are open does. ──
  const paintAll = () => {
    const grown = isGrownCard("settings");
    all.setAttribute("aria-pressed", String(grown));
    all.setAttribute("aria-label", grown ? "Collapse settings" : "All settings");
    all.title = grown ? "Collapse settings: puts Settings back in its place" : "All settings: opens Settings at full size";
  };
  all.addEventListener("click", () => {
    turn();
    dropdown?.close();
    if (isGrownCard("settings")) {
      void collapseGrow("cog");
      return;
    }
    requestCard("settings");
    void expandCard("settings", "cog");
  });
  onGrowChange(paintAll);

  panel.addEventListener("click", (e) => {
    const logo = (e.target as HTMLElement).closest<HTMLElement>("[data-quick]");
    if (!logo) return;
    const part = logo.dataset.quick as Part;
    show(open === part ? null : part);
  });

  dropdown = makeDropdown({
    root,
    trigger: cog,
    panel,
    // A row's menu (a split half's list) and the right-click menu are portaled to <body>:
    // a press in them is a press in this panel.
    alsoInside: () => [...document.querySelectorAll(".set__menu:not([hidden]), .ctx-menu")],
    onOpen: () => {
      paintAll();
      keepRightInWindow(root, panel);
      enterRows([...panel.querySelectorAll(".quick__bar > *")]);
      // A part left open last time comes back with the panel, drawn fresh.
      if (open) show(open);
    },
  });
  window.addEventListener("resize", () => { if (!panel.hidden) keepRightInWindow(root, panel); });
}

/** The panel hangs from the cog's left edge (`left: 0`); in a narrow window it would pass
 *  the right edge, so move it left by the difference, less --panel-edge-gap. */
function keepRightInWindow(anchor: HTMLElement, panel: HTMLElement): void {
  panel.style.left = "";
  const gap = parseFloat(getComputedStyle(panel).getPropertyValue("--panel-edge-gap")) || 0;
  const right = anchor.getBoundingClientRect().left + panel.offsetWidth;
  const over = right - (document.documentElement.clientWidth - gap);
  if (over > 0) panel.style.left = `${-over}px`;
}
