// Config › Agent (CONSOLE.md §9; AGENT-PAGE.md, the owner, 2026-11-13): every section of the
// agent's words (agent/prompts/AGENT-PROMPT.md) in the Config frame (config-kit.ts: Read | Edit,
// the versions, Discard | Save). The tabs are the call's steps, Greet … Failure; each line sits
// under the tool that speaks it, and every section says when it plays (agent/prompts/GUIDE.md).
// One layout for both readers (AGENT-PAGE.md §8, the owner, 2026-11-14): a block per tool. Plain
// (the default, for the analyst new to this) shows titles only; Code (for the power user) adds
// each tool's name and each section's key. A
// save makes a new version that the next test call runs with, and writes the prompt file;
// Record and test records the kept scripts with the new words and runs the tests, after a free
// dry run has quoted what it will cost.
//
// The owner's notes (2026-11-13): the names say what a section does at first try; Read shows the
// words as text and Edit as boxes, with a fade between them; the name sits left of the words, as
// on the Cartesia rows; "minimize jitter and inconsistency", so the words never move when the
// box fades in, and every size, color and timing is an existing token.
import { estimatePrompt, recordPrompt, savePrompt, type PromptSettings, type Recording } from "../agent/api";
import { promptSettings } from "../agent/stores";
import { settingsView, type Kit } from "./config-kit";
import { el } from "./page";
import type { View } from "./subnav";
import { remember, remembered, tabBar as bar } from "./tab-bar";
import { attachPicker, type PickItem } from "./word-picker";

type Values = Record<string, string>;
type Kind = "instructions" | "act" | "field" | "hears" | "text";
type Order = "semantic" | "flow";

// Core (the core prompt and the 911 tools; the owner, 2026-11-14), then the call's steps, in its order; one
// word each (the owner, 2026-11-13). prompts.py › GUIDE_TABS.
const TABS = ["Core", "Greet", "Verify", "Menu", "Schedule", "Anytime", "Failure"] as const;
type Tab = (typeof TABS)[number];
// A step's tab, for the tools offered there (flow.py › tool_steps; "every" is Anytime).
const STEP_TAB: Record<string, Tab> = { greet: "Greet", verify: "Verify", menu: "Menu", schedule: "Schedule", every: "Anytime" };
// The tab a browser remembered from the five groups before (AGENT-PAGE.md §4).
const OLD_TAB: Record<string, Tab> = { General: "Core", "Open and close": "Anytime", Visits: "Menu", Safety: "Anytime" };

/** One section's place on the page (GUIDE.md): its tab, its name, the tool it sits under ("" for
 * none) and when it plays. */
export type GuideEntry = { key: string; tab: Tab; name: string; under: string; plays: string };
/** What prompt_view sends beyond PromptSettings: the guide, and each tool's steps. */
type Guided = Omit<PromptSettings, "tools"> & { guide: GuideEntry[]; tools: { name: string; does: string; steps: string[] }[] };

// The guide, as the last view loaded it: other pages name sections by it (System's links).
let GUIDE: GuideEntry[] = [];
/** Take the guide from a view's reply (this page's or System's). */
export const useGuide = (g: GuideEntry[] | undefined) => {
  if (g?.length) GUIDE = g;
};
/** A section's name on this page, for another page's links ("Used in"). */
export const sectionName = (key: string) => GUIDE.find((g) => g.key === key)?.name ?? key;
/** Whether the page has a section by this key. */
export const hasSection = (key: string) => GUIDE.some((g) => g.key === key);

/** Ask Config › Agent to show one section: its tab, then the section, which flashes (Config ›
 * System's links). The Agent view need not be built yet: it reads the ask when it is. */
export const SHOW_SECTION = "deets:show-section";
/** Config › System saved new variables: the `{` list and the checks use them at once. */
export const SYSTEM_SAVED = "deets:system-saved";

