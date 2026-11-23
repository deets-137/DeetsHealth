// Config › Voice (VOICE-VENDORS.md §2.5, the owner's V-3 A; CONSOLE.md §7): one page for the
// voice. Two cards side by side, Hearing and Speaking, each with its Vendor first; the vendor's
// own levers below it. Every vendor, model, range, default, rate and recommended voice comes
// from the voice catalog (the `deetshealth-voice` crate, through ehr); none is written here.
// Only Cartesia is in the catalog so far, so its rows are the only vendor rows built.
//
// The top row: Read | Edit (Read greys every control; Edit opens them), then the versions (the
// owner, 2026-11-13: each saved version is "MM-DD | n", the one the agent runs with is "Active";
// an older one shows read only, its changed values glowing), Discard and Save. Save makes a new
// version in the settings store (CONSOLE.md §7.3) and the active one; it sends every setting of
// every vendor (V-2 A). A key itself never comes here (SECURITY.md §4.6).
//
// Each row (the owner, 2026-11-13): an "i" button at the left that opens the setting's help,
// its name, then its control. No line under the name. The frame, the rows and the controls are
// config-kit.ts, shared with Config › Anthropic.
import * as api from "../agent/api";
import {
  saveVoice,
  type VendorCycle,
  type CartesiaVoice,
  type CartesiaVoices,
  type CatalogLever,
  type CatalogVendor,
  type Range,
  type VoiceSettings,
  type VoiceValues,
} from "../agent/api";
import type { State } from "../agent/store";
import { cartesiaVoices, resources, voiceSettings } from "../agent/stores";
import { cap, settingsView, type Kit } from "./config-kit";
import { row as dtRow, table as dtTable, titleLine } from "./data-table";
import { fillTable, heightsOf } from "./list-window";
import { el } from "./page";
import type { View } from "./subnav";

/** What the page edits: the vendor of each direction, the shared levers, and Cartesia's levers
 * (the only vendor rows built). Null is "the vendor's default". The voice is its id (a name can
 * belong to several voices); null only for a voice from the docs not yet resolved. `raw` is the
 * whole version as read: a save sends it back with the page's changes over it, so every
 * vendor's settings travel, the ones not shown too (V-2 A). */
type Draft = {
  raw: VoiceValues;
  hearingVendor: string;
  speakingVendor: string;
  voice: { name: string; id: string | null };
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
  planPrice: number;
  offset: number;
  cycleDay: number;
};

// The help behind each "i" (the owner's to edit; CONSOLE.md §7.2).
const HELP: Record<string, string> = {
  // Drafts for the owner's words (VOICE-VENDORS.md §2.5).
  hearingVendor:
    "The company whose engine hears the caller and turns speech into words. Each vendor has its own rows below. The choice applies from the next call.",
  speakingVendor:
    "The company whose engine speaks the agent's words. It can differ from the hearing vendor. Each vendor has its own rows below. The choice applies from the next call.",
  voice:
    "Who speaks for the agent. The list is Cartesia's stable voices for agents: Cartesia says they do better on calls than studio or emotive voices. Listen will play one fixed sentence in the voice (about 150 credits, once). Skylar is a draft until you pick one by ear.",
  // A draft for the owner's words (CONSOLE.md §7.8).
  voiceCheck:
    "The menu's voices are checked against Cartesia's own list, read at most every 10 minutes (no credits). A voice Cartesia no longer has is marked and cannot be chosen. More voices opens every English voice Cartesia lists, to search and choose from. Refresh reads the list again now.",
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
  planPrice:
    "What the plan costs a month, in dollars, from Cartesia's billing page. Analytics › Cost turns credits into dollars with it (the price over the plan's credits). 0: not set, and Cost shows Cartesia in credits.",
  left: "The plan's credits, less the offset, less what this machine's voice calls used this cycle. Cartesia's dashboard has the true balance.",
  key: "Never shown here. On this machine it is in .env under the name shown; in the cloud, the secret manager (CONSOLE.md §7.4). A voice call needs the key of each vendor chosen above.",
  apiVersion: "The Cartesia API version. Pipecat fixes it; it moves when Pipecat moves.",
  rates: "What Cartesia charges: Ink, 3 credits a second of audio; Sonic, about 1 credit a character.",
};

