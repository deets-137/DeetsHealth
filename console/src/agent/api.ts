// The agent's dev server, through Vite's proxy (agent/deets_agent/dev_server.py). The reference
// for every route and message is docs/architecture/DEV-API.md; docs:check (12, 14) keeps this
// file, the server and that doc in step. Every path is on the page's own origin; the proxy adds
// the launch token.

/** One line of a dev call record (agent/deets_agent/call_record.py). */
export type CallEvent = { t_ms: number; kind: string; [field: string]: unknown };

/** catalog.py › VERDICTS and GRADER_KINDS. */
export const VERDICTS = ["pass", "fail", "unknown"] as const;
export type Verdict = (typeof VERDICTS)[number];
export type GraderKind = "human" | "model" | "code";

export type Rating = {
  grader_kind: GraderKind;
  grader: string;
  dimension: string;
  verdict: Verdict;
  score: number | null;
  note: string | null;
  rubric_version: string;
  rated_at: string;
};

/** One row of the catalog (agent/deets_agent/catalog.py › calls). */
export type CallRow = {
  id: string;
  started: string | null;
  source: string;
  mode: string | null;
  scenario: string | null;
  outcome: string;
  end_reason: string | null;
  reached: string[];
  turns: number;
  duration_ms: number;
  cost_usd: number;
  flags: string[];
  label: string | null;
  note: string | null;
  keep: boolean;
  ratings: Rating[];
};

export type Catalog = {
  calls: CallRow[];
  dimensions: Record<string, string>;
  outcomes: string[];
  rubric_version: string;
};

/** A refusal's status for `AgentDown`; none when Vite answered for an agent it could not reach
 * (its `x-deets-agent: unreachable` header: console/vite.config.ts), which is "down", not a
 * refusal. */
const statusOf = (r: Response): number | undefined =>
  r.headers.get("x-deets-agent") === "unreachable" ? undefined : r.status;

/** Thrown when the dev server is not running (npm run dev:web) or refused. `status`: the HTTP
 * status of a refusal; none when the agent did not answer at all (store.ts tells the two apart:
 * LOADING.md §2). The message is fixed words, never a value. */
export class AgentDown extends Error {
  constructor(
    message: string,
    readonly status?: number,
  ) {
    super(message);
  }
}

/** Thrown when the catalog's gate refuses an annotation. `reason` names a field, never a value
 * (catalog.py › AnnotationError), so the page may show it. */
export class AnnotationRefused extends Error {
  constructor(readonly reason: string) {
    super(reason);
  }
}

/** A route's path under the page's base: `/agent/…` on this PC, `/demo/agent/…` in the demo
 * build (docs/ops/DEMO.md §3.1), where the dev server takes it as the same route. */
export const at = (path: string): string => import.meta.env.BASE_URL.replace(/\/$/, "") + path;

/** The same, as a WebSocket address on the page's own host. */
export const socketAt = (path: string): string =>
  `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}${at(path)}`;

