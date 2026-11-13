// Config › Cartesia (CONSOLE.md §7): every Cartesia lever the agent runs with, its usage, and
// whether the key is set. The top row: Read | Edit (Read greys every control; Edit opens them),
// then the versions (the owner, 2026-11-13: each saved version is "MM-DD | n", the one the agent
// runs with is "Active"; an older one shows read only, its changed values glowing), Discard and
// Save. Save makes a new version in the settings store (CONSOLE.md §7.3) and the active one. The
// key itself never comes here (SECURITY.md §4.6).
//
// Each row (the owner, 2026-11-13): an "i" button at the left that opens the setting's help,
// its name, then its control. No line under the name.
import {
  cartesiaSettings,
  resources,
  saveCartesia,
  type CartesiaCycle,
  type CartesiaSettings,
  type Range,
  type VoiceValues,
} from "../agent/api";
import { el } from "./page";
import { placePill, type View } from "./subnav";

type Mode = "read" | "edit";

/** What the page edits: one value per lever. Null is "Cartesia's default". */
type Draft = {
  voice: string;
  speed: number | null;
  volume: number | null;
  emotion: string | null;
  normalization: string;
  pronunciations: string[][]; // [word, say it like]
  turn: Record<string, number | null>;
  eager: boolean;
  bargeIn: boolean;
  keyterms: string[][]; // [word or phrase]
  idleS: number;
  maxS: number;
  planCredits: number;
  offset: number;
  cycleDay: number;
};

// The help behind each "i" (the owner's to edit; CONSOLE.md §7.2).
const HELP: Record<string, string> = {
  voice:
    "Who speaks for the agent. The list is Cartesia's stable voices for agents: Cartesia says they do better on calls than studio or emotive voices. Listen will play one fixed sentence in the voice (about 150 credits, once). Skylar is a draft until you pick one by ear.",
  speed:
    "How fast the voice speaks, from 0.6 to 1.5. The line on the track marks Cartesia's default (1.0). Slower can help older callers. Applies from the next call.",
  volume: "How loud the voice is, from 0.5 to 2.0. The line on the track marks Cartesia's default (1.0).",
  emotion:
    "A tone for every line: neutral, calm, content, sad or angry. Cartesia's default sends no tone. Calm is worth a listening test.",
  normalization:
    "How Sonic reads numbers, dates and phone numbers. Auto reads them in English. Off reads text exactly as written, so every number must already be spelled out.",
  pronunciations:
    "A word and how to say it, for words Sonic gets wrong: drug names, provider names, the hospital's places. Add an entry only after a real misreading; a fix for one region can be wrong in another. Becomes a Cartesia pronunciation dictionary on save.",
  ttsModel: "The voice engine, pinned by name. Never sonic-latest: a new model changes how words sound, so it moves only after a listening test.",
  profile:
    "A name for one set of the four turn values below; Cartesia sends no profile, only the four values. Choosing one moves the sliders. Balanced is Cartesia's default (0.8, 0.3, 5600 ms, 0.6). Responsive replies sooner, for fast back-and-forth (0.7, 0.4, 4500 ms). Patient waits longer, for callers who pause to think or look things up (0.8, 0.1, 8000 ms, early guess 0.3). Move any slider and the profile reads Custom.",
  turn_start_threshold:
    "How sure Ink must be that the caller has started to speak before it says so, from 0.5 to 0.9. That moment stops the agent's speech. Higher: fewer false stops from a cough or noise, but slower to stop the agent.",
  turn_eager_end_threshold:
    "How sure Ink must be for its early guess that the turn has ended, from 0.3 to 0.8. Used only when Early end of turn is on.",
  turn_end_threshold:
    "How sure Ink must be that the turn has ended, from 0.05 to 0.5. Lower waits for more certainty: fewer callers cut off, slower replies.",
  turn_end_timeout_ms:
    "The longest Ink waits after speech stops before it ends the turn anyway, from 640 to 11,200 ms. A slow speaker keeps the turn; a long pause still ends it.",
  eager:
    "Starts Claude on Ink's early guess that the turn has ended, and drops the reply if the caller goes on. Faster replies, but a guess could start a tool (a text, a booking), and every guess costs Claude tokens. Off until we tune for the 800 ms target.",
  bargeIn:
    "When the caller starts to speak, the agent stops speaking. The 911 line is the one exception: it always plays to its end.",
  keyterms:
    "Words or phrases Ink listens for: up to 100, 1,200 characters in all. Hospital-level lists only (provider names, places, common drugs), never a word from one patient's chart: the list travels in the connection's address.",
  sttModel: "The speech-in engine, pinned by name. ink-2 is Cartesia's stable model.",
  idleS:
    "A voice test call ends after this many seconds with no speech from the caller. It guards the plan's credits: Ink bills every second, silence included. A dev guard only; a real call's rule is FUTURE-SETTINGS.md §12.",
  maxS: "A voice test call ends at this length, in seconds.",
  planCredits:
    "The plan's credits for the cycle (Free: 20,000), then a manual offset: credits used outside these calls, such as the playground or another machine. Credits left subtracts both, and this cycle's voice calls.",
  cycleDay: "The day of the month the plan's credits renew: the day the account was opened.",
  left: "The plan's credits, less the offset, less what this machine's voice calls used this cycle. Cartesia's dashboard has the true balance.",
  key: "Never shown here. On this machine it is CARTESIA_API_KEY in .env; in the cloud, the secret manager (CONSOLE.md §7.4).",
  apiVersion: "The Cartesia API version. Pipecat fixes it; it moves when Pipecat moves.",
  rates: "What Cartesia charges: Ink, 3 credits a second of audio; Sonic, about 1 credit a character.",
};

