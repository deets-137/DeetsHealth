// Rules (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. One place for
// every rule (the owner's 2A, 2026-11-22). Booking is the Booking rules table, moved here from
// Config › Hospital (hospital.ts › bookingRulesView); the other views are empty until the owner
// designs them.
import { bookingRulesView } from "./hospital";
import type { Page } from "./page";
import { blankView, subnavPage } from "./subnav";

export function rulesPage(): Page {
  return subnavPage({
    id: "rules",
    title: "Rules",
    lastViewKey: "deets.console.rules",
    lever: "rules",
    // The owner's order (2026-11-14): Agents, Flags, Tools; Booking first, the one built view.
    views: [bookingRulesView(), blankView("agents", "Agents"), blankView("flags", "Flags"), blankView("tools", "Tools")],
  });
}
