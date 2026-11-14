// A tab bar inside a Config view (Config › Agent's groups and Semantic | Flow; Config › System's
// Variables | Tools): the same pill as every other tab bar (subnav.ts › placePill). Moved from
// agent.ts on 2026-11-13 so System uses the same one.
import { el } from "./page";
import { placePill } from "./subnav";

/** A click draws the view again, so the bar is new each time; `from` is the tab chosen before
 * the click. The new pill starts on that tab and slides to the chosen one. */
export function tabBar(label: string, parts: [string, string][], value: string, on: (v: string) => void, from?: string) {
  const b = el(`<div class="tabs prompt-bar" role="tablist"><span class="nav-highlight tab-pill" aria-hidden="true"></span></div>`);
  b.setAttribute("aria-label", label);
  const pill = b.querySelector<HTMLElement>(".tab-pill")!;
  const buttons = parts.map(([v, word]) => {
    const t = el(`<button class="tab" type="button" role="tab"></button>`) as HTMLButtonElement;
    t.dataset.value = v;
    t.textContent = word;
    t.setAttribute("aria-selected", String(v === value));
    t.addEventListener("click", () => v !== value && on(v));
    b.append(t);
    return t;
  });
  const find = (v: string | undefined) => buttons.find((t) => t.dataset.value === v);
  let placed = false;
  return {
    el: b,
    button: find,
    // The first placement: on the old tab at once, then a slide to the new one (or at once
    // when there is no old tab). Each later one (the card's width changed) slides, so it never
    // cuts a slide short.
    place: () => {
      const to = find(value);
      if (!to?.offsetWidth) return; // not on screen yet
      if (placed) return placePill(pill, to, true);
      placed = true;
      const old = from !== undefined && from !== value ? find(from) : undefined;
      if (!old) return placePill(pill, to, false);
      placePill(pill, old, false);
      void pill.offsetWidth; // start the slide from the old tab
      placePill(pill, to, true);
    },
  };
}

/** A way to read a page (a tab, an order), kept in this browser; not a change of settings. */
export const remembered = <T extends string>(key: string, allowed: readonly T[], fallback: T): T => {
  try {
    const v = localStorage.getItem(key);
    return allowed.includes(v as T) ? (v as T) : fallback;
  } catch {
    return fallback;
  }
};

export const remember = (key: string, value: string) => {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Private windows may refuse storage: the page still works, and opens on the default.
  }
};

/** Open another Config view (the nav's own button), so one view can point at another. */
export function openConfigView(id: string) {
  document.querySelector<HTMLButtonElement>(`.subnav .nav-item[data-view="${id}"]`)?.click();
}
