// Teams (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. People is the
// list of who may sign in (people.ts). Org Chart was removed on 2026-11-22 (the owner: "for now").
import type { Page } from "./page";
import { peopleView } from "./people";
import { subnavPage } from "./subnav";

export function teamsPage(): Page {
  return subnavPage({
    id: "teams",
    title: "Teams",
    lastViewKey: "deets.console.teams",
    lever: "people",
    views: [peopleView()],
  });
}