const TURN_WORD: Record<string, string> = {
  turn_start_threshold: "Start of a turn",
  turn_eager_end_threshold: "Early end guess",
  turn_end_threshold: "End of a turn",
  turn_end_timeout_ms: "Longest wait (ms)",
};
const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
// The "i": the letter itself, lower case, in the console's one serif face (the owner,
// 2026-11-13: "after a lower case classical serif font").
const INFO = `<span class="i-glyph" aria-hidden="true">i</span>`;

const TURN_KEYS = ["turn_start_threshold", "turn_eager_end_threshold", "turn_end_threshold", "turn_end_timeout_ms"] as const;

/** The stored values (api.ts › VoiceValues) as the page edits them. */
function fromValues(v: VoiceValues): Draft {
  return {
    voice: v.voice.name,
    speed: v.speed,
    volume: v.volume,
    emotion: v.emotion,
    normalization: v.normalization,
    pronunciations: v.pronunciations.map((r) => [...r]),
    turn: Object.fromEntries(TURN_KEYS.map((k) => [k, v[k]])),
    eager: v.eager_end_of_turn,
    bargeIn: v.barge_in,
    keyterms: v.keyterms.map((r) => [...r]),
    idleS: v.idle_s,
    maxS: v.max_s,
    planCredits: v.plan_credits,
    offset: v.offset,
    cycleDay: v.cycle_day,
  };
}

/** The page's draft as the whole set a save sends. A voice from Cartesia's docs carries its id;
 * the dev server finds any other's by name. */
function toValues(d: Draft, voices: { name: string; id: string | null }[], current: VoiceValues): VoiceValues {
  const id = d.voice === current.voice.name ? current.voice.id : (voices.find((v) => v.name === d.voice)?.id ?? null);
  return {
    voice: { name: d.voice, id },
    speed: d.speed,
    volume: d.volume,
    emotion: d.emotion,
    normalization: d.normalization,
    pronunciations: d.pronunciations,
    turn_start_threshold: d.turn.turn_start_threshold ?? null,
    turn_eager_end_threshold: d.turn.turn_eager_end_threshold ?? null,
    turn_end_threshold: d.turn.turn_end_threshold ?? null,
    turn_end_timeout_ms: d.turn.turn_end_timeout_ms ?? null,
    eager_end_of_turn: d.eager,
    barge_in: d.bargeIn,
    keyterms: d.keyterms,
    idle_s: d.idleS,
    max_s: d.maxS,
    plan_credits: d.planCredits,
    offset: d.offset,
    cycle_day: d.cycleDay,
  };
}

/** The levers that differ between two drafts: a draft's name, or "turn.<key>" for a turn value. */
function differing(a: Draft, b: Draft): Set<string> {
  const out = new Set<string>();
  for (const k of Object.keys(a) as (keyof Draft)[]) {
    if (k === "turn") {
      for (const t of TURN_KEYS) if (a.turn[t] !== b.turn[t]) out.add(`turn.${t}`);
    } else if (JSON.stringify(a[k]) !== JSON.stringify(b[k])) out.add(k);
  }
  return out;
}

/** The decimals a slider's step needs: 0.05 → 2, 80 → 0. */
const decimals = (step: number) => (String(step).split(".")[1] ?? "").length;

