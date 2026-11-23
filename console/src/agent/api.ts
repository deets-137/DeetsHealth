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

/** The words of a wall's refusal for want of a role (dev_server.py: 403 `{error: "forbidden",
 * lever, level}`), or null when the reply is another refusal. Fixed words. */
export function roleRefusal(reply: unknown): string | null {
  const r = reply as { error?: unknown; lever?: unknown; level?: unknown } | null;
  if (!r || r.error !== "forbidden" || typeof r.lever !== "string" || typeof r.level !== "string") return null;
  return `Your role does not allow this (needs ${leverName(r.lever as Lever)} ${r.level === "edit" ? "Edit" : "View"})`;
}

async function get<T>(path: string): Promise<T> {
  let r: Response;
  try {
    r = await fetch(at(path), { headers: { accept: "application/json" } });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  if (!r.ok) {
    const denied = r.status === 403 ? roleRefusal(await r.json().catch(() => null)) : null;
    throw new AgentDown(denied ?? (r.status === 404 ? "not found" : "the agent's dev server refused"), statusOf(r));
  }
  return (await r.json()) as T;
}

/** `epic`: whether the dev server gave ehr Epic's sandbox (CHAT.md §13). `voice`: whether
 * the chosen voice vendors' keys are in .env (CHAT.md §17; VOICE-VENDORS.md). */
export const health = () =>
  get<{ key: boolean; live: string | null; calls: number; epic: boolean; voice: boolean }>("/agent/health");

/** The query a new call sends: the EHR it reads, the label typed beside Start, and this page's
 * mark for it (docs/ops/DEMO.md §3.6). */
export const callQuery = (ehr: EhrSource, label: string, tag: string): string =>
  new URLSearchParams({ ehr, ...(label ? { label } : {}), tag }).toString();

/** One voice vendor's cycle (vendors/src/voice/cost.rs › Cycle; CHAT.md §17), priced by ehr from this
 * machine's call records and live calls: a floor, not the vendor's own count. `left` may go below
 * zero. `amounts`: each direction's raw use (seconds heard, characters spoken). */
export type VendorCycle = {
  vendor: string;
  name: string;
  unit: string;
  plan: number;
  /** The active settings' manual offset: credits used outside these calls. */
  offset: number;
  used: number;
  left: number;
  calls: number;
  amounts: { direction: string; product: string; per: string; amount: number }[];
  cycle_start: string;
  renews: string;
  usd_per_credit: number | null;
  live: number;
};
/** One call's use for the Resources tab: Claude's dollars and, for a voice call, the voice's
 * credits by vendor (null for a text call, or while ehr is not up); a live call's running
 * totals, else its record's (CHAT.md §17). */
export type CallUse = { seat: "text" | "voice"; usd: number; credits: Record<string, number> | null; live: boolean };
/** The Resources tab's numbers. `vendors`: each chosen voice vendor's cycle (empty while ehr is
 * not up). `call`: the call the page shows, if any. */
export type Resources = {
  vendors: VendorCycle[];
  limits: { idle_s: number; max_s: number };
  /** Claude's spend this cycle, from ehr (model/cost.rs); null while ehr is not up. */
  anthropic: AnthropicCycle | null;
  call: CallUse | null;
  voice: boolean;
};
export const resources = (call: string | null) =>
  get<Resources>(
    `/agent/resources${call ? `?${new URLSearchParams({ call })}` : ""}`,
  );

/** A lever's range and the vendor's default, as the sliders take it. */
export type Range = { min: number; max: number; step: number; default: number };

/** One lever of the voice catalog (vendors/src/voice/mod.rs › Lever; VOICE-VENDORS.md §2.1): its key,
 * what it holds, and the vendor's own default. */
export type CatalogLever = {
  key: string;
  kind: "number" | "whole" | "bool" | "word" | "voice" | "table";
  min?: number;
  max?: number;
  step?: number;
  nullable?: boolean;
  words?: string[];
  columns?: number;
  rows?: number;
  chars?: number;
  default?: unknown;
};
/** What a vendor offers in one direction (hearing or speaking). */
export type CatalogDirection = {
  product: string;
  model: string;
  own_turns: boolean;
  rate: { unit: string; per: string; amount: number };
  levers: CatalogLever[];
  /** Hearing: named sets of turn levers; not an API parameter (CARTESIA.md §9). */
  profiles?: Record<string, Record<string, number>>;
  /** Speaking: the vendor's recommended voices; an id where its docs print one. */
  voices?: { name: string; id?: string }[];
};
/** One voice vendor of the catalog. */
export type CatalogVendor = {
  id: string;
  name: string;
  /** The name of its key in .env (never the key). */
  key_env: string;
  baa: string;
  hearing?: CatalogDirection;
  speaking?: CatalogDirection;
  account: { unit: string; levers: CatalogLever[] };
};
export type VoiceCatalog = { vendors: CatalogVendor[]; shared: CatalogLever[] };

/** Every voice setting (VOICE-VENDORS.md §2.3): `hearing.vendor` and `speaking.vendor`, the
 * shared levers bare (`barge_in`), and each vendor's levers under its id (`cartesia.speed`). The
 * same keys as ehr's settings.rs › voice_list. A null number or word is "the vendor's default". */
export type VoiceValues = { "hearing.vendor": string; "speaking.vendor": string; [key: string]: unknown };

/** One saved version: its label is "MM-DD | n" (the owner's words). */
export type SettingVersion = { id: number; label: string; active: boolean; created_at: string; created_by: string };

/** Config › Voice (VOICE-VENDORS.md §2.5; CONSOLE.md §7): the saved versions, newest first; the
 * values of the version asked for (the active one by default); the active values, to mark what an
 * older version changed; the voice catalog. `store`: false when ehr runs without Postgres (no
 * versions, the defaults shown, no saving). Never a key: only whether each vendor's is set. */
export type VoiceSettings = {
  store: boolean;
  versions: SettingVersion[];
  version: SettingVersion | null;
  values: VoiceValues;
  active: VoiceValues;
  catalog: VoiceCatalog;
  keys: Record<string, boolean>;
  api_versions: Record<string, string>;
};
/** `version`: 0 or none for the active one. */
export const voiceSettings = (version = 0) =>
  get<VoiceSettings>(version ? `/agent/voice/settings?version=${version}` : "/agent/voice/settings");

/** One of a vendor's English voices (agent/deets_agent/voice_catalog.py; Cartesia's and
 * ElevenLabs' in one shape): public catalog facts only. `accent`: the voice's own accent
 * (Cartesia: "en-US"; ElevenLabs: "american"); `locales`: every accent it speaks. */
export type VendorVoice = {
  id: string;
  name: string;
  tagline: string;
  description: string;
  gender: "masculine" | "feminine" | "gender_neutral" | null;
  accent: string | null;
  locales: string[];
};
/** A vendor's voice list as the dev server holds it (10 minutes). `checked_at`: the last good
 * read, or null. `fault`: why the last read failed (the vendor's status), or null. */
export type VendorVoices = { key: boolean; checked_at: string | null; fault: string | null; ttl_s: number; voices: VendorVoice[] };
/** A vendor's voice list (Cartesia's, ElevenLabs'). `fresh`: read it again now (Refresh), when
 * the held list is older than 15 s. */
export const voiceVoices = (vendor = "cartesia", fresh = false) =>
  get<VendorVoices>(`/agent/voice/voices?${new URLSearchParams({ vendor, ...(fresh ? { fresh: "true" } : {}) })}`);

/** Save the whole set as a new, active version (CONSOLE.md §7.3). A refusal's reason names a
 * setting, never a value, so the page may show it. */
export const saveVoice = (values: VoiceValues) => saveSettings<VoiceSettings>("/agent/voice/settings", values);

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
  /** This cycle's spend, from ehr (model/cost.rs); null while ehr is not up. */
  usage: AnthropicCycle | null;
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
/** `checked`: the scripts dry-run so far (absent from a dev server older than 2026-11-18). */
export type Estimate = { state: "running" | "done" | "failed"; missing: string[]; usd: number | null; checked?: number };

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
  /** Their live roles, by id (docs/architecture/ACCESS-ROLES.md). */
  role_ids: string[];
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
  /** The asker's own user id; null without a roles store. */
  me: string | null;
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
  // ehr's reasons are fixed words that name a field or a rule, never a value.
  if (!r.ok || !reply) {
    const why = roleRefusal(reply) ?? (typeof reply?.detail === "string" ? reply.detail : "not saved");
    throw new AgentDown(why, statusOf(r));
  }
  return reply;
}

