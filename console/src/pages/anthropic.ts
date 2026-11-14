// Config › Anthropic (CONSOLE.md §8; the owner, 2026-11-13: "a new config subpage for
// Anthropic", option A: the page chooses the model). Every lever of the language model a test
// call runs with: the model, effort, thinking, the reply's token caps, the prompt cache's life,
// and the budget. The frame, the versions and the rows are config-kit.ts, the same as Config ›
// Cartesia. The key itself never comes here (SECURITY.md §4.6).
//
// The model decides what else is allowed (ANTHROPIC.md §3): thinking turns off only on Haiku
// and Sonnet, at effort High or below; Opus 5.5 always thinks. ehr checks the same rules on
// save (settings.rs › coherent), so the page greys what the model refuses.
import { saveAnthropic, type AnthropicSettings, type LlmValues } from "../agent/api";
import { anthropicSettings } from "../agent/stores";
import { settingsView, type Kit } from "./config-kit";
import { el } from "./page";
import type { View } from "./subnav";

// The help behind each "i" (the owner's to edit; CONSOLE.md §8.2).
const HELP: Record<string, string> = {
  model:
    "The Claude model that chooses each step of the call and its words. Haiku 5.5 is the cheapest and the start (the owner, 2026-10-20: move up only when a named test call shows Haiku falls short). Sonnet 5.5 costs 20 times as much, Opus 5.5 40 times. A change empties the prompt cache for the next call. The tests always replay with Haiku.",
  effort:
    "How much the model spends on each reply, thinking included: Low, Medium, High, Extra high, Max. Lower is a faster first word and fewer tokens. Each model has its own default; the agent runs Low, because a spoken reply is a sentence or two.",
  thinking:
    "On: the model may think before it answers (adaptive thinking). Off: it answers at once, for a faster first word. Off works on Haiku and Sonnet only, at effort High or below; Opus 5.5 always thinks. With thinking off, a model can write a tool call as words instead of making it: test before you keep it off.",
  maxTokens:
    "The most tokens one reply may use, thinking included. A spoken reply needs about 100. Haiku thinks first, and at 600 one reply spent the whole cap thinking and said nothing (2026-11-12). Unused room costs nothing.",
  retry:
    "A reply cut off by the cap with nothing to say is dropped, the caller hears the holding line, and the request is sent once more with this cap. At least the first cap.",
  cacheTtl:
    "How long Anthropic keeps the system prompt's cache between requests. 5 minutes is Anthropic's default: a call's turns keep it warm. 1 hour keeps it across the gaps between calls, but each write costs 2 times the input price, not 1.25 times.",
  budget:
    "The money for Claude in a cycle (the owner: $100 a month), then a manual offset: dollars spent outside these calls, such as the Console's workbench or another machine. Dollars left subtracts both, and this cycle's test calls.",
  cycleDay: "The day of the month the budget renews.",
  left: "The budget, less the offset, less what this machine's call records spent this cycle. An estimate from each record's token counts; the Claude Console's usage page has the true amount.",
  key: "Never shown here. On this machine it is ANTHROPIC_API_KEY in .env; in the cloud, the secret manager (CONSOLE.md §7.4).",
  region:
    "Every request asks for inference in the United States only (ANTHROPIC.md §2). Anthropic charges 1.1 times the price for it. Fixed in code, not a setting.",
  features:
    "Only generally available API features: Anthropic reports beta features as outside the BAA. The agent drops Pipecat's beta flag (llm.py). Refusal fallbacks, effort changes during a call and task budgets are betas, so they stay off.",
  rates:
    "What Anthropic charges for the chosen model, in dollars per 1M tokens, before the 1.1 times for US-only inference. A cache write costs 1.25 times the input price (5 minutes) or 2 times (1 hour).",
};

const EFFORT_WORD: Record<string, string> = { low: "Low", medium: "Medium", high: "High", xhigh: "Extra high", max: "Max" };

const dollars = (x: number) => `${x < 0 ? "−" : ""}$${Math.abs(x).toFixed(2)}`;
const rate = (x: number) => `$${x < 1 ? x.toFixed(2) : x.toFixed(2).replace(/\.00$/, "")}`;

/** The settings that differ between two sets. */
function differing(a: LlmValues, b: LlmValues): Set<string> {
  const out = new Set<string>();
  for (const k of Object.keys(a) as (keyof LlmValues)[]) if (a[k] !== b[k]) out.add(k);
  return out;
}

export const anthropicView = (): View =>
  settingsView<LlmValues, LlmValues, AnthropicSettings>({
    id: "anthropic",
    title: "Anthropic",
    store: anthropicSettings,
    save: (_s, d) => saveAnthropic(d),
    fromValues: (v) => ({ ...v }),
    differing,
    render,
  });

