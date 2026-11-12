// Hover hints — the app's one hover-text primitive (DeetsMusic's docs/features/ONBOARDING.md §1).
//
// Every hint would otherwise be the native `title` box: the Windows tooltip, about a
// second of delay, no theme. This module replaces all of them with a themed box that wears
// the menu material.
//
// It is CENTRAL by adoption, not by edit. You still write `title="…"` in markup or
// `el.title = "…"` in code. On boot this module sweeps the page, moves every `title` value
// to `data-hint`, and REMOVES the attribute so Windows never draws its own box. A
// MutationObserver does the same for every element added later and for every later
// `title` write, so state-driven hints keep working untouched.
//
// **The PHI wall (SECURITY.md §4.3).** A hint names a control or a kind of thing, never a
// value from the record. So DeetsHealth has only WRITTEN hints — an author's words on a
// control. DeetsMusic's row hints (the box read a row's own title line and showed it in
// full) are left out on purpose: here a row's title line is record text. A `title` is
// never set from record data.
//
// Settings (Settings › Menus, hints and notices):
//   hoverHints       off = no hover text at all
//   hoverHintDelay   how long the pointer rests first
//
// Accessibility: `aria-label` stays the accessible name. Where an element had a `title`
// and no accessible name of its own (an icon button with no label), adoption promotes the
// hint to `aria-label`, so removing the attribute takes nothing away. The box itself is
// `aria-hidden` and never takes the pointer.
//
// Not shown for a touch pointer (there is no hover to rest), and the rise is dropped under
// reduced motion (styles/hint.css).

import { setting } from "./settings-store";
import "./styles/hint.css";

/** Settings › Hints appear after, in ms. */
const DELAY = { quick: 250, normal: 600, slow: 1100 } as const;
/** Move to another anchor within this long of a hint closing and the next one is instant. */
const WARM_MS = 400;

let box: HTMLElement | null = null;
let titleEl: HTMLElement | null = null;
let anchor: HTMLElement | null = null;
let timer = 0;
let closedAt = 0;
let showing = false;
/** Pressed under the pointer: stays quiet until the pointer moves off it. A press answers
 *  the question the hint was there to answer, so re-showing it reads as a twitch. */
let pressed: HTMLElement | null = null;

function ensureBox(): HTMLElement {
  if (box) return box;
  box = document.createElement("div");
  box.className = "hint";
  box.setAttribute("aria-hidden", "true");
  box.hidden = true;
  titleEl = document.createElement("span");
  titleEl.className = "hint__title";
  box.append(titleEl);
  document.body.appendChild(box);
  return box;
}

// ── Adoption: every `title` becomes a `data-hint`, and the attribute goes ──────────

function adopt(el: Element): void {
  const raw = el.getAttribute("title");
  if (raw === null) return;
  el.removeAttribute("title"); // before anything else: Windows must never get the chance
  const text = raw.trim();
  const host = el as HTMLElement;
  if (!text) {
    delete host.dataset.hint;
  } else {
    host.dataset.hint = text;
    // The hint was the only name this control had — keep it for a screen reader.
    const named = el.hasAttribute("aria-label") || el.hasAttribute("aria-labelledby");
    if (!named && !(el.textContent ?? "").trim()) el.setAttribute("aria-label", text);
  }
  if (anchor === host && showing) fill(host); // a hint that changed under the pointer
}

function sweep(root: ParentNode): void {
  if (root instanceof Element && root.hasAttribute("title")) adopt(root);
  root.querySelectorAll?.("[title]").forEach(adopt);
}

// ── What the pointer is on ────────────────────────────────────────────────────────

/** Walk up from the pointer. The first element with a written hint owns it. */
function resolve(from: Element | null): { el: HTMLElement; text: string } | null {
  for (let el = from as HTMLElement | null; el && el !== document.body; el = el.parentElement) {
    if (el === box) return null;
    const written = el.dataset.hint;
    if (written) return { el, text: written };
  }
  return null;
}

// ── Showing ───────────────────────────────────────────────────────────────────────

/** The floating tier (z 100): pop panels, menus, flyouts, the settings and slot pickers. */
const FLOATING = ".pop, .ctx-menu, .flyout, .set__menu, .slot-picker__menu";

