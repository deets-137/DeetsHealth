// Config › Agent (CONSOLE.md §9; the owner, 2026-11-13): every section of the agent's words
// (agent/prompts/AGENT-PROMPT.md) in the Config frame (config-kit.ts: Read | Edit, the versions,
// Discard | Save). Under the top row, a tab bar of five groups and a Semantic | Flow bar:
// Semantic sorts a group's sections under headings (what the section is); Flow lists them in the
// order of the call. A save makes a new version that the next test call runs with, and writes
// the prompt file; Record and test records the kept scripts with the new words and runs the
// tests, after a free dry run has quoted what it will cost.
//
// The owner's notes (2026-11-13): the names say what a section does at first try; Read shows the
// words as text and Edit as boxes, with a fade between them; the name sits left of the words, as
// on the Cartesia rows; "minimize jitter and inconsistency", so the words never move when the
// box fades in, and every size, color and timing is an existing token.
import {
  estimatePrompt,
  promptSettings,
  recordPrompt,
  savePrompt,
  type Estimate,
  type PromptSettings,
  type Recording,
} from "../agent/api";
import { settingsView, type Kit } from "./config-kit";
import { el } from "./page";
import type { View } from "./subnav";
import { remember, remembered, tabBar as bar } from "./tab-bar";
import { attachPicker, type PickItem } from "./word-picker";

type Values = Record<string, string>;
type Kind = "instructions" | "act" | "field" | "hears" | "text";
type Order = "semantic" | "flow";

const TABS = ["General", "Open and close", "Verify", "Visits", "Safety"] as const;
type Tab = (typeof TABS)[number];

/** Ask Config › Agent to show one section: its tab, then the section, which flashes (Config ›
 * System's links). The Agent view need not be built yet: it reads the ask when it is. */
export const SHOW_SECTION = "deets:show-section";
/** Config › System saved new variables: the `{` list and the checks use them at once. */
export const SYSTEM_SAVED = "deets:system-saved";

// Semantic's headings, in their order. An action's fields sit under it in "When to act" (the
// owner, 2026-11-13), so fields have no heading of their own.
const HEADINGS: [string, Kind[]][] = [
  ["Instructions", ["instructions"]],
  ["When to act", ["act", "field"]],
  ["Caller hears", ["hears"]],
  ["Text message", ["text"]],
];
// Flow's tag for each kind, over the name.
const TAG: Record<Kind, string> = {
  instructions: "Instructions",
  act: "When to act",
  field: "Field",
  hears: "Caller hears",
  text: "Text message",
};

/** Each section: its tab and its name, in the order of the call within the tab (Flow's
 * order); each field right after its action. The kind comes from the key.
 * agent/tests/test_prompts.py checks that every section the agent reads is here once. */
