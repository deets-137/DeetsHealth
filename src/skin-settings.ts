// Skin-only settings (Settings › Skin settings; each row shows only under its skin).
// Ocean: `oceanEdges` → `data-ocean-edges` on <html>; skin.css turns the card edges to sand.
//        `oceanSand` (0–100) → --ocean-sand, the sand band's width.
// Glass: `glassBacklight` / `glassTint` / `glassCanvasGlow` (0–100) → --glass-backlight,
//        --glass-tint (a %), --glass-canvas; `glassCanvasDim` → --glass-canvas-dim.
//        `glassFancy` → `data-glass-fancy`; off publishes GLASS_LOCKED instead of the sliders.
// The skin blocks in skin.css read them; other skins ignore them. DeetsMusic's Press record
// player and fancy scrubbers are music parts and did not come across.

import { setting, onSettingsChange, GLASS_LOCKED } from "./settings-store";

type SliderKey = "glassTint" | "glassBacklight" | "glassCanvasGlow" | "glassCanvasDim" | "oceanSand";
const PROPS: Record<SliderKey, [string, string]> = {
  glassTint: ["--glass-tint", "%"],
  glassBacklight: ["--glass-backlight", ""],
  glassCanvasGlow: ["--glass-canvas", ""],
  glassCanvasDim: ["--glass-canvas-dim", ""],
  oceanSand: ["--ocean-sand", ""],
};
const clamp = (v: number) => Math.max(0, Math.min(100, Math.round(Number(v) || 0)));
const isSlider = (k: string): k is SliderKey => k in PROPS;
/** The value a slider publishes: a Glass slider holds its locked value while Fancy Glass is off. */
const sliderValue = (k: SliderKey): number =>
  k !== "oceanSand" && !setting("glassFancy") ? GLASS_LOCKED[k] : setting(k);

/** Show a skin slider value without writing the store (a slider drag). */
export function previewSkin(key: SliderKey, v: number): void {
  const [prop, unit] = PROPS[key];
  document.documentElement.style.setProperty(prop, `${clamp(v)}${unit}`);
}

export function initSkinSettings(): void {
  const applyEdges = () => {
    document.documentElement.dataset.oceanEdges = setting("oceanEdges");
  };
  const applyGlassFancy = () => {
    document.documentElement.dataset.glassFancy = setting("glassFancy") ? "on" : "off";
  };
  // The plain scrubber handle on every slider (the fancy ones are DeetsMusic's).
  document.documentElement.dataset.fancyScrub = "off";
  applyEdges();
  applyGlassFancy();
  const applySliders = () => (Object.keys(PROPS) as SliderKey[]).forEach((k) => previewSkin(k, sliderValue(k)));
  applySliders();
  onSettingsChange((k) => {
    if (k === "oceanEdges") applyEdges();
    if (k === "glassFancy") {
      applyGlassFancy();
      applySliders();
    }
    if (isSlider(k)) previewSkin(k, sliderValue(k));
  });
}
