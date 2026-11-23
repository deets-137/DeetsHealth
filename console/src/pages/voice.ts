// Config › Voice (VOICE-VENDORS.md §2.5, the owner's V-3 A; CONSOLE.md §7): one page for the
// voice. Two cards side by side, Hearing and Speaking, each with its Vendor first; the chosen
// vendor's own levers below it. Every vendor, model, range, default, rate and recommended voice
// comes from the voice catalog (the `deetshealth-vendors` crate, through ehr); none is written
// here. Two vendors are built: Cartesia (Ink hears, Sonic speaks) and ElevenLabs (Scribe hears,
// Flash speaks; VOICE-VENDORS.md §11). What every vendor has the same way (the voice menu and
// its list, usage, the key, the rates) is drawn once for whichever vendor is chosen.
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
  type VendorVoice,
  type VendorVoices,
  type CatalogLever,
  type CatalogVendor,
  type Range,
  type VoiceSettings,
  type VoiceValues,
} from "../agent/api";
import type { State } from "../agent/store";
import { resources, vendorVoices, voiceSettings } from "../agent/stores";
import { cap, settingsView, type Kit } from "./config-kit";
import { row as dtRow, table as dtTable, titleLine } from "./data-table";
import { fillTable, heightsOf } from "./list-window";
import { el } from "./page";
import type { View } from "./subnav";

/** The vendors whose rows the page builds, and whose voice lists the dev server holds. */
const BUILT = ["cartesia", "elevenlabs"] as const;
type Built = (typeof BUILT)[number];
const isBuilt = (id: string): id is Built => (BUILT as readonly string[]).includes(id);

/** A voice: its id (a name can belong to several voices); null only for a voice from the docs not
 * yet resolved. */
type VoicePick = { name: string; id: string | null };

/** A vendor's account levers (the same four in every catalog file; cost.rs reads them). */
type Account = { planCredits: number; planPrice: number; offset: number; cycleDay: number };

/** ElevenLabs' own levers (`elevenlabs.<key>`). */
type Eleven = {
  voice: VoicePick;
  speed: number | null;
  stability: number | null;
  similarity: number | null;
  style: number | null;
  speakerBoost: boolean;
  normalization: string;
  vadConfidence: number | null;
  vadStart: number | null;
  turnStop: number | null;
  zeroRetention: boolean;
};

/** What the page edits: the vendor of each direction, the shared levers, each built vendor's
 * levers and each one's account. Null is "the vendor's default". `raw` is the whole version as
 * read: a save sends it back with the page's changes over it, so every vendor's settings travel,
 * the ones not shown too (V-2 A). */