export const SECTIONS: [string, Tab, string][] = [
  ["role", "General", "Role and rules"],
  ["line.one_moment", "General", "Looking it up"],
  ["line.say_again", "General", "Did not hear clearly"],
  ["line.not_on_this_call", "General", "Not possible on this call"],
  ["line.not_now", "General", "Failed for now"],
  ["line.ehr_down", "General", "Scheduling system down"],

  ["line.greeting", "Open and close", "Greeting"],
  ["tool.end_call", "Open and close", "End the call"],
  ["line.goodbye", "Open and close", "Goodbye"],
  ["tool.hand_off", "Open and close", "Connect to a person"],
  ["arg.hand_off.kind", "Open and close", "Reason for a person"],
  ["arg.hand_off.summary", "Open and close", "Summary for staff"],
  ["arg.hand_off.evidence", "Open and close", "Caller's words for staff"],
  ["line.hand_off", "Open and close", "Connecting to a person"],
  ["tool.offer_staff", "Open and close", "Offer a person"],
  ["arg.offer_staff.why", "Open and close", "Reason for the offer"],
  ["line.offer_staff", "Open and close", "Cannot help, offer a person"],
  ["line.offer_staff_private", "Open and close", "Private reason, offer a person"],

  ["task.greet", "Verify", "Step 1: name and date of birth"],
  ["line.ask_details", "Verify", "Ask for details"],
  ["line.name_not_said", "Verify", "Full name not heard"],
  ["line.birth_not_possible", "Verify", "Date of birth not real"],
  ["tool.identify", "Verify", "Read back details"],
  ["arg.identify.given_name", "Verify", "First name"],
  ["arg.identify.family_name", "Verify", "Last name"],
  ["arg.identify.birth_date", "Verify", "Date of birth"],
  ["line.read_back", "Verify", "Read back details"],
  ["tool.confirm_details", "Verify", "Look up and send code"],
  ["text.code", "Verify", "Code text"],
  ["line.code_sent", "Verify", "Code sent"],
  ["task.verify", "Verify", "Step 2: check the code"],
  ["tool.check_code", "Verify", "Check the code"],
  ["arg.check_code.code", "Verify", "Code"],
  ["line.code_length", "Verify", "Code not 6 digits"],
  ["line.code_wrong", "Verify", "Code does not match"],
  ["tool.resend_code", "Verify", "Send a new code"],
  ["line.code_resent", "Verify", "New code sent"],
  ["line.no_more_codes", "Verify", "No more codes"],
  ["line.verified", "Verify", "Verified"],
  ["line.locked", "Verify", "Cannot verify"],
  ["line.hand_off_code", "Verify", "Code does not work"],

  ["task.menu", "Visits", "Step 3: what the caller needs"],
  ["tool.find_slots", "Visits", "Find open times"],
  ["arg.find_slots.reason", "Visits", "Reason for the visit"],
  ["arg.find_slots.earliest_day", "Visits", "Earliest day"],
  ["line.slots", "Visits", "Open times"],
  ["line.slot_option", "Visits", "One open time (part)"],
  ["line.no_slots", "Visits", "No open times"],
  ["task.schedule", "Visits", "Step 4: choose and book"],
  ["line.which_option", "Visits", "Which time?"],
  ["tool.choose_option", "Visits", "Read back a time"],
  ["arg.choose_option.option", "Visits", "Option number"],
  ["line.confirm_slot", "Visits", "Confirm the time"],
  ["tool.book", "Visits", "Book the time"],
  ["line.booked", "Visits", "Booked"],
  ["line.slot_taken", "Visits", "Time just taken"],
  ["tool.next_visit", "Visits", "Tell the next visit"],
  ["line.next_visit", "Visits", "Next visit"],
  ["line.visit_type", "Visits", "Visit type (part)"],
  ["line.visit_reason", "Visits", "Visit reason (part)"],
  ["line.visit_online", "Visits", "Online visit (part)"],
  ["line.no_visits", "Visits", "No visits coming up"],
  ["note.after_next_visit", "Visits", "After the next visit"],
  ["tool.change_visit", "Visits", "Change or cancel a visit"],
  ["line.change_visit", "Visits", "Change or cancel a visit"],
  ["line.change_booked", "Visits", "Visit stays booked"],

  ["tool.emergency", "Safety", "Emergency"],
  ["line.emergency", "Safety", "Call 911"],
  ["tool.safety_reminder", "Safety", "911 reminder"],
  ["line.emergency_reminder", "Safety", "911 reminder"],
  ["note.after_reminder", "Safety", "After the 911 reminder"],
  ["tool.medical_question", "Safety", "Medical question"],
  ["line.medical_question", "Safety", "Cannot answer medical questions"],
  ["tool.flag_for_staff", "Safety", "Flag for a nurse"],
  ["arg.flag_for_staff.kind", "Safety", "Kind of flag"],
  ["arg.flag_for_staff.summary", "Safety", "Summary for the nurse"],
  ["arg.flag_for_staff.evidence", "Safety", "Caller's words for the nurse"],
  ["note.after_flag", "Safety", "After a flag"],
];

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

/** A section's name on this page, for another page's links ("Used in"). */
export const sectionName = (key: string) => SECTIONS.find(([k]) => k === key)?.[2] ?? key;

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
const dollars = (x: number) => (x > 0 && x < 0.01 ? "under $0.01" : `$${x.toFixed(2)}`);

