// Settings card — DeetsMusic's settings-card.ts, cut to the shell's rows. The title menu
// keeps the fast switches (Theme / Skin / Surface) and one "Settings…" row that summons
// this card into a slot. Everything else lives here, in one scroll with folding section
// headers. A control lives in exactly one place.
//
// Row kinds: TOGGLE (label + dot), CHOICE (label + one split pill of up to three options;
// more than three becomes a small menu), SPLIT (label + one pill cut into halves — an
// action or a menu), RANGE (label + a slider; a drag previews, the release writes), and
// HEAD (a sub-heading inside a section). Any row may carry `when`: it shows only while that
// holds (the skin-only rows). Rows read the settings store and re-paint on change.
//
// The quick panel (quick-panel.ts) draws sections of this card with this card's own code
// (`mountSettingsParts`), so a row there and a row here can never disagree.
//
// All markup here is fixed app text. Nothing from a record is ever a setting.

import "./styles/settings.css";
import { setting, setSetting, onSettingsChange, DEFAULTS, type Settings } from "./settings-store";
import { currentSkin, onSkinChange, applySkin, defaultSkin, type SkinName } from "./skin";
import { applyTheme, defaultTheme, type ThemeName } from "./theme";
import { withAppearanceTransition } from "./appearance";
import { makeSlider } from "./slider";
import { previewSkin } from "./skin-settings";
import { makeDropdown, type DropdownHandle } from "./dropdown";
import { toast, type ToastHandle } from "./toast";
import * as frames from "./frames";
import { enterRows } from "./pop";
import { takeSettingRequest, onSettingRequest } from "./layout-bus";
import type { CardDef, CardInstance, MountOpts } from "./cards";
import { scrollSnapshot, applyScrollSnapshot } from "./card-memory";
import { SIZE_KEYS, sizeSeen, type SurfaceName } from "./surface";

/** Escape text for markup. Every label and hint here is app text; this keeps it that way. */
export const esc = (s: string): string =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;").replace(/'/g, "&#39;");

/** Fancy Glass's hover hint. The cost is DeetsMusic's bench (its DEBUGGING.md §Fancy Glass). */
const GLASS_FANCY_HINT = "Glass only. A live blur behind the cards, a moving background, and four sliders. Without a graphics card: about 85% fewer frames";
/** The four Glass sliders show under Glass with Fancy Glass on (off holds GLASS_LOCKED). */
const glassSliders = (): boolean => currentSkin() === "glass" && setting("glassFancy");

type BoolKey = { [K in keyof Settings]: Settings[K] extends boolean ? K : never }[keyof Settings];
type NumKey = { [K in keyof Settings]: Settings[K] extends number ? K : never }[keyof Settings];
type Option = { value: string; label: string };

/** More options than this and a choice becomes a small menu instead of a split pill. */
const SPLIT_MAX = 3;

interface ToggleRow {
  kind: "toggle";
  id: string;
  label: string;
  hint?: () => string | undefined;
  get: () => boolean;
  set: (on: boolean) => void;
}
interface ChoiceRow {
  kind: "choice";
  id: string;
  label: string;
  hint?: string;
  key: keyof Settings;
  options: Option[];
  /** Force the dropdown form even at or under SPLIT_MAX. */
  menu?: boolean;
}
/** One half of a split pill. */
type Half =
  | { type: "action"; label: string; hint?: string; run: (el: HTMLElement) => void }
  | { type: "menu"; options: Option[]; get: () => string; set: (v: string) => void };
interface SplitRow {
  kind: "split";
  id: string;
  label: string;
  hint?: () => string | undefined;
  halves: Half[];
  /** Heads the indented `sub` rows under it (its label is underlined); or is one of them. */
  group?: boolean;
  sub?: boolean;
}
/** A sub-heading inside one section's fold. Not a setting: skipped by the header's count. */
interface HeadRow {
  kind: "head";
  id: string;
  label: string;
}
/** A slider. A drag calls `preview` only — a store write would re-render the card under the
 *  pointer — and the release writes the store. */
interface RangeRow {
  kind: "range";
  id: string;
  label: string;
  hint?: string;
  key: NumKey;
  min: number;
  max: number;
  unit: string;
  /** The smallest move, for the drag and the arrow keys. Default 1 (Shift: ten steps). */
  step?: number;
  preview: (v: number) => void;
}
type Row = (ToggleRow | ChoiceRow | SplitRow | RangeRow | HeadRow) & { when?: () => boolean };
const shown = (rows: Row[]): Row[] => rows.filter((r) => !r.when || r.when());
/** The header's count means settings, not sub-headings. */
const settingRows = (rows: Row[]): Row[] => shown(rows).filter((r) => r.kind !== "head");
const headRow = (id: string, label: string): HeadRow => ({ kind: "head", id, label });

