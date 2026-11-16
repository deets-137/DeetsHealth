// Config › System (SYSTEM-PAGE.md §3.6; the owner, 2026-11-13: "grant visibility and
// configurability where possible"): every variable and every tool the agent's words can use, in
// the Config frame (Read | Edit, the versions, Discard | Save). Two tabs, each a set of tables
// (the owner, 2026-11-13: "use tables where there are multiple rows of things"):
// - Variables: the clinic's variables (a value and what it is, to edit; `{clinic}` built in,
//   the others the analyst's own, added and removed here); the code values (read only: what each
//   is, where it comes from; never a value).
// - Tools: what each tool does, the name to type in the words, and its steps. Read only.
// Each row's title says what it is; the rest is under its "i", as on every Config row: the
// sections that use it, its fields, the lines it can speak, links to Config › Agent.
// A save makes a new active version for the next call and writes agent/prompts/VARIABLES.md.
// Every size, color and timing is an existing token.
import { saveSystem, type SystemSettings, type SystemValues, type SystemVariable } from "../agent/api";
import { systemSettings } from "../agent/stores";
import { SHOW_SECTION, SYSTEM_SAVED, hasSection, sectionName, useGuide, type GuideEntry } from "./agent";
import { settingsView, type Kit } from "./config-kit";
import { addButton, addTo, field, grow, listTools, row, table, titleLine } from "./data-table";
import { el } from "./page";
import type { View } from "./subnav";
import { openConfigView, remember, remembered, tabBar } from "./tab-bar";

type Draft = SystemVariable[];
type Code = SystemSettings["code_values"][number];
type Tool = SystemSettings["tools"][number];
const TABS = ["variables", "tools"] as const;
type Tab = (typeof TABS)[number];
const TAB_KEY = "deets.console.system.tab";
const NAME = /^[a-z][a-z0-9_]*$/;
const STEP_WORDS: Record<string, string> = { every: "Every step", greet: "Greet", verify: "Verify", menu: "Menu", schedule: "Schedule" };
const RECORD_WORDS: Record<string, string> = { reads: "Reads the patient's record", writes: "Writes to the patient's record", none: "Does not touch the record" };

/** Show a section on Config › Agent: its tab, the section in view, a flash. */
function goToSection(key: string) {
  window.dispatchEvent(new CustomEvent(SHOW_SECTION, { detail: key }));
  openConfigView("agent");
}

/** Links to sections on Config › Agent, by their titles there; "None" when there are none. */
function sectionLinks(keys: string[]): HTMLElement {
  const span = el(`<span class="sys-links"></span>`);
  if (!keys.length) {
    span.textContent = "None";
    return span;
  }
  keys.forEach((key, i) => {
    if (i) span.append(" · ");
    const a = el(`<button class="cfg-link" type="button"></button>`) as HTMLButtonElement;
    a.textContent = sectionName(key);
    a.title = "Show it on Config › Agent";
    a.addEventListener("click", () => goToSection(key));
    span.append(a);
  });
  return span;
}

/** What is wrong with one variable, in the page's words, or null (SYSTEM-PAGE.md §3.2, as
 * prompts.py › check_variables and ehr's settings.rs › variables read it). */
function problem(v: SystemVariable, all: Draft, s: SystemSettings): string | null {
  if (!NAME.test(v.name)) return "A name holds only a to z, 0 to 9 and _, and starts with a letter";
  if (v.name.length > s.limits.name) return `A name has at most ${s.limits.name} characters`;
  if (s.code_values.some((c) => c.name === v.name)) return `{${v.name}} is filled by the code`;
  if (all.filter((x) => x.name === v.name).length > 1) return `{${v.name}} is there twice`;
  if (!v.value.trim()) return "Needs a value";
  if (/[{}]/.test(v.value)) return "A value holds no { or }";
  if (/[\u0000-\u001f\u007f-\u009f]/.test(v.value) || /[\u0000-\u001f\u007f-\u009f]/.test(v.meaning)) return "One line only";
  if (v.value.length > s.limits.value) return `A value has at most ${s.limits.value} characters`;
  if (v.meaning.length > s.limits.meaning) return `What it is has at most ${s.limits.meaning} characters`;
  return null;
}