function render(s: AnthropicSettings, d: LlmValues, kit: Kit) {
  const { row, keyed: k, fixed, select, number, pair, card, rerender } = kit;
  const model = s.models.find((m) => m.id === d.model) ?? s.models[0];
  const offAllowed = (m = model, effort = d.effort) => m.thinking_off && s.thinking_off_efforts.includes(effort);

  // A model or effort that does not allow thinking off turns it back on; the cards are drawn
  // again so the rows show what the new model allows.
  const keepAllowed = () => {
    if (d.thinking === "off" && !offAllowed(s.models.find((m) => m.id === d.model) ?? model, d.effort)) d.thinking = "on";
    rerender();
  };

  const modelSelect = select(
    s.models.map((m): [string, string] => [m.id, m.name]),
    d.model,
    (v) => ((d.model = v), keepAllowed()),
  );
  const effortSelect = select(
    s.efforts.map((e): [string, string] => [e, e === model.effort_default ? `${EFFORT_WORD[e] ?? e} (${model.name}'s default)` : (EFFORT_WORD[e] ?? e)]),
    d.effort,
    (v) => ((d.effort = v), keepAllowed()),
  );
  const thinking = pair(d.thinking === "on", (v) => (d.thinking = v ? "on" : "off"), "Thinking");
  if (!offAllowed()) {
    // Greyed in Edit too (config-kit's data-off), with the reason on hover.
    const off = thinking.querySelector<HTMLButtonElement>('[data-on="false"]')!;
    off.dataset.off = "";
    thinking.title = model.thinking_off ? "Off needs effort High or below" : `${model.name} cannot turn thinking off`;
  }

  const modelCard = card(
    "Model",
    "model",
    k("model", row("Model", HELP.model, modelSelect)),
    k("effort", row("Effort", HELP.effort, effortSelect)),
    k("thinking", row("Thinking", HELP.thinking, thinking)),
  );

  const r = s.ranges;
  const reply = card(
    "Reply and cache",
    "reply",
    k("max_tokens", row("Max tokens", HELP.maxTokens, number(d.max_tokens, r.max_tokens.min, r.max_tokens.max, (v) => (d.max_tokens = v)))),
    k("retry_max_tokens", row(
      "Retry cap",
      HELP.retry,
      number(d.retry_max_tokens, r.retry_max_tokens.min, r.retry_max_tokens.max, (v) => (d.retry_max_tokens = v)),
    )),
    k("cache_ttl", row(
      "Cache life",
      HELP.cacheTtl,
      select(
        [
          ["5m", "5 minutes (Anthropic's default)"],
          ["1h", "1 hour"],
        ],
        d.cache_ttl,
        (v) => (d.cache_ttl = v as LlmValues["cache_ttl"]),
      ),
    )),
  );

  const left = fixed("");
  const showLeft = () => {
    left.textContent = dollars(d.budget_usd - d.offset_usd - s.usage.spent);
  };
  showLeft();
  const minus = el(`<span class="cfg-minus" aria-hidden="true">−</span>`);
  const usage = card(
    "Usage",
    "usage",
    k("budget_usd", row(
      "Budget ($)",
      HELP.budget,
      number(d.budget_usd, 0, 1_000_000, (v) => ((d.budget_usd = v), showLeft()), "Budget"),
      minus,
      number(d.offset_usd, 0, 1_000_000, (v) => ((d.offset_usd = v), showLeft()), "Offset", 0.01),
    )),
    k("cycle_day", row("Cycle day", HELP.cycleDay, number(d.cycle_day, 1, 28, (v) => (d.cycle_day = v)))),
    row("Dollars left", HELP.left, left),
  );

  const p = model.price;
  const facts = card(
    "Key and facts",
    "facts",
    row("API key", HELP.key, fixed(s.key ? "Set" : "Not set: add ANTHROPIC_API_KEY to .env")),
    row("Region", HELP.region, fixed(s.inference_geo === "us" ? `US only (${s.geo_price}× price)` : s.inference_geo)),
    row("Features", HELP.features, fixed("Generally available only")),
    // In the Cartesia row's form (the owner, 2026-11-13).
    row(
      "Rates",
      HELP.rates,
      fixed(`In: ${rate(p.input)} | Out: ${rate(p.output)} | Cache read: ${rate(p.cache_read)} per 1M tokens`),
    ),
  );

  // Model and Reply side by side when there is room, Usage and Key and facts below them
  // (config.css › .cfg-body[data-layout="anthropic"]). Budget glows with the offset beside it.
  return {
    cards: [modelCard, reply, usage, facts],
    layout: "anthropic",
    glow: (diff: Set<string>) => {
      if (diff.has("offset_usd")) diff.add("budget_usd");
    },
  };
}
