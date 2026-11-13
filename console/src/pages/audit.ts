// Audit Logs (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. It sits
// at the nav's foot, between Security and Settings. Each view is empty until the owner designs it.
import type { Page } from "./page";
import { blankView, subnavPage } from "./subnav";

export function auditPage(): Page {
  return {
    ...subnavPage({
      id: "audit",
      title: "Audit Logs",
      lastViewKey: "deets.console.audit",
      // The order is the owner's (2026-11-13): Config, Calls, Security.
      views: [
        blankView("config", "Config"),
        blankView("calls", "Calls"),
        blankView("security", "Security"),
      ],
    }),
    bottom: true,
  };
}