function differing(a: Draft, b: Draft): Set<string> {
  const out = new Set<string>();
  const byName = (d: Draft) => new Map(d.map((v) => [v.name, v]));
  const x = byName(a);
  const y = byName(b);
  for (const name of new Set([...x.keys(), ...y.keys()])) {
    const p = x.get(name);
    const q = y.get(name);
    if (!p || !q || p.meaning !== q.meaning || p.value !== q.value) out.add(name);
  }
  return out;
}

/** A cell's title: the name as a chip (`{clinic}`, `book`), so it reads as what is typed. */
export function chip(text: string): HTMLElement {
  const c = el(`<span class="prompt-tool"></span>`);
  c.textContent = text;
  return c;
}

export const systemView = (): View => {
  let tab: Tab = remembered(TAB_KEY, TABS, "variables");
  let slideTab: string | undefined;
  let latest: SystemSettings | undefined;
  /** The variables added on this page and not saved yet: their names are boxes. */
  const fresh = new WeakSet<SystemVariable>();

  return settingsView<SystemValues, Draft, SystemSettings>({
    id: "system",
    title: "System",
    store: systemSettings,
    took: (s) => {
      latest = s;
      // The sections' titles and tabs, for the links to Config › Agent.
      useGuide((s as SystemSettings & { guide?: GuideEntry[] }).guide);
    },
    save: async (_s, d) => {
      const s = await saveSystem({ variables: d.map(({ name, meaning, value }) => ({ name, meaning, value })) });
      // Config › Agent's `{` list and checks take the new variables at once.
      window.dispatchEvent(new CustomEvent(SYSTEM_SAVED, { detail: s.active.variables }));
      return s;
    },
    fromValues: (v) => structuredClone(v.variables ?? []),
    differing,
    blocked: (d) => {
      if (!latest) return null;
      if (d.length > latest.limits.count) return `At most ${latest.limits.count} variables`;
      for (const b of latest.built_in) if (!d.some((v) => v.name === b)) return `{${b}} is built in`;
      for (const v of d) {
        const p = problem(v, d, latest);
        if (p) return `{${v.name}}: ${p}`;
      }
      return null;
    },
    render: (s, d, kit) => render(s, d, kit),
  });

  function render(s: SystemSettings, d: Draft, kit: Kit) {
    const card = el(`<section class="cfg-card prompt-card"></section>`);
    card.dataset.card = "system";
    const from = slideTab;
    slideTab = undefined;
    const bar = tabBar(
      "System",
      [
        ["variables", "Variables"],
        ["tools", "Tools"],
      ],
      tab,
      (v) => {
        slideTab = tab;
        tab = v as Tab;
        remember(TAB_KEY, tab);
        kit.rerender(true);
      },
      from,
    );
    const bars = el(`<div class="prompt-bars"></div>`);
    bars.append(bar.el);
    const body = el(`<div class="prompt-sections"></div>`);
    if (tab === "variables") body.append(...variables(s, d, kit));
    else body.append(...tools(s));
    card.append(bars, body);
    const settle = () => {
      card.querySelectorAll<HTMLTextAreaElement>("textarea").forEach(grow);
      bar.place();
    };
    requestAnimationFrame(settle);
    new ResizeObserver(settle).observe(card);
    return {
      cards: [card],
      layout: "prompt",
      // An older version: the Variables tab shows a dot when a variable changed.
      glow: (diff: Set<string>) => bar.button("variables")?.classList.toggle("prompt-tab-changed", diff.size > 0),
    };
  }

  function variables(s: SystemSettings, d: Draft, kit: Kit): HTMLElement[] {
    const done = () => {
      kit.changed();
      kit.rerender();
    };
    // Add puts a blank variable at the foot of the table, its name a box until Save (TABLES.md
    // §3.7); Save waits until it has a good name and a value.
    const add = addButton("Add a variable", () => {
      if (d.length >= s.limits.count) return;
      const v = { name: "", meaning: "", value: "" };
      fresh.add(v);
      addTo(d, v, done);
    });
    const out = titleLine(
      "Clinic variables",
      d.length,
      "The clinic's own text. Any section of the agent's words may hold these: type { there. Never protected data.",
      add,
    );
    const clinic = table(
      "Clinic variables",
      [{ head: "Variable", kind: "choice" }, "What it is", "Value", { head: "Used in", kind: "short" }],
      { remove: true },
    );
    for (const v of d) {
      const builtIn = s.built_in.includes(v.name);
      const used = s.used[v.name] ?? [];
      const err = el(`<p class="prompt-problem" role="status" hidden></p>`);
      const check = () => {
        const p = problem(v, d, s);
        err.textContent = p ?? "";
        err.hidden = !p;
        tr.classList.toggle("prompt-bad", !!p);
      };
      const valueCell = el(`<div class="sys-cell dt-cell"></div>`);
      valueCell.append(field(`${v.name || "New variable"}: value`, v.value, s.limits.value, (x) => ((v.value = x), check(), kit.changed())), err);
      // A new variable's name is a box until Save; a saved one's name is fixed.
      const name = fresh.has(v)
        ? field("Name, like pharmacy_hours", v.name, s.limits.name, (x) => {
            v.name = x.trim();
            tr.dataset.key = v.name;
            check();
            kit.changed();
          })
        : chip(`{${v.name}}`);
      // Removing one the words still use is refused on save, with the section named.
      const tools = listTools(d, v, `{${v.name}}`, done, { move: false, remove: builtIn ? null : true });
      if (builtIn) tools.querySelector<HTMLElement>('[data-tool="remove"]')!.title = "Built in: it cannot be removed";
      const [tr, help] = row(
        v,
        `{${v.name}}`,
        [
          name,
          field(`${v.name}: what it is`, v.meaning, s.limits.meaning, (x) => ((v.meaning = x), check(), kit.changed())),
          valueCell,
          used.length === 1 ? "1 section" : `${used.length} sections`,
        ],
        [
          ["Kind:", builtIn ? "Built in. Its value can change; it cannot be removed." : "Added on this page."],
          ["Used in:", sectionLinks(used)],
        ],
        tools,
      );
      tr.dataset.key = v.name;
      check();
      clinic.body.append(tr, help);
    }
    out.push(clinic.wrap);

    // `today` is the code's own; the rest come from the caller or the record during a call.
    const own = s.code_values.filter((c) => !c.from_call);
    const call = s.code_values.filter((c) => c.from_call);
    out.push(
      ...titleLine(
        "Code values",
        s.code_values.length,
        "Filled by the code, each in its own sections only, during a call. Read only: a value never shows here.",
      ),
    );
    const code = table("Code values", [{ head: "Variable", kind: "choice" }, "What it is", "Filled from", { head: "Protected data", kind: "short" }]);
    for (const c of [...own, ...call]) code.body.append(...codeRow(c));
    out.push(code.wrap);
    return out;
  }

  function codeRow(c: Code): HTMLTableRowElement[] {
    return row(
      null,
      `{${c.name}}`,
      [chip(`{${c.name}}`), c.meaning, c.source, c.protected ? "Yes" : "No"],
      [["Used in:", sectionLinks(c.sections)]],
    );
  }

  function tools(s: SystemSettings): HTMLElement[] {
    // The tools of some steps first, in the call's order; then the tools of every step.
    const every = s.tools.filter((t) => t.steps.includes("every"));
    const some = s.tools.filter((t) => !t.steps.includes("every"));
    const out = titleLine(
      "Tools",
      s.tools.length,
      "What the agent can do during a call. To name one in the agent's words, type a space and a period.",
    );
    const t = table("Tools", [{ head: "Tool", kind: "choice" }, { head: "Name in the words", kind: "choice" }, "What it does", { head: "Steps", kind: "choice" }]);
    for (const tool of [...some, ...every]) t.body.append(...toolRow(tool));
    out.push(t.wrap);
    return out;
  }

  function toolRow(t: Tool): HTMLTableRowElement[] {
    const words = [`tool.${t.name}`, ...t.args.map((a) => `arg.${t.name}.${a}`)].filter(hasSection);
    return row(
      null,
      sectionName(`tool.${t.name}`),
      [sectionName(`tool.${t.name}`), chip(t.name), t.does, t.steps.map((x) => STEP_WORDS[x] ?? x).join(", ")],
      [
        ["Record:", RECORD_WORDS[t.record] ?? ""],
        ["Fields:", t.args.length ? t.args.map((a) => sectionName(`arg.${t.name}.${a}`)).join(", ") : "None"],
        ["Lines it can speak:", sectionLinks(t.lines)],
        ["Its words:", sectionLinks(words)],
      ],
    );
  }
};
