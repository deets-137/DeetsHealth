// The first-load wave (docs/architecture/LOADING.md §6; the owner's fork A, 2026-11-14). The
// model is DeetsMusic's reveal (src/reveal.ts, UX-COVERUPS.md §7).
//
// prepaint.js marks <html data-reveal="hold"> before the first paint, and app.css keeps the app
// hidden under it, so no piece ever shows early. main.ts builds the shell and the page; every
// store whose first read starts meanwhile joins the hold (store.ts). The hold ends when each of
// those reads has answered (or failed), at --reveal-cap at the latest, or at once on a click or
// a key. Then one wave runs from the app title at the top left to the last piece at the bottom
// right, over --reveal-span. Only pieces on the first screen move; the rest just show. CSS lifts
// the hold at --reveal-safety whatever happens, so an error never leaves a blank screen.
// Reduced motion: the same hold, then the screen at once. Only on a load or a reload: a page
// switch keeps its fade.

import { closeHold, heldReads } from "./agent/store";
import { waveDelays } from "./reveal-wave";
import { reducedMotion, tokenMs } from "./tokens";

/** The panels: a piece inside one never starts before it. */
const PANELS = ".nav, .glass";
/** The pieces inside the panels (and the app title above them). The outermost match wins: a
 * button inside a row moves with its row. */
const PIECES = [
  ".app-title",
  ".nav-highlight",
  ".nav-item",
  "h1",
  "h2",
  ".tabs",
  ".cfg-top > *",
  ".cfg-row",
  ".sys-row",
  ".word-row",
  ".people-card",
  ".call-row",
  "tr",
  ".empty",
  "button",
  "input",
  "select",
  "textarea",
  "label.cfg-button",
].join(", ");

const html = document.documentElement;

/** True while the first load holds or waves: other entry motion waits (Chat's own wave). */
export const revealing = (): boolean => !!html.dataset.reveal;

const frame = (): Promise<void> => new Promise((r) => requestAnimationFrame(() => r()));
const sleep = (ms: number): Promise<"cap"> => new Promise((r) => window.setTimeout(() => r("cap"), ms));

/** Waits for the held reads (the list may grow while it waits: a page that reads after its
 * first answer), the cap, or a click or a key. */
async function hold(): Promise<void> {
  const cap = sleep(tokenMs("--reveal-cap") || 1500);
  let press: () => void = () => {};
  const pressed = new Promise<"press">((r) => (press = () => r("press")));
  const opts = { capture: true, once: true } as const;
  window.addEventListener("pointerdown", press, opts);
  window.addEventListener("keydown", press, opts);
  try {
    for (;;) {
      const reads = [...heldReads()];
      const got = await Promise.race([Promise.all(reads).then(() => "ready" as const), cap, pressed]);
      if (got !== "ready") return;
      // Drawing an answer may start another first read (Config's credits): wait for it too.
      await frame();
      if (heldReads().length === reads.length) return;
    }
  } finally {
    closeHold();
    window.removeEventListener("pointerdown", press, opts);
    window.removeEventListener("keydown", press, opts);
  }
}

/** On the first screen: inside the window and inside its scroll box. */
function seen(el: HTMLElement, r: DOMRect): boolean {
  if (r.width === 0 && r.height === 0) return false;
  if (r.bottom <= 0 || r.top >= innerHeight || r.right <= 0 || r.left >= innerWidth) return false;
  const box = el.parentElement?.closest<HTMLElement>(".own-scrollbars");
  if (!box) return true;
  const b = box.getBoundingClientRect();
  return r.bottom > b.top && r.top < b.bottom && r.right > b.left && r.left < b.right;
}

/** The wave: each piece starts on its own turn, not all at once with a delay (DeetsMusic found
 * that the second way draws every layer in the first frame). Until its turn a piece waits at
 * opacity 0 (`reveal-pre`), which costs no drawing. */
function wave(): void {
  const app = document.querySelector<HTMLElement>(".app");
  if (!app) return;
  const panels = [...app.querySelectorAll<HTMLElement>(PANELS)];
  const pieces = [...app.querySelectorAll<HTMLElement>(PIECES)].filter((p) => !p.parentElement?.closest(PIECES));
  const all = [...new Set([...panels, ...pieces])]
    .map((el) => ({ el, r: el.getBoundingClientRect() }))
    .filter(({ el, r }) => seen(el, r));
  // A line is about one control tall: a tall panel must not pull the pieces in its top half
  // onto its own line.
  const line = parseFloat(getComputedStyle(html).getPropertyValue("--control-height")) || 44;
  const delays = waveDelays(
    all.map(({ r }) => ({ top: r.top, left: r.left, height: Math.min(r.height, line) })),
    tokenMs("--reveal-span") || 500,
  );
  const at = new Map(all.map(({ el }, i) => [el, delays[i]]));
  // A piece never starts before its panel.
  for (const { el } of all) {
    const panel = el.parentElement?.closest<HTMLElement>(PANELS);
    const p = panel ? at.get(panel) : undefined;
    if (p !== undefined) at.set(el, Math.max(at.get(el)!, p));
  }
  const queue = [...at].map(([el, ms]) => ({ el, ms })).sort((a, b) => a.ms - b.ms);
  for (const { el } of queue) el.classList.add("reveal-pre");
  html.dataset.reveal = "wave";

  const start = (el: HTMLElement) => {
    el.classList.remove("reveal-pre");
    el.classList.add("reveal-in");
    const end = (e: AnimationEvent) => {
      if (e.target !== el) return;
      el.classList.remove("reveal-in");
      el.removeEventListener("animationend", end);
    };
    el.addEventListener("animationend", end);
  };
  const t0 = performance.now();
  let next = 0;
  const step = () => {
    const now = performance.now() - t0;
    while (next < queue.length && queue[next].ms <= now) start(queue[next++].el);
    if (next < queue.length) requestAnimationFrame(step);
  };
  step();
  // A frame loop the browser stops (a hidden tab) must not leave pieces at opacity 0.
  const span = tokenMs("--reveal-span") || 500;
  window.setTimeout(() => {
    while (next < queue.length) start(queue[next++].el);
  }, span + 250);
  window.setTimeout(() => delete html.dataset.reveal, span + (tokenMs("--dur-wave-in") || 360) + 300);
}

/** The first load: hold, then the wave (main.ts calls it once the shell and page are built). */
export async function firstReveal(): Promise<void> {
  if (html.dataset.reveal !== "hold") return;
  await frame(); // the page's first reads have started
  await hold();
  await frame(); // the answers are drawn: the boxes are the ones that will show
  // Entry motion that ran under the hold (a page's fade-in) ends now, so it does not mix
  // with the wave. Endless motion (the backdrop's drift) runs on.
  for (const a of document.getAnimations()) {
    if (a.effect?.getTiming().iterations !== Infinity) a.finish();
  }
  if (reducedMotion()) {
    delete html.dataset.reveal;
    return;
  }
  wave();
}
