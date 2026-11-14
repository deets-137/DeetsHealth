// Config › Cartesia (CONSOLE.md §7): every Cartesia lever the agent runs with, its usage, and
// whether the key is set. The top row: Read | Edit (Read greys every control; Edit opens them),
// then the versions (the owner, 2026-11-13: each saved version is "MM-DD | n", the one the agent
// runs with is "Active"; an older one shows read only, its changed values glowing), Discard and
// Save. Save makes a new version in the settings store (CONSOLE.md §7.3) and the active one. The
// key itself never comes here (SECURITY.md §4.6).
//
// Each row (the owner, 2026-11-13): an "i" button at the left that opens the setting's help,
// its name, then its control. No line under the name. The frame, the rows and the controls are
// config-kit.ts, shared with Config › Anthropic.
import { saveCartesia, type CartesiaCycle, type CartesiaSettings, type VoiceValues } from "../agent/api";
import { cartesiaSettings, resources } from "../agent/stores";
import { cap, settingsView, type Kit } from "./config-kit";
import { el } from "./page";
import type { View } from "./subnav";

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

export const cartesiaView = (): View => {
  let cycle: CartesiaCycle | undefined;
  return settingsView<VoiceValues, Draft, CartesiaSettings>({
    id: "cartesia",
    title: "Cartesia",
    store: cartesiaSettings,
    save: (s, d) => saveCartesia(toValues(d, s.tts.voices, s.values)),
    fromValues,
    differing,
    extra: async () => {
      const use = resources(null);
      await use.refresh("open");
      cycle = use.get().value?.cartesia;
    },
    render: (s, d, kit) => render(s, d, kit, cycle),
  });
};

function render(s: CartesiaSettings, d: Draft, kit: Kit, cycle: CartesiaCycle | undefined) {
  const { row, keyed: k, fixed, select, slider, number, pair, table, card } = kit;
  const n = (x: number) => Math.round(x).toLocaleString("en-US");

  const listen = el(`<button class="cfg-button" type="button" data-off disabled>Listen</button>`);
  listen.title = "Not built yet";
  const voices: [string, string][] = s.tts.voices.map((v) => [v.name, v.name]);
  if (!voices.some(([name]) => name === d.voice)) voices.unshift([d.voice, d.voice]);

  const voice = card(
    "Voice (Sonic)",
    "voice",
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
  // setting of its own: choosing one moves the sliders, and moving a slider shows the profile
  // the four now match, or Custom.
  const valueOf = (key: string) => d.turn[key] ?? s.stt.ranges[key].default;
  const matching = () =>
    Object.keys(s.stt.profiles).find((p) =>
      Object.entries(s.stt.profiles[p]).every(([key, x]) => Math.abs(valueOf(key) - x) < s.stt.ranges[key].step / 2),
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
      for (const [key, x] of Object.entries(values)) {
        const r = s.stt.ranges[key];
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
    turnSliders[key] = slider(s.stt.ranges[key], d.turn[key], (v) => ((d.turn[key] = v), showProfile()));
    return k(`turn.${key}`, row(TURN_WORD[key] ?? key, HELP[key] ?? "", turnSliders[key]));
  };
  // In the order a turn happens: it starts, it ends, the longest wait; then the early guess
  // beside its own switch; then barge-in and the words Ink listens for.
  const hearing = card(
    "Hearing (Ink)",
    "hearing",
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
    "Usage",
    "usage",
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
    "facts",
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

  // Voice and Hearing side by side when there is room, the rest below them (config.css ›
  // .cfg-body[data-layout="cartesia"]). On an older version the profile glows with any turn
  // value; Plan credits with the offset beside it.
  return {
    cards: [voice, hearing, guard, usage, facts],
    layout: "cartesia",
    glow: (diff: Set<string>) => {
      if ([...diff].some((x) => x.startsWith("turn."))) diff.add("profile");
      if (diff.has("offset")) diff.add("planCredits");
    },
  };
}