async function get<T>(path: string): Promise<T> {
  let r: Response;
  try {
    r = await fetch(at(path), { headers: { accept: "application/json" } });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  if (!r.ok) throw new AgentDown(r.status === 404 ? "not found" : "the agent's dev server refused", statusOf(r));
  return (await r.json()) as T;
}

/** `epic`: whether the dev server gave ehr Epic's sandbox (CHAT.md §13). `voice`: whether
 * the Cartesia key is in .env (CHAT.md §17). */
export const health = () =>
  get<{ key: boolean; live: string | null; calls: number; epic: boolean; voice: boolean }>("/agent/health");

/** The query a new call sends: the EHR it reads, the label typed beside Start, and this page's
 * mark for it (docs/ops/DEMO.md §3.6). */
export const callQuery = (ehr: EhrSource, label: string, tag: string): string =>
  new URLSearchParams({ ehr, ...(label ? { label } : {}), tag }).toString();

/** A voice call's use of Cartesia so far (agent/deets_agent/voice.py › VoiceMeter). */
export type CartesiaUse = { ink_s: number; sonic_chars: number; credits: number; seconds?: number };

/** Cartesia's credits this cycle (agent/deets_agent/resources.py; CHAT.md §17). Counted from
 * this machine's voice calls only: a floor, not Cartesia's own count. */
export type CartesiaCycle = {
  plan: number;
  /** The active settings' manual offset: credits used outside these calls. */
  offset: number;
  used: number;
  left: number;
  calls: number;
  ink_s: number;
  sonic_chars: number;
  cycle_start: string;
  renews: string;
  live: CartesiaUse | null;
  rates: { ink_per_s: number; sonic_per_char: number };
  limits: { idle_s: number; max_s: number };
};
/** One call's use for the Resources tab: Claude's dollars and, for a voice call, Cartesia's
 * credits; a live call's running totals, else its record's (CHAT.md §17). */
export type CallUse = { seat: "text" | "voice"; usd: number; credits: number | null; live: boolean };
/** The Resources tab's numbers. `call`: the call the page shows, if any. */
export const resources = (call: string | null) =>
  get<{ cartesia: CartesiaCycle; anthropic: AnthropicCycle; call: CallUse | null; voice: boolean }>(
    `/agent/resources${call ? `?${new URLSearchParams({ call })}` : ""}`,
  );

/** A lever's range and Cartesia's default (agent/deets_agent/config.py). */
export type Range = { min: number; max: number; step: number; default: number };

/** Every voice setting (CONSOLE.md §7.3): the same names as ehr's settings.rs › VOICE and the
 * agent's config.voice_defaults. A null number or word is "Cartesia's default". */
export type VoiceValues = {
  voice: { name: string; id: string | null };
  speed: number | null;
  volume: number | null;
  emotion: string | null;
  normalization: string;
  pronunciations: string[][]; // [word, say it like]
  turn_start_threshold: number | null;
  turn_eager_end_threshold: number | null;
  turn_end_threshold: number | null;
  turn_end_timeout_ms: number | null;
  eager_end_of_turn: boolean;
  barge_in: boolean;
  keyterms: string[][]; // [word or phrase]
  idle_s: number;
  max_s: number;
  plan_credits: number;
  offset: number;
  cycle_day: number;
};

/** One saved version: its label is "MM-DD | n" (the owner's words). */
export type SettingVersion = { id: number; label: string; active: boolean; created_at: string; created_by: string };

/** Config › Cartesia (CONSOLE.md §7): the saved versions, newest first; the values of the version
 * asked for (the active one by default); the active values, to mark what an older version
 * changed; each lever's range. `store`: false when ehr runs without Postgres (no versions, the
 * defaults shown, no saving). Never the key: only whether it is set. */
export type CartesiaSettings = {
  key: boolean;
  store: boolean;
  versions: SettingVersion[];
  version: SettingVersion | null;
  values: VoiceValues;
  active: VoiceValues;
  api_version: string;
  tts: {
    model: string;
    voices: { name: string; id: string | null }[];
    emotions: string[];
    ranges: Record<"speed" | "volume", Range>;
  };
  stt: {
    model: string;
    /** Cartesia's named sets of the four turn values; not an API parameter (CARTESIA.md §9). */
    profiles: Record<string, Record<string, number>>;
    ranges: Record<string, Range>;
  };
  rates: { ink_per_s: number; sonic_per_char: number };
};
/** `version`: 0 or none for the active one. */
export const cartesiaSettings = (version = 0) =>
  get<CartesiaSettings>(version ? `/agent/cartesia?version=${version}` : "/agent/cartesia");

/** Save the whole set as a new, active version (CONSOLE.md §7.3). A refusal's reason names a
 * setting, never a value, so the page may show it. */
export const saveCartesia = (values: VoiceValues) => saveSettings<CartesiaSettings>("/agent/cartesia", values);

/** Every llm setting (CONSOLE.md §8): the same names as ehr's settings.rs › LLM and the agent's
 * config.llm_defaults. */
export type LlmValues = {
  model: string;
  effort: string;
  /** "on": adaptive thinking; "off": the model's own way to turn it off. */
  thinking: "on" | "off";
  max_tokens: number;
  retry_max_tokens: number;
  cache_ttl: "5m" | "1h";
  budget_usd: number;
  offset_usd: number;
  cycle_day: number;
};

/** One model the page offers (agent/deets_agent/config.py › LLM_MODELS). Prices in dollars per
 * 1M tokens. */
export type LlmModel = {
  id: string;
  name: string;
  effort_default: string;
  thinking_off: boolean;
  price: { input: number; output: number; cache_read: number };
};

/** Claude's dollars this cycle, from this machine's call records (resources.py ›
 * anthropic_cycle): a floor, not Anthropic's own count. */
export type AnthropicCycle = {
  budget: number;
  offset: number;
  spent: number;
  left: number;
  calls: number;
  cycle_start: string;
  renews: string;
  live: number | null;
};

/** Config › Anthropic (CONSOLE.md §8): the versions and values as for Cartesia; each model's
 * rules and prices; this cycle's spend; the fixed facts. Never the key: only whether it is set. */
export type AnthropicSettings = {
  key: boolean;
  store: boolean;
  versions: SettingVersion[];
  version: SettingVersion | null;
  values: LlmValues;
  active: LlmValues;
  models: LlmModel[];
  efforts: string[];
  /** The efforts at which thinking may be off. */
  thinking_off_efforts: string[];
  ranges: Record<"max_tokens" | "retry_max_tokens", { min: number; max: number }>;
  /** A cache write's price, as a multiple of the input price, by the cache's life. */
  cache_write: Record<string, number>;
  inference_geo: string;
  geo_price: number;
  usage: AnthropicCycle;
};
/** `version`: 0 or none for the active one. */
export const anthropicSettings = (version = 0) =>
  get<AnthropicSettings>(version ? `/agent/anthropic?version=${version}` : "/agent/anthropic");
export const saveAnthropic = (values: LlmValues) => saveSettings<AnthropicSettings>("/agent/anthropic", values);

/** A System variable (SYSTEM-PAGE.md §3.1): the clinic's own text, never protected data.
 * `built_in`: the code's own; its value can change, it cannot be removed. */
export type SystemVariable = { name: string; meaning: string; value: string; built_in?: boolean };
/** A code value: filled by the code, in its own sections only. Never a value here. */
export type CodeValue = {
  name: string;
  meaning: string;
  source: string;
  protected: boolean;
  /** From the caller or the record during a call; false for the code's own (`today`). */
  from_call: boolean;
  sections: string[];
};
/** A tool, for System › Tools (agent/prompts/TOOLS.md and the code). `steps`: "every", or the
 * steps that offer it. */
export type ToolInfo = {
  name: string;
  does: string;
  record: "reads" | "writes" | "none";
  lines: string[];
  steps: string[];
  args: string[];
};
/** The System service's one setting, as ehr keeps it. */
export type SystemValues = { variables: SystemVariable[] };

/** Config › System (SYSTEM-PAGE.md §3.6): the versions and one version's System variables; the
 * code values and the tools (read only); which sections use each variable (in the active words). */
export type SystemSettings = {
  store: boolean;
  versions: SettingVersion[];
  version: SettingVersion | null;
  values: SystemValues;
  active: SystemValues;
  code_values: CodeValue[];
  tools: ToolInfo[];
  used: Record<string, string[]>;
  limits: { name: number; meaning: number; value: number; count: number };
  built_in: string[];
};
export const systemSettings = (version = 0) =>
  get<SystemSettings>(version ? `/agent/system?version=${version}` : "/agent/system");
export const saveSystem = (values: SystemValues) => saveSettings<SystemSettings>("/agent/system", values);

/** Config › Hospital (HOSPITAL-CONFIG.md): the hospital's lists and numbers, as ehr's
 * hospital.rs keeps them. Facts about the hospital, never protected data. */
/** Which patients a visit type is for (BOOKING-RULES.md §3.4): new to the department's
 * specialty, established in it, or either. */
export type PatientKind = "either" | "new" | "established";
/** `code`: the type's ID in the hospital's visit type list (`visit_type_list`). */
export type VisitTypeRow = { code: string; record: string; spoken: string; online: boolean; book: boolean; patients: PatientKind };
export type VisitGroupRow = { name: string; words: string[]; visit_types: string[] };
/** A reason's code system, by name: "hospital" is the hospital's own list (`diagnosis_list`). */
export type ReasonSystem = "" | "snomed" | "icd10cm" | "hospital";
export type ReasonRow = { system: ReasonSystem; code: string; words: string[]; visit_type: string; specialty: string };
export type TimeOfDayRow = { name: string; from: string; hours: number };
export type HoursRow = { day: string; opens: string; closes: string };
export type ClosedDayRow = { date: string; name: string };
/** A facility (Epic's EAF; HOSPITAL-CONFIG.md §8.2): `id` the FHIR id, `code` the EAF ID. */
export type FacilityRow = { record: string; spoken: string; id: string; code: string; address: string; phone: string };
/** A department (Epic's DEP): `facility` the FHIR id of its facility ("" for none), `code` the
 * DEP ID, `specialty` its ID in the specialty list, `hours` from Epic's directory. */
export type LocationRow = {
  record: string;
  spoken: string;
  id: string;
  address: string;
  phone: string;
  facility: string;
  code: string;
  specialty: string;
  hours: HoursRow[];
};
/** `code`: the specialty's ID in the hospital's specialty list (`specialty_list`). */
export type SpecialtyRow = { code: string; spoken: string; words: string[]; primary_care: boolean };
/** What a Booking rule does with the visits it matches (BOOKING-RULES.md §3.5). */
export type RuleOutcome = "book" | "book_and_flag" | "pcp_only" | "referral_needed" | "hand_off";
/** One Booking rule: each condition empty for "any" (`patients` "either"); the first row that
 * matches wins. `specialty` a specialty's ID, `department` and `visit_type` the record's names,
 * `provider` the name as the record shows it. */
export type BookingRule = {
  specialty: string;
  department: string;
  visit_type: string;
  provider: string;
  patients: PatientKind;
  outcome: RuleOutcome;
};
export type HospitalValues = {
  time_zone: string;
  /** Each list's OID, set once ("urn:oid:…", or empty): the owner's A, 2026-11-15. */
  visit_type_list: string;
  visit_types: VisitTypeRow[];
  visit_groups: VisitGroupRow[];
  diagnosis_list: string;
  reasons: ReasonRow[];
  times_of_day: TimeOfDayRow[];
  hours: HoursRow[];
  closed_days: ClosedDayRow[];
  facilities: FacilityRow[];
  locations: LocationRow[];
  specialty_list: string;
  specialties: SpecialtyRow[];
  slots_offered: number;
  narrow_over: number;
  groups_named: number;
  past_days: number;
  /** A caller seen by a specialty within this many years is established in it (1 to 10). */
  established_years: number;
  /** The owner's P-A (BOOKING-RULES.md §3.3): offer one sooner time with another clinician. */
  sooner_with_others: boolean;
  ahead_windows: number[];
  /** Put after the caller's words in the note a booking files in Epic (the owner's N2). */
  booking_note_label: string;
  /** The Booking rules, in order (BOOKING-RULES.md §3.5; the owner's E-A). */
  booking_rules: BookingRule[];
};
export type HospitalSettings = {
  store: boolean;
  versions: SettingVersion[];
  version: SettingVersion | null;
  values: HospitalValues;
  active: HospitalValues;
  /** Every list and number, in the page's order. */
  keys: (keyof HospitalValues)[];
  /** What ehr's hospital.rs allows: a cell's characters, a list's rows (`places`: the
   * facilities' and the departments'), a cell's words, the times of day, the search windows. */
  limits: { cell: number; rows: number; places: number; words: number; times: number; windows: number };
};
export const hospitalSettings = (version = 0) =>
  get<HospitalSettings>(version ? `/agent/hospital?version=${version}` : "/agent/hospital");
export const saveHospital = (values: HospitalValues) => saveSettings<HospitalSettings>("/agent/hospital", values);

/** Places › Import from Epic (HOSPITAL-CONFIG.md §8): Epic's directory as the hospital's rows,
 * not merged and not saved. Facts about the hospital, never protected data. */
export type ImportedPlaces = {
  values: Pick<HospitalValues, "facilities" | "locations" | "specialty_list" | "specialties">;
  others: number;
  other_specialty_lists: number;
  reads: number;
};
export async function importPlaces(addresses: string[]): Promise<ImportedPlaces> {
  let r: Response;
  try {
    r = await fetch(at("/agent/hospital/import-places"), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ addresses }),
    });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  if (!r.ok) {
    const body = (await r.json().catch(() => null)) as { detail?: unknown } | null;
    throw new AgentDown(typeof body?.detail === "string" ? body.detail : "the import failed", statusOf(r));
  }
  return (await r.json()) as ImportedPlaces;
}

