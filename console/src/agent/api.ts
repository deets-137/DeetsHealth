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
