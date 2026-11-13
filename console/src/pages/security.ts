// Security (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. It sits at
// the nav's foot, above Audit Logs. Each view is empty until the owner designs it.
import type { Page } from "./page";
import { blankView, subnavPage } from "./subnav";

export function securityPage(): Page {
  return {
    ...subnavPage({
      id: "security",
      title: "Security",
      lastViewKey: "deets.console.security",
      // The order is the owner's (2026-11-13): Auth, then Roles.
      views: [blankView("auth", "Auth"), blankView("roles", "Roles")],
    }),
    bottom: true,
  };
}