/** The last Record and test (dev_server.py › record_and_test): counts and names of kept
 * scripts only. */
export type Recording = {
  state: "running" | "passed" | "failed";
  scripts: { name: string; ok: boolean }[];
  tests: boolean | null;
  spent_usd: number | null;
  /** Each failed test's name and the first line of its error (DATABASE.md §8.5). Synthetic
   * callers only, so a line may quote the agent's words. */
  failures?: { test: string; line: string }[];
  /** What the call store took (DATABASE.md §8.3): null while running or without a store. */
  filed?: { store: boolean; run_id?: number; records?: number; pruned?: number } | null;
};

/** Config › Agent (CONSOLE.md §9): the versions of the agent's words, one version's sections
 * (by key, as the agent reads them), the placeholders each section must hold, and Record and
 * test. */
export type PromptSettings = {
  key: boolean;
  store: boolean;
  versions: SettingVersion[];
  version: SettingVersion | null;
  values: Record<string, string>;
  active: Record<string, string>;
  /** The code values each section must hold (SYSTEM-PAGE.md §3.1). */
  placeholders: Record<string, string[]>;
  /** What the `{` and " ." lists offer (SYSTEM-PAGE.md §3.6): the active System variables, the
   * code values, the tools, and the tools each section may name. */
  variables: SystemVariable[];
  code_values: CodeValue[];
  tools: { name: string; does: string }[];
  tool_scope: Record<string, string[]>;
  /** `match`: the kept recordings were made and tested with the prompt file's words. */
  recordings: { match: boolean; scripts: number };
  /** The dry run's quote for the saved words; null until one is asked for (estimatePrompt). */
  estimate: Estimate | null;
  recording: Recording | null;
};
export const promptSettings = (version = 0) =>
  get<PromptSettings>(version ? `/agent/prompt?version=${version}` : "/agent/prompt");