export const agentView = (): View => {
  let order: Order = remembered(ORDER_KEY, ["semantic", "flow"] as const, "semantic");
  let tab: Tab = remembered(TAB_KEY, TABS, "General");
  // Set by a click on a tab bar, read by the next drawing: the pill slides from that tab.
  let slideTab: string | undefined;
  let slideOrder: string | undefined;
  // The latest settings, for the Record and test line (it updates without drawing the cards),
  // and for the variables and tools the lists and checks use.
  let latest: PromptSettings | undefined;
  let poll: number | undefined;
  // Another page's ask to show a section (SHOW_SECTION), until the next drawing shows it.
  let pending: string | undefined;
  let redraw: ((toTop?: boolean) => void) | undefined;

  window.addEventListener(SHOW_SECTION, (e) => {
    const key = (e as CustomEvent<string>).detail;
    const found = SECTIONS.find(([k]) => k === key);
    if (!found) return;
    tab = found[1];
    remember(TAB_KEY, tab);
    pending = key;
    redraw?.();
  });
  window.addEventListener(SYSTEM_SAVED, (e) => {
    if (!latest) return;
    latest.variables = (e as CustomEvent<PromptSettings["variables"]>).detail;
    redraw?.();
  });

  return settingsView<Values, Values, PromptSettings>({
    id: "agent",
    title: "Agent",
    load: async (version) => (latest = await promptSettings(version)),
    save: async (_s, d) => (latest = await savePrompt(d)),
    fromValues: (v) => ({ ...v }),
    differing,
    blocked: (d) => {
      if (!latest) return null;
      for (const [key, t, name] of SECTIONS) {
        const p = problem(key, d[key] ?? "", latest);
        if (p) return `${t} › ${name}: ${p}`;
      }
      return null;
    },
    render: (s, d, kit) => render(s, d, kit),
  });

  function render(s: PromptSettings, d: Values, kit: Kit) {
    redraw = kit.rerender;
    // The variables and tools: the latest known (a System save updates them), else this load's.
    const w: Words = latest ?? s;
    const wanted = (key: string) => s.placeholders[key] ?? [];
    const card = el(`<section class="cfg-card prompt-card"></section>`);
    card.dataset.card = "prompt";

    // ---- the tab bar and Semantic | Flow, on one row
    const bars = el(`<div class="prompt-bars"></div>`);
    // The tab chosen before a click, for this one drawing only (a pill slides from it).
    const tabFrom = slideTab;
    const orderFrom = slideOrder;
    slideTab = slideOrder = undefined;
    const tabBar = bar(
      "Section group",
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
    const orderBar = bar(
      "Order",
      [
        ["semantic", "Semantic"],
        ["flow", "Flow"],
      ],
      order,
      (v) => {
        slideOrder = order;
        order = v as Order;
        remember(ORDER_KEY, order);
        kit.rerender(true);
      },
      orderFrom,
    );
    bars.append(tabBar.el, orderBar.el);
    const markBad = () => {
      for (const t of TABS) {
        const bad = SECTIONS.some(([key, st]) => st === t && problem(key, d[key] ?? "", w));
        tabBar.button(t)?.classList.toggle("prompt-tab-bad", bad);
      }
    };
    markBad();

    // ---- one section: its name at the left (as on a Cartesia row), its words at the right.
    // The words are two layers in one place: the text (Read) and the box (Edit), with the same
    // padding, edge width, type and wrapping, so the box fades in over the text without a shift.
    const section = (key: string, name: string) => {
      const kind = kindOf(key);
      const need = wanted(key);
      const r = el(`
        <div class="prompt-section">
          <div class="prompt-label"><span class="prompt-tag"></span><span class="prompt-name"></span><span class="prompt-keeps"></span></div>
          <div class="cfg-control prompt-control">
            <div class="prompt-words"></div>
            <textarea class="cfg-field prompt-text" rows="1" spellcheck="true"></textarea>
          </div>
          <p class="prompt-problem" role="status"></p>
        </div>`);
      r.dataset.key = key;
      r.dataset.kind = kind;
      const tagEl = r.querySelector<HTMLElement>(".prompt-tag")!;
      tagEl.textContent = TAG[kind];
      tagEl.hidden = order !== "flow";
      const nameEl = r.querySelector<HTMLElement>(".prompt-name")!;
      nameEl.textContent = name;
      nameEl.title = key; // the key in the prompt file, on hover only
      const keeps = r.querySelector<HTMLElement>(".prompt-keeps")!;
      keeps.textContent = need.length ? `Keeps ${need.map((p) => `{${p}}`).join(" ")}` : "";
      keeps.hidden = !need.length;
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

    const rows = SECTIONS.filter(([, t]) => t === tab);
    const body = el(`<div class="prompt-sections"></div>`);
    if (order === "semantic") {
      for (const [heading, kinds] of HEADINGS) {
        const these = rows.filter(([key]) => kinds.includes(kindOf(key)));
        if (!these.length) continue;
        const h = el(`<h3 class="prompt-heading"><span></span><span class="prompt-count"></span></h3>`);
        h.firstElementChild!.textContent = heading;
        h.lastElementChild!.textContent = String(these.length);
        body.append(h, ...these.map(([key, , name]) => section(key, name)));
      }
    } else {
      body.append(...rows.map(([key, , name]) => section(key, name)));
    }

    card.append(bars, recordLine(s), body);
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
          const changed = SECTIONS.some(([key, st]) => st === t && diff.has(key));
          tabBar.button(t)?.classList.toggle("prompt-tab-changed", changed);
        }
      },
    };
  }

  /** Record and test, as one quiet line (PROMPT-EDITOR.md §5): whether the kept recordings match
   * the saved words; the quote from a free dry run; the button, a link like Remove on the
   * Cartesia tables; the last run. It updates itself without drawing the cards again. */
  function recordLine(s: PromptSettings) {
    const wrap = el(`<p class="prompt-record"><span class="prompt-record-state"></span><button class="cfg-link" type="button">Record and test</button><span class="prompt-record-result"></span></p>`);
    const stateEl = wrap.querySelector<HTMLElement>(".prompt-record-state")!;
    const button = wrap.querySelector<HTMLButtonElement>("button")!;
    const result = wrap.querySelector<HTMLElement>(".prompt-record-result")!;
    const show = (p: PromptSettings) => {
      const run = p.recording;
      const running = run?.state === "running";
      const quoting = p.estimate?.state === "running";
      stateEl.textContent = p.recordings.match ? "Recordings match the saved words" : "Recordings are out of date";
      stateEl.classList.toggle("prompt-record-old", !p.recordings.match);
      button.disabled = running || quoting || !p.key;
      button.textContent = running ? "Recording" : quoting ? "Working out the cost" : `Record and test${quote(p.estimate, p.recordings)}`;
      button.title = !p.key
        ? "Needs ANTHROPIC_API_KEY in .env"
        : `Records the kept scripts with the saved words (Claude is called only for new lines), then runs the agent's tests`;
      result.textContent = run ? describe(run, p.recordings.scripts) : "";
    };
    const current = latest ?? s;
    show(current);
    // The quote: asked for once per set of words, when the recordings are out of date.
    if (!current.recordings.match && !current.estimate && current.recording?.state !== "running") {
      void estimatePrompt().then((e) => {
        if (latest) latest.estimate = e;
        show(latest ?? s);
        follow(show);
      });
    }
    button.addEventListener("click", async () => {
      try {
        const run = await recordPrompt();
        if (latest) latest.recording = run;
        show(latest ?? s);
        follow(show);
      } catch (e) {
        result.textContent = e instanceof Error ? e.message : "Not started";
      }
    });
    if (current.recording?.state === "running" || current.estimate?.state === "running") follow(show);
    return wrap;
  }

  /** Read the dry run's or the recording's state every 2 seconds until both are done. */
  function follow(show: (p: PromptSettings) => void) {
    if (poll !== undefined) return;
    poll = window.setInterval(async () => {
      try {
        const now = await promptSettings(0);
        if (latest) {
          latest.recording = now.recording;
          latest.recordings = now.recordings;
          latest.estimate = now.estimate;
        }
        show(latest ?? now);
        if (now.recording?.state !== "running" && now.estimate?.state !== "running") {
          window.clearInterval(poll);
          poll = undefined;
        }
      } catch {
        window.clearInterval(poll);
        poll = undefined;
      }
    }, 2000);
  }
}

