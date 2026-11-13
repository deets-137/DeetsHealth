// Teams (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. Each view is
// empty until the owner designs it.
import type { Page } from "./page";
import { blankView, subnavPage } from "./subnav";

export function teamsPage(): Page {
  return subnavPage({
    id: "teams",
    title: "Teams",
    lastViewKey: "deets.console.teams",
    // The order is the owner's (2026-11-13): Org Chart, then People.
    views: [blankView("org-chart", "Org Chart"), blankView("people", "People")],
  });
}
