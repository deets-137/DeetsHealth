// The parts every Config settings view shares (CONSOLE.md §7, §8): the top row (Read | Edit,
// the versions, the status, Discard | Save), the rows (an "i" that opens the setting's help,
// its name, its control), the controls (a fixed value, a select, a slider with a line at the
// default, a number box, Off | On, a table), the cards, and an older version's glow. Each
// provider's view (cartesia.ts, anthropic.ts) gives its own values, help words and cards.
//
// Moved from cartesia.ts on 2026-11-13 without a change of behavior, so Config › Cartesia and
// Config › Anthropic work the same way.
import type { Range, SettingVersion } from "../agent/api";
import type { Store } from "../agent/store";
import { holdReload } from "../reload";
import { addButton, addTo, field as dtField, growAll, listTools, row as dtRow, table as dtTable } from "./data-table";
import { flushWindows } from "./list-window";
import { el } from "./page";
import { placePill, type View } from "./subnav";

/** Each settings view's own hold on the demo's live reload (reload.ts). */
let views = 0;

export type Mode = "read" | "edit";

/** What every settings view reads: the store, the versions, the values shown and the active
 * values (dev_server.py › settings_view). */
export type SettingsBase<V> = {
  store: boolean;
  versions: SettingVersion[];
  version: SettingVersion | null;
  values: V;
  active: V;
};

/** The parts a view's `render` builds its cards with. */
export type Kit = {
  row: (label: string, help: string, ...control: HTMLElement[]) => HTMLElement;
  /** Names a row by the setting it shows, so an older version's changes can glow. */
  keyed: (key: string, r: HTMLElement) => HTMLElement;
  fixed: (text: string) => HTMLElement;
  select: (options: [string, string][], value: string, on: (v: string) => void) => HTMLSelectElement;
  slider: (range: Range, value: number | null, on: (v: number | null) => void) => HTMLElement & { set?: (v: number | null) => void };
  number: (value: number, min: number, max: number, on: (v: number) => void, label?: string, step?: number) => HTMLInputElement;
  pair: (value: boolean, on: (v: boolean) => void, label: string) => HTMLElement;
  table: (rows: string[][], columns: string[]) => HTMLElement;
  card: (title: string, name: string, ...rows: HTMLElement[]) => HTMLElement;
  /** Draw the cards again from the draft: for a change that alters other rows (a model that
   * allows other values). Edits stay. */
  rerender: (toTop?: boolean) => void;
  /** Call after an edit the kit did not make (a text box): Discard and Save follow it. */
  changed: () => void;
  /** Gives controls drawn after the cards (a long list's window, a search's rows) the page's
   * mode: in Read each is off. */
  apply: (nodes: Iterable<Element>) => void;
};

export type Spec<V, D, S extends SettingsBase<V>> = {
  id: string;
  title: string;
  /** The view's store, by version (0: the active one; agent/stores.ts). The kit reads it on
   * open and on a version pick, and sends Save through version 0's `write` (LOADING.md §3). */
  store: (version: number) => Store<S>;
  /** Called with each settings the view takes: the first load, a version, a save. */
  took?: (s: S) => void;
  /** The Save request only; the kit sends it through the store. */
  save: (s: S, d: D) => Promise<S>;
  fromValues: (v: V) => D;
  /** The setting names that differ between two drafts. */
  differing: (a: D, b: D) => Set<string>;
  /** After the first load: anything else the view reads (Cartesia's credits this cycle). */
  extra?: () => Promise<void>;
  /** The cards, and the layout's name (config.css › .cfg-body[data-layout]). `glow` may add
   * rows that glow with others (a profile with its sliders). */
  render: (s: S, d: D, kit: Kit) => { cards: HTMLElement[]; layout: string; glow?: (diff: Set<string>) => void };
  /** Why Save must wait (a section with the wrong placeholders), or null. */
  blocked?: (d: D) => string | null;
  /** A control of the view's own in the title bar, just left of Discard | Save (Config › Agent's
   * Test | cost, the owner, 2026-11-14). Built once; the view keeps it current itself. */
  top?: () => HTMLElement;
};

// The "i": the letter itself, lower case, in the console's one serif face (the owner,
// 2026-11-13: "after a lower case classical serif font").
const INFO = `<span class="i-glyph" aria-hidden="true">i</span>`;

