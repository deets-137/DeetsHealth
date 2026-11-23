// Analytics (ANALYTICS.md): one panel with its own nav (subnav.ts). Logs is built (§3.1); the
// other views are empty until the owner designs them, in his order: Outcome and Cost, then Speed
// and Flags, Home last.
import { logsView } from "./logs";
import type { Page } from "./page";
import { blankView, subnavPage } from "./subnav";

export function analyticsPage(): Page {
  return subnavPage({
    id: "analytics",
    title: "Analytics",
    lastViewKey: "deets.console.analytics",
    lever: "analytics",
    // The order is the owner's (2026-11-12): Home first, Flags last.
    views: [
      blankView("home", "Home"),
      blankView("outcome", "Outcome"),
      blankView("cost", "Cost"),
      logsView(),
      blankView("speed", "Speed"),
      blankView("flags", "Flags"),
    ],
  });
}