// A tool's own words, as a row inside its block: the block's head already holds the tool's title
// (AGENT-PAGE.md §8).
const TOOL_ROW = "When to use it";

const kindOf = (key: string): Kind => {
  const head = key.split(".")[0];
  if (head === "tool") return "act";
  if (head === "arg") return "field";
  if (head === "line") return "hears";
  if (head === "text") return "text";
  return "instructions"; // role, task, note
};

// The analyst's place: a way to read the page, not a change, so it is kept in this browser
// (the order and the tab). Read | Edit is not kept: it opens on Read each time.
const ORDER_KEY = "deets.console.agent.order";
const TAB_KEY = "deets.console.agent.tab";

/** The tab this browser opened last; a name from the five groups before maps to its new tab. */
function rememberedTab(): Tab {
  try {
    const v = localStorage.getItem(TAB_KEY) ?? "";
    if ((TABS as readonly string[]).includes(v)) return v as Tab;
    return OLD_TAB[v] ?? TABS[0];
  } catch {
    return TABS[0];
  }
}

/** What the page knows of the variables and tools (dev_server.py › prompt_view). */
type Words = Pick<PromptSettings, "variables" | "code_values" | "tools" | "tool_scope" | "placeholders">;

/** The `{placeholders}` in a text, as the agent and ehr read them (prompts.py › check,
 * settings.rs › placeholders). Null for a stray brace. */
function placeholders(text: string): Set<string> | null {
  const found = new Set<string>();
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if ((c === "{" || c === "}") && text[i + 1] === c) {
      i++;
    } else if (c === "}") {
      return null;
    } else if (c === "{") {
      const end = text.indexOf("}", i + 1);
      const open = text.indexOf("{", i + 1);
      if (end < 0 || (open >= 0 && open < end)) return null;
      found.add(text.slice(i + 1, end).split(/[:!]/)[0]);
      i = end;
    }
  }
  return found;
}

/** What is wrong with a section's words, in the page's words, or null (SYSTEM-PAGE.md §3.3, as
 * prompts.py › check and mention_problems read it): its own code values are there; any other
 * `{name}` is a System variable; a tool in backticks is one its steps offer. */
function problem(key: string, text: string, w: Words): string | null {
  if (!text.trim()) return "Empty";
  const found = placeholders(text);
  if (!found) return "A { or } with no partner";
  const own = w.placeholders[key] ?? [];
  const missing = own.filter((p) => !found.has(p));
  if (missing.length) return `Needs ${missing.map((p) => `{${p}}`).join(", ")}`;
  const codes = new Set(w.code_values.map((c) => c.name));
  const vars = new Set(w.variables.map((v) => v.name));
  for (const p of found) {
    if (own.includes(p)) continue;
    if (codes.has(p)) return `{${p}} is filled only in its own section`;
    if (!vars.has(p)) return `{${p}} is not a System variable`;
  }
  const allowed = new Set(w.tool_scope[key] ?? []);
  for (const t of toolsIn(text, w)) {
    if (!allowed.has(t)) return allowed.size ? `\`${t}\` is not offered in this step` : `The caller hears this: no tool names here`;
  }
  return null;
}

/** The tools a text names: a known tool's name in backticks (prompts.py › mentions). */
function toolsIn(text: string, w: Words): string[] {
  const known = new Set(w.tools.map((t) => t.name));
  return [...text.matchAll(/`([a-z][a-z0-9_]*)`/g)].map((m) => m[1]).filter((t) => known.has(t));
}

/** The words as Read shows them: the same characters as the box, each variable and each tool
 * marked, so the text sits exactly where the box's text will (no jump when Edit fades the box
 * in). */