const TURN_WORD: Record<string, string> = {
  turn_start_threshold: "Start of a turn",
  turn_eager_end_threshold: "Early end guess",
  turn_end_threshold: "End of a turn",
  turn_end_timeout_ms: "Longest wait (ms)",
};

const TURN_KEYS = ["turn_start_threshold", "turn_eager_end_threshold", "turn_end_threshold", "turn_end_timeout_ms"] as const;

/** Cartesia's lever `key` in a version: `cartesia.<key>` (VOICE-VENDORS.md §2.3). */
const C = (key: string) => `cartesia.${key}`;
const rows = (x: unknown): string[][] => (Array.isArray(x) ? x.map((r) => (Array.isArray(r) ? r.map(String) : [])) : []);
const num = (x: unknown): number | null => (typeof x === "number" ? x : null);

/** The stored values (api.ts › VoiceValues) as the page edits them. */
function fromValues(v: VoiceValues): Draft {
  const voice = (v[C("voice")] ?? {}) as { name?: string; id?: string | null };
  return {
    raw: { ...v },
    hearingVendor: v["hearing.vendor"],
    speakingVendor: v["speaking.vendor"],
    voice: { name: voice.name ?? "", id: voice.id ?? null },
    speed: num(v[C("speed")]),
    volume: num(v[C("volume")]),
    emotion: (v[C("emotion")] as string | null) ?? null,
    normalization: String(v[C("normalization")] ?? "auto"),
    pronunciations: rows(v.pronunciations),
    turn: Object.fromEntries(TURN_KEYS.map((k) => [k, num(v[C(k)])])),
    eager: Boolean(v[C("eager_end_of_turn")]),
    bargeIn: Boolean(v.barge_in),
    keyterms: rows(v.keyterms),
    idleS: Number(v.idle_s),
    maxS: Number(v.max_s),
    planCredits: Number(v[C("plan_credits")]),
    planPrice: Number(v[C("plan_price_usd")] ?? 0),
    offset: Number(v[C("offset")]),
    cycleDay: Number(v[C("cycle_day")]),
  };
}

/** The page's draft as the whole set a save sends: the version as read, with the page's changes
 * over it. The voice carries its id from Cartesia's list; with none (no list held), the dev
 * server finds it by name and checks it. */
function toValues(d: Draft): VoiceValues {
  return {
    ...d.raw,
    "hearing.vendor": d.hearingVendor,
    "speaking.vendor": d.speakingVendor,
    [C("voice")]: { ...d.voice },
    [C("speed")]: d.speed,
    [C("volume")]: d.volume,
    [C("emotion")]: d.emotion,
    [C("normalization")]: d.normalization,
    pronunciations: d.pronunciations,
    ...Object.fromEntries(TURN_KEYS.map((k) => [C(k), d.turn[k] ?? null])),
    [C("eager_end_of_turn")]: d.eager,
    barge_in: d.bargeIn,
    keyterms: d.keyterms,
    idle_s: d.idleS,
    max_s: d.maxS,
    [C("plan_credits")]: d.planCredits,
    [C("plan_price_usd")]: d.planPrice,
    [C("offset")]: d.offset,
    [C("cycle_day")]: d.cycleDay,
  };
}

/** The levers that differ between two drafts: a draft's name, or "turn.<key>" for a turn value.
 * `raw` is the version as read, not a lever: never compared. */
function differing(a: Draft, b: Draft): Set<string> {
  const out = new Set<string>();
  for (const k of Object.keys(a) as (keyof Draft)[]) {
    if (k === "raw") continue;
    if (k === "turn") {
      for (const t of TURN_KEYS) if (a.turn[t] !== b.turn[t]) out.add(`turn.${t}`);
    } else if (JSON.stringify(a[k]) !== JSON.stringify(b[k])) out.add(k);
  }
  return out;
}