interface Section {
  title: string;
  rows: Row[];
  /** Open until the user folds it. Every other section starts collapsed. */
  defaultOpen?: boolean;
}

// Section folds. Only a user's own fold persists, as title → open; a section with no entry
// takes its `defaultOpen`.
const FOLDS_KEY = "deets.settings.folds";
const loadFolds = (): Record<string, boolean> => {
  try {
    return JSON.parse(localStorage.getItem(FOLDS_KEY) ?? "{}") ?? {};
  } catch {
    return {};
  }
};

const storeToggle = (id: string, label: string, key: BoolKey, hint?: () => string | undefined): ToggleRow => ({
  kind: "toggle",
  id,
  label,
  hint,
  get: () => setting(key),
  set: (on) => setSetting(key, on),
});

// ── Settings › Reset: one row per group of store keys ──
interface ResetGroup {
  id: string;
  label: string;
  hint: string;
  keys: (keyof Settings)[];
  /** The theme and skin, which live outside the store (theme.ts, skin.ts). */
  look?: boolean;
}
const RESET_GROUPS: ResetGroup[] = [
  { id: "look", label: "Theme and skin", hint: "The first-launch pair for your Windows light or dark mode", keys: [], look: true },
  { id: "motion", label: "Motion", hint: "The three Animate rows", keys: ["appearanceMotion", "cardSwapMotion", "backgroundMotion"] },
  {
    id: "skinrows", label: "Skin settings", hint: "The Ocean edges and sand, and Fancy Glass and its four sliders",
    keys: ["oceanEdges", "oceanSand", "glassFancy", "glassCanvasGlow", "glassCanvasDim", "glassBacklight", "glassTint"],
  },
  {
    id: "menus", label: "Menus, hints and notices", hint: "Open menus on hover, the two hover-hint rows, and Show notices",
    keys: ["menuMode", "hoverHints", "hoverHintDelay", "toasts"],
  },
  {
    id: "window", label: "Window",
    hint: "Resize changes surface, Keep on top, the two open sizes, Max window when short, and the Growing cards rows",
    keys: ["surfaceAutoFlip", "alwaysOnTop", "sizeMidi", "sizeMax", "maxShortWindow", "cardGrow", "cardGrowOutside", "cardGrowPick", "cardDrill", "cardDrillBring", "cardMemoryDisk"],
  },
];
/** The groups the Look and feel row resets; they also get their own indented rows. */
const LOOK_AND_FEEL = ["look", "motion", "skinrows", "menus"];
interface ResetSnapshot {
  values: Partial<Settings>;
  look?: { theme: ThemeName; skin: SkinName };
}
const snapshotOf = (groups: ResetGroup[]): ResetSnapshot => ({
  values: Object.fromEntries(groups.flatMap((g) => g.keys).map((k) => [k, setting(k)])) as Partial<Settings>,
  look: groups.some((g) => g.look)
    ? { theme: document.documentElement.dataset.theme as ThemeName, skin: currentSkin() }
    : undefined,
});
const defaultsOf = (groups: ResetGroup[]): ResetSnapshot => ({
  values: Object.fromEntries(groups.flatMap((g) => g.keys).map((k) => [k, DEFAULTS[k]])) as Partial<Settings>,
  look: groups.some((g) => g.look) ? { theme: defaultTheme(), skin: defaultSkin() } : undefined,
});
const sameSnapshot = (a: ResetSnapshot, b: ResetSnapshot): boolean =>
  JSON.stringify(a.values) === JSON.stringify(b.values) && a.look?.theme === b.look?.theme && a.look?.skin === b.look?.skin;
/** Write a snapshot: the store keys first, then the theme and skin under the look change. */
function applySnapshot(s: ResetSnapshot): void {
  for (const k of Object.keys(s.values) as (keyof Settings)[]) setSetting(k, s.values[k] as never);
  if (!s.look) return;
  const { theme, skin } = s.look;
  const newSkin = currentSkin() !== skin;
  if (document.documentElement.dataset.theme === theme && !newSkin) return;
  withAppearanceTransition(newSkin ? "skin" : "theme", () => { applyTheme(theme); applySkin(skin); }, {
    skin: newSkin ? skin : undefined,
  });
}

export const settingsCard: CardDef = {
  id: "settings",
  title: "Settings",
  mount: (host, opts) => mountSettings(host, opts),
};

/** One section of the quick panel: a Settings section by title, whole, or only the rows
 *  `rows` names. */
export interface SettingsPart {
  title: string;
  rows?: string[];
}

/** The quick panel's rows: the Settings card's own sections, drawn by the same code into
 *  `host`. No header, no search, no folds, and it never answers a row request (the card does). */
export function mountSettingsParts(host: HTMLElement, parts: SettingsPart[]): CardInstance {
  return mountSettings(host, undefined, parts);
}