// ---- access by role and the audit trail (docs/architecture/ACCESS-ROLES.md §5, §5.1, §6) ----

/** The levers, in access.rs's order. */
export const LEVERS = [
  "phi",
  "auth",
  "roles",
  "people",
  "agent",
  "vendors",
  "rules",
  "testing",
  "analytics",
  "flags",
  "audit_logs",
] as const;
export type Lever = (typeof LEVERS)[number];
export type Level = "none" | "view" | "edit";

const LEVER_NAMES: Record<Lever, string> = {
  phi: "PHI",
  auth: "Auth",
  roles: "Roles",
  people: "People",
  agent: "Agent",
  vendors: "Vendors",
  rules: "Rules",
  testing: "Testing",
  analytics: "Analytics",
  flags: "Flags",
  audit_logs: "Audit logs",
};
/** A lever as the console names it. */
export const leverName = (lever: Lever): string => LEVER_NAMES[lever] ?? lever;

/** Who you are (`GET /agent/me`): your id, your roles and every lever's level. `store`: false
 * without a roles store (then the launch token has Edit everywhere). Never your email. */
export type Me = {
  user_id: string | null;
  door: "launch" | "access";
  role_ids: string[];
  roles_version: number;
  levels: Record<Lever, Level>;
  store: boolean;
};
export const me = () => get<Me>("/agent/me");

