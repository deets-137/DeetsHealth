// The three themes and the menu that picks them (DeetsMotion's theme.ts, 2026-10-29, less
// its Launch-at-login row). The choice is kept in localStorage: a per-browser convenience,
// never protected data. public/prepaint.js applies it before the first paint; keep its
// key and defaults in step with this file.

import { setHtml } from "./pages/page";

export type Theme = "dark" | "light" | "moonlight";

export const THEMES: { id: Theme; name: string; about: string }[] = [
  { id: "dark", name: "Dark", about: "White on black" },
  { id: "light", name: "Light", about: "Coffee on Solarized cream, with orange" },
  { id: "moonlight", name: "Moonlight", about: "White on navy, with moon blue" },
];

export const THEME_KEY = "deets.console.theme";

const isTheme = (v: unknown): v is Theme => THEMES.some((t) => t.id === v);

export const currentTheme = () => document.documentElement.dataset.theme as Theme;

/** Sets the theme, saves it, and gives the browser's own bar the page's base color. */
export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(THEME_KEY, theme);
  } catch {
    /* storage blocked: the theme holds for this visit only */
  }
  syncThemeColor();
}

/** The installed app's title bar and a phone browser's bar take this color. */
export function syncThemeColor() {
  const meta = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
  if (meta) meta.content = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim();
}

/** The theme prepaint.js chose, checked; Dark if it is not one we know. */
export function initialTheme(): Theme {
  const t = document.documentElement.dataset.theme;
  return isTheme(t) ? t : "dark";
}

const esc = (s: string) => s.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);

/** DeetsMusic's menu: the app name opens it; "Theme ›" flies out the three themes. */
export function initThemeMenu() {
  const button = document.getElementById("app-title") as HTMLButtonElement;
  const menu = document.getElementById("app-menu") as HTMLElement;
  setHtml(
    menu,
    `<button class="menu-row" id="theme-row" type="button" role="menuitem" aria-haspopup="menu" aria-expanded="false">` +
    `<span>Theme</span><span class="menu-chev" aria-hidden="true">›</span></button>` +
    `<div class="flyout" id="theme-flyout" role="menu" aria-label="Theme" hidden>` +
    THEMES.map(
      (t) =>
        `<button class="menu-row" role="menuitemradio" data-theme-id="${t.id}" type="button" title="${esc(t.about)}">` +
        `<span class="swatch swatch--${t.id}" aria-hidden="true"></span><span class="menu-name">${esc(t.name)}</span>` +
        `<span class="menu-dot" aria-hidden="true"></span></button>`,
    ).join("") +
    `</div>`,
  );
  const row = menu.querySelector("#theme-row") as HTMLButtonElement;
  const flyout = menu.querySelector("#theme-flyout") as HTMLElement;
  const items = [...flyout.querySelectorAll<HTMLButtonElement>("[data-theme-id]")];

  const sync = () => items.forEach((i) => i.setAttribute("aria-checked", String(i.dataset.themeId === currentTheme())));

  const openFlyout = (focus: boolean) => {
    sync();
    flyout.hidden = false;
    row.setAttribute("aria-expanded", "true");
    if (focus) (items.find((i) => i.getAttribute("aria-checked") === "true") ?? items[0]).focus();
  };
  const closeFlyout = () => {
    flyout.hidden = true;
    row.setAttribute("aria-expanded", "false");
  };
  const open = (focus: boolean) => {
    menu.hidden = false;
    button.setAttribute("aria-expanded", "true");
    if (focus) row.focus();
  };
  const close = (refocus: boolean) => {
    if (menu.hidden) return;
    closeFlyout();
    menu.hidden = true;
    button.setAttribute("aria-expanded", "false");
    if (refocus) button.focus();
  };

  // e.detail is 0 for a keyboard press: then focus moves into the menu.
  button.addEventListener("click", (e) => (menu.hidden ? open(e.detail === 0) : close(false)));
  button.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      open(true);
    }
  });

  // The flyout follows the pointer, as in DeetsMusic; the keyboard opens it with → or Enter.
  row.addEventListener("pointerenter", () => openFlyout(false));
  row.addEventListener("click", (e) => openFlyout(e.detail === 0));
  menu.addEventListener("pointerleave", closeFlyout);

  flyout.addEventListener("click", (e) => {
    const item = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-theme-id]");
    if (!item) return;
    applyTheme(item.dataset.themeId as Theme);
    close(e.detail === 0);
  });

  menu.addEventListener("keydown", (e) => {
    const inFlyout = flyout.contains(document.activeElement);
    if (e.key === "Escape") {
      e.preventDefault();
      if (inFlyout) {
        closeFlyout();
        row.focus();
      } else close(true);
    } else if (e.key === "ArrowRight" && document.activeElement === row) {
      e.preventDefault();
      openFlyout(true);
    } else if (e.key === "ArrowLeft" && inFlyout) {
      e.preventDefault();
      closeFlyout();
      row.focus();
    } else if ((e.key === "ArrowDown" || e.key === "ArrowUp") && inFlyout) {
      e.preventDefault();
      const at = items.indexOf(document.activeElement as HTMLButtonElement);
      items[(at + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length].focus();
    } else if (e.key === "Tab") close(false);
  });

  document.addEventListener("pointerdown", (e) => {
    if (!menu.hidden && !menu.contains(e.target as Node) && !button.contains(e.target as Node)) close(false);
  });
  window.addEventListener("blur", () => close(false));
}