function showWords(target: HTMLElement, text: string, w: Words) {
  target.replaceChildren();
  const known = new Set(w.tools.map((t) => t.name));
  const re = /\{[a-z_][a-z0-9_]*(?:[:!][^}]*)?\}|`([a-z][a-z0-9_]*)`/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m[1] !== undefined && !known.has(m[1])) continue; // backticks round a word, not a tool
    if (m.index! > last) target.append(text.slice(last, m.index));
    const mark = el(`<span></span>`);
    mark.className = m[1] !== undefined ? "prompt-tool" : "prompt-mark";
    mark.textContent = m[0];
    target.append(mark);
    last = m.index! + m[0].length;
  }
  if (last < text.length) target.append(text.slice(last));
}

/** The `{` list for one section: every System variable, then the section's own code values. */
function variableItems(key: string, w: Words): PickItem[] {
  const own = w.placeholders[key] ?? [];
  return [
    ...w.variables.map((v) => ({ insert: `{${v.name}}`, name: `{${v.name}}`, detail: v.meaning, note: v.value, group: "System variables" })),
    ...w.code_values
      .filter((c) => own.includes(c.name))
      .map((c) => ({ insert: `{${c.name}}`, name: `{${c.name}}`, detail: c.meaning, note: "Filled by the code", group: "This section's code values" })),
  ];
}

/** The " ." list for one section: the tools its steps offer, or why it may name none. */
function toolItems(key: string, w: Words): PickItem[] | string {
  const allowed = new Set(w.tool_scope[key] ?? []);
  if (!allowed.size) return "The caller hears or reads this section, so no tool name belongs here";
  return w.tools.filter((t) => allowed.has(t.name)).map((t) => ({ insert: `\`${t.name}\``, name: t.name, detail: t.does }));
}

function differing(a: Values, b: Values): Set<string> {
  const out = new Set<string>();
  for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) if (a[k] !== b[k]) out.add(k);
  return out;
}

/** Dollars to the cent; under a cent says so (a kept script often costs a tenth of a cent). */
/** How long the Test status waits after each answer before it asks again. */
const TEST_POLL_MS = 2000;

const dollars = (x: number) => (x > 0 && x < 0.01 ? "under $0.01" : `$${x.toFixed(2)}`);

