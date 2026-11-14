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

/** Thrown when the dev server is not running (npm run dev:web) or refused. */
export class AgentDown extends Error {}

/** Thrown when the catalog's gate refuses an annotation. `reason` names a field, never a value
 * (catalog.py › AnnotationError), so the page may show it. */
export class AnnotationRefused extends Error {
  constructor(readonly reason: string) {
    super(reason);
  }
}

async function get<T>(path: string): Promise<T> {
  let r: Response;
  try {
    r = await fetch(path, { headers: { accept: "application/json" } });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  if (!r.ok) throw new AgentDown(r.status === 404 ? "not found" : "the agent's dev server refused");
  return (await r.json()) as T;
}

/** `epic`: whether the dev server gave ehr Epic's sandbox (CHAT.md §13). `voice`: whether
 * the Cartesia key is in .env (CHAT.md §17). */
export const health = () =>
  get<{ key: boolean; live: string | null; epic: boolean; voice: boolean }>("/agent/health");

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
export const resources = () => get<{ cartesia: CartesiaCycle; voice: boolean }>("/agent/resources");

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

/** The last Record and test (dev_server.py › record_and_test): counts and names of kept
 * scripts only. */
export type Recording = {
  state: "running" | "passed" | "failed";
  scripts: { name: string; ok: boolean }[];
  tests: boolean | null;
  spent_usd: number | null;
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
export type Estimate = { state: "running" | "done"; missing: string[]; usd: number | null };

/** Ask for the quote: a dry run with no Claude call, in the background. */
export async function estimatePrompt(): Promise<Estimate | null> {
  try {
    const r = await fetch("/agent/prompt/estimate", { method: "POST" });
    if (!r.ok) return null;
    return ((await r.json()) as { estimate: Estimate | null }).estimate;
  } catch {
    return null;
  }
}

export async function recordPrompt(): Promise<Recording> {
  let r: Response;
  try {
    r = await fetch("/agent/prompt/record", { method: "POST" });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  const body = (await r.json().catch(() => null)) as { detail?: unknown; recording?: Recording } | null;
  if (!r.ok || !body?.recording) throw new AgentDown(typeof body?.detail === "string" ? body.detail : "not started");
  return body.recording;
}

async function saveSettings<T>(path: string, values: unknown): Promise<T> {
  let r: Response;
  try {
    r = await fetch(path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ values }),
    });
  } catch {
    throw new AgentDown("the agent's dev server is not running");
  }
  if (!r.ok) {
    const body = (await r.json().catch(() => null)) as { detail?: unknown } | null;
    throw new AgentDown(typeof body?.detail === "string" ? body.detail : "not saved");
  }
  return (await r.json()) as T;
}

/** The voice seat's offer (CHAT.md §17): the browser's WebRTC offer, with every ICE candidate in
 * it; the answer comes back. A refusal's reason names no value, so the page may show it. */
export async function voiceOffer(
  ehr: EhrSource,
  offer: RTCSessionDescriptionInit,
): Promise<RTCSessionDescriptionInit> {
  let r: Response;
  try {
    r = await fetch(`/agent/voice?ehr=${ehr}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ type: offer.type, sdp: offer.sdp }),
    });
  } catch {
    throw new AgentDown("the agent did not answer");
  }
  if (!r.ok) {
    const body = (await r.json().catch(() => null)) as { detail?: unknown } | null;
    throw new AgentDown(typeof body?.detail === "string" ? body.detail : "the agent refused the call");
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
    r = await fetch("/agent/annotations", {
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
  if (!r.ok) throw new AgentDown("the agent's dev server refused");
}

export type WatchMessage =
  | { type: "hello"; live: string | null }
  | { type: "started"; call: string }
  | { type: "event"; call: string; event: CallEvent }
  | { type: "ended"; call: string };

/** The watch stream: the live call's events as the record writes them. Reconnects on its own
 * while the page is open. Returns a function that stops it. */
export function watch(on: (m: WatchMessage) => void, onUp?: (up: boolean) => void): () => void {
  let ws: WebSocket | undefined;
  let stopped = false;
  let retry = 0;
  const open = () => {
    const scheme = location.protocol === "https:" ? "wss:" : "ws:";
    ws = new WebSocket(`${scheme}//${location.host}/agent/watch`);
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
