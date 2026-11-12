// The settings store — one typed object under `deets.settings`, the single source for every
// user preference the Settings card edits (DeetsMusic's SETTINGS.md is the design).
//
// Read sites call `setting("x")` at the moment they act (no fan-out needed); things that must
// react live (the window's always-on-top, the dropdown mode) subscribe with `onSettingsChange`.
//
// **The PHI wall (SECURITY.md §4.3).** localStorage is plain text on disk: preferences only.
// Nothing from a record, and no identifier (the chosen real site included), is ever a setting.
// A preference about a record item lives in the sealed store instead.

export interface Settings {
  // ── window ──
  /** Keep the window above other windows. main.ts applies it. */
  alwaysOnTop: boolean;
  /** Every dropdown opens on hover instead of click. */
  menuMode: "click" | "hover";
  /** Free resize flips the surface past its band (surface.ts). Off = each surface keeps its floor. */
  surfaceAutoFlip: boolean;
  /** Open sizes: the window size each surface opens at, "WxH" in logical px. surface.ts
   *  applies them on every open; a resize by hand is not remembered. */
  sizeMidi: string;
  sizeMax: string;
  /** A Max window dragged shorter than its stage column can hold: "flip" turns it into Midi,
   *  "floor" stops the drag at Max's own floor. Only "flip" needs `surfaceAutoFlip` on. */
  maxShortWindow: "flip" | "floor";
  // ── cards ──
  /** Card grow (DeetsMusic's CARD-GROW.md): a card opens over its neighbor from the gap beside
   *  it, the hover button in its header, or its title menu. Off: no zones, no button, no items. */
  cardGrow: boolean;
  /** A click outside a grown card collapses it; Pin holds the card against that. */
  cardGrowOutside: boolean;
  /** A pick in a grown card's title picker: keep the grow on the slot, or collapse first. */
  cardGrowPick: "keep" | "collapse";
  /** A drill (one card opening another at an item): the card it opens takes the place of the
   *  card you are reading and Back returns the chain, or it is summoned into another slot.
   *  No card drills yet; the layout already follows this. No Settings row until one does. */
  cardDrill: "inplace" | "summon";
  /** A drill whose card is ALREADY on screen: bring it to the card you are reading, or open it
   *  where it sits. No Settings row until a card drills. */
  cardDrillBring: boolean;
  /** Card memory (card-memory.ts): a remounted card comes back where it was. On: the places are
   *  also saved, so they survive a restart. View state only, never a record key. */
  cardMemoryDisk: boolean;
  // ── look ──
  /** Theme/skin switches play the launch animation. The OS reduced-motion preference still wins. */
  appearanceMotion: boolean;
  /** A card swap, summon or replace plays the skin's swap motion (card-swap.ts). */
  cardSwapMotion: boolean;
  /** The skins' moving backgrounds (Ocean swell, Glass aurora, Cyber storm): on = 30 fps,
   *  reduced = 15 fps, off = still (the storm hides). src/ambient.ts applies it. */
  backgroundMotion: "on" | "reduced" | "off";
  /** Ocean only: the card edges break into grains of sand, or the plain soft glow. */
  oceanEdges: "sand" | "soft";
  /** Ocean sand only, 0–100: how far the sand band reaches into a card. */
  oceanSand: number;
  /** Glass only: the live frost. On = a real blur behind each card, the aurora drifts, and the
   *  four Glass sliders show. Off = the frost is painted into each card, and the sliders hold
   *  GLASS_LOCKED. `data-glass-fancy` on <html>. */
  glassFancy: boolean;
  /** Glass only, 0–100: how much theme color fills a card. --glass-tint. */
  glassTint: number;
  /** Glass only, 0–100: the light behind each card, under its tint. --glass-backlight. */
  glassBacklight: number;
  /** Glass only, 0–100: how brightly the aurora glows on the background. --glass-canvas. */
  glassCanvasGlow: number;
  /** Glass only, 0–100: darkens the canvas between the cards. --glass-canvas-dim. */
  glassCanvasDim: number;
  // ── menus, hints and notices ──
  /** Hover hints (src/hint.ts): the themed box every `title` becomes. Off = no hover text. */
  hoverHints: boolean;
  /** How long the pointer rests before a hint shows. */
  hoverHintDelay: "quick" | "normal" | "slow";
  /** Which toasts show: failures = warn + error + the one-time notices; all = every kind. */
  toasts: "failures" | "all";
}

const KEY = "deets.settings";

/** The Glass slider values while Fancy Glass is off (DeetsMusic, the owner's call 2026-09-16).
 *  They are also the sliders' defaults, so turning Fancy Glass on starts from the same look. */
export const GLASS_LOCKED = { glassTint: 65, glassBacklight: 85, glassCanvasGlow: 40, glassCanvasDim: 10 } as const;

// Every default below is DeetsMusic's, where the owner chose it, unless a line says otherwise.
export const DEFAULTS: Settings = {
  alwaysOnTop: false,
  menuMode: "click",
  surfaceAutoFlip: true,
  sizeMidi: "495x670", // DeetsMusic's Midi, desk-tested 2026-09-15
  sizeMax: "1100x950", // DeetsMusic's Max; fits a 1920 × 1080 work area (1032)
  maxShortWindow: "flip", // a window that no longer fits a surface becomes the one that does
  cardGrow: true,
  cardGrowOutside: true, // a grow is temporary; Pin covers "keep it"
  cardGrowPick: "keep",
  cardDrill: "inplace", // a drill belongs to the card you are reading
  cardDrillBring: false, // a card already on screen is not worth moving two cards for
  cardMemoryDisk: false, // memory only; an old place after a restart can be stale
  appearanceMotion: true,
  cardSwapMotion: true,
  backgroundMotion: "on",
  oceanEdges: "soft", // Soft is Ocean's true default; Sand is opt-in
  oceanSand: 15,
  glassFancy: false, // the painted frost; the live blur failed on software drawing (DeetsMusic DEBUGGING.md)
  glassTint: 65,
  glassBacklight: 85,
  glassCanvasGlow: 40,
  glassCanvasDim: 10,
  hoverHints: true,
  hoverHintDelay: "normal",
  toasts: "all",
};

function load(): Settings {
  let stored: Partial<Settings> = {};
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) stored = JSON.parse(raw) as Partial<Settings>;
  } catch {
    /* corrupt or unavailable — defaults */
  }
  // Only keys this build knows are kept: a key a later build dropped must not ride along.
  const known: Partial<Settings> = {};
  for (const k of Object.keys(DEFAULTS) as (keyof Settings)[]) {
    if (k in stored) (known as Record<string, unknown>)[k] = stored[k];
  }
  return { ...DEFAULTS, ...known };
}

let state: Settings = load();
const listeners = new Set<(changed: keyof Settings) => void>();

/** Read one setting (always current). */
export function setting<K extends keyof Settings>(key: K): Settings[K] {
  return state[key];
}

/** Write one setting, persist, and notify subscribers. No-op if unchanged. */
export function setSetting<K extends keyof Settings>(key: K, value: Settings[K]): void {
  if (state[key] === value) return;
  state = { ...state, [key]: value };
  try {
    localStorage.setItem(KEY, JSON.stringify(state));
  } catch {
    /* storage disabled — still applies for the session */
  }
  listeners.forEach((cb) => cb(key));
}

/** Subscribe to changes; the callback gets the key that changed. Returns an unsubscribe fn. */
export function onSettingsChange(cb: (changed: keyof Settings) => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}
