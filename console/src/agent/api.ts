// The agent's dev server, through Vite's proxy (agent/deets_agent/dev_server.py; CHAT.md §5).
// Every path is on the page's own origin; the proxy adds the launch token.

/** One line of a dev call record (agent/deets_agent/call_record.py). */
export type CallEvent = { t_ms: number; kind: string; [field: string]: unknown };

export type Rating = {
  grader_kind: "human" | "model" | "code";
  grader: string;
  dimension: string;
  verdict: "pass" | "fail" | "unknown";
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

export const health = () => get<{ key: boolean; live: string | null }>("/agent/health");

/** The sandbox (ehr/fixtures/sandbox.json): synthetic patients and their visits. */
export type SandboxPatient = { id: string; given: string; family: string; birth_date: string; phone: string };
export type SandboxVisit = { patient: string; starts_at: string; minutes: number; clinician: string };
export type Sandbox = { place: string; patients: SandboxPatient[]; visits: SandboxVisit[] };
export const sandbox = () => get<Sandbox>("/agent/sandbox");
export const catalog = () => get<Catalog>("/agent/catalog");
export const callEvents = (id: string) =>
  get<{ id: string; events: CallEvent[] }>(`/agent/calls/${encodeURIComponent(id)}`).then((r) => r.events);

export async function annotate(a: Record<string, unknown>): Promise<void> {
  const r = await fetch("/agent/annotations", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(a),
  });
  if (!r.ok) throw new AgentDown("the annotation was refused");
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
