// Analytics (ANALYTICS.md): one panel with its own nav (subnav.ts). Every view is empty until
// the owner designs it.
import type { Page } from "./page";
import { blankView, subnavPage } from "./subnav";

export function analyticsPage(): Page {
  return subnavPage({
    id: "analytics",
    title: "Analytics",
    lastViewKey: "deets.console.analytics",
    // The order is the owner's (2026-11-12): Home first, Flags last.
    views: [
      blankView("home", "Home"),
      blankView("outcome", "Outcome"),
      blankView("cost", "Cost"),
      blankView("logs", "Logs"),
      blankView("speed", "Speed"),
      blankView("flags", "Flags"),
    ],
  });
}