/** The decimals a step needs: 0.05 → 2, 80 → 0. */
/** The most characters a list's cell takes (a word, a phrase, how to say it). */
const LIST_CELL = 200;

export const decimals = (step: number) => (String(step).split(".")[1] ?? "").length;
export const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export function settingsView<V, D, S extends SettingsBase<V>>(spec: Spec<V, D, S>): View {
  return {
    id: spec.id,
    title: spec.title,
    build() {
      const root = el(`
        <div class="cfg" data-mode="read">
          <div class="cfg-top">
            <div class="tabs cfg-mode" role="radiogroup" aria-label="Read or edit">
              <span class="nav-highlight tab-pill" aria-hidden="true"></span>
              <button class="tab" type="button" role="radio" data-mode="read" aria-checked="true">Read</button>
              <button class="tab" type="button" role="radio" data-mode="edit" aria-checked="false">Edit</button>
            </div>
            <select class="cfg-field cfg-version" aria-label="Version"></select>
            <p class="cfg-status" role="status"></p>
            <div class="tabs cfg-actions" role="group" aria-label="Changes">
              <button class="tab" type="button" data-act="discard" disabled>Discard</button>
              <button class="tab cfg-save" type="button" data-act="save" disabled>Save</button>
            </div>
          </div>
          <div class="cfg-frame">
            <div class="cfg-scroll own-scrollbars">
              <p class="empty">Loading</p>
            </div>
          </div>
        </div>`);
      const bar = root.querySelector<HTMLElement>(".cfg-mode")!;
      const pill = bar.querySelector<HTMLElement>(".tab-pill")!;
      const parts = [...bar.querySelectorAll<HTMLButtonElement>("[data-mode]")];
      const status = root.querySelector<HTMLElement>(".cfg-status")!;
      const discard = root.querySelector<HTMLButtonElement>('[data-act="discard"]')!;
      const save = root.querySelector<HTMLButtonElement>('[data-act="save"]')!;
      const versionSelect = root.querySelector<HTMLSelectElement>(".cfg-version")!;
      const scroll = root.querySelector<HTMLElement>(".cfg-scroll")!;
      if (spec.top) root.querySelector(".cfg-top")!.insertBefore(spec.top(), root.querySelector(".cfg-actions"));

      // Read each time the view opens: a person changes a value on purpose (the Chat page's
      // Text bar follows the same rule).
      let mode: Mode = "read";
      const holdKey = `config:${++views}`;
      let settings: S | undefined;
      let saved: D | undefined; // the version shown, as stored
      let draft: D | undefined; // the version shown, with this page's edits
      let active: D | undefined; // the active version, to mark what an older one changed
      let animate = false;
      let saving = false;
      const pairs: (() => void)[] = []; // places each Off | On pill

      /** True when the page shows an older version: read only, its changes glowing. */
      const older = () => !!settings?.version && !settings.version.active;

      // No line of help here (the owner, 2026-11-13: "it's clear to a healthcare analyst"); the
      // status shows only a save's result or a refusal.
      const say = (text: string) => (status.textContent = text);
      const showStatus = () => {
        const n = saved && draft ? spec.differing(saved, draft).size : 0;
        // The demo's live reload waits while this view holds an edit not saved (reload.ts).
        holdReload(holdKey, n > 0);
        // Discard | Save always show (the owner, 2026-11-13); each greys when it has nothing to do.
        discard.disabled = n === 0 || saving;
        const blocked = draft && spec.blocked ? spec.blocked(draft) : null;
        save.disabled = saving || mode !== "edit" || n === 0 || older() || !settings?.store || !!blocked;
        save.title = !settings?.store ? "No settings store: start Postgres (npm run db:start), then restart npm run dev" : (blocked ?? "");
        if (n) say("");
      };
      const showMode = () => {
        root.dataset.mode = mode;
        // An older version is read only: choose Active to edit.
        const edit = parts.find((b) => b.dataset.mode === "edit")!;
        edit.disabled = older();
        edit.title = older() ? "Choose Active to edit" : "";
        parts.forEach((b) => b.setAttribute("aria-checked", String(b.dataset.mode === mode)));
        placePill(pill, parts.find((b) => b.dataset.mode === mode), animate);
        animate = false;
        applyMode(scroll);
        pairs.forEach((p) => p());
        showStatus();
      };
      // Read disables each control, not the whole body, so the "i" buttons still open. A
      // control marked data-off (Listen; a value the model does not allow) stays off in both.
      const CONTROLS = ".cfg-control input, .cfg-control select, .cfg-control button, .cfg-control textarea";
      const applyMode = (root: ParentNode) =>
        root
          .querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLButtonElement>(CONTROLS)
          .forEach((x) => (x.disabled = mode === "read" || x.dataset.off !== undefined));
      const apply = (nodes: Iterable<Element>) => {
        for (const n of nodes) applyMode(n);
        pairs.forEach((p) => p());
      };
      bar.addEventListener("click", (e) => {
        const b = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-mode]");
        if (!b || b.disabled || b.dataset.mode === mode) return;
        mode = b.dataset.mode as Mode;
        animate = true;
        showMode();
      });
      new ResizeObserver(() => showMode()).observe(bar);
      discard.addEventListener("click", () => {
        if (!saved) return;
        draft = structuredClone(saved);
        render();
      });

      // ---- the versions: newest first, Default last (the first version of every
      // implementation: the code's defaults). The one the agent runs with reads
      // "Active | [MM-DD | n]" (the owner, 2026-11-13), the rest "MM-DD | n". With no store, one
      // disabled entry says so.
      const showVersions = () => {
        const s = settings!;
        const opts: [string, string][] = s.versions.map((v) => [String(v.id), v.active ? `Active | [${v.label}]` : v.label]);
        if (!s.store) opts.push(["0", "No settings store"]);
        else if (!s.versions.length) opts.push(["0", "Active | [Default]"]);
        versionSelect.replaceChildren(
          ...opts.map(([value, word]) => {
            const o = document.createElement("option");
            o.value = value;
            o.textContent = word;
            return o;
          }),
        );
        versionSelect.value = s.version ? String(s.version.id) : "0";
        versionSelect.disabled = !s.store || s.versions.length < 2;
        versionSelect.title = s.version ? `Saved ${s.version.created_at.replace("T", " ").replace("Z", " UTC")}` : "";
      };
      const take = (next: S) => {
        spec.took?.(next);
        settings = next;
        active = spec.fromValues(next.active);
        saved = spec.fromValues(next.values);
        draft = structuredClone(saved);
      };
      /** Show one version (0: the active one). Edits not saved are dropped. */
      const load = async (version: number) => {
        const st = spec.store(version);
        await st.refresh("user");
        const got = st.get();
        if (got.status !== "ready" || !got.value) {
          say("That version could not be read");
          if (settings) showVersions();
          return;
        }
        take(got.value);
        if (older()) mode = "read";
        showVersions();
        render();
      };
      versionSelect.addEventListener("change", () => {
        say("");
        void load(Number(versionSelect.value));
      });
      save.addEventListener("click", async () => {
        if (!settings || !draft || save.disabled) return;
        saving = true;
        showStatus();
        say("Saving");
        try {
          const [s, d] = [settings, draft];
          const next = await spec.store(0).write(() => spec.save(s, d));
          take(next);
          mode = "read";
          animate = true;
          showVersions();
          render();
          say(next.version ? `Saved as ${next.version.label}. Active from the next call` : "Saved");
        } catch (e) {
          say(e instanceof Error && e.message ? `Not saved: ${e.message}` : "Not saved");
        } finally {
          saving = false;
          showStatus();
        }
      });

      // Once a frame, not once a key: the check compares the whole draft with the saved one
      // (TABLES.md §6.1).
      let statusQueued = false;
      const changed = () => {
        if (statusQueued) return;
        statusQueued = true;
        requestAnimationFrame(() => {
          statusQueued = false;
          showStatus();
        });
      };

      // ---- the parts of a row

      /** A row: the "i" button, the name, the control; the help opens under them. The "i" stays
       * live in Read: help is reading, not editing. */
      const row = (label: string, help: string, ...control: HTMLElement[]) => {
        const r = el(`
          <div class="cfg-row">
            <button class="cfg-info" type="button" aria-expanded="false">${INFO}</button>
            <span class="cfg-name"></span>
            <div class="cfg-control"></div>
            <div class="cfg-help"><p class="cfg-help-text"></p></div>
          </div>`);
        const info = r.querySelector<HTMLButtonElement>(".cfg-info")!;
        const box = r.querySelector<HTMLElement>(".cfg-help")!;
        r.querySelector(".cfg-name")!.textContent = label;
        info.setAttribute("aria-label", `About ${label}`);
        r.querySelector(".cfg-help-text")!.textContent = help;
        // The help rolls down and up (config.css › .cfg-help: the Chat page's help card's
        // collapse, --dur-collapse); instant with reduced motion.
        info.addEventListener("click", () => {
          const open = !box.classList.contains("open");
          box.classList.toggle("open", open);
          info.setAttribute("aria-expanded", String(open));
        });
        r.querySelector(".cfg-control")!.append(...control);
        return r;
      };
      const keyed = (key: string, r: HTMLElement) => ((r.dataset.key = key), r);
      const fixed = (text: string) => {
        const s = el(`<span class="cfg-fixed"></span>`);
        s.textContent = text;
        return s;
      };
      const select = (options: [string, string][], value: string, on: (v: string) => void) => {
        const s = document.createElement("select");
        s.className = "cfg-field";
        for (const [v, word] of options) {
          const o = document.createElement("option");
          o.value = v;
          o.textContent = word;
          s.append(o);
        }
        s.value = value;
        s.addEventListener("change", () => (on(s.value), changed()));
        return s;
      };
      /** A slider with a line on its track at the provider's default. Null is the default:
       * sliding back onto the line returns to it. The value beside it has a fixed width, so the
       * track never changes size. */
      const slider = (
        range: Range,
        value: number | null,
        on: (v: number | null) => void,
      ): HTMLElement & { set?: (v: number | null) => void } => {
        const wrap = el(`<span class="cfg-slider"><span class="cfg-track"><input type="range" class="cfg-range" /><span class="cfg-mark" aria-hidden="true"></span></span><output class="cfg-value"></output></span>`);
        const input = wrap.querySelector<HTMLInputElement>("input")!;
        const out = wrap.querySelector<HTMLOutputElement>("output")!;
        const places = decimals(range.step);
        input.min = String(range.min);
        input.max = String(range.max);
        input.step = String(range.step);
        wrap.style.setProperty("--at", String((range.default - range.min) / (range.max - range.min)));
        let v = value;
        const show = () => {
          input.value = String(v ?? range.default);
          out.textContent = (v ?? range.default).toFixed(places);
        };
        input.addEventListener("input", () => {
          const x = Number(input.value);
          v = Math.abs(x - range.default) < range.step / 2 ? null : x;
          on(v);
          show();
          changed();
        });
        show();
        // Moved from outside (a turn profile): the value only; the caller records the change.
        return Object.assign(wrap, {
          set: (x: number | null) => {
            v = x;
            show();
          },
        });
      };
      /** A number box. `step`: 1 for whole numbers, 0.01 for dollars and cents. */
      const number = (value: number, min: number, max: number, on: (v: number) => void, label?: string, step = 1) => {
        const input = document.createElement("input");
        input.type = "number";
        input.className = "cfg-field cfg-number";
        input.min = String(min);
        input.max = String(max);
        input.step = String(step);
        const places = decimals(step);
        input.value = places ? value.toFixed(places) : String(value);
        if (label) {
          input.placeholder = label;
          input.setAttribute("aria-label", label);
        }
        const takeValue = (final: boolean) => {
          const raw = Number(input.value);
          if (!final && (input.value === "" || Number.isNaN(raw))) return;
          const stepped = Math.round((raw || min) / step) * step;
          const v = Number(Math.min(max, Math.max(min, stepped)).toFixed(places));
          if (final) input.value = places ? v.toFixed(places) : String(v);
          on(v);
          changed();
        };
        input.addEventListener("input", () => takeValue(false));
        input.addEventListener("change", () => takeValue(true));
        return input;
      };
      /** Off | On, the same bar as Read | Edit. */
      const pair = (value: boolean, on: (v: boolean) => void, label: string) => {
        const p = el(`<div class="tabs cfg-pair" role="radiogroup"><span class="nav-highlight tab-pill" aria-hidden="true"></span><button class="tab" type="button" role="radio" data-on="false">Off</button><button class="tab" type="button" role="radio" data-on="true">On</button></div>`);
        p.setAttribute("aria-label", label);
        const pPill = p.querySelector<HTMLElement>(".tab-pill")!;
        const bs = [...p.querySelectorAll<HTMLButtonElement>("[data-on]")];
        let v = value;
        let slide = false;
        const place = () => {
          bs.forEach((b) => b.setAttribute("aria-checked", String(b.dataset.on === String(v))));
          placePill(pPill, bs.find((b) => b.dataset.on === String(v)), slide);
          slide = false;
        };
        p.addEventListener("click", (e) => {
          const b = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-on]");
          if (!b || b.disabled || mode === "read") return;
          v = b.dataset.on === "true";
          slide = true;
          on(v);
          place();
          changed();
        });
        new ResizeObserver(place).observe(p);
        pairs.push(place);
        return p;
      };
      /** A list as the one table (data-table.ts; TABLES.md §3.16): Add at the top right, level
       * with the row's name; each entry's cells edited in place, and its ✕. The order is not
       * kept, so there is no ↑ ↓. A new entry is blank; the view's `blocked` holds Save until it
       * has words. */
      const table = (rows: string[][], columns: string[]) => {
        const wrap = el(`<div class="cfg-list"><div class="cfg-list-top"></div></div>`);
        const done = () => {
          changed();
          render();
        };
        wrap.querySelector(".cfg-list-top")!.append(addButton(`Add to ${columns.join(", ")}`, () => addTo(rows, columns.map(() => ""), done)));
        const t = dtTable(columns.join(", "), columns, { remove: true });
        for (const r of rows) {
          const cells = columns.map((c, i) =>
            dtField(c, r[i] ?? "", LIST_CELL, (v) => {
              r[i] = v;
              changed();
            }),
          );
          t.body.append(...dtRow(r, r[0] || "A new row", cells, [], listTools(rows, r, r[0] ? `"${r[0]}"` : "this row", done, { move: false })));
        }
        wrap.append(t.wrap);
        return wrap;
      };
      /** A card; `name` places it in the layout (config.css › .cfg-card[data-card]). */
      const card = (title: string, name: string, ...rows: HTMLElement[]) => {
        const c = el(`<section class="cfg-card"><h2></h2></section>`);
        c.querySelector("h2")!.textContent = title;
        c.dataset.card = name;
        c.append(...rows);
        return c;
      };

      const kit: Kit = { row, keyed, fixed, select, slider, number, pair, table, card, rerender: (toTop) => render(toTop), changed, apply };

      // ---- the view

      function render(toTop = false) {
        if (!settings || !draft) return;
        pairs.length = 0;
        const { cards, layout, glow } = spec.render(settings, draft, kit);
        const body = document.createElement("div");
        body.className = "cfg-body";
        body.dataset.layout = layout;
        body.append(...cards);
        // An older version: each value that differs from the active one glows (the owner,
        // 2026-11-13: "highlight the changed values with a gentle glow").
        if (older() && active) {
          const diff = spec.differing(active, draft);
          glow?.(diff);
          body.querySelectorAll<HTMLElement>("[data-key]").forEach((r) => {
            r.classList.toggle("cfg-changed", diff.has(r.dataset.key!));
          });
        }
        // Keep the scroll place when the cards are drawn again (a model change in Edit).
        const top = scroll.scrollTop;
        scroll.replaceChildren(body);
        scroll.scrollTop = toTop ? 0 : top;
        // A long list's rows, near the kept scroll place, before the first frame (TABLES.md §6).
        flushWindows();
        showMode();
        // A table's text boxes as tall as their words (data-table.ts › field), all at once.
        requestAnimationFrame(() => growAll(body));
      }

      // The first load, from the active version's store. Only its first value draws the cards:
      // later values come from this view's own Save and version picks, which draw themselves,
      // so an edit in progress is never drawn over. If the agent is down, the store tries again
      // and the cards draw when it answers (LOADING.md §2, rule 7).
      let first = true;
      spec.store(0).subscribe(scroll, (s) => {
        if (!first) return;
        if (!s.value) {
          if (s.status === "down" || s.status === "refused") {
            scroll.replaceChildren(
              el(`<p class="empty">${s.status === "down" ? "The agent's dev server is not running" : "The agent's dev server refused"}</p>`),
            );
            versionSelect.hidden = true;
          }
          return;
        }
        first = false;
        versionSelect.hidden = false;
        take(s.value);
        void (async () => {
          await spec.extra?.().catch(() => undefined);
          showVersions();
          render();
        })();
      });
      return root;
    },
  };
}