/** "Pronunciations, row 2: a cell is empty", or null. Names a list and a row, never a value. */
function blankCell(name: string, rows: string[][]): string | null {
  const i = rows.findIndex((r) => r.some((c) => !c.trim()));
  return i < 0 ? null : `${name}, row ${i + 1}: a cell is empty`;
}

/** The Voice menu's state for the view's life: Cartesia's list as the store holds it, whether
 * the table of every voice is open, and its search. */
type Voices = {
  state: State<CartesiaVoices> | null;
  open: boolean;
  search: HTMLElement;
  box: HTMLInputElement;
  kit: Kit | null;
  /** Draws the table's rows again for the search, without drawing the cards. */
  refill: (() => void) | null;
};

const MORE = "more";

export const voiceView = (): View => {
  let cycle: VendorCycle | undefined;
  const v = voicesState();
  return settingsView<VoiceValues, Draft, VoiceSettings>({
    id: "voice",
    title: "Voice",
    store: voiceSettings,
    save: (_s, d) => saveVoice(toValues(d)),
    fromValues,
    differing,
    // A row added in place starts blank (TABLES.md §3.16): Save waits until each cell has
    // words, so an empty word never reaches Cartesia.
    blocked: (d) => blankCell("Pronunciations", d.pronunciations) ?? blankCell("Keyterms", d.keyterms),
    extra: async () => {
      // Cartesia's voice list: its own store, so the settings never wait on Cartesia. Its answer
      // draws the cards again (edits stay); the search box keeps the cursor (it is one box for
      // the view's life).
      cartesiaVoices.subscribe(v.search, (st) => {
        const before = v.state;
        v.state = st;
        if (v.kit && (st.value !== before?.value || st.status !== before?.status)) v.kit.rerender();
      });
      const use = resources(null);
      await use.refresh("open");
      cycle = use.get().value?.vendors.find((x) => x.vendor === "cartesia");
    },
    render: (s, d, kit) => ((v.kit = kit), render(s, d, kit, cycle, v)),
  });
};

/** The search box over the table of every voice (TABLES.md §6.3): one box for the view's life,
 * so a search's redraw never takes the cursor out of it. Ctrl+F goes to it while it shows. */
function voicesState(): Voices {
  const search = el(`
    <div class="cfg-search" role="search">
      <input class="cfg-field cfg-search-box" type="search" spellcheck="false" autocomplete="off"
        placeholder="Search voices" aria-label="Search voices" aria-keyshortcuts="Control+F Meta+F" />
    </div>`);
  const box = search.querySelector<HTMLInputElement>("input")!;
  box.title = "Ctrl+F. Press Ctrl+F again for the browser's own find. Esc clears.";
  const v: Voices = { state: null, open: false, search, box, kit: null, refill: null };
  let queued = false;
  box.addEventListener("input", () => {
    // One redraw a frame, however fast the typing.
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      v.refill?.();
    });
  });
  box.addEventListener("keydown", (e) => {
    if (e.key !== "Escape") return;
    e.preventDefault();
    if (!box.value) return box.blur();
    box.value = "";
    box.dispatchEvent(new Event("input"));
  });
  document.addEventListener("keydown", (e) => {
    if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey || e.key.toLowerCase() !== "f") return;
    if (!box.isConnected || !box.offsetParent || document.activeElement === box) return;
    e.preventDefault();
    box.focus();
    box.select();
  });
  return v;
}

const GENDER: Record<string, string> = { masculine: "Masculine", feminine: "Feminine", gender_neutral: "Neutral" };

/** "3 min ago", from an ISO time. */
function ago(iso: string): string {
  const m = Math.max(0, Math.round((Date.now() - Date.parse(iso)) / 60_000));
  return m < 1 ? "just now" : m < 60 ? `${m} min ago` : `${Math.round(m / 60)} h ago`;
}

/** A vendor in the catalog. */
const vendorOf = (s: VoiceSettings, id: string): CatalogVendor | undefined => s.catalog.vendors.find((x) => x.id === id);

/** A lever of a vendor's direction (or its account) by key. */
function leverOf(levers: CatalogLever[] | undefined, key: string): CatalogLever {
  const found = levers?.find((l) => l.key === key);
  if (!found) throw new Error(`the catalog has no lever ${key}`);
  return found;
}