export const agentView = (): View => {
  let order: Order = remembered(ORDER_KEY, ["semantic", "flow"] as const, "semantic");
  let tab: Tab = rememberedTab();
  // Set by a click on a tab bar, read by the next drawing: the pill slides from that tab.
  let slideTab: string | undefined;
  // The latest settings, for the Record and test line (it updates without drawing the cards),
  // and for the variables and tools the lists and checks use.
  let latest: Guided | undefined;
  let polling = false; // the Test status is being followed (follow)
  let watching = false; // the Test button follows the active version's store
  // Another page's ask to show a section (SHOW_SECTION), until the next drawing shows it.
  let pending: string | undefined;
  let redraw: ((toTop?: boolean) => void) | undefined;

  /** Open a section's tab and bring it into view (System's links; this page's own links). */
  const showSection = (key: string) => {
    const found = GUIDE.find((g) => g.key === key);
    if (!found) return;
    tab = found.tab;
    remember(TAB_KEY, tab);
    pending = key;
    redraw?.(true);
  };
  window.addEventListener(SHOW_SECTION, (e) => showSection((e as CustomEvent<string>).detail));
  window.addEventListener(SYSTEM_SAVED, (e) => {
    if (!latest) return;
    latest.variables = (e as CustomEvent<PromptSettings["variables"]>).detail;
    redraw?.();
  });

  // Test | cost, in the title bar left of Discard | Save (the owner, 2026-11-14; it replaced the
  // Record and test line). Red (--stop) while the kept recordings do not match the saved words,
  // green (--go) once they do. Red: a first press works out the cost (a free dry run; "$?.??"
  // until it is known), a second press records and runs the tests, the green filling in as the
  // steps finish (each script, then the tests: a count, not a guess of time).
  const test = el(
    `<button class="cfg-test" type="button" data-state="due"><span class="cfg-test-fill" aria-hidden="true"></span><span class="cfg-test-word">Test</span><span class="cfg-test-cost">$?.??</span></button>`,
  ) as HTMLButtonElement;
  const testCost = test.querySelector<HTMLElement>(".cfg-test-cost")!;
  const testFill = test.querySelector<HTMLElement>(".cfg-test-fill")!;
  const showTest = (p: PromptSettings) => {
    const run = p.recording;
    const total = p.recordings.scripts;
    const running = run?.state === "running";
    const quoting = p.estimate?.state === "running";
    const done = p.recordings.match && !running;
    test.dataset.state = running ? "running" : done ? "done" : "due";
    test.disabled = done || running || quoting || !p.key;
    // The steps: each script, then the tests.
    const steps = run ? run.scripts.length + (run.tests === null ? 0 : 1) : 0;
    testFill.style.setProperty("--share", running ? String(Math.min(1, steps / (total + 1))) : "0");
    const e = p.estimate;
    testCost.textContent = done
      ? "Passed"
      : running
        ? `${Math.min(run!.scripts.length + 1, total)} of ${total}`
        : run?.state === "failed" && e?.state !== "done"
          ? "Failed"
          : e?.state === "done" && e.usd !== null
            ? shortDollars(e.usd)
            : "$?.??";
    test.title = !p.key
      ? "Needs ANTHROPIC_API_KEY in .env"
      : done
        ? `The kept recordings match the saved words${run ? `. ${describe(run, total)}` : ""}`
        : running
          ? describe(run!, total)
          : quoting
            ? "Working out the cost: a dry run, no Claude call"
            : e?.state === "done"
              ? `Press to record ${e.missing.length} of ${total} scripts with the saved words (Claude is called only for new lines), then run the agent's tests${run?.state === "failed" ? `. Last run: ${describe(run, total)}` : ""}`
              : "The kept recordings are out of date. Press to work out the cost of recording them";
    test.setAttribute("aria-label", `Test, ${testCost.textContent}`);
  };
  test.addEventListener("click", async () => {
    const p = latest;
    if (!p) return;
    try {
      if (!p.estimate) {
        // The first press: the quote.
        p.estimate = await promptSettings(0).act(() => estimatePrompt());
      } else {
        // The second: record and test (also after a quote that failed: it records without one).
        p.recording = await promptSettings(0).act(() => recordPrompt());
      }
      showTest(p);
      follow(showTest);
    } catch (err) {
      test.title = err instanceof Error ? err.message : "Not started";
    }
  });

  const take = (s: PromptSettings) => {
    latest = s as Guided;
    useGuide(latest.guide);
    showTest(latest);
    // From the first load on, each answer of the active version's store (the Test poll's
    // ticks above all) updates the Test button. Subscribed here, not when the view is made,
    // so nothing is read before Config › Agent opens.
    if (!watching) {
      watching = true;
      promptSettings(0).subscribe(test, (st) => {
        if (st.status !== "ready" || !st.value || !latest) return;
        latest.recording = st.value.recording;
        latest.recordings = st.value.recordings;
        latest.estimate = st.value.estimate;
        showTest(latest);
      });
    }
    if (latest.recording?.state === "running" || latest.estimate?.state === "running") follow(showTest);
    return latest;
  };

  return settingsView<Values, Values, PromptSettings>({
    id: "agent",
    title: "Agent",
    store: promptSettings,
    took: take,
    save: (_s, d) => savePrompt(d),
    fromValues: (v) => ({ ...v }),
    differing,
    blocked: (d) => {
      if (!latest) return null;
      for (const g of GUIDE) {
        const p = problem(g.key, d[g.key] ?? "", latest);
        if (p) return `${g.tab} › ${g.name}: ${p}`;
      }
      return null;
    },
    render: (s, d, kit) => render(s as Guided, d, kit),
    top: () => test,
  });

  function render(s: Guided, d: Values, kit: Kit) {
    redraw = kit.rerender;
    // The variables and tools: the latest known (a System save updates them), else this load's.
    const w: Guided = latest ?? s;
    const guide = w.guide ?? [];
    const wanted = (key: string) => s.placeholders[key] ?? [];
    const card = el(`<section class="cfg-card prompt-card"></section>`);
    card.dataset.card = "prompt";

    // ---- the tab bar and Semantic | Flow, on one row
    const bars = el(`<div class="prompt-bars"></div>`);
    // The tab chosen before a click, for this one drawing only (a pill slides from it).
    const tabFrom = slideTab;
    slideTab = undefined;
    const tabBar = bar(
      "Step of the call",
      TABS.map((t) => [t, t]),
      tab,
      (v) => {
        slideTab = tab;
        tab = v as Tab;
        remember(TAB_KEY, tab);
        kit.rerender(true);
      },
      tabFrom,
    );
    // Plain | Code changes nothing but the detail, so a click draws nothing again: the card's
    // `data-order` rolls every key and chip in or out in place (config.css › .prompt-key-roll),
    // and only this bar is made again, so its pill slides. The page keeps its scroll place.
    card.dataset.order = order;
    const makeOrderBar = (from?: string) =>
      bar(
        "Detail",
        [
          ["semantic", "Plain"],
          ["flow", "Code"],
        ],
        order,
        (v) => {
          const before = order;
          order = v as Order;
          remember(ORDER_KEY, order);
          card.dataset.order = order;
          const old = orderBar;
          orderBar = makeOrderBar(before);
          old.el.replaceWith(orderBar.el);
          orderBar.place();
        },
        from,
      );
    let orderBar = makeOrderBar();
    bars.append(tabBar.el, orderBar.el);
    const markBad = () => {
      for (const t of TABS) {
        const bad = guide.some((g) => g.tab === t && problem(g.key, d[g.key] ?? "", w));
        tabBar.button(t)?.classList.toggle("prompt-tab-bad", bad);
      }
    };
    markBad();

    // ---- one section: the "i", its name (the name alone says what the row is), its words.
    // The "i" rolls down the rest, as on every Config row (config-kit.ts › row): when it plays
    // and the code values it keeps. The words are two layers in one place: the
    // text (Read) and the box (Edit), with the same padding, edge width, type and wrapping, so
    // the box fades in over the text without a shift.
    const section = (g: GuideEntry) => {
      const key = g.key;
      const kind = kindOf(key);
      const need = wanted(key);
      const r = el(`
        <div class="prompt-section prompt-has-info">
          <button class="cfg-info" type="button" aria-expanded="false"><span class="i-glyph" aria-hidden="true">i</span></button>
          <div class="prompt-label"><span class="prompt-title"><span class="prompt-mark-kind"></span><span class="prompt-name"></span></span></div>
          <div class="cfg-control prompt-control">
            <div class="prompt-words"></div>
            <textarea class="cfg-field prompt-text" rows="1" spellcheck="true"></textarea>
          </div>
          <div class="cfg-help"><div class="cfg-help-text prompt-help"></div></div>
          <p class="prompt-problem" role="status"></p>
        </div>`);
      r.dataset.key = key;
      r.dataset.kind = kind;
      const name = g.name;
      // Inside its block a tool's own words read "When to use it": the head holds its title.
      r.querySelector<HTMLElement>(".prompt-name")!.textContent = kind === "act" ? TOOL_ROW : name;
      // AI or HC before the title (the owner, 2026-11-14): a prompt the model reads, or fixed
      // words the caller hears or reads exactly as written.
      const fixed = kind === "hears" || kind === "text";
      const mark = r.querySelector<HTMLElement>(".prompt-mark-kind")!;
      mark.textContent = fixed ? "HC" : "AI";
      mark.dataset.by = fixed ? "code" : "model";
      mark.title = fixed ? "Hardcoded: the caller gets these exact words" : "AI: a prompt the agent reads";
      mark.setAttribute("aria-label", mark.title);
      mark.setAttribute("role", "img");
      // The key, always there; it shows in Code only (it rolls down under the title). It may
      // wrap after a dot, never inside a word.
      const roll = el(`<span class="prompt-key-roll"><span class="prompt-key"></span></span>`);
      key.split(".").forEach((part, i) => {
        if (i) roll.firstElementChild!.append(".", document.createElement("wbr"));
        roll.firstElementChild!.append(part);
      });
      r.querySelector(".prompt-label")!.append(roll);
      const help = r.querySelector<HTMLElement>(".prompt-help")!;
      const fact = (label: string, text: string) => {
        const p = el(`<p><span class="prompt-help-key"></span> </p>`);
        p.firstElementChild!.textContent = label;
        p.append(text);
        help.append(p);
      };
      fact("Plays when:", g.plays);
      if (need.length) fact("Keeps:", need.map((p) => `{${p}}`).join(" "));
      const info = r.querySelector<HTMLButtonElement>(".cfg-info")!;
      const helpBox = r.querySelector<HTMLElement>(".cfg-help")!;
      info.setAttribute("aria-label", `About ${name}`);
      // The help rolls down and up (config.css › .cfg-help); the "i" stays live in Read.
      info.addEventListener("click", () => {
        const open = !helpBox.classList.contains("open");
        helpBox.classList.toggle("open", open);
        info.setAttribute("aria-expanded", String(open));
      });
      const words = r.querySelector<HTMLElement>(".prompt-words")!;
      const box = r.querySelector<HTMLTextAreaElement>("textarea")!;
      const err = r.querySelector<HTMLElement>(".prompt-problem")!;
      box.value = d[key] ?? "";
      box.setAttribute("aria-label", name);
      showWords(words, box.value, w);
      // `{` and " ." open the lists of what this section may hold (SYSTEM-PAGE.md §3.6).
      attachPicker(box, { variables: () => variableItems(key, w), tools: () => toolItems(key, w) });
      const showProblem = () => {
        const p = problem(key, box.value, w);
        err.textContent = p ?? "";
        err.hidden = !p;
        r.classList.toggle("prompt-bad", !!p);
      };
      showProblem();
      box.addEventListener("input", () => {
        d[key] = box.value;
        showWords(words, box.value, w);
        grow(box);
        showProblem();
        markBad();
        kit.changed();
      });
      return r;
    };

    // A tool offered in more than one step sits once, in its first step; each other step shows a
    // link to it (AGENT-PAGE.md §2).
    // A tool's home is where the guide puts it: its first step's tab, or Core for the 911 tools
    // (tools of every step, so Anytime links to them).
    const home = (t: { name: string; steps: string[] }) => guide.find((g) => g.key === `tool.${t.name}`)?.tab ?? STEP_TAB[t.steps[0]];
    const links = w.tools.filter((t) => t.steps.some((x) => STEP_TAB[x] === tab) && home(t) !== tab);
    const linkRow = () => {
      const p = el(`<p class="prompt-links"><span>Also here:</span></p>`);
      links.forEach((t, i) => {
        if (i) p.append(" · ");
        const a = el(`<button class="cfg-link" type="button"></button>`) as HTMLButtonElement;
        a.textContent = `${sectionName(`tool.${t.name}`)} · in ${home(t)}`;
        a.addEventListener("click", () => {
          slideTab = tab;
          showSection(`tool.${t.name}`);
        });
        p.append(a);
      });
      return p;
    };

    // One layout for both readers (AGENT-PAGE.md §8, the owner, 2026-11-14): the sections with
    // no tool first, then a block per tool: its title at the head, then its own words, its
    // fields, and every line and note it can produce. Code adds the tool's name as a chip (and
    // each section's key under its name).
    const rows = guide.filter((g) => g.tab === tab);
    const body = el(`<div class="prompt-sections"></div>`);
    const blocks = new Map<string, HTMLElement>();
    for (const g of rows) {
      if (!g.under) {
        body.append(section(g));
        continue;
      }
      let block = blocks.get(g.under);
      if (!block) {
        // The chip is always there; it shows in Code only (it rolls in from the left).
        block = el(
          `<div class="prompt-block"><p class="prompt-block-head"><span class="prompt-chip-roll"><span class="prompt-chip-clip"><span class="prompt-tool"></span></span></span><span class="prompt-block-name"></span></p></div>`,
        );
        const head = block.firstElementChild!;
        head.querySelector(".prompt-tool")!.textContent = g.under;
        head.querySelector(".prompt-block-name")!.textContent = sectionName(`tool.${g.under}`);
        blocks.set(g.under, block);
        body.append(block);
      }
      block.append(section(g));
    }
    if (links.length) body.append(linkRow());

    card.append(bars, body);
    // The boxes take the height of their words once on screen, and again when the card's width
    // changes; the pills are placed the same way.
    const settle = () => {
      card.querySelectorAll<HTMLTextAreaElement>("textarea").forEach(grow);
      tabBar.place();
      orderBar.place();
      // Another page asked for a section: bring it into view, and flash it once.
      // Only once the card is on screen: the ask may come while another Config view shows.
      const asked = pending && card.offsetWidth ? card.querySelector<HTMLElement>(`[data-key="${pending}"]`) : null;
      if (asked) {
        pending = undefined;
        asked.scrollIntoView({ block: "center", behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth" });
        asked.classList.add("prompt-flash");
        asked.addEventListener("animationend", () => asked.classList.remove("prompt-flash"), { once: true });
      }
    };
    requestAnimationFrame(settle);
    new ResizeObserver(settle).observe(card);

    return {
      cards: [card],
      layout: "prompt",
      // An older version: a tab with a changed section shows a dot, in either order.
      glow: (diff: Set<string>) => {
        for (const t of TABS) {
          const changed = guide.some((g) => g.tab === t && diff.has(g.key));
          tabBar.button(t)?.classList.toggle("prompt-tab-changed", changed);
        }
      },
    };
  }

  /** Read the dry run's or the recording's state until both are done: the active version's
   * store polls, 2 seconds after each answer (LOADING.md §2, rule 5), each tick in the ledger as
   * a `poll` of `settings.agent`. The subscription in the view's build draws each answer. */
  function follow(_show: (p: PromptSettings) => void) {
    if (polling) return;
    polling = true;
    promptSettings(0).poll(TEST_POLL_MS, (now) => {
      const on = !!now && (now.recording?.state === "running" || now.estimate?.state === "running");
      if (!on) polling = false;
      return on;
    });
  }
}

/** The Test button's cost: "$0.02", "<$0.01" for a few lines, "$0.00" when nothing is new. */
const shortDollars = (x: number) => (x > 0 && x < 0.01 ? "<$0.01" : `$${x.toFixed(2)}`);

function describe(run: Recording, total: number): string {
  const failed = run.scripts.filter((x) => !x.ok).length;
  if (run.state === "running") return `Script ${Math.min(run.scripts.length + 1, total)} of ${total}`;
  const spent = run.spent_usd === null ? "" : `. Spent ${dollars(run.spent_usd)}`;
  if (run.state === "passed") return `Passed: ${total} scripts, the tests pass${spent}`;
  const parts = [];
  if (failed) parts.push(`${failed} ${failed === 1 ? "script" : "scripts"} failed`);
  if (run.tests === false) parts.push("the tests fail");
  // Each failed test on its own line, with the first line of its error (DATABASE.md §8.5).
  const why = (run.failures ?? []).map((f) => `\n• ${f.test}: ${f.line}`).join("");
  return `Failed: ${parts.join(", ") || "see the dev server's log"}${spent}${why}`;
}

/** A text box as tall as its words. */
function grow(box: HTMLTextAreaElement) {
  if (!box.isConnected) return;
  box.style.height = "auto";
  box.style.height = `${box.scrollHeight}px`;
}