function fill(el: HTMLElement): boolean {
  const found = resolve(el);
  if (!found || found.el !== el) return false;
  const b = ensureBox();
  titleEl!.textContent = found.text;
  // A control inside a floating panel: the hint goes above that tier, or the panel it
  // explains covers it. Everywhere else it stays under menus.
  b.classList.toggle("hint--over", !!el.closest(FLOATING));
  return true;
}

function place(el: HTMLElement, px: number): void {
  const b = ensureBox();
  const r = el.getBoundingClientRect();
  const gap = parseFloat(getComputedStyle(b).getPropertyValue("--hint-gap")) || 8;
  const edge = 6; // never touch the window edge
  const bw = b.offsetWidth;
  const bh = b.offsetHeight;
  // Follow the pointer across a wide control, so the hint sits where you are looking.
  const centre = Number.isFinite(px) ? px : r.left + r.width / 2;
  const left = Math.max(edge, Math.min(Math.round(centre - bw / 2), window.innerWidth - bw - edge));
  let top = Math.round(r.bottom + gap);
  if (top + bh > window.innerHeight - edge) top = Math.round(r.top - gap - bh); // no room below: flip above
  b.style.left = `${left}px`;
  b.style.top = `${Math.max(edge, top)}px`;
}

function show(el: HTMLElement, px: number): void {
  if (!el.isConnected) return;
  const b = ensureBox();
  b.hidden = false;
  if (!fill(el)) {
    b.hidden = true;
    return;
  }
  place(el, px);
  void b.offsetWidth; // the box was display:none — start the fade from 0
  b.classList.add("is-on");
  showing = true;
}

function hide(): void {
  if (!anchor && !showing && !timer) return; // nothing is up — a scroll must cost nothing
  clearTimeout(timer);
  timer = 0;
  anchor = null;
  if (!box) return;
  if (showing) closedAt = performance.now();
  showing = false;
  box.classList.remove("is-on");
  box.hidden = true;
}

function enter(target: Element | null, px: number): void {
  if (!setting("hoverHints")) return;
  const found = resolve(target);
  if (!found) {
    pressed = null;
    if (anchor) hide();
    return;
  }
  if (found.el !== pressed) pressed = null; // the pointer moved on — the press is spent
  if (found.el === anchor || found.el === pressed) return;
  const warm = showing || performance.now() - closedAt < WARM_MS;
  const el = found.el;
  hide();
  anchor = el;
  const wait = warm ? 0 : DELAY[setting("hoverHintDelay")];
  if (wait === 0) show(el, px);
  else timer = window.setTimeout(() => show(el, px), wait);
}

let started = false;

/** Adopt every hint on the page and start listening. Call once, after the DOM exists. */
export function initHints(): void {
  if (started) return;
  started = true;
  sweep(document);

  new MutationObserver((recs) => {
    for (const r of recs) {
      if (r.type === "attributes") {
        if (r.target instanceof Element) adopt(r.target);
      } else {
        r.addedNodes.forEach((n) => {
          if (n instanceof Element) sweep(n);
        });
      }
    }
    if (anchor && !anchor.isConnected) hide(); // the control under the pointer was re-rendered away
  }).observe(document.documentElement, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ["title"],
  });

  document.addEventListener("pointerover", (e) => {
    if (e.pointerType === "touch") return; // nothing rests on a touch screen
    enter(e.target as Element | null, e.clientX);
  });
  // A press is intent; the hint has said what it had to say, and must not come back
  // under the pointer that is still resting on the control it just pressed.
  document.addEventListener(
    "pointerdown",
    (e) => {
      pressed = resolve(e.target as Element | null)?.el ?? null;
      hide();
    },
    true,
  );
  document.addEventListener("pointerleave", hide);
  window.addEventListener("blur", hide);
  window.addEventListener("resize", hide);
  document.addEventListener("scroll", hide, true); // capture: a card's own scroller too
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") hide();
  });
  // Keyboard: the hint follows focus the way it follows the pointer. `:focus-visible`
  // keeps it to keyboard focus — a click already focuses what it pressed.
  document.addEventListener("focusin", (e) => {
    const el = e.target as Element | null;
    if (!el?.matches?.(":focus-visible")) return;
    enter(el, NaN);
  });
  document.addEventListener("focusout", hide);
}
