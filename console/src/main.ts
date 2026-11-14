// The console's boot: the theme, the fonts, then the shell. The styles come from
// index.html's <head> (styles/index.css), so the first paint is already styled.
import { initNav } from "./nav";
import { chatPage } from "./pages/chat";
import { analyticsPage } from "./pages/analytics";
import { teamsPage } from "./pages/teams";
import { configPage } from "./pages/config";
import { rulesPage } from "./pages/rules";
import { securityPage } from "./pages/security";
import { auditPage } from "./pages/audit";
import { settingsPage } from "./pages/settings";
import { initialTheme, initThemeMenu, syncThemeColor } from "./theme";
import { startLiveReload } from "./reload";
import { firstReveal } from "./reveal";
import { health, on, recent } from "./agent/ledger";

// The ledger in the browser's console, in dev only (LOADING.md §5): deetsLedger.health(),
// .recent(), .on(fn). The built console and the demo leave it out.
if (import.meta.env.DEV) Object.assign(window, { deetsLedger: { health, recent, on } });

async function boot() {
  // prepaint.js set the theme; this only corrects an unknown saved value. Nothing is
  // saved until the user picks a theme, so the OS's light or dark keeps leading till then.
  document.documentElement.dataset.theme = initialTheme();
  syncThemeColor();
  // Wait for the bundled serif, so the nav pill is measured in the right face.
  await document.fonts.ready;
  initThemeMenu();
  // The top's order is the owner's (2026-11-14): Chat, Config, Rules, Teams, Analytics.
  // The pages with `bottom` keep this order at the nav's foot: Security, Audit Logs, Settings.
  initNav([
    chatPage(),
    configPage(),
    rulesPage(),
    teamsPage(),
    analyticsPage(),
    securityPage(),
    auditPage(),
    settingsPage(),
  ]);

  // The first-load wave (LOADING.md §6): wait for the page's first data, then one wave.
  void firstReveal();

  // A hidden tab stops the backdrop's drift (app.css › backdrop).
  const idle = () => document.documentElement.toggleAttribute("data-idle", document.hidden);
  document.addEventListener("visibilitychange", idle);
  idle();
  // The demo build only (docs/ops/DEMO.md §3.2); nothing on this PC's dev server.
  startLiveReload();
}

boot().catch((e) => console.error("[boot]", e instanceof Error ? e.name : "error"));
