// Settings (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. It sits at
// the nav's foot, last. Each view is empty until the owner designs it.
import type { Page } from "./page";
import { blankView, subnavPage } from "./subnav";

export function settingsPage(): Page {
  return {
    ...subnavPage({
      id: "settings",
      title: "Settings",
      lastViewKey: "deets.console.settings",
      // The owner's first item (2026-11-13).
      views: [blankView("personalization", "Personalization")],
    }),
    bottom: true,
  };
}
