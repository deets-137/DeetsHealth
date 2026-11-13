// Config (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. Each view is
// empty until the owner designs it.
import type { Page } from "./page";
import { blankView, subnavPage } from "./subnav";

export function configPage(): Page {
  return subnavPage({
    id: "config",
    title: "Config",
    lastViewKey: "deets.console.config",
    // The order is the owner's (2026-11-13): Agent, then System.
    views: [blankView("agent", "Agent"), blankView("system", "System")],
  });
}