type Draft = {
  raw: VoiceValues;
  hearingVendor: string;
  speakingVendor: string;
  // Cartesia's levers.
  voice: VoicePick;
  speed: number | null;
  volume: number | null;
  emotion: string | null;
  normalization: string;
  turn: Record<string, number | null>;
  eager: boolean;
  el: Eleven;
  // Shared.
  pronunciations: string[][]; // [word, say it like]
  bargeIn: boolean;
  keyterms: string[][]; // [word or phrase]
  idleS: number;
  maxS: number;
  accounts: Record<Built, Account>;
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
  speed:
    "How fast the voice speaks, from 0.6 to 1.5. The line on the track marks Cartesia's default (1.0). Slower can help older callers. Applies from the next call.",
  volume: "How loud the voice is, from 0.5 to 2.0. The line on the track marks Cartesia's default (1.0).",
  emotion:
    "A tone for every line: neutral, calm, content, sad or angry. Cartesia's default sends no tone. Calm is worth a listening test.",
  normalization:
    "How Sonic reads numbers, dates and phone numbers. Auto reads them in English. Off reads text exactly as written, so every number must already be spelled out.",
  pronunciations:
    "A word and how to say it, for words the voice gets wrong: drug names, provider names, the hospital's places. Add an entry only after a real misreading; a fix for one region can be wrong in another. One list for every vendor. Not sent to a vendor yet (CONSOLE.md §7.6).",
  ttsModel: "The voice engine, pinned by name. Never a latest alias: a new model changes how words sound, so it moves only after a listening test.",
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
    "Words or phrases the hearing engine listens for: up to 100, 1,200 characters in all. Hospital-level lists only (provider names, places, common drugs), never a word from one patient's chart: the list travels with the connection.",
  sttModel: "The speech-in engine, pinned by name. ink-2 is Cartesia's stable model.",
  idleS:
    "A voice test call ends after this many seconds with no speech from the caller. It guards the plan's credits: the hearing engine bills every second, silence included. A dev guard only; a real call's rule is FUTURE-SETTINGS.md §12.",
  maxS: "A voice test call ends at this length, in seconds.",
  cycleDay: "The day of the month the plan's credits renew: the day the account was opened.",
  key: "Never shown here. On this machine it is in .env under the name shown; in the cloud, the secret manager (CONSOLE.md §7.4). A voice call needs the key of each vendor chosen above.",
  apiVersion: "The Cartesia API version. Pipecat fixes it; it moves when Pipecat moves.",
  // ElevenLabs (drafts for the owner's words; VOICE-VENDORS.md §11).
  elVoice:
    "Who speaks for the agent. The list is ElevenLabs' own made voices that read well on a call; More voices opens every English voice the account can use. A library or cloned voice is left out: the free plan cannot use a library voice through the API, and cloning is outside zero retention. Sarah is a draft until you pick one by ear.",
  elSpeed: "How fast the voice speaks, from 0.7 to 1.2. The line on the track marks ElevenLabs' default (1.0).",
  elStability:
    "How even the voice stays from line to line, from 0 to 1. Higher is steadier and flatter; lower is more expressive and less predictable. ElevenLabs' default is 0.5.",
  elSimilarity:
    "How closely the voice keeps to its original recording, from 0 to 1. Too high can carry the recording's noise. ElevenLabs' default is 0.75.",
  elStyle:
    "How much of the voice's own style it adds, from 0 to 1. Above 0 the voice is slower to start each line. ElevenLabs' default is 0.",
  elSpeakerBoost: "Makes the voice sound more like its original. ElevenLabs' default is on; off can start each line a little sooner.",
  elNormalization:
    "How Flash reads numbers, dates and phone numbers. Auto lets ElevenLabs decide; On always spells them out; Off reads text exactly as written.",
  elPronunciations:
    "A word and how to say it: one list for every vendor. Flash v2.5 reads no phoneme tags (ElevenLabs' docs: only Flash v2, Turbo v2 and English v1 do), so it does not use this list.",
  elTtsModel: "The voice engine, pinned by name: eleven_flash_v2_5, ElevenLabs' fastest. It moves only after a listening test.",
  elSttModel:
    "The speech-in engine, pinned by name: scribe_v2_realtime. Scribe gives words only; Pipecat decides when the caller's turn starts and ends.",
  elTurns:
    "Scribe has no turns of its own. Pipecat's voice detector (Silero) hears when the caller makes sound; the Smart Turn model reads the speech and decides whether the turn is over or the caller only paused. Both run on this machine and cost no credits.",
  vadConfidence:
    "How sure the voice detector must be that a sound is speech, from 0.3 to 0.9. Higher: fewer false starts from a cough or noise, but a soft voice can be missed. Pipecat's default is 0.7.",
  vadStart:
    "How long speech must last before the caller's turn starts and the agent stops, from 0.1 to 0.5 s. Pipecat's default is 0.2 s.",
  turnStop:
    "The longest the agent waits after the caller stops before it ends the turn anyway, from 1 to 8 s, when Smart Turn is not sure. Pipecat's default is 3 s.",
  zeroRetention:
    "On: every request asks ElevenLabs to keep no audio and no text (enable_logging off). Only an Enterprise plan honors it: on this plan ElevenLabs accepts the request and keeps it anyway (tested 2026-11-23). So ElevenLabs takes synthetic calls only, On or Off, until a BAA and an Enterprise plan (VOICE-VENDORS.md §11.7).",
};

/** Each vendor's help for the rows every vendor has. */
const VENDOR_HELP: Record<Built, { voiceCheck: string; planCredits: string; planPrice: string; left: string; rates: string }> = {
  cartesia: {
    voiceCheck:
      "The menu's voices are checked against Cartesia's own list, read at most every 10 minutes (no credits). A voice Cartesia no longer has is marked and cannot be chosen. More voices opens every English voice Cartesia lists, to search and choose from. Refresh reads the list again now.",
    planCredits:
      "The plan's credits for the cycle (Free: 20,000), then a manual offset: credits used outside these calls, such as the playground or another machine. Credits left subtracts both, and this cycle's voice calls.",
    planPrice:
      "What the plan costs a month, in dollars, from Cartesia's billing page. Analytics › Cost turns credits into dollars with it (the price over the plan's credits). 0: not set, and Cost shows Cartesia in credits.",
    left: "The plan's credits, less the offset, less what this machine's voice calls used this cycle. Cartesia's dashboard has the true balance.",
    rates: "What Cartesia charges: Ink, 3 credits a second of audio; Sonic, about 1 credit a character.",
  },
  elevenlabs: {
    voiceCheck:
      "The menu's voices are checked against the account's own list, read at most every 10 minutes (no credits). A voice the account no longer has is marked and cannot be chosen. More voices opens every English voice the account can use. Refresh reads the list again now.",
    planCredits:
      "The plan's credits for the cycle (Free: 10,000; the free plan stops at its limit and never bills more), then a manual offset: credits used outside these calls, such as the website. Credits left subtracts both, and this cycle's voice calls.",
    planPrice:
      "What the plan costs a month, in dollars. 0 on the free plan: Analytics › Cost then shows ElevenLabs in credits.",
    left: "The plan's credits, less the offset, less what this machine's voice calls used this cycle. ElevenLabs' usage page has the true balance.",
    rates:
      "What ElevenLabs charges, from the voice catalog. Flash's rate is the account's own. Scribe's is not confirmed yet: check it against ElevenLabs' usage page after the first call (VOICE-VENDORS.md §11.1).",
  },
};