/** One role (access.rs › Role): a level on every lever. */
export type Role = { id: string; name: string; domain: "hospital" | "team"; levels: Record<Lever, Level> };

/** Security › Roles: the versions, one version's roles, the active roles. */
export type Roles = {
  versions: SettingVersion[];
  version: SettingVersion | null;
  roles: Role[];
  active: Role[];
  levers: Lever[];
  levels: Level[];
};
export const roles = (version = 0) => get<Roles>(version ? `/agent/roles?version=${version}` : "/agent/roles");

/** Saves the whole list as a new, active version. A refusal names the role's place or the rule. */
export async function saveRoles(list: Role[]): Promise<Roles> {
  let r: Response;
  try {
    r = await fetch(at("/agent/roles"), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ roles: list }),
    });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  const reply = (await r.json().catch(() => null)) as (Roles & { detail?: unknown }) | null;
  if (!r.ok || !reply) {
    const why = roleRefusal(reply) ?? (typeof reply?.detail === "string" ? reply.detail : "not saved");
    throw new AgentDown(why, statusOf(r));
  }
  return reply;
}

/** Grants or revokes one role. The People view again; a refusal names the rule. */
export const grantRole = (person: string, role: string) =>
  peopleChange("POST", `/agent/people/${encodeURIComponent(person)}/roles/${encodeURIComponent(role)}`);
export const revokeRole = (person: string, role: string) =>
  peopleChange("DELETE", `/agent/people/${encodeURIComponent(person)}/roles/${encodeURIComponent(role)}`);

/** One audit row (audit.rs): ids and field names only. */
export type AuditRow = {
  seq: number;
  at: string;
  actor: string | null;
  source: "person" | "system" | "unknown";
  roles: string[];
  roles_version: number | null;
  lever: Lever | null;
  level: Level | null;
  action: string;
  object_kind: string | null;
  object_id: string | null;
  data_class: string | null;
  fields: string[];
  before_version: number | null;
  after_version: number | null;
  allowed: boolean;
};
/** Rows newest first: before `before` (0: from the newest); `changes`: changes and refusals only. */
export const audit = (before = 0, changes = false, limit = 100) =>
  get<{ rows: AuditRow[] }>(
    `/agent/audit?${new URLSearchParams({ before: String(before), limit: String(limit), changes: String(changes) })}`,
  );
export const auditVerify = () => get<{ ok: boolean; rows: number; first_bad: number | null }>("/agent/audit/verify");

/** One call in Analytics › Logs (ANALYTICS.md §3.1; ehr's `ListCalls`, dev_server.py › logs):
 * names and numbers, and a synthetic call's own label. */
