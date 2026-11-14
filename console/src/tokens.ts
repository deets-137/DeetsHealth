// Reading skin tokens from script, for the few timings a script must wait on.

/** A duration token from skin.css, in ms ("120ms" or "0.12s"). 0 when it is not set. */
export function tokenMs(name: string): number {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return 0;
  return v.endsWith("ms") ? n : n * 1000;
}

/** A plain number token from skin.css (no unit). `fallback` when it is not set. */
export function tokenNumber(name: string, fallback: number): number {
  const n = parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name));
  return Number.isFinite(n) ? n : fallback;
}

/** A token's raw value from skin.css (an easing curve, say). `fallback` when it is not set. */
export function tokenString(name: string, fallback: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
}

/** A "this wide or less" media query from a width token. CSS repeats the width in its own
 * @media rules, because a media query cannot read a token. */
const widthQuery = (name: string, fallback: string) => matchMedia(`(max-width: ${tokenString(name, fallback)})`);

/** The compact layout: the nav as dropdowns (CONSOLE.md §11). */
export const compactQuery = () => widthQuery("--compact", "720px");

/** The narrow layout: the rail under the stage (app.css › split-page). */
export const narrowQuery = () => widthQuery("--narrow", "1100px");

/** True when the OS asks for less motion: a script skips its waits. */
export const reducedMotion = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
