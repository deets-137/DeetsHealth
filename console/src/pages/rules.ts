// Rules (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. Every view is
// empty until the owner designs it.
import type { Page } from "./page";
import { blankView, subnavPage } from "./subnav";

export function rulesPage(): Page {
  return subnavPage({
    id: "rules",
    title: "Rules",
    lastViewKey: "deets.console.rules",
    lever: "rules",
    // The order is the owner's (2026-11-14): Agents, Flags, Tools.
    views: [blankView("agents", "Agents"), blankView("flags", "Flags"), blankView("tools", "Tools")],
  });
}
