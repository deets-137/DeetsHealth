// Config (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. Agent is the
// agent's words (§9); System is empty until the owner designs it; Cartesia is §7, Anthropic §8.
import { agentView } from "./agent";
import { anthropicView } from "./anthropic";
import { cartesiaView } from "./cartesia";
import type { Page } from "./page";
import { blankView, subnavPage } from "./subnav";

export function configPage(): Page {
  return subnavPage({
    id: "config",
    title: "Config",
    lastViewKey: "deets.console.config",
    // The order is the owner's (2026-11-13): Agent, then System, then Cartesia, then Anthropic.
    views: [agentView(), blankView("system", "System"), cartesiaView(), anthropicView()],
  });
}
