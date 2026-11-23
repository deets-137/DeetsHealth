// Config (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. Agent is the
// agent's words (§9); System is the variables and the tools (§10, SYSTEM-PAGE.md); Hospital is
// the hospital's lists (HOSPITAL-CONFIG.md); Voice is §7 (VOICE-VENDORS.md), Anthropic §8.
import { agentView } from "./agent";
import { anthropicView } from "./anthropic";
import { voiceView } from "./voice";
import { hospitalView } from "./hospital";
import type { Page } from "./page";
import { subnavPage } from "./subnav";
import { systemView } from "./system";

export function configPage(): Page {
  return subnavPage({
    id: "config",
    title: "Config",
    lastViewKey: "deets.console.config",
    // The order is the owner's (2026-11-13): Agent, then System, then Cartesia (now Voice), then Anthropic.
    // Hospital sits beside System (HOSPITAL-CONFIG.md), until the owner places it.
    // Each view's lever (ACCESS-ROLES.md §8.3): the words Agent, the vendors Vendors, the
    // hospital's lists Rules.
    views: [
      { ...agentView(), lever: "agent" },
      { ...systemView(), lever: "agent" },
      { ...hospitalView(), lever: "rules" },
      { ...voiceView(), lever: "vendors" },
      { ...anthropicView(), lever: "vendors" },
    ],
  });
}