export type LoggedCall = {
  id: string;
  started: string | null;
  origin: string;
  seat: string | null;
  ehr: string | null;
  mode: string | null;
  model: string | null;
  prompt_version: string | null;
  system_version: string | null;
  hospital_version: string | null;
  scenario: string | null;
  outcome: string;
  end_reason: string | null;
  reached: string[];
  turns: number;
  duration_ms: number;
  cost_usd: number;
  /** Each caller turn's wait, in ms. */
  turn_waits_ms: number[];
  tools: string[];
  tool_failed: boolean[];
  staff_flags: string[];
  review_flags: string[];
  label: string | null;
  ratings_passed: number;
  ratings_failed: number;
  /** Requests to Claude answered live, and from the replay cache (a kept-script draw). */
  requests_live: number;
  requests_replayed: number;
  /** Demo can play it: its record file is on the dev server. */
  record: boolean;
};
export type Logs = { calls: LoggedCall[]; class: "synthetic"; limit: number };

/** Analytics' sums (ANALYTICS.md §3.2; ehr's `CallStats`, dev_server.py › stats). `outcomes`:
 * booked, handed_off, abandoned, crashed, locked, ended, in that order. */
export type StatTotals = {
  calls: number;
  outcomes: number[];
  cost_usd: number;
  voice: VoiceCost;
  flagged_calls: number;
  staff_flags: number;
  review_flags: number;
  turns_answered: number;
  turns_within: number;
  wait_p50_ms: number;
  wait_p90_ms: number;
  replayed_calls: number;
};
/** The voice's cost of some calls, priced by ehr with the vendors crate (VOICE-VENDORS.md §9):
 * credits by vendor id, dollars at the active plans' prices, and `priced` (every vendor with
 * credits has a price, so `usd` is the whole voice). */
export type VoiceCost = { credits: { vendor: string; credits: number }[]; usd: number; priced: boolean };
export type Stats = {
  totals: StatTotals;
  prev: StatTotals | null;
  days: { day: string; calls: number; outcomes: number[]; cost_usd: number; voice: VoiceCost; staff_flags: number; review_flags: number }[];
  reached: { name: string; count: number }[];
  end_reasons: { name: string; count: number }[];
  models: { model: string; calls: number; cost_usd: number }[];
  costliest: { id: string; started: string; cost_usd: number; voice: VoiceCost; outcome: string }[];
  wait_bucket_ms: number;
  wait_hist: number[];
  stages: { turns: number; ink_ms: number; claude_ms: number; tools_ms: number; ehr_ms: number; voice_ms: number; other_ms: number } | null;
  tools: { tool: string; calls: number; failed: number; p50_ms: number; p90_ms: number }[];
  slowest: { call_id: string; started: string; turn: number; wait_ms: number }[];
  staff_flags: { kind: string; source: string; count: number; unverified: number }[];
  review_flags: { name: string; count: number }[];
  versions: { kind: string; label: string; calls: number; booked: number; turns_answered: number; turns_within: number; wait_p50_ms: number; wait_p90_ms: number; cost_usd: number }[];
  class: "synthetic";
  /** The range asked for, in days (null: all). */
  range_days: number | null;
  /** Each voice vendor's name, by its catalog id. */
  vendor_names: Record<string, string>;
};
/** The sums for the last `days` days (null: all) and the filter bar's other filters. */
export const stats = (days: number | null, filters: Record<string, string>) =>
  get<Stats>(`/agent/stats?${new URLSearchParams({ ...(days === null ? {} : { days: String(days) }), ...filters })}`);

/** One staff flag's words (PHI View; each read audited in ehr as `flag.view`). Protected data:
 * shown on the page only, never logged or put in a hint. */
export type FlagWords = { seq: number; kind: string; source: string; unverified: number; summary: string; evidence: string[] };
export const flagWords = (callId: string) => get<{ flags: FlagWords[] }>(`/agent/flags/${encodeURIComponent(callId)}`);
/** The calls of the last `days` days (0: today) on the dev server's clock; all when null. */
export const logs = (days: number | null) =>
  get<Logs>(`/agent/logs?${new URLSearchParams({ ...(days === null ? {} : { days: String(days) }), limit: "2000" })}`);

/** Records a look that changes nothing (a page opened) in the audit trail. Never throws: a look
 * the trail could not record does not stop the page. */
export async function logLook(lever: Lever, level: Level, action: string, objectKind = "", objectId = ""): Promise<void> {
  try {
    await fetch(at("/agent/log"), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ lever, level, action, object_kind: objectKind, object_id: objectId }),
    });
  } catch {
    /* the agent is down: the page says so through its stores */
  }
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