function mountSettings(host: HTMLElement, mountOpts?: MountOpts, parts?: SettingsPart[]): CardInstance {
  // The search pill: the header idiom, and the slide-down field under it.
  host.innerHTML = parts
    ? `<div class="set quick__rows"></div>`
    : `<header class="panel__head"><h2 class="panel__title">Settings</h2>` +
      `<button class="panel__action" type="button" data-set-search aria-expanded="false" aria-label="Search" title="Finds a setting by name">` +
      `<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.5"/><path d="M11 11l3 3"/></svg></button></header>` +
      `<div class="lib-searchbar" data-frames="settings-search"><div class="lib-searchbar__inner">` +
      `<label class="lib-search"><svg class="lib-search__icon" viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="4.5"/><path d="M11 11l3 3"/></svg>` +
      `<input class="lib-search__input" data-set-q type="search" placeholder="Search settings…" autocomplete="off" spellcheck="false" /></label>` +
      `</div></div><div class="panel__body set app-scroll"></div>`;
  const body = host.querySelector<HTMLElement>(".set")!;
  // The quick panel has no search: detached stand-ins keep the wiring below unchanged.
  const searchBar = host.querySelector<HTMLElement>(".lib-searchbar") ?? document.createElement("div");
  const searchBtn = host.querySelector<HTMLButtonElement>("[data-set-search]") ?? document.createElement("button");
  const searchInput = host.querySelector<HTMLInputElement>("[data-set-q]") ?? document.createElement("input");
  /** The live query. */
  let query = "";
  let searchOpen = false;

  const flash = (el: HTMLElement, text: string) => {
    const was = el.textContent;
    el.textContent = text;
    window.setTimeout(() => (el.textContent = was), 1200);
  };

  // ── Window › the open sizes: a W × H menu and Set current ──
  const SIZE_PRESETS: Record<SurfaceName, string[]> = {
    midi: ["495x670", "540x780", "600x900"],
    max: ["960x750", "1100x950", "1400x1000"],
  };
  const sizeLabel = (v: string) => v.replace("x", " × ");
  const sizeRow = (id: string, label: string, hint: string, s: SurfaceName): Row => {
    const key = SIZE_KEYS[s];
    return {
      kind: "split", id, label,
      hint: () => hint,
      halves: [
        {
          type: "menu",
          // A size saved by Set current that is not one of the three leads the list.
          get options() {
            const cur = setting(key);
            const presets = SIZE_PRESETS[s];
            return (presets.includes(cur) ? presets : [cur, ...presets]).map((v) => ({ value: v, label: sizeLabel(v) }));
          },
          get: () => setting(key),
          set: (v) => setSetting(key, v),
        },
        {
          type: "action", label: "Set current", hint: "Saves the size this surface has now, or had last",
          run: (el) => {
            const v = sizeSeen(s);
            if (!v) return flash(el, "Open it first");
            setSetting(key, v); // the card re-renders; the menu shows the new size
          },
        },
      ],
    };
  };

  // Settings › Reset: Reset asks first (Confirm / Cancel); a confirmed reset offers Undo.
  // The snapshot is taken at Confirm, so a change made while the question showed is undone too.
  let resetAsk: ToastHandle | null = null;
  const RESET_UNDO_MS = 6000;
  const resetRow = (label: string, hint: string, groups: ResetGroup[], id: string, nest?: "group" | "sub"): Row => ({
    kind: "split", id, label, group: nest === "group", sub: nest === "sub",
    hint: () => hint,
    halves: [
      {
        type: "action", label: "Reset", hint: "Asks first. You can undo it after",
        run: (el) => {
          if (sameSnapshot(snapshotOf(groups), defaultsOf(groups))) return flash(el, "Default");
          resetAsk?.dismiss();
          resetAsk = toast({
            kind: "info",
            sticky: true,
            text: `Reset ${label} to the defaults?`,
            actions: [
              {
                label: "Confirm",
                run: () => {
                  resetAsk = null;
                  const before = snapshotOf(groups);
                  applySnapshot(defaultsOf(groups));
                  toast({
                    kind: "success",
                    text: `${label} reset to the defaults.`,
                    timeout: RESET_UNDO_MS,
                    actions: [{ label: "Undo", run: () => applySnapshot(before) }],
                  });
                },
              },
              { label: "Cancel", run: () => (resetAsk = null) },
            ],
          });
        },
      },
    ],
  });

  const sections: Section[] = [
    // Labels: one short active statement each; the hint (hover) only where a word is missing.
    {
      title: "Window",
      rows: [
        headRow("g-window", "Window"),
        storeToggle("autoflip", "Resize changes surface", "surfaceAutoFlip", () => "On: a window made wide becomes Max, and one made narrow or short becomes Midi. Off: the window resizes inside the current surface, and each one keeps its own floor"),
        storeToggle("aot", "Keep on top", "alwaysOnTop", () => "The window stays above other windows"),
        headRow("g-sizes", "Window sizes"),
        sizeRow("sizemidi", "Midi opens at", "The window size for Midi. Set current saves the size it has now or had last", "midi"),
        sizeRow("sizemax", "Max opens at", "The window size for Max. Set current saves the size it has now or had last", "max"),
        {
          kind: "choice", id: "maxshort", label: "Max window when short", key: "maxShortWindow",
          hint: "Dragged under 745 px tall, Max becomes Midi, or it stops there. Becomes Midi needs Resize changes surface on",
          options: [{ value: "flip", label: "Becomes Midi" }, { value: "floor", label: "Stops at floor" }],
        },
        headRow("g-cards", "Growing cards"),
        storeToggle("cardgrow", "Grow cards from edges", "cardGrow", () => "Click the gap beside a card to open it over its neighbor. Hover a card's title for the button"),
        storeToggle("cardgrowoutside", "Collapse on outside click", "cardGrowOutside", () => "A click outside a grown card collapses it. Pin holds it open"),
        {
          kind: "choice", id: "cardgrowpick", label: "Grown card on a new pick", key: "cardGrowPick",
          hint: "Pick another card in a grown card's title: it keeps the size, or collapses first",
          options: [{ value: "keep", label: "Keeps size" }, { value: "collapse", label: "Collapses" }],
        },
        storeToggle("cardmemorydisk", "Keep card places on restart", "cardMemoryDisk", () => "Opens each card where you left it, also after you restart DeetsHealth"),
      ],
    },
    {
      title: "Motion",
      rows: [
        storeToggle("motion", "Animate look changes", "appearanceMotion", () => "Theme and skin changes play the launch animation. Off: they change at once"),
        storeToggle("cardswap", "Animate card swaps", "cardSwapMotion", () => "Cards move to their new places in the skin's own motion. Off: they change at once"),
        {
          kind: "choice", id: "bgmotion", label: "Animate backgrounds", key: "backgroundMotion",
          hint: "The moving Ocean, Glass, and Cyber backgrounds. Reduced: fewer updates, less CPU. Off: they hold still",
          options: [{ value: "on", label: "On" }, { value: "reduced", label: "Reduced" }, { value: "off", label: "Off" }],
        },
      ],
    },
    {
      // Every row here is gated on one skin, so the whole section is not drawn under a skin
      // that sets nothing (Press and Cyber today).
      title: "Skin settings",
      rows: [
        {
          kind: "choice", id: "oceanedges", label: "Draw card edges", key: "oceanEdges",
          hint: "Ocean only. Sand: the card edges break into grains, like a dark beach",
          options: [{ value: "sand", label: "Sand" }, { value: "soft", label: "Soft" }],
          when: () => currentSkin() === "ocean",
        },
        {
          kind: "range", id: "oceansand", label: "Sand width", key: "oceanSand", min: 0, max: 100, unit: "%",
          hint: "Ocean only. How far the sand reaches into each card",
          preview: (v) => previewSkin("oceanSand", v),
          when: () => currentSkin() === "ocean" && setting("oceanEdges") === "sand",
        },
        // Glass: Fancy Glass first (it shows the sliders), then the layers in paint order, back
        // to front — the background (glow, then its dim), then the card (backlight, then tint).
        {
          ...storeToggle("glassfancy", "Fancy Glass", "glassFancy", () => GLASS_FANCY_HINT),
          when: () => currentSkin() === "glass",
        },
        {
          kind: "range", id: "glasscanvas", label: "Canvas glow", key: "glassCanvasGlow", min: 0, max: 100, unit: "%",
          hint: "Glass only. How brightly the colors glow on the background. The cards do not change",
          preview: (v) => previewSkin("glassCanvasGlow", v),
          when: glassSliders,
        },
        {
          kind: "range", id: "glassdim", label: "Dim canvas", key: "glassCanvasDim", min: 0, max: 100, unit: "%",
          hint: "Glass only. Darkens the space between the cards. The cards stay as bright",
          preview: (v) => previewSkin("glassCanvasDim", v),
          when: glassSliders,
        },
        {
          kind: "range", id: "glassbacklight", label: "Backlight", key: "glassBacklight", min: 0, max: 100, unit: "%",
          hint: "Glass only. A light behind each card, under its tint",
          preview: (v) => previewSkin("glassBacklight", v),
          when: glassSliders,
        },
        {
          kind: "range", id: "glasstint", label: "Tint cards", key: "glassTint", min: 0, max: 100, unit: "%",
          hint: "Glass only. The card color over the backlight. Less tint: more glow",
          preview: (v) => previewSkin("glassTint", v),
          when: glassSliders,
        },
      ],
    },
    {
      title: "Menus, hints and notices",
      rows: [
        {
          kind: "toggle",
          id: "hover",
          label: "Open menus on hover",
          get: () => setting("menuMode") === "hover",
          set: (on) => setSetting("menuMode", on ? "hover" : "click"),
        },
        storeToggle("hints", "Show hover hints", "hoverHints", () => "Rest the pointer on a control and a small box says what it does"),
        {
          kind: "choice", id: "hintdelay", label: "Hints appear after", key: "hoverHintDelay",
          hint: "How long the pointer rests first",
          options: [{ value: "quick", label: "A moment" }, { value: "normal", label: "A pause" }, { value: "slow", label: "A while" }],
          when: () => setting("hoverHints"),
        },
        {
          kind: "choice", id: "toasts", label: "Show notices", key: "toasts",
          hint: "Everything: confirmations too. Failures: only when an action couldn't do what it said",
          options: [{ value: "all", label: "Everything" }, { value: "failures", label: "Failures" }],
        },
      ],
    },
    {
      title: "Reset",
      rows: [
        resetRow("Look and feel", "The theme and skin, and every row of Motion, Skin settings and Menus, hints and notices", RESET_GROUPS.filter((g) => LOOK_AND_FEEL.includes(g.id)), "reset-lookfeel", "group"),
        ...RESET_GROUPS.filter((g) => LOOK_AND_FEEL.includes(g.id)).map((g) => resetRow(g.label, g.hint, [g], `reset-${g.id}`, "sub")),
        ...RESET_GROUPS.filter((g) => !LOOK_AND_FEEL.includes(g.id)).map((g) => resetRow(g.label, g.hint, [g], `reset-${g.id}`)),
        resetRow("Everything", "Every row in this list, in one step", RESET_GROUPS, "reset-all"),
      ],
    },
  ];

  const byId = (id: string): Row | undefined => sections.flatMap((s) => s.rows).find((r) => r.id === id);

  // A choice with more than SPLIT_MAX options is a one-half split: a menu.
  const menuOf = (r: ChoiceRow): Half => ({
    type: "menu",
    options: r.options,
    get: () => String(setting(r.key)),
    set: (v) => setSetting(r.key, v as never),
  });
  const halvesOf = (r: Row): Half[] | undefined =>
    r.kind === "split" ? r.halves : r.kind === "choice" && (r.menu || r.options.length > SPLIT_MAX) ? [menuOf(r)] : undefined;

  const halfHTML = (id: string, h: Half, i: number): string => {
    const at = `data-row="${id}" data-half="${i}"`;
    if (h.type === "action") {
      const tip = h.hint ? ` title="${esc(h.hint)}"` : "";
      return `<button class="set__half" type="button" ${at}${tip}>${esc(h.label)}</button>`;
    }
    const cur = h.options.find((o) => o.value === h.get())?.label ?? "";
    return `<span class="set__menu-wrap"><button class="set__half set__half--menu" type="button" ${at} aria-haspopup="menu" aria-expanded="false">${esc(cur)}<span class="set__caret" aria-hidden="true">▾</span></button></span>`;
  };

  // The row requestSetting opened carries `is-flash`. A negative animation-delay keeps the
  // fade's clock when a later render rebuilds the row.
  let flashId: string | null = null;
  let flashAt = 0;
  const flashOf = (id: string): { cls: string; style: string } =>
    id === flashId
      ? { cls: " is-flash", style: ` style="animation-delay: -${Math.round(performance.now() - flashAt)}ms"` }
      : { cls: "", style: "" };

  // The hint rides the row as a hover hint (`title`) — the labels stand on their own.
  const rowHTML = (r: Row): string => {
    if (r.kind === "head") return `<h4 class="set__sub-head">${esc(r.label)}</h4>`;
    const hint = r.kind === "choice" || r.kind === "range" ? r.hint : r.hint?.();
    const tip = hint ? ` title="${esc(hint)}"` : "";
    const label = `<span class="set__label">${esc(r.label)}</span>`;
    const fx = flashOf(r.id);
    const mark = ` data-set-row="${r.id}"${fx.style}`;
    if (r.kind === "range") {
      const v = setting(r.key);
      const fill = ((v - r.min) / (r.max - r.min)) * 100;
      return (
        `<div class="set__row set__row--range${fx.cls}"${mark}${tip}>${label}` +
        `<div class="set__range scrub" role="slider" tabindex="0" data-range="${r.id}" aria-label="${esc(r.label)}" ` +
        `aria-valuemin="${r.min}" aria-valuemax="${r.max}" aria-valuenow="${v}" style="--slider-fill: ${fill}%">` +
        `<div class="scrub__track"><div class="scrub__fill"></div></div><span class="scrub__handle" aria-hidden="true"></span></div>` +
        `<span class="set__range-val">${v}${esc(r.unit)}</span></div>`
      );
    }
    if (r.kind === "toggle") {
      return `<button class="set__row set__row--toggle${fx.cls}" type="button" role="switch" data-row="${r.id}"${mark} aria-checked="${r.get()}"${tip}>${label}<span class="set__dot" aria-hidden="true"></span></button>`;
    }
    const halves = halvesOf(r);
    if (halves) {
      const sub = r.kind !== "split" ? "" : r.sub ? " set__row--sub" : r.group ? " set__row--group" : "";
      return `<div class="set__row set__row--choice${sub}${fx.cls}"${mark}${tip}>${label}<div class="set__split">${halves.map((h, i) => halfHTML(r.id, h, i)).join("")}</div></div>`;
    }
    const choice = r as ChoiceRow;
    const cur = String(setting(choice.key));
    const opts = choice.options
      .map((o) => `<button class="set__half" type="button" data-row="${choice.id}" data-value="${esc(o.value)}" aria-pressed="${o.value === cur}">${esc(o.label)}</button>`)
      .join("");
    return `<div class="set__row set__row--choice${fx.cls}"${mark}${tip}>${label}<div class="set__split" role="radiogroup" aria-label="${esc(choice.label)}">${opts}</div></div>`;
  };

  // ── menu halves: a small menu portaled to <body> (a card's backdrop-filter under Glass
  //    would trap a fixed panel, and the scrolling body would clip an absolute one).
  //    Rebuilt on every render. ──
  let menus: { dd: DropdownHandle; panel: HTMLElement; obs: MutationObserver }[] = [];
  const dropMenus = () => {
    menus.forEach((m) => { m.dd.destroy(); m.obs.disconnect(); m.panel.remove(); });
    menus = [];
  };
  const wireMenus = () => {
    body.querySelectorAll<HTMLElement>(".set__half--menu").forEach((trigger) => {
      const half = halvesOf(byId(trigger.dataset.row!)!)?.[Number(trigger.dataset.half)];
      if (half?.type !== "menu") return;
      const cur = half.get();
      const panel = document.createElement("div");
      panel.className = "set__menu";
      panel.hidden = true;
      panel.setAttribute("role", "menu");
      panel.innerHTML = half.options
        .map((o) => `<button class="set__menu-row" type="button" role="menuitemradio" data-value="${esc(o.value)}" aria-checked="${o.value === cur}"><span>${esc(o.label)}</span><span class="set__dot" aria-hidden="true"></span></button>`)
        .join("");
      document.body.appendChild(panel);

      // Hang under the trigger, right edges aligned; flip above near the bottom.
      const place = () => {
        if (panel.hidden) return;
        const r = trigger.getBoundingClientRect();
        const vw = document.documentElement.clientWidth;
        const vh = document.documentElement.clientHeight;
        const gap = parseFloat(getComputedStyle(panel).marginTop) || 0;
        const w = panel.offsetWidth;
        const h = panel.offsetHeight;
        panel.style.left = `${Math.max(gap, Math.min(r.right - w, vw - w - gap))}px`;
        panel.style.top = `${r.bottom + gap + h > vh ? Math.max(0, r.top - h - 2 * gap) : r.bottom}px`;
      };
      const obs = new MutationObserver(place);
      obs.observe(panel, { attributes: true, attributeFilter: ["hidden"] });

      const dd = makeDropdown({ root: trigger.parentElement!, trigger, panel });
      panel.addEventListener("click", (e) => {
        const row = (e.target as HTMLElement).closest<HTMLElement>("[data-value]");
        if (!row) return;
        dd.close();
        half.set(row.dataset.value!);
      });
      menus.push({ dd, panel, obs });
    });
  };
  // ── range rows: rebuilt on every render; a drag previews, the release writes the store ──
  const rangeOf = (el: HTMLElement): RangeRow | undefined => {
    const r = byId(el.dataset.range ?? "");
    return r?.kind === "range" ? r : undefined;
  };
  const wireRanges = () => {
    body.querySelectorAll<HTMLElement>(".set__range").forEach((el) => {
      const r = rangeOf(el);
      if (!r) return;
      const out = el.nextElementSibling;
      const step = r.step ?? 1;
      const valueAt = (frac: number) => {
        const raw = r.min + frac * (r.max - r.min);
        return Math.round(Math.round(raw / step) * step * 100) / 100; // a half-step must not drift
      };
      makeSlider(el, {
        axis: "x",
        onDrag: (frac) => {
          const v = valueAt(frac);
          r.preview(v);
          el.setAttribute("aria-valuenow", String(v));
          if (out) out.textContent = `${v}${r.unit}`;
        },
        onCommit: (frac) => setSetting(r.key, valueAt(frac)),
      });
    });
  };

  // The panel is fixed; a scroll of the card would leave it behind — close instead.
  const closeMenus = () => menus.forEach((m) => m.dd.close());
  body.addEventListener("scroll", closeMenus, { passive: true });
  window.addEventListener("resize", closeMenus);

  // ── section folds: the header is a button; a collapsed section renders no rows ──
  const folds = loadFolds();
  const isOpen = (s: Section) => folds[s.title] ?? !!s.defaultOpen;
  /** A section with a match is open for as long as the query holds, and every fold comes
   *  back exactly as it was when the field is cleared. */
  const openNow = (s: Section) => (query ? true : isOpen(s));
  const saveFolds = () => {
    try {
      localStorage.setItem(FOLDS_KEY, JSON.stringify(folds));
    } catch {
      /* storage unavailable — the fold still works for the session */
    }
  };
  const toggleSection = (title: string) => {
    const s = sections.find((x) => x.title === title);
    if (!s) return;
    const was = isOpen(s);
    frames.during("fold", 250, was ? "close" : "open");
    folds[title] = !was;
    saveFolds();
    render();
    // The opened section's rows slide in under its header (src/pop.ts); a close stays instant.
    const drawn = body.querySelector(`.set__section[data-sec="${CSS.escape(title)}"]`);
    if (!was) enterRows([...(drawn?.children ?? [])].slice(1));
  };
  // ── the search ────────────────────────────────────────────────────────────
  // Every space-separated word must hit this row's own text: its label, its section's
  // title, or its hint. A row `when` has gated off is never matched.
  const rowText = (s: Section, r: Row): string => {
    const hint = r.kind === "choice" || r.kind === "range" ? r.hint : r.kind === "head" ? undefined : r.hint?.();
    return `${r.label} ${s.title} ${hint ?? ""}`.toLowerCase();
  };
  const matchRow = (s: Section, r: Row): boolean => {
    const text = rowText(s, r);
    return query.split(/\s+/).every((w) => text.includes(w));
  };
  /** The rows a section draws now: its own, or only the ones the query hits. A sub-heading
   *  is dropped while searching — it names a group that is no longer whole. */
  const rowsOf = (s: Section): Row[] =>
    query ? shown(s.rows).filter((r) => r.kind !== "head" && matchRow(s, r)) : shown(s.rows);

  const headHTML = (s: Section, count = settingRows(s.rows).length): string => {
    const open = openNow(s);
    return (
      `<h3 class="set__head${open ? "" : " is-collapsed"}"><button class="set__fold" type="button" data-fold="${esc(s.title)}" aria-expanded="${open}" title="Opens and closes this section">` +
      `<svg class="lib-shelf__chev" viewBox="0 0 10 6" aria-hidden="true"><path d="M1 1l4 4 4-4" /></svg>` +
      `<span>${esc(s.title)}</span>` +
      `${count ? `<span class="lib-shelf__count">${count}</span>` : ""}</button></h3>`
    );
  };

  const render = () => {
    dropMenus();
    const active = document.activeElement as HTMLElement | null;
    const rangeFocus = active && body.contains(active) ? active.dataset.range : undefined; // a key step keeps focus
    if (parts) {
      // The quick panel: each part under a plain sub-heading, always open, in the order the
      // panel asked for. A part with nothing to show is left out, heading and all.
      body.innerHTML = parts
        .map((p) => {
          const s = sections.find((x) => x.title === p.title);
          if (!s) return "";
          // A sub-heading that repeats the section's name (Window's first) would read twice.
          const rows = shown(s.rows).filter(
            (r) => (!p.rows || p.rows.includes(r.id)) && !(r.kind === "head" && r.label === s.title),
          );
          const inside = rows.map(rowHTML).join("");
          if (!inside) return "";
          return `<section class="set__section" data-sec="${esc(s.title)}"><h4 class="set__sub-head">${esc(s.title)}</h4>${inside}</section>`;
        })
        .join("");
    } else {
      // A section whose every row is gated off shows nothing at all — no empty header. While
      // the field holds text, a section with no matching row goes the same way.
      const drawn = sections.filter((s) => (query ? rowsOf(s).length > 0 : settingRows(s.rows).length > 0));
      body.innerHTML = drawn
        .map((s) => {
          const inside = openNow(s) ? rowsOf(s).map(rowHTML).join("") : "";
          const count = query ? rowsOf(s).length : undefined;
          return `<section class="set__section" data-sec="${esc(s.title)}">${headHTML(s, count)}${inside}</section>`;
        })
        .join("");
    }
    if (rangeFocus) body.querySelector<HTMLElement>(`[data-range="${rangeFocus}"]`)?.focus({ preventScroll: true });
    wireMenus();
    wireRanges();
    markScrollable();
  };
  // The scrollbar thumb fades in only while the rows outgrow the card (settings.css).
  const markScrollable = () => body.classList.toggle("is-scrollable", body.scrollHeight > body.clientHeight + 1);
  const scrollObserver = new ResizeObserver(markScrollable); // a window resize or surface switch
  scrollObserver.observe(body);

  body.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    const fold = t.closest<HTMLElement>("[data-fold]")?.dataset.fold;
    if (fold !== undefined) {
      toggleSection(fold);
      return;
    }
    // A split half (menu halves open through their dropdown, which stops the click).
    const halfEl = t.closest<HTMLElement>("[data-half]");
    if (halfEl?.dataset.row) {
      const half = halvesOf(byId(halfEl.dataset.row)!)?.[Number(halfEl.dataset.half)];
      if (half?.type === "action") half.run(halfEl);
      return;
    }
    const opt = t.closest<HTMLElement>("[data-value]");
    if (opt?.dataset.row && opt.dataset.value !== undefined) {
      const r = byId(opt.dataset.row);
      if (r?.kind === "choice") setSetting(r.key, opt.dataset.value as never);
      return;
    }
    const toggle = t.closest<HTMLElement>(".set__row--toggle");
    if (toggle?.dataset.row) {
      const r = byId(toggle.dataset.row);
      if (r?.kind === "toggle") r.set(!r.get());
    }
  });

  // A focused slider: arrows step 1 (Shift: 10), Home / End jump to the ends.
  body.addEventListener("keydown", (e) => {
    const el = (e.target as HTMLElement).closest<HTMLElement>(".set__range");
    const r = el ? rangeOf(el) : undefined;
    if (!r) return;
    const one = r.step ?? 1;
    const step = e.shiftKey ? one * 10 : one;
    const delta = ({ ArrowRight: step, ArrowUp: step, ArrowLeft: -step, ArrowDown: -step } as Record<string, number>)[e.key];
    const to = e.key === "Home" ? r.min : e.key === "End" ? r.max : delta === undefined ? null : Math.round((setting(r.key) + delta) * 100) / 100;
    if (to === null) return;
    e.preventDefault();
    setSetting(r.key, Math.max(r.min, Math.min(r.max, to)));
  });

  // Re-paint on any change, whoever made it. A full re-render is cheap here.
  const unsubStore = onSettingsChange(render);
  const unsubSkin = onSkinChange(() => render()); // skin-only rows come and go

  // ── requestSetting (layout-bus.ts): unfold the row's section, scroll to the row, highlight it ──
  const focusRow = (id: string) => {
    const s = sections.find((x) => x.rows.some((r) => r.id === id));
    if (!s) return;
    // A request names one row: the search that hid the rest is over.
    if (query) {
      query = "";
      searchInput.value = "";
      searchOpen = false;
      paintSearch();
    }
    if (!isOpen(s)) {
      folds[s.title] = true; // an unfold like the user's own: it persists
      saveFolds();
    }
    flashId = id;
    flashAt = performance.now();
    render();
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    body.querySelector<HTMLElement>(`[data-set-row="${id}"]`)?.scrollIntoView({ block: "center", behavior: reduce ? "auto" : "smooth" });
  };
  const takeRequest = () => {
    if (parts) return; // the quick panel never takes a row request
    const id = takeSettingRequest();
    if (id) focusRow(id);
  };
  const unsubRequest = onSettingRequest(takeRequest); // a request while this card is on-screen

  // ── the search bar ────────────────────────────────────────────────────────
  const paintSearch = () => {
    searchBar.classList.toggle("is-open", searchOpen);
    searchBtn.setAttribute("aria-expanded", String(searchOpen));
    searchBtn.classList.toggle("is-active", !!query);
  };
  const openSearch = () => {
    searchOpen = true;
    paintSearch();
    searchInput.focus();
    searchInput.select();
  };
  const closeSearch = () => {
    searchOpen = false;
    const had = !!query;
    query = "";
    searchInput.value = "";
    paintSearch();
    if (had) render(); // the folds come back exactly as they were
  };
  searchBtn.addEventListener("click", () => (searchOpen ? closeSearch() : openSearch()));
  searchInput.addEventListener("input", () => {
    query = searchInput.value.trim().toLowerCase();
    paintSearch();
    render();
  });
  searchInput.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      closeSearch();
    }
  });

  render();
  takeRequest(); // this card was mounted BY a request
  // Card memory: the scroll place. The open sections have their own store (FOLDS_KEY), and a
  // row request above wins — it scrolls to its own row.
  if (!parts) applyScrollSnapshot(body, mountOpts?.memory);

  return {
    snapshot: () => scrollSnapshot(body),
    destroy() {
      unsubStore();
      unsubSkin();
      unsubRequest();
      dropMenus();
      window.removeEventListener("resize", closeMenus);
      scrollObserver.disconnect();
      host.innerHTML = "";
    },
  };
}