/** A number lever as the sliders take it: its range, its step, the vendor's default. */
const rangeOf = (l: CatalogLever): Range => ({ min: l.min ?? 0, max: l.max ?? 1, step: l.step ?? 1, default: Number(l.default ?? l.min ?? 0) });

/** The Voice menu: Cartesia's shortlist for agents (the catalog's), each checked against
 * Cartesia's list (a voice it no longer has is marked and cannot be chosen; a name with several
 * voices shows each with its tagline), the voice in the draft if it is not one of them, then More
 * voices. Each option's value is the voice's id; a docs voice with no id yet (no list held) is
 * "name:". */
function voiceMenu(shortlist: { name: string; id?: string }[], d: Draft, list: CartesiaVoice[] | null) {
  const opts: [string, string][] = [];
  const off = new Set<string>();
  for (const short of shortlist) {
    if (!list) {
      opts.push([short.id ?? `name:${short.name}`, short.name]);
      continue;
    }
    // A voice whose id Cartesia's docs print is matched by that id; the rest by name.
    const hits = short.id
      ? list.filter((x) => x.id === short.id)
      : list.filter((x) => x.name.toLowerCase() === short.name.toLowerCase());
    if (!hits.length) {
      opts.push([`missing:${short.name}`, `${short.name}: not in Cartesia's list`]);
      off.add(`missing:${short.name}`);
    }
    for (const x of hits) opts.push([x.id, hits.length > 1 && x.tagline ? `${x.name}: ${x.tagline}` : x.name]);
  }
  const current = d.voice.id ?? `name:${d.voice.name}`;
  if (!opts.some(([value]) => value === current)) {
    const gone = list && d.voice.id && !list.some((x) => x.id === d.voice.id);
    opts.unshift([current, gone ? `${d.voice.name}: not in Cartesia's list` : d.voice.name]);
  }
  if (list?.length) opts.push([MORE, "More voices…"]);
  return { opts, off, current };
}

/** The table of every English voice Cartesia lists (TABLES.md; §6 for the window and search):
 * Name, Gender, Accent, Tagline, then Use. The "i" opens the description and the id. */
function voicesCard(d: Draft, kit: Kit, v: Voices, list: CartesiaVoice[]) {
  // A new table for each search: a long list's window ends only when its rows leave the page
  // (list-window.ts), so the same body is never filled twice.
  const newTable = () =>
    dtTable("Cartesia's English voices", [
      { head: "Name", kind: "short" },
      { head: "Gender", kind: "choice" },
      { head: "Accent", kind: "choice" },
      "Tagline",
      { head: "", kind: "choice" },
    ]);
  let t = newTable();
  const close = el(`<button class="cfg-button" type="button">Close</button>`);
  close.addEventListener("click", () => {
    v.open = false;
    kit.rerender();
  });
  // The card: its title and count, the search and Close at the right (TABLES.md §3: the title
  // line), then the table.
  const c = el(`<section class="cfg-card" data-card="voices"></section>`);
  const line = titleLine("Cartesia's English voices", "", null, v.search, close);
  const count = line[0].querySelector<HTMLElement>(".dt-count")!;
  c.append(...line, t.wrap);
  const fill = () => {
    const terms = v.box.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const rows = terms.length
      ? list.filter((x) => {
          const words = `${x.name} ${x.tagline} ${x.description} ${x.accent ?? ""} ${x.gender ? GENDER[x.gender] : ""}`.toLowerCase();
          return terms.every((w) => words.includes(w));
        })
      : list;
    const next = newTable();
    next.wrap.dataset.empty = terms.length ? "No match" : "None yet";
    count.textContent = terms.length ? `${rows.length} of ${list.length}` : String(list.length);
    t.wrap.replaceWith(next.wrap);
    t = next;
    const draw = (x: CartesiaVoice) => {
      const chosen = x.id === d.voice.id;
      const use = el(`<div class="cfg-control"><button class="cfg-button" type="button"></button></div>`);
      const b = use.querySelector<HTMLButtonElement>("button")!;
      b.textContent = chosen ? "In use" : "Use";
      if (chosen) b.dataset.off = "";
      b.setAttribute("aria-label", chosen ? `${x.name} is the voice` : `Use ${x.name}`);
      b.addEventListener("click", () => {
        d.voice = { name: x.name, id: x.id };
        kit.changed();
        kit.rerender();
      });
      const facts: [string, string][] = [];
      if (x.description) facts.push(["About", x.description]);
      if (x.locales.length > 1) facts.push(["Accents", x.locales.join(", ")]);
      facts.push(["Id", x.id]);
      return dtRow(x, x.name, [x.name, x.gender ? GENDER[x.gender] : "", x.accent ?? "", x.tagline, use], facts);
    };
    fillTable(
      t,
      rows.map((x) => ({ item: x, draw: () => draw(x) })),
      { heights: heightsOf("cartesia-voices"), drawn: (nodes) => kit.apply(nodes) },
    );
    kit.apply([t.body]);
  };
  v.refill = () => (t.wrap.isConnected ? fill() : undefined);
  fill();
  return c;
}

