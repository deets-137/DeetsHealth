// Config › System (SYSTEM-PAGE.md §3.6; the owner, 2026-11-13: "grant visibility and
// configurability where possible"): every variable and every tool the agent's words can use, in
// the Config frame (Read | Edit, the versions, Discard | Save). Two tabs:
// - Variables: the System variables (the clinic's own text: a value and a meaning to edit;
//   `{clinic}` built in, the others the analyst's own, added and removed here); the code values
//   (read only: what each is, where it comes from, its section; never a value).
// - Tools: what each tool does, its steps, its arguments, the lines it can speak, and a link to
//   its words on Config › Agent. Read only.
// A save makes a new active version for the next call and writes agent/prompts/VARIABLES.md.
// Every size, color and timing is an existing token: the rows are Config › Agent's sections.
import { saveSystem, systemSettings, type SystemSettings, type SystemValues, type SystemVariable } from "../agent/api";
import { SECTIONS, SHOW_SECTION, SYSTEM_SAVED, sectionName } from "./agent";
import { settingsView, type Kit } from "./config-kit";
import { el } from "./page";
import type { View } from "./subnav";
import { openConfigView, remember, remembered, tabBar } from "./tab-bar";

type Draft = SystemVariable[];
const TABS = ["variables", "tools"] as const;
type Tab = (typeof TABS)[number];
const TAB_KEY = "deets.console.system.tab";
const NAME = /^[a-z][a-z0-9_]*$/;
const STEP_WORDS: Record<string, string> = { every: "Every step", greet: "Greet", verify: "Verify", menu: "Menu", schedule: "Schedule" };
const RECORD_WORDS: Record<string, string> = { reads: "Reads the record", writes: "Writes to the record", none: "" };

/** Show a section on Config › Agent: its tab, the section in view, a flash. */
function goToSection(key: string) {
  window.dispatchEvent(new CustomEvent(SHOW_SECTION, { detail: key }));
  openConfigView("agent");
}

/** Links to sections on Config › Agent, by their names there. */
function sectionLinks(label: string, keys: string[]): HTMLElement {
  const p = el(`<p class="sys-line"><span class="sys-key"></span></p>`);
  p.firstElementChild!.textContent = label;
  if (!keys.length) {
    p.append("None yet");
    return p;
  }
  keys.forEach((key, i) => {
    if (i) p.append(" · ");
    const a = el(`<button class="cfg-link sys-go" type="button"></button>`) as HTMLButtonElement;
    a.textContent = sectionName(key);
    a.title = "Show it on Config › Agent";
    a.addEventListener("click", () => goToSection(key));
    p.append(a);
  });
  return p;
}