export const savePrompt = (values: Record<string, string>) => saveSettings<PromptSettings>("/agent/prompt", values);

/** Start Record and test. A refusal's reason is a fixed line the page may show. */
/** Record and test's quote (dev_server.py › estimate_record): the kept scripts that need new
 * replies, by name, and the dollars recording them cost last time. */
/** "failed": the quote stopped on an error; Record and test still records, with no quote. */
export type Estimate = { state: "running" | "done" | "failed"; missing: string[]; usd: number | null };

/** Ask for the quote: a dry run with no Claude call, in the background. */
export async function estimatePrompt(): Promise<Estimate | null> {
  try {
    const r = await fetch(at("/agent/prompt/estimate"), { method: "POST" });
    if (!r.ok) return null;
    return ((await r.json()) as { estimate: Estimate | null }).estimate;
  } catch {
    return null;
  }
}

export async function recordPrompt(): Promise<Recording> {
  let r: Response;
  try {
    r = await fetch(at("/agent/prompt/record"), { method: "POST" });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  const body = (await r.json().catch(() => null)) as { detail?: unknown; recording?: Recording } | null;
  if (!r.ok || !body?.recording) throw new AgentDown(typeof body?.detail === "string" ? body.detail : "not started", statusOf(r));
  return body.recording;
}

async function saveSettings<T>(path: string, values: unknown): Promise<T> {
  let r: Response;
  try {
    r = await fetch(at(path), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ values }),
    });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  if (!r.ok) {
    const body = (await r.json().catch(() => null)) as { detail?: unknown } | null;
    throw new AgentDown(typeof body?.detail === "string" ? body.detail : "not saved", statusOf(r));
  }
  return (await r.json()) as T;
}