const TURN_WORD: Record<string, string> = {
  turn_start_threshold: "Start of a turn",
  turn_eager_end_threshold: "Early end guess",
  turn_end_threshold: "End of a turn",
  turn_end_timeout_ms: "Longest wait (ms)",
};

const TURN_KEYS = ["turn_start_threshold", "turn_eager_end_threshold", "turn_end_threshold", "turn_end_timeout_ms"] as const;
const ACCOUNT_KEYS: [keyof Account, string][] = [
  ["planCredits", "plan_credits"],
  ["planPrice", "plan_price_usd"],
  ["offset", "offset"],
  ["cycleDay", "cycle_day"],
];

/** A vendor's lever `key` in a version: `cartesia.<key>` (VOICE-VENDORS.md §2.3). */
const C = (key: string) => `cartesia.${key}`;
const E = (key: string) => `elevenlabs.${key}`;
const rows = (x: unknown): string[][] => (Array.isArray(x) ? x.map((r) => (Array.isArray(r) ? r.map(String) : [])) : []);
const num = (x: unknown): number | null => (typeof x === "number" ? x : null);
const pick = (x: unknown): VoicePick => {
  const v = (x ?? {}) as { name?: string; id?: string | null };
  return { name: v.name ?? "", id: v.id ?? null };
};
const accountOf = (v: VoiceValues, id: Built): Account => ({
  planCredits: Number(v[`${id}.plan_credits`] ?? 0),
  planPrice: Number(v[`${id}.plan_price_usd`] ?? 0),
  offset: Number(v[`${id}.offset`] ?? 0),
  cycleDay: Number(v[`${id}.cycle_day`] ?? 1),
});

/** The stored values (api.ts › VoiceValues) as the page edits them. */
function fromValues(v: VoiceValues): Draft {
  return {
    raw: { ...v },
    hearingVendor: v["hearing.vendor"],
    speakingVendor: v["speaking.vendor"],
    voice: pick(v[C("voice")]),
    speed: num(v[C("speed")]),
    volume: num(v[C("volume")]),
    emotion: (v[C("emotion")] as string | null) ?? null,
    normalization: String(v[C("normalization")] ?? "auto"),
    turn: Object.fromEntries(TURN_KEYS.map((k) => [k, num(v[C(k)])])),
    eager: Boolean(v[C("eager_end_of_turn")]),
    el: {
      voice: pick(v[E("voice")]),
      speed: num(v[E("speed")]),
      stability: num(v[E("stability")]),
      similarity: num(v[E("similarity")]),
      style: num(v[E("style")]),
      speakerBoost: v[E("speaker_boost")] !== false,
      normalization: String(v[E("normalization")] ?? "auto"),
      vadConfidence: num(v[E("vad_confidence")]),
      vadStart: num(v[E("vad_start_secs")]),
      turnStop: num(v[E("turn_stop_secs")]),
      // A version without the lever is on: security first.
      zeroRetention: v[E("zero_retention")] !== false,
    },
    pronunciations: rows(v.pronunciations),
    bargeIn: Boolean(v.barge_in),
    keyterms: rows(v.keyterms),
    idleS: Number(v.idle_s),
    maxS: Number(v.max_s),
    accounts: { cartesia: accountOf(v, "cartesia"), elevenlabs: accountOf(v, "elevenlabs") },
  };
}

/** The page's draft as the whole set a save sends: the version as read, with the page's changes
 * over it. Each voice carries its id from its vendor's list; with none (no list held), the dev
 * server finds it and checks it. */
