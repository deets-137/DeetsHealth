// Teams (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. Org Chart is
// empty until the owner designs it; People is the list of who may sign in (people.ts).
import type { Page } from "./page";
import { peopleView } from "./people";
import { blankView, subnavPage } from "./subnav";

export function teamsPage(): Page {
  return subnavPage({
    id: "teams",
    title: "Teams",
    lastViewKey: "deets.console.teams",
    lever: "people",
    // The order is the owner's (2026-11-13): Org Chart, then People.
    views: [blankView("org-chart", "Org Chart"), peopleView()],
  });
}
