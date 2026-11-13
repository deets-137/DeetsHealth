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
import { placePill, type View } from "./subnav";

type Values = Record<string, string>;
type Kind = "instructions" | "act" | "field" | "hears" | "text";
type Order = "semantic" | "flow";

const TABS = ["General", "Open and close", "Verify", "Visits", "Safety"] as const;
type Tab = (typeof TABS)[number];

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
const SECTIONS: [string, Tab, string][] = [
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
const remembered = <T extends string>(key: string, allowed: readonly T[], fallback: T): T => {
  try {
    const v = localStorage.getItem(key);
    return allowed.includes(v as T) ? (v as T) : fallback;
  } catch {
    return fallback;
  }
};
const remember = (key: string, value: string) => {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Private windows may refuse storage: the page still works, and opens on the default.
  }
};

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

/** What is wrong with a section's words, in the page's words, or null. */
function problem(text: string, wanted: string[]): string | null {
  if (!text.trim()) return "Empty";
  const found = placeholders(text);
  if (!found) return "A { or } with no partner";
  const missing = wanted.filter((p) => !found.has(p));
  const extra = [...found].filter((p) => !wanted.includes(p));
  if (missing.length) return `Needs ${missing.map((p) => `{${p}}`).join(", ")}`;
  if (extra.length) return `${extra.map((p) => `{${p}}`).join(", ")} is not filled in by the agent`;
  return null;
}

/** The words as Read shows them: the same characters as the box, each placeholder marked, so
 * the text sits exactly where the box's text will (no jump when Edit fades the box in). */
function showWords(target: HTMLElement, text: string) {
  target.replaceChildren();
  const re = /\{[a-z_]+(?:[:!][^}]*)?\}/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    if (m.index! > last) target.append(text.slice(last, m.index));
    const mark = el(`<span class="prompt-mark"></span>`);
    mark.textContent = m[0];
    target.append(mark);
    last = m.index! + m[0].length;
  }
  if (last < text.length) target.append(text.slice(last));
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
  // The latest settings, for the Record and test line (it updates without drawing the cards).
  let latest: PromptSettings | undefined;
  let poll: number | undefined;

  return settingsView<Values, Values, PromptSettings>({
    id: "agent",
    title: "Agent",
    load: async (version) => (latest = await promptSettings(version)),
    save: async (_s, d) => (latest = await savePrompt(d)),
    fromValues: (v) => ({ ...v }),
    differing,
    blocked: (d) => {
      for (const [key, t, name] of SECTIONS) {
        const p = problem(d[key] ?? "", latest?.placeholders[key] ?? []);
        if (p) return `${t} › ${name}: ${p}`;
      }
      return null;
    },
    render: (s, d, kit) => render(s, d, kit),
  });

  function render(s: PromptSettings, d: Values, kit: Kit) {
    const wanted = (key: string) => s.placeholders[key] ?? [];
    const card = el(`<section class="cfg-card prompt-card"></section>`);
    card.dataset.card = "prompt";

    // ---- the tab bar and Semantic | Flow, on one row
    const bars = el(`<div class="prompt-bars"></div>`);
    const tabBar = bar("Section group", TABS.map((t) => [t, t]), tab, (v) => {
      tab = v as Tab;
      remember(TAB_KEY, tab);
      kit.rerender(true);
    });
    const orderBar = bar(
      "Order",
      [
        ["semantic", "Semantic"],
        ["flow", "Flow"],
      ],
      order,
      (v) => {
        order = v as Order;
        remember(ORDER_KEY, order);
        kit.rerender(true);
      },
    );
    bars.append(tabBar.el, orderBar.el);
    const markBad = () => {
      for (const t of TABS) {
        const bad = SECTIONS.some(([key, st]) => st === t && problem(d[key] ?? "", wanted(key)));
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
      showWords(words, box.value);
      const showProblem = () => {
        const p = problem(box.value, need);
        err.textContent = p ?? "";
        err.hidden = !p;
        r.classList.toggle("prompt-bad", !!p);
      };
      showProblem();
      box.addEventListener("input", () => {
        d[key] = box.value;
        showWords(words, box.value);
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

/** A tab bar like Read | Edit: a pill slides to the chosen part. */
function bar(label: string, parts: [string, string][], value: string, on: (v: string) => void) {
  const b = el(`<div class="tabs prompt-bar" role="tablist"><span class="nav-highlight tab-pill" aria-hidden="true"></span></div>`);
  b.setAttribute("aria-label", label);
  const pill = b.querySelector<HTMLElement>(".tab-pill")!;
  const buttons = parts.map(([v, word]) => {
    const t = el(`<button class="tab" type="button" role="tab"></button>`) as HTMLButtonElement;
    t.dataset.value = v;
    t.textContent = word;
    t.setAttribute("aria-selected", String(v === value));
    t.addEventListener("click", () => v !== value && on(v));
    b.append(t);
    return t;
  });
  return {
    el: b,
    button: (v: string) => buttons.find((t) => t.dataset.value === v),
    place: () => placePill(pill, buttons.find((t) => t.dataset.value === value), false),
  };
}