function toValues(d: Draft): VoiceValues {
  const accounts = Object.fromEntries(
    BUILT.flatMap((id) => ACCOUNT_KEYS.map(([field, key]) => [`${id}.${key}`, d.accounts[id][field]])),
  );
  return {
    ...d.raw,
    "hearing.vendor": d.hearingVendor,
    "speaking.vendor": d.speakingVendor,
    [C("voice")]: { ...d.voice },
    [C("speed")]: d.speed,
    [C("volume")]: d.volume,
    [C("emotion")]: d.emotion,
    [C("normalization")]: d.normalization,
    ...Object.fromEntries(TURN_KEYS.map((k) => [C(k), d.turn[k] ?? null])),
    [C("eager_end_of_turn")]: d.eager,
    [E("voice")]: { ...d.el.voice },
    [E("speed")]: d.el.speed,
    [E("stability")]: d.el.stability,
    [E("similarity")]: d.el.similarity,
    [E("style")]: d.el.style,
    [E("speaker_boost")]: d.el.speakerBoost,
    [E("normalization")]: d.el.normalization,
    [E("vad_confidence")]: d.el.vadConfidence,
    [E("vad_start_secs")]: d.el.vadStart,
    [E("turn_stop_secs")]: d.el.turnStop,
    [E("zero_retention")]: d.el.zeroRetention,
    pronunciations: d.pronunciations,
    barge_in: d.bargeIn,
    keyterms: d.keyterms,
    idle_s: d.idleS,
    max_s: d.maxS,
    ...accounts,
  };
}

/** The levers that differ between two drafts: a draft's name; "turn.<key>" for a Cartesia turn
 * value, "el.<field>" for an ElevenLabs lever, "<vendor>.<field>" for an account lever. `raw` is
 * the version as read, not a lever: never compared. */
function differing(a: Draft, b: Draft): Set<string> {
  const out = new Set<string>();
  const same = (x: unknown, y: unknown) => JSON.stringify(x) === JSON.stringify(y);
  for (const k of Object.keys(a) as (keyof Draft)[]) {
    if (k === "raw") continue;
    if (k === "turn") {
      for (const t of TURN_KEYS) if (a.turn[t] !== b.turn[t]) out.add(`turn.${t}`);
    } else if (k === "el") {
      for (const f of Object.keys(a.el) as (keyof Eleven)[]) if (!same(a.el[f], b.el[f])) out.add(`el.${f}`);
    } else if (k === "accounts") {
      for (const id of BUILT)
        for (const [f] of ACCOUNT_KEYS) if (a.accounts[id][f] !== b.accounts[id][f]) out.add(`${id}.${f}`);
    } else if (!same(a[k], b[k])) out.add(k);
  }
  return out;
}

/** "Pronunciations, row 2: a cell is empty", or null. Names a list and a row, never a value. */
function blankCell(name: string, rows: string[][]): string | null {
  const i = rows.findIndex((r) => r.some((c) => !c.trim()));
  return i < 0 ? null : `${name}, row ${i + 1}: a cell is empty`;
}

/** The Voice menu's state for the view's life: each vendor's list as its store holds it, whether
 * the table of every voice is open, and its search. */
type Voices = {
  states: Partial<Record<Built, State<VendorVoices>>>;
  open: boolean;
  search: HTMLElement;
  box: HTMLInputElement;
  kit: Kit | null;
  /** Draws the table's rows again for the search, without drawing the cards. */
  refill: (() => void) | null;
};

const MORE = "more";