/** The button's quote: " (3 of 14 scripts, about $0.01)", or nothing new to record. */
function quote(e: Estimate | null, r: PromptSettings["recordings"]): string {
  if (r.match) return "";
  if (!e || e.state !== "done" || e.usd === null) return "";
  if (!e.missing.length) return " (nothing new to record)";
  return ` (${e.missing.length} of ${r.scripts} scripts, about ${dollars(e.usd)})`;
}

function describe(run: Recording, total: number): string {
  const failed = run.scripts.filter((x) => !x.ok).length;
  if (run.state === "running") return `Script ${Math.min(run.scripts.length + 1, total)} of ${total}`;
  const spent = run.spent_usd === null ? "" : `. Spent ${dollars(run.spent_usd)}`;
  if (run.state === "passed") return `Passed: ${total} scripts, the tests pass${spent}`;
  const parts = [];
  if (failed) parts.push(`${failed} ${failed === 1 ? "script" : "scripts"} failed`);
  if (run.tests === false) parts.push("the tests fail");
  return `Failed: ${parts.join(", ") || "see the dev server's log"}${spent}`;
}

/** A text box as tall as its words. */
function grow(box: HTMLTextAreaElement) {
  if (!box.isConnected) return;
  box.style.height = "auto";
  box.style.height = `${box.scrollHeight}px`;
}