/** One person who may sign in (Teams › People; docs/ops/DEMO.md §3.5; ehr's users.rs). */
export type Person = {
  id: string;
  email: string;
  name: string;
  owner: boolean;
  added_by: string | null;
  added_at: string;
  last_seen_at: string | null;
  /** The card (edited on Security › Auth): empty when not set. */
  first_name: string;
  last_name: string;
  /** E.164, or null. */
  phone: string | null;
  phone_verified: boolean;
  /** Where they sign in from: "access" or "idp". */
  source: string;
  /** When the picture was saved; null when there is none. */
  photo_at: string | null;
};

/** The People view: the list, whether the store and Cloudflare's group are there, the invite
 * link, who is asking ("owner", or their email), and after a change, Cloudflare's trouble if
 * it had any (the list is saved either way). */
export type People = {
  people: Person[];
  store: boolean;
  linked: boolean;
  invite: string | null;
  you: string;
  warning?: string | null;
  /** After an add: the new person's id (a card with no email yet has no other key). */
  added?: string;
};

export const people = () => get<People>("/agent/people");

async function peopleChange(method: "POST" | "PATCH" | "PUT" | "DELETE", path: string, body?: unknown): Promise<People> {
  let r: Response;
  try {
    const blob = body instanceof Blob;
    r = await fetch(at(path), {
      method,
      headers: body === undefined ? {} : { "content-type": blob ? body.type : "application/json" },
      body: body === undefined ? undefined : blob ? body : JSON.stringify(body),
    });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  const reply = (await r.json().catch(() => null)) as (People & { detail?: unknown }) | null;
  // ehr's reasons are fixed words that name a field, never a value.
  if (!r.ok || !reply) throw new AgentDown(typeof reply?.detail === "string" ? reply.detail : "not saved", statusOf(r));
  return reply;
}

export const addPerson = (email: string, name: string) => peopleChange("POST", "/agent/people", { email, name });
export const removePerson = (id: string) => peopleChange("DELETE", `/agent/people/${encodeURIComponent(id)}`);

/** A card's Save on Security › Auth: the whole profile. */
export type Profile = { first_name: string; last_name: string; phone: string; email: string };
export const updatePerson = (id: string, p: Profile) =>
  peopleChange("PATCH", `/agent/people/${encodeURIComponent(id)}`, p);

/** Saves a person's picture, a 256 px WebP the page made; null clears it. */
export const setPersonPhoto = (id: string, webp: Blob | null) =>
  peopleChange(webp ? "PUT" : "DELETE", `/agent/people/${encodeURIComponent(id)}/photo`, webp ?? undefined);

/** A person's picture as an object URL (the caller revokes it), or null when there is none.
 * Fetched, not set as an <img> address, so it goes through the same door as every request. */
export async function personPhoto(id: string): Promise<string | null> {
  let r: Response;
  try {
    r = await fetch(at(`/agent/people/${encodeURIComponent(id)}/photo`), { cache: "no-store" });
  } catch {
    return null;
  }
  if (!r.ok) return null;
  return URL.createObjectURL(await r.blob());
}

/** The ICE servers for a voice call, asked for before the offer (docs/ops/DEMO.md §3.7): none
 * on this PC; Cloudflare STUN and TURN for a guest. A refusal's reason names no value. */
export async function iceServers(): Promise<RTCIceServer[]> {
  let r: Response;
  try {
    r = await fetch(at("/agent/ice"), { headers: { accept: "application/json" } });
  } catch {
    throw new AgentDown("the agent did not answer");
  }
  if (!r.ok) {
    const body = (await r.json().catch(() => null)) as { detail?: unknown } | null;
    throw new AgentDown(typeof body?.detail === "string" ? body.detail : "no relay for a voice call", statusOf(r));
  }
  return ((await r.json()) as { iceServers: RTCIceServer[] }).iceServers;
}

/** The voice seat's offer (CHAT.md §17): the browser's WebRTC offer, with every ICE candidate in
 * it; the answer comes back. A refusal's reason names no value, so the page may show it. */
export async function voiceOffer(
  query: string,
  offer: RTCSessionDescriptionInit,
): Promise<RTCSessionDescriptionInit> {
  let r: Response;
  try {
    r = await fetch(at(`/agent/voice?${query}`), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ type: offer.type, sdp: offer.sdp }),
    });
  } catch {
    throw new AgentDown("the agent did not answer");
  }
  if (!r.ok) {
    const body = (await r.json().catch(() => null)) as { detail?: unknown } | null;
    throw new AgentDown(typeof body?.detail === "string" ? body.detail : "the agent refused the call", statusOf(r));
  }
  return (await r.json()) as RTCSessionDescriptionInit;
}

