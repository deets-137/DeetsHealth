// The grow handle (CHAT.md §15.2; the owner's sketch and fork B, 2026-11-14): a strip at a
// rail panel's bottom edge, part of the panel. While the rail sits under the stage (1100 px or
// less), a tap shows the panel's whole list; a second tap, on the same strip now at the list's
// foot, brings it back to its usual height. Nothing is saved: each page load starts collapsed.
// On a wide screen the strip is hidden and the panel is as before (chat.css › the grow handle).
import { el } from "./page";
import { reducedMotion, tokenMs, tokenString } from "../tokens";

const CHEVRON = `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 9l6 6 6-6"/></svg>`;
const WORD = { closed: "Show the whole list", open: "Show less" };

/** Adds the handle to `panel`. */
export function growHandle(panel: HTMLElement): void {
  const handle = el(`<button class="grow-handle" type="button" aria-expanded="false">${CHEVRON}</button>`) as HTMLButtonElement;
  handle.setAttribute("aria-label", WORD.closed);
  handle.title = WORD.closed;
  panel.append(handle);

  // The strip shows only when it does something (the owner, 2026-11-14): collapsed, when the
  // shown tab's list is cut off; grown, when the panel is taller than its usual height. A grown
  // panel whose tab now fits keeps its state, so a longer tab shows whole again.
  let checking = 0;
  const check = () => {
    checking = 0;
    const body = panel.querySelector<HTMLElement>(".tab-body:not([hidden])");
    const useful = panel.hasAttribute("data-grown")
      ? panel.getBoundingClientRect().height > parseFloat(getComputedStyle(panel).minHeight || "0") + 1
      : !!body && body.scrollHeight > body.clientHeight + 1;
    handle.hidden = !useful;
  };
  const soon = () => {
    if (!checking) checking = requestAnimationFrame(check);
  };
  // A list that loads, a tab that changes, a row that opens, or a new window size.
  new MutationObserver(soon).observe(panel, { childList: true, subtree: true, attributes: true, attributeFilter: ["hidden", "class"] });
  new ResizeObserver(soon).observe(panel);
  panel.addEventListener("transitionend", soon); // a row that glides open ends taller

  let running: Animation | null = null;
  handle.addEventListener("click", () => {
    const open = !panel.hasAttribute("data-grown");
    const from = panel.getBoundingClientRect().height;
    running?.cancel();
    panel.toggleAttribute("data-grown", open);
    handle.setAttribute("aria-expanded", String(open));
    handle.setAttribute("aria-label", open ? WORD.open : WORD.closed);
    handle.title = open ? WORD.open : WORD.closed;
    // After a collapse the strip moved up with the panel's foot: bring it back into view.
    const settle = () => {
      if (!open) handle.scrollIntoView({ block: "nearest", behavior: reducedMotion() ? "auto" : "smooth" });
    };
    if (reducedMotion()) return settle();
    // The panel lays out once at its new height; its height glides there from the old one. The
    // animation holds nothing after it ends, so at rest the panel's height is its own.
    const to = panel.getBoundingClientRect().height;
    if (from === to) return settle();
    panel.classList.add("growing");
    const a = panel.animate([{ height: `${from}px` }, { height: `${to}px` }], {
      duration: tokenMs("--dur-collapse"),
      easing: tokenString("--ease", "ease"),
    });
    running = a;
    // A quick second tap cancels this one: only the newest animation clears the state.
    a.onfinish = a.oncancel = () => {
      if (running !== a) return;
      panel.classList.remove("growing");
      running = null;
    };
    a.finished.then(settle, () => {});
  });
}
