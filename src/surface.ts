// Surface system (midi | max) — DeetsMusic's surface.ts without Mini (SURFACES-AND-CARDS.md §1).
//
// `data-surface` on <html> is the same lever as `data-theme` / `data-skin`: CSS gates
// layout off the attribute. Surface is a DELIBERATE USER CHOICE with a resize allowance:
// the user picks a surface (the title menu), and free resizing within a surface's band
// stays put — only crossing a band threshold (plus a hysteresis dead-band) flips to the
// other surface.
//
// Open sizes: every open — a surface pick, the launch — sets the window to the surface's
// size from the settings store (sizeMidi / sizeMax). A resize by hand is not remembered.

import { setting, onSettingsChange, DEFAULTS } from "./settings-store";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";

export type SurfaceName = "midi" | "max";

const STORAGE_KEY = "deets.surface";
export type SizeKey = "sizeMidi" | "sizeMax";
/** The setting that holds each surface's open size. */
export const SIZE_KEYS: Record<SurfaceName, SizeKey> = { midi: "sizeMidi", max: "sizeMax" };
const DEFAULT_SURFACE: SurfaceName = "midi";

// ── Band table (DeetsMusic's numbers, the owner's, 2026-09-15 and 2026-09-17) ──
// Width: ≤ MIDI_CEIL → midi · above → max, with HYST px of dead-band each way.
// Height matters for max alone: its stage column holds the Patient card over the Timeline,
// and below MAX_FLOOR_H a short-and-wide window is a Midi, not a Max. Same dead-band.
// These are DeetsMusic's measured numbers for its stage (a square cover over the queue).
// The Patient card will set its own need when it is designed; the numbers move then.
const MIDI_CEIL = 820;
const HYST = 40; // must drag this far past the threshold before the surface flips
const MAX_FLOOR_H = 745;

/** The floor of each surface: the size below which its layout stops reading well. */
export const MIN_SIZES: Record<SurfaceName, { w: number; h: number }> = {
  midi: { w: 495, h: 670 },
  max: { w: 495, h: 750 },
};
// With "Resize changes surface" on and Max's height flip on, a Max window must be draggable
// DOWN to its height band, so it takes Midi's height floor; the flip at MAX_FLOOR_H fires
// before Midi's own 670 is reached. Otherwise nothing flips on height, and Max keeps its 750.
const flipFloor = (k: SurfaceName): { w: number; h: number } =>
  k === "max" && heightFlips() ? { w: MIN_SIZES.max.w, h: Math.min(MIN_SIZES.max.h, MIN_SIZES.midi.h) } : MIN_SIZES[k];

const appWindow = getCurrentWindow();

let active: SurfaceName = DEFAULT_SURFACE;
let minKey = "";
let applyingSize = false; // suppress auto-flip while we programmatically resize
// The size each surface had while it was on screen, this session only (never stored). The
// Settings card's Set current reads it, so a row can be set from the other surface.
const seen: Partial<Record<SurfaceName, string>> = {};

/** True while the screen itself can show a Max window. On a short screen (a 768 px laptop)
 *  the open size is clamped to the work area, and a height flip would make Max unpickable. */
const roomForMax = (): boolean => window.screen.availHeight >= MAX_FLOOR_H + HYST;
/** Does a short Max window flip to Midi, or stop at its own floor? */
const heightFlips = (): boolean => setting("surfaceAutoFlip") && setting("maxShortWindow") === "flip";

/** Where a resize takes the CURRENT surface — flips only past threshold + hysteresis. */
function flipFor(width: number, height: number, cur: SurfaceName): SurfaceName {
  if (cur === "midi") {
    // Both sides must clear the band: a wide but short window stays a Midi.
    if (width > MIDI_CEIL + HYST && (!heightFlips() || height > MAX_FLOOR_H + HYST || !roomForMax())) return "max";
    return "midi";
  }
  if (width < MIDI_CEIL - HYST) return "midi";
  return heightFlips() && height < MAX_FLOOR_H && roomForMax() ? "midi" : "max";
}

function persistChoice(s: SurfaceName): void {
  try {
    localStorage.setItem(STORAGE_KEY, s);
  } catch {
    /* storage disabled — surface still applies for the session */
  }
}

/** "495x670" → `{ w: 495, h: 670 }`; anything else → null. */
export function parseSize(v: string): { w: number; h: number } | null {
  const m = /^(\d{3,4})x(\d{3,4})$/.exec(String(v).trim());
  return m ? { w: Number(m[1]), h: Number(m[2]) } : null;
}

/** A surface's open size: the setting (or its default when the stored text is bad), kept at
 *  or above the surface's minimum and inside the screen's work area. Whole logical px. */
