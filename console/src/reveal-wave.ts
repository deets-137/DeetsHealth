// The first-load wave's order and delays (docs/architecture/LOADING.md §6), as a pure rule over
// boxes. Ported from DeetsMusic's src/reveal-wave.ts (UX-COVERUPS.md §7.2), the owner's design
// there: the same rule, so the two apps arrive the same way.

export interface Box {
  top: number;
  left: number;
  height: number;
}

/**
 * Each box's delay in ms. Boxes are sorted by top, then left; a box whose top is within half a
 * box of a line's first box is on that line. Delay = (line + column) ÷ (lines + columns − 2) ×
 * `spanMs`, so the wave runs left to right and top to bottom at once, and takes the same time
 * for 8 pieces or 80. A single box: no delay.
 */
export function waveDelays(boxes: Box[], spanMs: number): number[] {
  const idx = boxes.map((_, i) => i).sort((a, b) => boxes[a].top - boxes[b].top || boxes[a].left - boxes[b].left);
  const lines: number[][] = [];
  for (const i of idx) {
    const line = lines[lines.length - 1];
    const head = line ? boxes[line[0]] : null;
    if (head && boxes[i].top - head.top < Math.max(1, head.height) / 2) line.push(i);
    else lines.push([i]);
  }
  lines.forEach((l) => l.sort((a, b) => boxes[a].left - boxes[b].left));
  const cols = Math.max(0, ...lines.map((l) => l.length));
  const steps = lines.length + cols - 2;
  const out = boxes.map(() => 0);
  lines.forEach((l, li) => l.forEach((i, ci) => (out[i] = steps > 0 ? ((li + ci) / steps) * spanMs : 0)));
  return out;
}