function render(s: VoiceSettings, d: Draft, kit: Kit, cycle: VendorCycle | undefined, v: Voices) {
  const { row, keyed: k, fixed, select, slider, number, pair, table, card } = kit;
  const n = (x: number) => Math.round(x).toLocaleString("en-US");

  // The catalog's Cartesia: its models, ranges, profiles, voices and rates.
  const cart = vendorOf(s, "cartesia");
  if (!cart?.hearing || !cart.speaking) throw new Error("the catalog has no Cartesia");
  const ink = cart.hearing;
  const sonic = cart.speaking;
  const turnRange = (key: string) => rangeOf(leverOf(ink.levers, key));

  // Each direction's Vendor menu: every vendor in the catalog that offers it. A change draws the
  // cards again, since each vendor has its own rows.
  const vendorRow = (dir: "hearing" | "speaking") => {
    const field = dir === "hearing" ? "hearingVendor" : "speakingVendor";
    const offering = s.catalog.vendors.filter((x) => x[dir]).map((x): [string, string] => [x.id, x.name]);
    const menu = select(offering, d[field], (id) => {
      d[field] = id;
      kit.rerender();
    });
    return k(field, row("Vendor", HELP[field], menu));
  };
  // A vendor whose rows the page does not build yet (none today: Cartesia is the only one).
  const notBuilt = (id: string) => row("Settings", "", fixed(`${vendorOf(s, id)?.name ?? id}: its rows are not built yet`));

  const listen = el(`<button class="cfg-button" type="button" data-off disabled>Listen</button>`);
  listen.title = "Not built yet";
  const held = v.state?.value ?? null;
  // An empty list is never a check: it would grey every name (the server calls it a fault too).
  const list = held && held.checked_at && held.voices.length ? held.voices : null;
  const menu = voiceMenu(sonic.voices ?? [], d, list);
  const voiceSelect = select(menu.opts, menu.current, (value) => {
    if (value === MORE) {
      v.open = true;
      voiceSelect.value = menu.current;
      kit.rerender();
      requestAnimationFrame(() => v.box.focus());
      return;
    }
    const found = list?.find((x) => x.id === value) ?? (sonic.voices ?? []).find((x) => x.id === value);
    const name = value.startsWith("name:") ? value.slice(5) : (found?.name ?? d.voice.name);
    d.voice = { name, id: value.startsWith("name:") ? null : value };
  });
  for (const o of voiceSelect.options) if (menu.off.has(o.value)) o.disabled = true;

  // Where the menu's list came from, and Refresh.
  const st = v.state;
  const checked =
    !st || (st.status === "loading" && !held)
      ? "Checking with Cartesia"
      : !held
        ? st.status === "refused"
          ? "Not checked: the agent's dev server refused"
          : "Not checked: the agent's dev server did not answer"
        : !held.key
          ? "Not checked: no Cartesia key in .env"
          : held.fault && held.checked_at
            ? `Checked ${ago(held.checked_at)}; the last read failed (${held.fault})`
            : held.fault
              ? `Not checked: ${held.fault}`
              : `Checked ${ago(held.checked_at!)}: ${n(held.voices.length)} English voices`;
  const refresh = el(`<button class="cfg-button" type="button">Refresh</button>`) as HTMLButtonElement;
  refresh.addEventListener("click", () => {
    refresh.disabled = true;
    void cartesiaVoices.write(() => api.voiceVoices("cartesia", true)).catch(() => kit.rerender());
  });
  if (held && !held.key) refresh.dataset.off = "";

  const pronunciations = k("pronunciations", row("Pronunciations", HELP.pronunciations, table(d.pronunciations, ["Word", "Say it like"])));
  const speakingProduct = vendorOf(s, d.speakingVendor)?.speaking?.product ?? d.speakingVendor;
  const voice = d.speakingVendor !== "cartesia"
    ? card(`Speaking (${speakingProduct})`, "voice", vendorRow("speaking"), notBuilt(d.speakingVendor), pronunciations)
    : card(
    `Speaking (${sonic.product})`,
    "voice",
    vendorRow("speaking"),
    row("Model", HELP.ttsModel, fixed(sonic.model)),
    k("voice", row("Voice", HELP.voice, voiceSelect, listen)),
    row("Voice list", HELP.voiceCheck, fixed(checked), refresh),
    k("speed", row("Speed", HELP.speed, slider(rangeOf(leverOf(sonic.levers, "speed")), d.speed, (v) => (d.speed = v)))),
    k("volume", row("Volume", HELP.volume, slider(rangeOf(leverOf(sonic.levers, "volume")), d.volume, (v) => (d.volume = v)))),
    k("emotion", row(
      "Emotion",
      HELP.emotion,
      select(
        [["", "Cartesia's default"], ...(leverOf(sonic.levers, "emotion").words ?? []).map((e): [string, string] => [e, cap(e)])],
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
    pronunciations,
  );

  // The turn profile is a name for one set of the four turn values (Cartesia's docs), not a
  // setting of its own: choosing one moves the sliders, and moving a slider shows the profile
  // the four now match, or Custom.
  const profiles = ink.profiles ?? {};
  const valueOf = (key: string) => d.turn[key] ?? turnRange(key).default;
  const matching = () =>
    Object.keys(profiles).find((p) =>
      Object.entries(profiles[p]).every(([key, x]) => Math.abs(valueOf(key) - x) < turnRange(key).step / 2),
    ) ?? "custom";
  const profileSelect = select(
    [
      ...Object.keys(profiles).map((p): [string, string] => [p, p === "balanced" ? "Balanced (Cartesia's default)" : cap(p)]),
      ["custom", "Custom"],
    ],
    matching(),
    (p) => {
      const values = profiles[p];
      if (!values) return;
      for (const [key, x] of Object.entries(values)) {
        const r = turnRange(key);
        d.turn[key] = Math.abs(x - r.default) < r.step / 2 ? null : x;
        turnSliders[key]?.set?.(d.turn[key]);
      }
    },
  );
  const custom = profileSelect.querySelector<HTMLOptionElement>('option[value="custom"]')!;
  const showProfile = () => {
    profileSelect.value = matching();
    custom.disabled = profileSelect.value !== "custom"; // Custom is shown, never chosen
  };
  const turnSliders: Record<string, ReturnType<typeof slider>> = {};
  const turnRow = (key: string) => {
    turnSliders[key] = slider(turnRange(key), d.turn[key], (v) => ((d.turn[key] = v), showProfile()));
    return k(`turn.${key}`, row(TURN_WORD[key] ?? key, HELP[key] ?? "", turnSliders[key]));
  };
  // The shared rows of hearing: one value whichever vendor hears.
  const bargeIn = k("bargeIn", row("Barge-in", HELP.bargeIn, pair(d.bargeIn, (v) => (d.bargeIn = v), "Barge-in")));
  const keyterms = k("keyterms", row("Keyterms", HELP.keyterms, table(d.keyterms, ["Word or phrase"])));
  const hearingProduct = vendorOf(s, d.hearingVendor)?.hearing?.product ?? d.hearingVendor;
  // In the order a turn happens: it starts, it ends, the longest wait; then the early guess
  // beside its own switch; then barge-in and the words Ink listens for.
  const hearing = d.hearingVendor !== "cartesia"
    ? card(`Hearing (${hearingProduct})`, "hearing", vendorRow("hearing"), notBuilt(d.hearingVendor), bargeIn, keyterms)
    : card(
    `Hearing (${ink.product})`,
    "hearing",
    vendorRow("hearing"),
    row("Model", HELP.sttModel, fixed(ink.model)),
    k("profile", row("Turn profile", HELP.profile, profileSelect)),
    turnRow("turn_start_threshold"),
    turnRow("turn_end_threshold"),
    turnRow("turn_end_timeout_ms"),
    k("eager", row("Early end of turn", HELP.eager, pair(d.eager, (v) => (d.eager = v), "Early end of turn"))),
    turnRow("turn_eager_end_threshold"),
    bargeIn,
    keyterms,
  );
  if (d.hearingVendor === "cartesia") showProfile();

  const guard = card(
    "Test-call guard",
    "guard",
    k("idleS", row("Silence (s)", HELP.idleS, number(d.idleS, 10, 600, (v) => (d.idleS = v)))),
    k("maxS", row("Longest call (s)", HELP.maxS, number(d.maxS, 60, 3600, (v) => (d.maxS = v)))),
  );

  const left = fixed("");
  const showLeft = () => {
    left.textContent = cycle ? n(d.planCredits - d.offset - cycle.used) : "Not read";
  };
  showLeft();
  const minus = el(`<span class="cfg-minus" aria-hidden="true">−</span>`);
  const usage = card(
    `Usage (${cart.name})`,
    "usage",
    k("planCredits", row(
      "Plan credits",
      HELP.planCredits,
      number(d.planCredits, 0, 1_000_000_000, (v) => ((d.planCredits = v), showLeft()), "Plan credits"),
      minus,
      number(d.offset, 0, 1_000_000_000, (v) => ((d.offset = v), showLeft()), "Offset"),
    )),
    k("cycleDay", row("Cycle day", HELP.cycleDay, number(d.cycleDay, 1, 28, (v) => (d.cycleDay = v)))),
    k("planPrice", row("Plan price ($ a month)", HELP.planPrice, number(d.planPrice, 0, 100_000, (v) => (d.planPrice = v), "Plan price", 0.01))),
    row("Credits left", HELP.left, left),
  );

  // One key row for each vendor chosen now (one row when both directions use the same vendor).
  const chosen = [...new Set([d.hearingVendor, d.speakingVendor])].map((id) => vendorOf(s, id)).filter((x) => x !== undefined);
  const keyRows = chosen.map((x) =>
    row(`${x.name} API key`, HELP.key, fixed(s.keys[x.id] ? "Set" : `Not set: add ${x.key_env} to .env`)),
  );
  const per = (r: { unit: string; per: string; amount: number }) => `${r.amount} ${r.amount === 1 ? r.unit.replace(/s$/, "") : r.unit} per ${r.per}`;
  const facts = card(
    "Keys and facts",
    "facts",
    ...keyRows,
    row("API version", HELP.apiVersion, fixed(s.api_versions.cartesia ?? "")),
    // The owner's words (2026-11-13): "Ink: 3 credits per second | Sonic: 1 credit per character".
    row("Rates", HELP.rates, fixed(`${ink.product}: ${per(ink.rate)} | ${sonic.product}: ${per(sonic.rate)}`)),
  );

  // Speaking and Hearing side by side when there is room, the rest below them (config.css ›
  // .cfg-body). On an older version the profile glows with any turn value; Plan credits with the
  // offset beside it.
  const all = v.open && list?.length && d.speakingVendor === "cartesia" ? [voicesCard(d, kit, v, list)] : [];
  return {
    cards: [voice, ...all, hearing, guard, usage, facts],
    layout: all.length ? "voice-list" : "voice",
    glow: (diff: Set<string>) => {
      if ([...diff].some((x) => x.startsWith("turn."))) diff.add("profile");
      if (diff.has("offset")) diff.add("planCredits");
    },
  };
}