/** The EHR a test call reads: ehr's fakes, or Epic's public sandbox (CHAT.md §13). */
export type EhrSource = "local" | "epic";

/** The sandbox: `local`, ehr/fixtures/sandbox.json's synthetic patients; `epic`, Epic's public
 * test patients, read live through ehr. `phone` is "" when the chart has no mobile number. A
 * visit's `place` is its own (Epic); without one, the sandbox's. `more`: per patient id, the
 * visits past the ones listed. */
export type SandboxPatient = { id: string; given: string; family: string; birth_date: string; phone: string };
/** `visit_type` and `reason`: as the record has them (a dev view of test patients; the call
 * itself hears them after ehr's sensitive screen, FUTURE-SETTINGS.md §9). */
export type SandboxVisit = {
  patient: string;
  starts_at: string;
  minutes: number;
  clinician: string;
  place?: string;
  visit_type?: string | null;
  reason?: string | null;
  /** As Epic names it; absent means booked. A past visit is "fulfilled", "noshow" or "cancelled". */
  status?: string | null;
};
export type Sandbox = { place: string; patients: SandboxPatient[]; visits: SandboxVisit[]; more: Record<string, number> };
export const sandbox = (ehr: EhrSource) => get<Sandbox>(`/agent/sandbox?ehr=${ehr}`);
export const catalog = () => get<Catalog>("/agent/catalog");
export const callEvents = (id: string) =>
  get<{ id: string; events: CallEvent[] }>(`/agent/calls/${encodeURIComponent(id)}`).then((r) => r.events);

