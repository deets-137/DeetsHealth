// The compact dropdown (CONSOLE.md §11; the owner's sketch, 2026-11-14). On a narrow screen
// the big nav and each page's own nav become this: a rounded accent button that shows the
// current choice, and the console's glass menu under it with every choice. The nav and the
// dropdown are two views of one list: a pick here runs the same `go` as a click in the nav.
import { el } from "./pages/page";
import { compactQuery } from "./tokens";

export interface PickItem {
  id: string;
  title: string;
  /** Starts a new group under a divider (the nav's foot: Security, Audit Logs, Settings). */
  apart?: boolean;
}

export interface Picker {
  /** The button and its menu, to place in the compact bar. */
  el: HTMLElement;
  /** Marks the current choice. Call it each time the choice changes, by any route. */
  set(id: string): void;
}

/**
 * Builds a dropdown. `label` names the list for a screen reader ("Pages", "Chat mode").
 * `side` is the screen edge the menu hangs from.
 */
export function picker(label: string, items: PickItem[], side: "left" | "right", pick: (id: string) => void): Picker {
  const root = el(`
    <div class="picker">
      <button class="picker-button" type="button" aria-haspopup="menu" aria-expanded="false">
        <span class="picker-name"></span><span class="picker-chev" aria-hidden="true">▾</span>
      </button>
      <div class="menu picker-menu" role="menu" hidden></div>
    </div>`);
  root.dataset.side = side;
  const button = root.querySelector(".picker-button") as HTMLButtonElement;
  const name = root.querySelector(".picker-name") as HTMLElement;
  const menu = root.querySelector(".picker-menu") as HTMLElement;
  menu.setAttribute("aria-label", label);

  const rows = items.map((it) => {
    const b = el(`<button class="menu-row picker-row" type="button" role="menuitemradio" aria-checked="false"><span></span></button>`) as HTMLButtonElement;
    b.dataset.id = it.id;
    b.querySelector("span")!.textContent = it.title;
    return b;
  });
  menu.replaceChildren(
    ...items.flatMap((it, i) => (it.apart && i > 0 ? [el(`<hr class="picker-divider" />`), rows[i]] : [rows[i]])),
  );

  let current = items[0]?.id ?? "";
  const set = (id: string) => {
    current = id;
    name.textContent = items.find((it) => it.id === id)?.title ?? "";
    button.setAttribute("aria-label", `${label}: ${name.textContent}`);
    rows.forEach((r) => r.setAttribute("aria-checked", String(r.dataset.id === id)));
  };

  const open = (focus: boolean) => {
    menu.hidden = false;
    button.setAttribute("aria-expanded", "true");
    if (focus) (rows.find((r) => r.dataset.id === current) ?? rows[0])?.focus();
  };
  const close = (refocus: boolean) => {
    if (menu.hidden) return;
    menu.hidden = true;
    button.setAttribute("aria-expanded", "false");
    if (refocus) button.focus();
  };

  // e.detail is 0 for a keyboard press: then focus moves into the menu.
  button.addEventListener("click", (e) => (menu.hidden ? open(e.detail === 0) : close(false)));
  button.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      open(true);
    }
  });

  menu.addEventListener("click", (e) => {
    const row = (e.target as HTMLElement).closest<HTMLButtonElement>(".picker-row");
    if (!row) return;
    close(e.detail === 0);
    if (row.dataset.id !== current) pick(row.dataset.id!);
  });

  menu.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      e.preventDefault();
      close(true);
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const at = rows.indexOf(document.activeElement as HTMLButtonElement);
      rows[(at + (e.key === "ArrowDown" ? 1 : -1) + rows.length) % rows.length].focus();
    } else if (e.key === "Home" || e.key === "End") {
      e.preventDefault();
      rows[e.key === "Home" ? 0 : rows.length - 1].focus();
    } else if (e.key === "Tab") close(false);
  });

  document.addEventListener("pointerdown", (e) => {
    if (!root.contains(e.target as Node)) close(false);
  });
  window.addEventListener("blur", () => close(false));
  // The window grew past the compact width: the bar is gone, so is its menu.
  compactQuery().addEventListener("change", (e) => !e.matches && close(false));

  set(current);
  return { el: root, set };
}