export const voiceView = (): View => {
  let cycles: VendorCycle[] = [];
  const v = voicesState();
  return settingsView<VoiceValues, Draft, VoiceSettings>({
    id: "voice",
    title: "Voice",
    store: voiceSettings,
    save: (_s, d) => saveVoice(toValues(d)),
    fromValues,
    differing,
    // A row added in place starts blank (TABLES.md §3.16): Save waits until each cell has
    // words, so an empty word never reaches a vendor.
    blocked: (d) => blankCell("Pronunciations", d.pronunciations) ?? blankCell("Keyterms", d.keyterms),
    extra: async () => {
      // Each built vendor's voice list: its own store, so the settings never wait on a vendor.
      // An answer draws the cards again (edits stay); the search box keeps the cursor (it is
      // one box for the view's life). A list costs no credits.
      for (const id of BUILT) {
        vendorVoices(id).subscribe(v.search, (st) => {
          const before = v.states[id];
          v.states[id] = st;
          if (v.kit && (st.value !== before?.value || st.status !== before?.status)) v.kit.rerender();
        });
      }
      const use = resources(null);
      await use.refresh("open");
      cycles = use.get().value?.vendors ?? [];
    },
    render: (s, d, kit) => ((v.kit = kit), render(s, d, kit, cycles, v)),
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
  const v: Voices = { states: {}, open: false, search, box, kit: null, refill: null };
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

/** The Voice menu: the vendor's shortlist (the catalog's), each checked against the vendor's list
 * (a voice it no longer has is marked and cannot be chosen; a name with several voices shows each
 * with its tagline), the voice in the draft if it is not one of them, then More voices. Each
 * option's value is the voice's id; a docs voice with no id yet (no list held) is "name:". */
function voiceMenu(vendor: string, shortlist: { name: string; id?: string }[], chosen: VoicePick, list: VendorVoice[] | null) {
  const opts: [string, string][] = [];
  const off = new Set<string>();
  for (const short of shortlist) {
    if (!list) {
      opts.push([short.id ?? `name:${short.name}`, short.name]);
      continue;
    }
    // A voice whose id the vendor's docs print is matched by that id; the rest by name.
    const hits = short.id
      ? list.filter((x) => x.id === short.id)
      : list.filter((x) => x.name.toLowerCase() === short.name.toLowerCase());
    if (!hits.length) {
      opts.push([`missing:${short.name}`, `${short.name}: not in ${vendor}'s list`]);
      off.add(`missing:${short.name}`);
    }
    for (const x of hits) opts.push([x.id, hits.length > 1 && x.tagline ? `${x.name}: ${x.tagline}` : x.name]);
  }
  const current = chosen.id ?? `name:${chosen.name}`;
  if (!opts.some(([value]) => value === current)) {
    const gone = list && chosen.id && !list.some((x) => x.id === chosen.id);
    opts.unshift([current, gone ? `${chosen.name}: not in ${vendor}'s list` : chosen.name]);
  }
  if (list?.length) opts.push([MORE, "More voices…"]);
  return { opts, off, current };
}

/** The table of every English voice a vendor lists (TABLES.md; §6 for the window and search):
 * Name, Gender, Accent, Tagline, then Use. The "i" opens the description and the id. */
function voicesCard(vendor: CatalogVendor, chosen: () => VoicePick, use: (x: VoicePick) => void, kit: Kit, v: Voices, list: VendorVoice[]) {
  const title = `${vendor.name}'s English voices`;
  // A new table for each search: a long list's window ends only when its rows leave the page
  // (list-window.ts), so the same body is never filled twice.
  const newTable = () =>
    dtTable(title, [
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
  const line = titleLine(title, "", null, v.search, close);
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
    const draw = (x: VendorVoice) => {
      const inUse = x.id === chosen().id;
      const cell = el(`<div class="cfg-control"><button class="cfg-button" type="button"></button></div>`);
      const b = cell.querySelector<HTMLButtonElement>("button")!;
      b.textContent = inUse ? "In use" : "Use";
      if (inUse) b.dataset.off = "";
      b.setAttribute("aria-label", inUse ? `${x.name} is the voice` : `Use ${x.name}`);
      b.addEventListener("click", () => {
        use({ name: x.name, id: x.id });
        kit.changed();
        kit.rerender();
      });
      const facts: [string, string][] = [];
      if (x.description) facts.push(["About", x.description]);
      if (x.locales.length > 1) facts.push(["Accents", x.locales.join(", ")]);
      facts.push(["Id", x.id]);
      return dtRow(x, x.name, [x.name, x.gender ? GENDER[x.gender] : "", x.accent ?? "", x.tagline, cell], facts);
    };
    fillTable(
      t,
      rows.map((x) => ({ item: x, draw: () => draw(x) })),
      { heights: heightsOf(`${vendor.id}-voices`), drawn: (nodes) => kit.apply(nodes) },
    );
    kit.apply([t.body]);
  };
  v.refill = () => (t.wrap.isConnected ? fill() : undefined);
  fill();
  return c;
}

function render(s: VoiceSettings, d: Draft, kit: Kit, cycles: VendorCycle[], v: Voices) {
  const { row, keyed: k, fixed, select, slider, number, pair, table, card } = kit;
  const n = (x: number) => Math.round(x).toLocaleString("en-US");

  // The catalog's two built vendors: their models, ranges, profiles, voices and rates.
  const cart = vendorOf(s, "cartesia");
  const eleven = vendorOf(s, "elevenlabs");
  if (!cart?.hearing || !cart.speaking) throw new Error("the catalog has no Cartesia");
  if (!eleven?.hearing || !eleven.speaking) throw new Error("the catalog has no ElevenLabs");
  const ink = cart.hearing;
  const sonic = cart.speaking;
  const scribe = eleven.hearing;
  const flash = eleven.speaking;
  const turnRange = (key: string) => rangeOf(leverOf(ink.levers, key));

  // Each direction's Vendor menu: every vendor in the catalog that offers it. A change draws the
  // cards again, since each vendor has its own rows.
  const vendorRow = (dir: "hearing" | "speaking") => {
    const field = dir === "hearing" ? "hearingVendor" : "speakingVendor";
    const offering = s.catalog.vendors.filter((x) => x[dir]).map((x): [string, string] => [x.id, x.name]);
    const menu = select(offering, d[field], (id) => {
      d[field] = id;
      v.open = false;
      kit.rerender();
    });
    return k(field, row("Vendor", HELP[field], menu));
  };
  // A vendor whose rows the page does not build yet (none today).
  const notBuilt = (id: string) => row("Settings", "", fixed(`${vendorOf(s, id)?.name ?? id}: its rows are not built yet`));

  // ---- the voice and its list: drawn the same way for the speaking vendor
  const speaking = vendorOf(s, d.speakingVendor);
  const voiceOf = (): VoicePick => (d.speakingVendor === "elevenlabs" ? d.el.voice : d.voice);
  const setVoice = (x: VoicePick) => (d.speakingVendor === "elevenlabs" ? (d.el.voice = x) : (d.voice = x));
  const sv = isBuilt(d.speakingVendor) ? v.states[d.speakingVendor] : undefined;
  const held = sv?.value ?? null;
  // An empty list is never a check: it would grey every name (the server calls it a fault too).
  const list = held && held.checked_at && held.voices.length ? held.voices : null;
  const speakName = speaking?.name ?? d.speakingVendor;
  const menu = voiceMenu(speakName, speaking?.speaking?.voices ?? [], voiceOf(), list);
  const voiceSelect = select(menu.opts, menu.current, (value) => {
    if (value === MORE) {
      v.open = true;
      voiceSelect.value = menu.current;
      kit.rerender();
      requestAnimationFrame(() => v.box.focus());
      return;
    }
    const found = list?.find((x) => x.id === value) ?? (speaking?.speaking?.voices ?? []).find((x) => x.id === value);
    const name = value.startsWith("name:") ? value.slice(5) : (found?.name ?? voiceOf().name);
    setVoice({ name, id: value.startsWith("name:") ? null : value });
  });
  for (const o of voiceSelect.options) if (menu.off.has(o.value)) o.disabled = true;
  const listen = el(`<button class="cfg-button" type="button" data-off disabled>Listen</button>`);
  listen.title = "Not built yet";

  // Where the menu's list came from, and Refresh.
  const checked =
    !sv || (sv.status === "loading" && !held)
      ? `Checking with ${speakName}`
      : !held
        ? sv.status === "refused"
          ? "Not checked: the agent's dev server refused"
          : "Not checked: the agent's dev server did not answer"
        : !held.key
          ? `Not checked: no ${speakName} key in .env`
          : held.fault && held.checked_at
            ? `Checked ${ago(held.checked_at)}; the last read failed (${held.fault})`
            : held.fault
              ? `Not checked: ${held.fault}`
              : `Checked ${ago(held.checked_at!)}: ${n(held.voices.length)} English voices`;
  const refresh = el(`<button class="cfg-button" type="button">Refresh</button>`) as HTMLButtonElement;
  refresh.addEventListener("click", () => {
    if (!isBuilt(d.speakingVendor)) return;
    refresh.disabled = true;
    const id = d.speakingVendor;
    void vendorVoices(id).write(() => api.voiceVoices(id, true)).catch(() => kit.rerender());
  });
  if (held && !held.key) refresh.dataset.off = "";
  const help = isBuilt(d.speakingVendor) ? VENDOR_HELP[d.speakingVendor] : VENDOR_HELP.cartesia;
  const voiceRows = (voiceHelp: string) => [
    k(d.speakingVendor === "elevenlabs" ? "el.voice" : "voice", row("Voice", voiceHelp, voiceSelect, listen)),
    row("Voice list", help.voiceCheck, fixed(checked), refresh),
  ];

  // ---- Speaking
  const pronunciations = (h: string) => k("pronunciations", row("Pronunciations", h, table(d.pronunciations, ["Word", "Say it like"])));
  const elSlider = (key: string, field: "speed" | "stability" | "similarity" | "style", word: string, h: string) =>
    k(`el.${field}`, row(word, h, slider(rangeOf(leverOf(flash.levers, key)), d.el[field], (x) => (d.el[field] = x))));
  const voice =
    d.speakingVendor === "cartesia"
      ? card(
          `Speaking (${sonic.product})`,
          "voice",
          vendorRow("speaking"),
          row("Model", HELP.ttsModel, fixed(sonic.model)),
          ...voiceRows(HELP.voice),
          k("speed", row("Speed", HELP.speed, slider(rangeOf(leverOf(sonic.levers, "speed")), d.speed, (x) => (d.speed = x)))),
          k("volume", row("Volume", HELP.volume, slider(rangeOf(leverOf(sonic.levers, "volume")), d.volume, (x) => (d.volume = x)))),
          k("emotion", row(
            "Emotion",
            HELP.emotion,
            select(
              [["", "Cartesia's default"], ...(leverOf(sonic.levers, "emotion").words ?? []).map((e): [string, string] => [e, cap(e)])],
              d.emotion ?? "",
              (x) => (d.emotion = x || null),
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
              (x) => (d.normalization = x),
            ),
          )),
          pronunciations(HELP.pronunciations),
        )
      : d.speakingVendor === "elevenlabs"
        ? card(
            `Speaking (${flash.product})`,
            "voice",
            vendorRow("speaking"),
            row("Model", HELP.elTtsModel, fixed(flash.model)),
            ...voiceRows(HELP.elVoice),
            elSlider("speed", "speed", "Speed", HELP.elSpeed),
            elSlider("stability", "stability", "Stability", HELP.elStability),
            elSlider("similarity", "similarity", "Similarity", HELP.elSimilarity),
            elSlider("style", "style", "Style", HELP.elStyle),
            k("el.speakerBoost", row("Speaker boost", HELP.elSpeakerBoost, pair(d.el.speakerBoost, (x) => (d.el.speakerBoost = x), "Speaker boost"))),
            k("el.normalization", row(
              "Number reading",
              HELP.elNormalization,
              select(
                (leverOf(flash.levers, "normalization").words ?? []).map((w): [string, string] => [w, cap(w)]),
                d.el.normalization,
                (x) => (d.el.normalization = x),
              ),
            )),
            pronunciations(HELP.elPronunciations),
          )
        : card(`Speaking (${speaking?.speaking?.product ?? d.speakingVendor})`, "voice", vendorRow("speaking"), notBuilt(d.speakingVendor), pronunciations(HELP.pronunciations));

  // ---- Hearing
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
    turnSliders[key] = slider(turnRange(key), d.turn[key], (x) => ((d.turn[key] = x), showProfile()));
    return k(`turn.${key}`, row(TURN_WORD[key] ?? key, HELP[key] ?? "", turnSliders[key]));
  };
  // The shared rows of hearing: one value whichever vendor hears.
  const bargeIn = k("bargeIn", row("Barge-in", HELP.bargeIn, pair(d.bargeIn, (x) => (d.bargeIn = x), "Barge-in")));
  const keyterms = k("keyterms", row("Keyterms", HELP.keyterms, table(d.keyterms, ["Word or phrase"])));
  const elTurn = (key: string, field: "vadConfidence" | "vadStart" | "turnStop", word: string, h: string) =>
    k(`el.${field}`, row(word, h, slider(rangeOf(leverOf(scribe.levers, key)), d.el[field], (x) => (d.el[field] = x))));
  // In the order a turn happens: it starts, it ends, the longest wait; then barge-in and the
  // words the engine listens for.
  const hearing =
    d.hearingVendor === "cartesia"
      ? card(
          `Hearing (${ink.product})`,
          "hearing",
          vendorRow("hearing"),
          row("Model", HELP.sttModel, fixed(ink.model)),
          k("profile", row("Turn profile", HELP.profile, profileSelect)),
          turnRow("turn_start_threshold"),
          turnRow("turn_end_threshold"),
          turnRow("turn_end_timeout_ms"),
          k("eager", row("Early end of turn", HELP.eager, pair(d.eager, (x) => (d.eager = x), "Early end of turn"))),
          turnRow("turn_eager_end_threshold"),
          bargeIn,
          keyterms,
        )
      : d.hearingVendor === "elevenlabs"
        ? card(
            `Hearing (${scribe.product})`,
            "hearing",
            vendorRow("hearing"),
            row("Model", HELP.elSttModel, fixed(scribe.model)),
            row("Turns", HELP.elTurns, fixed("Pipecat: voice detector and Smart Turn")),
            elTurn("vad_confidence", "vadConfidence", "Speech sureness", HELP.vadConfidence),
            elTurn("vad_start_secs", "vadStart", "Start of a turn (s)", HELP.vadStart),
            elTurn("turn_stop_secs", "turnStop", "Longest wait (s)", HELP.turnStop),
            bargeIn,
            keyterms,
          )
        : card(`Hearing (${vendorOf(s, d.hearingVendor)?.hearing?.product ?? d.hearingVendor})`, "hearing", vendorRow("hearing"), notBuilt(d.hearingVendor), bargeIn, keyterms);
  if (d.hearingVendor === "cartesia") showProfile();

  const guard = card(
    "Test-call guard",
    "guard",
    k("idleS", row("Silence (s)", HELP.idleS, number(d.idleS, 10, 600, (x) => (d.idleS = x)))),
    k("maxS", row("Longest call (s)", HELP.maxS, number(d.maxS, 60, 3600, (x) => (d.maxS = x)))),
  );

  // ---- Usage: one card for each built vendor chosen now (one when both directions share one).
  const chosen = [...new Set([d.hearingVendor, d.speakingVendor])].map((id) => vendorOf(s, id)).filter((x) => x !== undefined);
  const usage = chosen.filter((x) => isBuilt(x.id)).map((x, i) => {
    const id = x.id as Built;
    const a = d.accounts[id];
    const h = VENDOR_HELP[id];
    const cycle = cycles.find((c) => c.vendor === id);
    const left = fixed("");
    const showLeft = () => {
      left.textContent = cycle ? n(a.planCredits - a.offset - cycle.used) : "Not read";
    };
    showLeft();
    const minus = el(`<span class="cfg-minus" aria-hidden="true">−</span>`);
    return card(
      `Usage (${x.name})`,
      // The grid's area (config.css): the second vendor's card sits under the first.
      i === 0 ? "usage" : "usage2",
      k(`${id}.planCredits`, row(
        "Plan credits",
        h.planCredits,
        number(a.planCredits, 0, 1_000_000_000, (y) => ((a.planCredits = y), showLeft()), "Plan credits"),
        minus,
        number(a.offset, 0, 1_000_000_000, (y) => ((a.offset = y), showLeft()), "Offset"),
      )),
      k(`${id}.cycleDay`, row("Cycle day", HELP.cycleDay, number(a.cycleDay, 1, 28, (y) => (a.cycleDay = y)))),
      k(`${id}.planPrice`, row("Plan price ($ a month)", h.planPrice, number(a.planPrice, 0, 100_000, (y) => (a.planPrice = y), "Plan price", 0.01))),
      row("Credits left", h.left, left),
    );
  });

  // ---- Keys and facts: a key row and the rates for each vendor chosen now.
  const keyRows = chosen.map((x) =>
    row(`${x.name} API key`, HELP.key, fixed(s.keys[x.id] ? "Set" : `Not set: add ${x.key_env} to .env`)),
  );
  const per = (r: { unit: string; per: string; amount: number }) => `${r.amount} ${r.amount === 1 ? r.unit.replace(/s$/, "") : r.unit} per ${r.per}`;
  const rateRows = chosen.map((x) => {
    // The owner's words (2026-11-13): "Ink: 3 credits per second | Sonic: 1 credit per character".
    const parts = [x.hearing, x.speaking].filter((dir) => dir !== undefined).map((dir) => `${dir.product}: ${per(dir.rate)}`);
    return row(`Rates (${x.name})`, isBuilt(x.id) ? VENDOR_HELP[x.id].rates : "", fixed(parts.join(" | ")));
  });
  const usesEleven = chosen.some((x) => x.id === "elevenlabs");
  const facts = card(
    "Keys and facts",
    "facts",
    ...keyRows,
    ...(chosen.some((x) => x.id === "cartesia") ? [row("Cartesia API version", HELP.apiVersion, fixed(s.api_versions.cartesia ?? ""))] : []),
    ...(usesEleven
      ? [k("el.zeroRetention", row("ElevenLabs zero retention", HELP.zeroRetention, pair(d.el.zeroRetention, (x) => (d.el.zeroRetention = x), "ElevenLabs zero retention")))]
      : []),
    ...rateRows,
  );

  // Speaking and Hearing side by side when there is room, the rest below them (config.css ›
  // .cfg-body). On an older version the profile glows with any turn value; Plan credits with the
  // offset beside it.
  const all = v.open && list?.length && speaking && isBuilt(speaking.id) ? [voicesCard(speaking, voiceOf, setVoice, kit, v, list)] : [];
  return {
    cards: [voice, ...all, hearing, guard, ...usage, facts],
    layout: `${all.length ? "voice-list" : "voice"}${usage.length > 1 ? "-2u" : ""}`,
    glow: (diff: Set<string>) => {
      if ([...diff].some((x) => x.startsWith("turn."))) diff.add("profile");
      for (const id of BUILT) if (diff.has(`${id}.offset`)) diff.add(`${id}.planCredits`);
    },
  };
}