function line(label: string, text: string): HTMLElement {
  const p = el(`<p class="sys-line"><span class="sys-key"></span><span></span></p>`);
  p.firstElementChild!.textContent = label;
  p.lastElementChild!.textContent = text;
  return p;
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
  if (v.meaning.length > s.limits.meaning) return `A meaning has at most ${s.limits.meaning} characters`;
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

/** A text box as tall as its words; one line of text only (Enter does not break it). */
function grow(box: HTMLTextAreaElement) {
  if (!box.isConnected) return;
  box.style.height = "auto";
  box.style.height = `${box.scrollHeight}px`;
}

export const systemView = (): View => {
  let tab: Tab = remembered(TAB_KEY, TABS, "variables");
  let slideTab: string | undefined;
  let latest: SystemSettings | undefined;

  return settingsView<SystemValues, Draft, SystemSettings>({
    id: "system",
    title: "System",
    load: async (version) => (latest = await systemSettings(version)),
    save: async (_s, d) => {
      latest = await saveSystem({ variables: d.map(({ name, meaning, value }) => ({ name, meaning, value })) });
      // Config › Agent's `{` list and checks take the new variables at once.
      window.dispatchEvent(new CustomEvent(SYSTEM_SAVED, { detail: latest.active.variables }));
      return latest;
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

  function heading(text: string, count: number): HTMLElement {
    const h = el(`<h3 class="prompt-heading"><span></span><span class="prompt-count"></span></h3>`);
    h.firstElementChild!.textContent = text;
    h.lastElementChild!.textContent = String(count);
    return h;
  }

  /** A row in the shape of a Config › Agent section: the name at the left, the rest at the
   * right. */
  function row(name: string, tags: string[], ...right: HTMLElement[]): HTMLElement {
    const r = el(`
      <div class="prompt-section">
        <div class="prompt-label"><span class="prompt-name"></span></div>
        <div class="sys-right"></div>
        <p class="prompt-problem" role="status" hidden></p>
      </div>`);
    r.querySelector(".prompt-name")!.textContent = name;
    const label = r.querySelector(".prompt-label")!;
    for (const t of tags) {
      const tag = el(`<span class="prompt-keeps"></span>`);
      tag.textContent = t;
      label.append(tag);
    }
    r.querySelector(".sys-right")!.append(...right);
    return r;
  }

  /** One editable line: the text (Read) and the box (Edit) in one place, as on Config › Agent. */
  function field(label: string, value: string, max: number, on: (v: string) => void): HTMLElement {
    const wrap = el(`
      <div class="sys-field">
        <span class="sys-key"></span>
        <div class="cfg-control prompt-control">
          <div class="prompt-words"></div>
          <textarea class="cfg-field prompt-text" rows="1" spellcheck="true"></textarea>
        </div>
      </div>`);
    wrap.querySelector(".sys-key")!.textContent = label;
    const words = wrap.querySelector<HTMLElement>(".prompt-words")!;
    const box = wrap.querySelector<HTMLTextAreaElement>("textarea")!;
    box.value = value;
    box.maxLength = max;
    box.setAttribute("aria-label", label);
    words.textContent = value;
    box.addEventListener("keydown", (e) => e.key === "Enter" && e.preventDefault());
    box.addEventListener("input", () => {
      // A pasted line break becomes a space: a value is one line.
      if (/[\r\n]/.test(box.value)) box.value = box.value.replace(/[\r\n]+/g, " ");
      words.textContent = box.value;
      grow(box);
      on(box.value);
    });
    return wrap;
  }

  function variables(s: SystemSettings, d: Draft, kit: Kit): HTMLElement[] {
    const out: HTMLElement[] = [heading("Clinic", d.length)];
    const note = el(`<p class="sys-note">The clinic's own text. Any section of the agent's words may hold these. Never protected data.</p>`);
    out.push(note);
    for (const v of d) {
      const builtIn = s.built_in.includes(v.name);
      const used = s.used[v.name] ?? [];
      const r = row(`{${v.name}}`, [builtIn ? "Built in" : "Added", `Used in ${used.length} ${used.length === 1 ? "section" : "sections"}`]);
      r.dataset.key = v.name;
      const right = r.querySelector<HTMLElement>(".sys-right")!;
      const err = r.querySelector<HTMLElement>(".prompt-problem")!;
      const check = () => {
        const p = problem(v, d, s);
        err.textContent = p ?? "";
        err.hidden = !p;
        r.classList.toggle("prompt-bad", !!p);
      };
      right.append(
        field("Value", v.value, s.limits.value, (x) => ((v.value = x), check(), kit.changed())),
        field("Meaning", v.meaning, s.limits.meaning, (x) => ((v.meaning = x), check(), kit.changed())),
        sectionLinks("Used in", used),
      );
      if (!builtIn) {
        // Removing one the words still use is refused on save, with the section named.
        const remove = el(`<div class="cfg-control sys-remove"><button class="cfg-link" type="button">Remove</button></div>`);
        remove.querySelector("button")!.addEventListener("click", () => {
          d.splice(d.indexOf(v), 1);
          kit.changed();
          kit.rerender();
        });
        right.append(remove);
      }
      check();
      out.push(r);
    }
    out.push(addRow(s, d, kit));

    const own = s.code_values.filter((c) => !c.from_call);
    const call = s.code_values.filter((c) => c.from_call);
    out.push(heading("Filled by the code", own.length));
    out.push(...own.map(codeRow));
    out.push(heading("From the call", call.length));
    out.push(el(`<p class="sys-note">Each is filled by the code in its own section only, during a call. Never shown here.</p>`));
    out.push(...call.map(codeRow));
    return out;
  }

  function codeRow(c: SystemSettings["code_values"][number]): HTMLElement {
    return row(
      `{${c.name}}`,
      c.protected ? ["Protected"] : [],
      line("", c.meaning),
      line("From", c.source),
      sectionLinks("Used in", c.sections),
    );
  }

  /** Add a variable (Edit only): a name, a meaning and a value, then Add. */
  function addRow(s: SystemSettings, d: Draft, kit: Kit): HTMLElement {
    const wrap = el(`
      <div class="prompt-section sys-add">
        <div class="prompt-label"><span class="prompt-name">Add a variable</span></div>
        <div class="sys-right">
          <div class="cfg-control sys-add-fields">
            <input class="cfg-field" type="text" placeholder="name" aria-label="Name" spellcheck="false" />
            <input class="cfg-field" type="text" placeholder="Meaning" aria-label="Meaning" />
            <input class="cfg-field" type="text" placeholder="Value" aria-label="Value" />
            <button class="cfg-button" type="button">Add</button>
          </div>
        </div>
        <p class="prompt-problem" role="status" hidden></p>
      </div>`);
    const [name, meaning, value] = [...wrap.querySelectorAll<HTMLInputElement>("input")];
    name.maxLength = s.limits.name;
    meaning.maxLength = s.limits.meaning;
    value.maxLength = s.limits.value;
    const err = wrap.querySelector<HTMLElement>(".prompt-problem")!;
    const add = () => {
      const v = { name: name.value.trim(), meaning: meaning.value.trim(), value: value.value.trim() };
      const p = d.length >= s.limits.count ? `At most ${s.limits.count} variables` : problem(v, [...d, v], s);
      err.textContent = p ?? "";
      err.hidden = !p;
      if (p) return;
      d.push(v);
      kit.changed();
      kit.rerender();
    };
    wrap.querySelector("button")!.addEventListener("click", add);
    for (const x of [name, meaning, value]) x.addEventListener("keydown", (e) => e.key === "Enter" && add());
    return wrap;
  }

  function tools(s: SystemSettings): HTMLElement[] {
    const every = s.tools.filter((t) => t.steps.includes("every"));
    const some = s.tools.filter((t) => !t.steps.includes("every"));
    const out: HTMLElement[] = [
      el(`<p class="sys-note">In the agent's words, type a space and a period to name a tool.</p>`),
      heading("In every step", every.length),
      ...every.map(toolRow),
      heading("In some steps", some.length),
      ...some.map(toolRow),
    ];
    return out;
  }

  function toolRow(t: SystemSettings["tools"][number]): HTMLElement {
    const words = [`tool.${t.name}`, ...t.args.map((a) => `arg.${t.name}.${a}`)].filter((k) => SECTIONS.some(([x]) => x === k));
    const r = row(
      t.name,
      RECORD_WORDS[t.record] ? [RECORD_WORDS[t.record]] : [],
      line("", t.does),
      line("Steps", t.steps.map((x) => STEP_WORDS[x] ?? x).join(", ")),
      line("Fields", t.args.length ? t.args.join(", ") : "None"),
      sectionLinks("Says", t.lines),
      sectionLinks("Its words", words),
    );
    r.querySelector(".prompt-name")!.classList.add("prompt-tool");
    return r;
  }
};
