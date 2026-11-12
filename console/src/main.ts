// The console's boot: the theme, the fonts, then the shell. The styles come from
// index.html's <head> (styles/index.css), so the first paint is already styled.
import { initNav } from "./nav";
import { chatPage } from "./pages/chat";
import { settingsPage } from "./pages/settings";
import { initialTheme, initThemeMenu, syncThemeColor } from "./theme";

async function boot() {
  // prepaint.js set the theme; this only corrects an unknown saved value. Nothing is
  // saved until the user picks a theme, so the OS's light or dark keeps leading till then.
  document.documentElement.dataset.theme = initialTheme();
  syncThemeColor();
  // Wait for the bundled serif, so the nav pill is measured in the right face.
  await document.fonts.ready;
  initThemeMenu();
  initNav([chatPage(), settingsPage()]);

  // A hidden tab stops the backdrop's drift (app.css › backdrop).
  const idle = () => document.documentElement.toggleAttribute("data-idle", document.hidden);
  document.addEventListener("visibilitychange", idle);
  idle();
}

boot().catch((e) => console.error("[boot]", e instanceof Error ? e.name : "error"));
