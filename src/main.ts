import { getCurrentWindow } from "@tauri-apps/api/window";
import { applyTheme, initTheme, type ThemeName } from "./theme";
import { applySkin, initSkin, type SkinName } from "./skin";
import { applySurface, initSurface, surfaceSized, type SurfaceName } from "./surface";
import { initStorm } from "./storm";
import { initAmbient } from "./ambient";
import { initBrowserDefaults } from "./browser-defaults";
import { initHints } from "./hint";
import { setting, onSettingsChange } from "./settings-store";
import { requestCard } from "./layout-bus";
import { runBootCover } from "./boot-cover";
import { initLayout } from "./layout";
import { initSkinSettings } from "./skin-settings";
import { makeDropdown, setDropdownMode } from "./dropdown";
import { withAppearanceTransition } from "./appearance";
import { initQuickPanel } from "./quick-panel";
import { initAppErrors } from "./app-errors";
import * as frames from "./frames";

// Wire the custom window buttons to the OS window. The titlebar drag is handled
// declaratively by data-tauri-drag-region on .drag-region in index.html.
const appWindow = getCurrentWindow();

window.addEventListener("DOMContentLoaded", () => {
  initTheme();
  initSkin();
  initSkinSettings(); // before the first paint, so a card never flashes the default look
  initSurface();
  initStorm(); // storm-layer position re-roll; inert unless the skin opts in
  initAmbient(); // pause the skins' decorative loops while the window is minimized
  // File drops belong to the page (tauri.conf.json `dragDropEnabled: false`). A drop no
  // element took must not navigate the webview to the file.
  window.addEventListener("dragover", (e) => {
    if (e.defaultPrevented) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "none";
  });
  window.addEventListener("drop", (e) => e.preventDefault());
  initBrowserDefaults(); // no native drag or right-click menu; text fields get our own
  initHints(); // every `title` becomes the themed hover box

  // ── Menu mode (click vs hover) — one setting drives every dropdown. The dropdown
  //    primitive owns the cross-instance fan-out (setDropdownMode). ──
  setDropdownMode(setting("menuMode")); // seed the primitive's global mode before any dropdown is made

  // ── Window buttons ──────────────────────────────────────────
  document.getElementById("tl-min")?.addEventListener("click", () => appWindow.minimize());
  document.getElementById("tl-max")?.addEventListener("click", () => appWindow.toggleMaximize());
  document.getElementById("tl-close")?.addEventListener("click", () => appWindow.close());

  // ── Settings that act on the window / the dropdown primitive: applied here on launch and
  //    whenever the Settings card changes them. ──
  let onTop: boolean | null = null;
  const applyAlwaysOnTop = () => {
    const on = setting("alwaysOnTop");
    if (on === onTop) return;
    onTop = on;
    appWindow.setAlwaysOnTop(on).catch((e) => console.error("[aot]", e));
  };
  applyAlwaysOnTop();
  onSettingsChange((k) => {
    if (k === "alwaysOnTop") applyAlwaysOnTop();
    if (k === "menuMode") setDropdownMode(setting("menuMode"));
  });

  // ── The title menu (mode follows the Hover-Menu setting; submenus stay hover) ──
  const settingsRoot = document.querySelector<HTMLElement>(".settings");
  const trigger = document.getElementById("settings-trigger");
  const menu = document.getElementById("settings-menu");
  if (settingsRoot && trigger && menu) {
    menu.classList.add("pop"); // arrives and leaves like the other panels
    menu.dataset.frames = "settings";
    const settingsDropdown = makeDropdown({ root: settingsRoot, trigger, panel: menu });
    const close = () => settingsDropdown.close();

    // Settings… → summon the Settings card into a slot.
    document.getElementById("settings-open")?.addEventListener("click", () => {
      requestCard("settings");
      close();
    });

    // Theme choices — the launch animation (appearance.ts); the menu closes under the
    // opaque cover, so the rise never shows it half-closed.
    document.querySelectorAll<HTMLElement>("[data-theme-choice]").forEach((el) => {
      el.addEventListener("click", () => {
        withAppearanceTransition("theme", () => applyTheme(el.dataset.themeChoice as ThemeName), { after: close });
      });
    });

    // Skin choices (same pattern) — the incoming skin's own entrance.
    document.querySelectorAll<HTMLElement>("[data-skin-choice]").forEach((el) => {
      el.addEventListener("click", () => {
        const skin = el.dataset.skinChoice as SkinName;
        withAppearanceTransition("skin", () => applySkin(skin), { skin, after: close });
      });
    });

    // Surface choices.
    document.querySelectorAll<HTMLElement>("[data-surface-choice]").forEach((el) => {
      el.addEventListener("click", () => {
        void applySurface(el.dataset.surfaceChoice as SurfaceName);
        close();
      });
    });
  }

  // ── The cog → the quick panel ──
  initQuickPanel();

  frames.init(); // dev-only frame telemetry (frames.ts)

  // ── Cards + layout: mount the Patient card (anchored top) and the content slots from the
  //    persisted assignment, and wire each slot's title picker. ──
  initLayout();
  // Failures from the Rust side become toasts (app-errors.ts). After the layout, so the
  // toast host has its place.
  void initAppErrors();
  // The launch cover (boot-cover.ts): the window shows once the window is at its size, then
  // the cards rise into place. Nothing to restore yet, so `restored` settles at once.
  runBootCover(Promise.resolve(), [surfaceSized()]);
});