function openSize(k: SurfaceName): { w: number; h: number } {
  const key = SIZE_KEYS[k];
  const { w, h } = parseSize(setting(key)) ?? parseSize(DEFAULTS[key])!;
  const min = MIN_SIZES[k];
  const maxW = Math.max(min.w, window.screen.availWidth);
  const maxH = Math.max(min.h, window.screen.availHeight);
  return {
    w: Math.round(Math.min(Math.max(w, min.w), maxW)),
    h: Math.round(Math.min(Math.max(h, min.h), maxH)),
  };
}

/** The window's size now, as a setting value ("495x670"). `setSize` with a LogicalSize sets
 *  the inner size and the titlebar is ours, so innerWidth/innerHeight are the same unit. */
export function currentSizeText(): string {
  return `${Math.round(window.innerWidth)}x${Math.round(window.innerHeight)}`;
}

/** The size a surface has now, or had while it was last on screen this session; null when
 *  it has not been on screen yet (Set current then has nothing to take). */
export function sizeSeen(s: SurfaceName): string | null {
  if (s === active) return currentSizeText();
  return seen[s] ?? null;
}

/** Set the window's minimum for the active surface (only when it changes). */
async function applyMinSize(): Promise<void> {
  const id = `${active}:${setting("surfaceAutoFlip")}:${setting("maxShortWindow")}`; // the floor follows both
  if (id === minKey) return;
  minKey = id;
  const { w, h } = flipFloor(active);
  try {
    await appWindow.setMinSize(new LogicalSize(w, h));
  } catch (e) {
    console.error("[surface] setMinSize failed", e);
  }
}

/** Resize the OS window to a surface's open size, without triggering a flip. */
async function applySize(s: SurfaceName): Promise<void> {
  const { w, h } = openSize(s);
  applyingSize = true;
  try {
    await applyMinSize(); // first: a smaller size would be refused under the old minimum
    await appWindow.setSize(new LogicalSize(w, h));
  } catch (e) {
    console.error("[surface] setSize failed", e);
  } finally {
    // Let the resize events from our own setSize drain before re-enabling auto-flip.
    window.setTimeout(() => {
      applyingSize = false;
    }, 100);
  }
}

// Reflect the active surface onto the title menu's radio items.
function markActive(s: SurfaceName): void {
  document.querySelectorAll<HTMLElement>("[data-surface-choice]").forEach((el) => {
    el.setAttribute("aria-checked", String(el.dataset.surfaceChoice === s));
  });
}

// Surface-change subscribers (the layout manager recomposes its slots on a flip).
type SurfaceListener = (s: SurfaceName, prev: SurfaceName) => void;
const listeners = new Set<SurfaceListener>();
/** Subscribe to surface flips (deliberate picks AND band crossings). Returns an unsubscribe fn. */
export function onSurfaceChange(cb: SurfaceListener): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

function activate(s: SurfaceName): void {
  const prev = active;
  active = s;
  document.documentElement.dataset.surface = s;
  persistChoice(s);
  markActive(s);
  if (prev !== s) listeners.forEach((cb) => cb(s, prev));
}

/** Deliberate selection (the title menu). Always sets the surface's open size — also when
 *  that surface already shows, so its own row puts a hand-resized window back. The switch
 *  itself is synchronous; only the resize is awaited. */
export async function applySurface(s: SurfaceName): Promise<void> {
  if (s !== active) activate(s);
  await applySize(s);
}

export function currentSurface(): SurfaceName {
  return active;
}

// The launch resize (initSurface): the launch cover (boot-cover.ts) waits for it, so the
// window shows already at its size.
let sized: Promise<void> = Promise.resolve();
export const surfaceSized = (): Promise<void> => sized;

export function initSurface(): void {
  const stored = localStorage.getItem(STORAGE_KEY);
  const saved: SurfaceName = stored === "max" || stored === "midi" ? stored : DEFAULT_SURFACE;
  activate(saved);
  // The window opens at tauri.conf's size, so set the chosen surface's open size.
  sized = applySize(saved);

  // The resize allowance: free within the band, flip past threshold + hysteresis.
  const observer = new ResizeObserver(() => {
    if (applyingSize) return;
    seen[active] = currentSizeText(); // a hand resize: this session only
    // "Resize changes surface" off: the window resizes freely and the surface only changes
    // by a deliberate pick.
    if (setting("surfaceAutoFlip")) {
      const next = flipFor(window.innerWidth, window.innerHeight, active);
      if (next !== active) {
        activate(next);
        void applyMinSize();
      }
    }
  });
  observer.observe(document.documentElement);

  // A new open size for the surface that shows applies at once (a menu pick, a Reset).
  // Set current writes the size the window already has, so that resize is a no-op.
  onSettingsChange((key) => {
    if (key === SIZE_KEYS[active]) void applySize(active);
    // Both of these move the OS floor (flipFloor): the toggle, and the short-window choice.
    if (key === "surfaceAutoFlip" || key === "maxShortWindow") void applyMinSize();
  });
}
