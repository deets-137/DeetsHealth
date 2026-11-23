// Analytics (ANALYTICS.md): one panel with its own nav (subnav.ts). Home, Outcome and Cost for an
// executive; Logs, Speed and Flags for an analyst (the owner, 2026-11-22). Logs is §3.1; the
// views on the sums are §3.2 (analytics-views.ts).
import { costView, flagsView, homeView, outcomeView, speedView } from "./analytics-views";
import { logsView } from "./logs";
import type { Page } from "./page";
import { subnavPage } from "./subnav";

export function analyticsPage(): Page {
  return subnavPage({
    id: "analytics",
    title: "Analytics",
    lastViewKey: "deets.console.analytics",
    lever: "analytics",
    // The order is the owner's (2026-11-12): Home first, Flags last.
    views: [homeView(), outcomeView(), costView(), logsView(), speedView(), flagsView()],
  });
}
