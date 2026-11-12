// Reading skin tokens from script, for the few timings a script must wait on.

/** A duration token from skin.css, in ms ("120ms" or "0.12s"). 0 when it is not set. */
export function tokenMs(name: string): number {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return 0;
  return v.endsWith("ms") ? n : n * 1000;
}

/** True when the OS asks for less motion: a script skips its waits. */
export const reducedMotion = () => matchMedia("(prefers-reduced-motion: reduce)").matches;