/** One annotation: the body of `POST /agent/annotations`. One shape per kind, the same fields as
 * catalog.py › FIELDS (docs:check 14). The catalog stamps `at` and `rubric_version` itself. */
export type Annotation =
  | { kind: "label" | "note"; call: string; text: string }
  | { kind: "keep"; call: string; value: boolean }
  | {
      kind: "rating";
      call: string;
      grader_kind: GraderKind;
      grader: string;
      dimension: string;
      verdict: Verdict;
      score?: number | null;
      note?: string | null;
    };
export type AnnotationKind = Annotation["kind"];
/** An annotation before the call is named. */
export type AnnotationDraft = WithoutCall<Annotation>;
type WithoutCall<A> = A extends unknown ? Omit<A, "call"> : never; // one Omit per kind

export async function annotate(a: Annotation): Promise<void> {
  let r: Response;
  try {
    r = await fetch(at("/agent/annotations"), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(a),
    });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  if (r.status === 400) {
    const body = (await r.json().catch(() => null)) as { detail?: unknown } | null;
    throw new AnnotationRefused(typeof body?.detail === "string" ? body.detail : "the annotation does not fit");
  }
  if (!r.ok) throw new AgentDown("the agent's dev server refused", statusOf(r));
}

/** One live call, as the watch stream names it (dev_server.py › LiveCall.info; docs/ops/DEMO.md
 * §3.6). `by`: "owner", or the guest's email from the Access token. `tag`: the mark the page
 * that started it sent, so that page knows its own call. */
export type LiveInfo = {
  call: string;
  started: string;
  seat: "text" | "voice";
  ehr: EhrSource;
  by: string;
  label: string | null;
  tag: string | null;
};

export type WatchMessage =
  | { type: "hello"; live: string | null; calls: LiveInfo[] }
  | { type: "started"; call: string; info: LiveInfo }
  | { type: "event"; call: string; event: CallEvent }
  | { type: "ended"; call: string };

/** The watch stream: the live call's events as the record writes them. Reconnects on its own
 * while the page is open. Returns a function that stops it. */
export function watch(on: (m: WatchMessage) => void, onUp?: (up: boolean) => void): () => void {
  let ws: WebSocket | undefined;
  let stopped = false;
  let retry = 0;
  const open = () => {
    ws = new WebSocket(socketAt("/agent/watch"));
    ws.addEventListener("open", () => onUp?.(true));
    ws.addEventListener("message", (e) => {
      try {
        on(JSON.parse(e.data as string) as WatchMessage);
      } catch {
        /* dropped */
      }
    });
    ws.addEventListener("close", () => {
      onUp?.(false);
      if (!stopped) retry = window.setTimeout(open, 2000);
    });
  };
  open();
  return () => {
    stopped = true;
    window.clearTimeout(retry);
    ws?.close();
  };
}