export const cartesiaView = (): View => ({
  id: "cartesia",
  title: "Cartesia",
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

    // Read each time the view opens: a person changes a value on purpose (the Chat page's
    // Text bar follows the same rule).
    let mode: Mode = "read";
    let settings: CartesiaSettings | undefined;
    let cycle: CartesiaCycle | undefined;
    let saved: Draft | undefined; // the version shown, as stored
    let draft: Draft | undefined; // the version shown, with this page's edits
    let active: Draft | undefined; // the active version, to mark what an older one changed
    let animate = false;
    let saving = false;
    const pairs: (() => void)[] = []; // places each Off | On pill
    let showLeft = () => {}; // set by render: the Credits left value

    /** True when the page shows an older version: read only, its changes glowing. */
    const older = () => !!settings?.version && !settings.version.active;

    // No line of help here (the owner, 2026-11-13: "it's clear to a healthcare analyst"); the
    // status shows only a save's result or a refusal.
    const say = (text: string) => (status.textContent = text);
    const showStatus = () => {
      const n = saved && draft ? differing(saved, draft).size : 0;
      // Discard | Save always show (the owner, 2026-11-13); each greys when it has nothing to do.
      discard.disabled = n === 0 || saving;
      save.disabled = saving || mode !== "edit" || n === 0 || older() || !settings?.store;
      save.title = !settings?.store ? "No settings store: start Postgres (npm run db:start), then restart npm run dev" : "";
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
      // Read disables each control, not the whole body, so the "i" buttons and Advanced still
      // open. A control marked data-off (Listen) stays off in both.
      scroll
        .querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLButtonElement>(
          ".cfg-control input, .cfg-control select, .cfg-control button",
        )
        .forEach((x) => (x.disabled = mode === "read" || x.dataset.off !== undefined));
      pairs.forEach((p) => p());
      showStatus();
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

    // ---- the versions: newest first, Default last (the first version of every implementation:
    // the code's defaults). The one the agent runs with reads "Active | [MM-DD | n]" (the owner,
    // 2026-11-13), the rest "MM-DD | n". With no store, one disabled entry says so.
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
    /** Show one version (0: the active one). Edits not saved are dropped. */
    const load = async (version: number) => {
      let next: CartesiaSettings;
      try {
        next = await cartesiaSettings(version);
      } catch {
        say("That version could not be read");
        if (settings) showVersions();
        return;
      }
      settings = next;
      active = fromValues(next.active);
      saved = fromValues(next.values);
      draft = structuredClone(saved);
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
        const next = await saveCartesia(toValues(draft, settings.tts.voices, settings.values));
        settings = next;
        active = fromValues(next.active);
        saved = fromValues(next.values);
        draft = structuredClone(saved);
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

    const changed = () => showStatus();

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
    /** A slider with a line on its track at Cartesia's default. Null is the default: sliding
     * back onto the line returns to it. The value beside it has a fixed width, so the track
     * never changes size. */
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
    const number = (value: number, min: number, max: number, on: (v: number) => void, label?: string) => {
      const input = document.createElement("input");
      input.type = "number";
      input.className = "cfg-field cfg-number";
      input.min = String(min);
      input.max = String(max);
      input.value = String(value);
      if (label) {
        input.placeholder = label;
        input.setAttribute("aria-label", label);
      }
      const take = (final: boolean) => {
        const raw = Number(input.value);
        if (!final && (input.value === "" || Number.isNaN(raw))) return;
        const v = Math.min(max, Math.max(min, Math.round(raw || min)));
        if (final) input.value = String(v);
        on(v);
        changed();
      };
      input.addEventListener("input", () => take(false));
      input.addEventListener("change", () => take(true));
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
    /** The add row (one text box per column, then Add), and under it the entries as a table
     * with a header row and a Remove per entry. No table while it is empty. */
    const table = (rows: string[][], columns: string[]) => {
      const wrap = el(`<div class="cfg-list"><div class="cfg-add"></div><table class="cfg-table"><thead><tr></tr></thead><tbody></tbody></table></div>`);
      const add = wrap.querySelector<HTMLElement>(".cfg-add")!;
      const t = wrap.querySelector<HTMLTableElement>("table")!;
      const head = t.querySelector("tr")!;
      const body = t.querySelector("tbody")!;
      for (const c of [...columns, ""]) {
        const th = document.createElement("th");
        th.scope = "col";
        th.textContent = c;
        head.append(th);
      }
      const draw = () => {
        t.hidden = rows.length === 0;
        body.replaceChildren(
          ...rows.map((cells, i) => {
            const tr = document.createElement("tr");
            for (const c of cells) {
              const td = document.createElement("td");
              td.textContent = c;
              tr.append(td);
            }
            const td = el(`<td class="cfg-remove"><button class="cfg-link" type="button">Remove</button></td>`);
            td.querySelector("button")!.addEventListener("click", () => {
              rows.splice(i, 1);
              draw();
              changed();
            });
            tr.append(td);
            return tr;
          }),
        );
      };
      const inputs = columns.map((c) => {
        const input = document.createElement("input");
        input.type = "text";
        input.className = "cfg-field";
        input.placeholder = c;
        input.setAttribute("aria-label", c);
        return input;
      });
      const button = el(`<button class="cfg-button" type="button">Add</button>`);
      const take = () => {
        const cells = inputs.map((x) => x.value.trim());
        if (cells.some((c) => !c)) return;
        rows.push(cells);
        inputs.forEach((x) => (x.value = ""));
        inputs[0].focus();
        draw();
        changed();
      };
      button.addEventListener("click", take);
      inputs.forEach((x) => x.addEventListener("keydown", (e) => e.key === "Enter" && take()));
      add.append(...inputs, button);
      draw();
      return wrap;
    };
    const card = (title: string, ...rows: HTMLElement[]) => {
      const c = el(`<section class="cfg-card"><h2></h2></section>`);
      c.querySelector("h2")!.textContent = title;
      c.append(...rows);
      return c;
    };

    // ---- the view

    function render() {
      if (!settings || !draft) return;
      const s = settings;
      const d = draft;
      pairs.length = 0;
      const n = (x: number) => Math.round(x).toLocaleString("en-US");

      const listen = el(`<button class="cfg-button" type="button" data-off disabled>Listen</button>`);
      listen.title = "Not built yet";
      /** Names a row by the lever it shows, so an older version's changes can glow. */
      const k = (key: string, r: HTMLElement) => ((r.dataset.key = key), r);
      const voices: [string, string][] = s.tts.voices.map((v) => [v.name, v.name]);
      if (!voices.some(([name]) => name === d.voice)) voices.unshift([d.voice, d.voice]);

      const voice = card(
        "Voice (Sonic)",
        row("Model", HELP.ttsModel, fixed(s.tts.model)),
        k("voice", row("Voice", HELP.voice, select(voices, d.voice, (v) => (d.voice = v)), listen)),
        k("speed", row("Speed", HELP.speed, slider(s.tts.ranges.speed, d.speed, (v) => (d.speed = v)))),
        k("volume", row("Volume", HELP.volume, slider(s.tts.ranges.volume, d.volume, (v) => (d.volume = v)))),
        k("emotion", row(
          "Emotion",
          HELP.emotion,
          select(
            [["", "Cartesia's default"], ...s.tts.emotions.map((e): [string, string] => [e, cap(e)])],
            d.emotion ?? "",
            (v) => (d.emotion = v || null),
          ),
        )),
        k("normalization", row(
          "Number reading",
          HELP.normalization,
          select(
            [
              ["auto", "Auto"],
              ["off", "Off"],
            ],
            d.normalization,
            (v) => (d.normalization = v),
          ),
        )),
        k("pronunciations", row("Pronunciations", HELP.pronunciations, table(d.pronunciations, ["Word", "Say it like"]))),
      );

      // The turn profile is a name for one set of the four turn values (Cartesia's docs), not a
      // setting of its own: choosing one moves the sliders, and moving a slider shows the
      // profile the four now match, or Custom.
      const valueOf = (k: string) => d.turn[k] ?? s.stt.ranges[k].default;
      const matching = () =>
        Object.keys(s.stt.profiles).find((p) =>
          Object.entries(s.stt.profiles[p]).every(([k, x]) => Math.abs(valueOf(k) - x) < s.stt.ranges[k].step / 2),
        ) ?? "custom";
      const profileSelect = select(
        [
          ...Object.keys(s.stt.profiles).map((p): [string, string] => [p, p === "balanced" ? "Balanced (Cartesia's default)" : cap(p)]),
          ["custom", "Custom"],
        ],
        matching(),
        (p) => {
          const values = s.stt.profiles[p];
          if (!values) return;
          for (const [k, x] of Object.entries(values)) {
            const r = s.stt.ranges[k];
            d.turn[k] = Math.abs(x - r.default) < r.step / 2 ? null : x;
            turnSliders[k]?.set?.(d.turn[k]);
          }
        },
      ) as HTMLSelectElement;
      const custom = profileSelect.querySelector<HTMLOptionElement>('option[value="custom"]')!;
      const showProfile = () => {
        profileSelect.value = matching();
        custom.disabled = profileSelect.value !== "custom"; // Custom is shown, never chosen
      };
      const turnSliders: Record<string, ReturnType<typeof slider>> = {};
      const turnRow = (k: string) => {
        turnSliders[k] = slider(s.stt.ranges[k], d.turn[k], (v) => ((d.turn[k] = v), showProfile()));
        const r = row(TURN_WORD[k] ?? k, HELP[k] ?? "", turnSliders[k]);
        r.dataset.key = `turn.${k}`;
        return r;
      };
      // In the order a turn happens: it starts, it ends, the longest wait; then the early guess
      // beside its own switch; then barge-in and the words Ink listens for.
      const hearing = card(
        "Hearing (Ink)",
        row("Model", HELP.sttModel, fixed(s.stt.model)),
        k("profile", row("Turn profile", HELP.profile, profileSelect)),
        turnRow("turn_start_threshold"),
        turnRow("turn_end_threshold"),
        turnRow("turn_end_timeout_ms"),
        k("eager", row("Early end of turn", HELP.eager, pair(d.eager, (v) => (d.eager = v), "Early end of turn"))),
        turnRow("turn_eager_end_threshold"),
        k("bargeIn", row("Barge-in", HELP.bargeIn, pair(d.bargeIn, (v) => (d.bargeIn = v), "Barge-in"))),
        k("keyterms", row("Keyterms", HELP.keyterms, table(d.keyterms, ["Word or phrase"]))),
      );
      showProfile();

      const guard = card(
        "Test-call guard",
        k("idleS", row("Silence (s)", HELP.idleS, number(d.idleS, 10, 600, (v) => (d.idleS = v)))),
        k("maxS", row("Longest call (s)", HELP.maxS, number(d.maxS, 60, 3600, (v) => (d.maxS = v)))),
      );

      const left = fixed("");
      showLeft = () => {
        left.textContent = cycle ? n(d.planCredits - d.offset - cycle.used) : "Not read";
      };
      showLeft();
      const minus = el(`<span class="cfg-minus" aria-hidden="true">−</span>`);
      const usage = card(
        "Usage",
        k("planCredits", row(
          "Plan credits",
          HELP.planCredits,
          number(d.planCredits, 0, 1_000_000_000, (v) => ((d.planCredits = v), showLeft()), "Plan credits"),
          minus,
          number(d.offset, 0, 1_000_000_000, (v) => ((d.offset = v), showLeft()), "Offset"),
        )),
        k("cycleDay", row("Cycle day", HELP.cycleDay, number(d.cycleDay, 1, 28, (v) => (d.cycleDay = v)))),
        row("Credits left", HELP.left, left),
      );

      const facts = card(
        "Key and facts",
        row("API key", HELP.key, fixed(s.key ? "Set" : "Not set: add CARTESIA_API_KEY to .env")),
        row("API version", HELP.apiVersion, fixed(s.api_version)),
        // The owner's words (2026-11-13): "Ink: 3 credits per second | Sonic: 1 credit per character".
        row(
          "Rates",
          HELP.rates,
          fixed(
            `Ink: ${s.rates.ink_per_s} ${s.rates.ink_per_s === 1 ? "credit" : "credits"} per second | ` +
              `Sonic: ${s.rates.sonic_per_char} ${s.rates.sonic_per_char === 1 ? "credit" : "credits"} per character`,
          ),
        ),
      );

      // Each card names its place in the layout (config.css › .cfg-body): Voice and Hearing
      // side by side when there is room, the rest below them.
      voice.dataset.card = "voice";
      hearing.dataset.card = "hearing";
      guard.dataset.card = "guard";
      usage.dataset.card = "usage";
      facts.dataset.card = "facts";
      const body = document.createElement("div");
      body.className = "cfg-body";
      body.append(voice, hearing, guard, usage, facts);
      // An older version: each value that differs from the active one glows (the owner,
      // 2026-11-13: "highlight the changed values with a gentle glow"). The profile glows with
      // any turn value; Plan credits with the offset beside it.
      if (older() && active) {
        const diff = differing(active, d);
        if ([...diff].some((x) => x.startsWith("turn."))) diff.add("profile");
        if (diff.has("offset")) diff.add("planCredits");
        body.querySelectorAll<HTMLElement>(".cfg-row[data-key]").forEach((r) => {
          r.classList.toggle("cfg-changed", diff.has(r.dataset.key!));
        });
      }
      scroll.replaceChildren(body);
      showMode();
    }

    void (async () => {
      try {
        settings = await cartesiaSettings();
      } catch {
        scroll.replaceChildren(el(`<p class="empty">The agent's dev server is not running</p>`));
        versionSelect.hidden = true;
        return;
      }
      cycle = await resources()
        .then((r) => r.cartesia)
        .catch(() => undefined);
      active = fromValues(settings.active);
      saved = fromValues(settings.values);
      draft = structuredClone(saved);
      showVersions();
      render();
    })();
    return root;
  },
});
