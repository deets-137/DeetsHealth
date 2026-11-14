// The ledger (docs/architecture/LOADING.md §5): one entry for every read, write and poll tick the
// console makes to the agent, as it ends. It is kept for debugging, telemetry and health later
// (the owner, 2026-11-14): Analytics › Speed and Logs, a connection mark, the deploy's tracing.
//
// No protected data, by construction: an entry holds a store's fixed name, a key only when the
// store declares it as a fixed word, and timings and outcomes. Never a value, a query string, a
// call id, a person id or a refusal's words. In memory only, for this page load.

/** What the request was. */
export type Op = "read" | "write" | "poll";
/** Why it was made. */
export type Why = "open" | "focus" | "after-write" | "poll" | "retry" | "back" | "user";
/** How it ended. `dropped`: it answered, but a newer read or a write had been asked since, so
 * its answer was not used. `bad-reply`: an answer the console could not read. */
export type Outcome = "ok" | "dropped" | "down" | "refused" | "bad-reply";

export type Entry = {
  /** ms since the page loaded (performance.now), when the answer came. */
  t: number;
  /** A fixed word: "people", "settings.cartesia", "resources". */
  store: string;
  /** Only when the store declares its keys as fixed words ("local", "epic"). */
  key?: string;
  op: Op;
  why: Why;
  /** From the request to the answer. */
  ms: number;
  outcome: Outcome;
  /** The HTTP status of a refusal. */
  status?: number;
  /** 1 for the first try; the retry's number after. */
  attempt?: number;
  /** The tick's number within its poll. */
  poll?: number;
};

/** One store's health over the ring (LOADING.md §5). */
export type Health = {
  store: string;
  /** When its last answer that was used came (ms since the page loaded); null for none yet. */
  lastOk: number | null;
  /** Failures in a row, up to now (`down`, `refused`, `bad-reply`). */
  failures: number;
  /** The median and the 95th percentile of `ms` over its last 50 answers; null for none. */
  p50: number | null;
  p95: number | null;
  /** Answers not used: a newer read or a write was asked first. */
  dropped: number;
  /** Every entry of this store in the ring. */
  count: number;
};

const RING = 500;
const WINDOW = 50;
const entries: Entry[] = [];
const listeners = new Set<(e: Entry) => void>();

/** Adds an entry (store.ts only). */
export function record(e: Entry): void {
  entries.push(e);
  if (entries.length > RING) entries.shift();
  for (const fn of listeners) {
    try {
      fn(e);
    } catch {
      /* a listener's error never stops the ledger */
    }
  }
}

/** The last `n` entries, oldest first. */
export const recent = (n = RING): Entry[] => entries.slice(-n);

/** Calls `fn` with each new entry. Returns a function that stops it. */
export function on(fn: (e: Entry) => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

const FAIL: ReadonlySet<Outcome> = new Set(["down", "refused", "bad-reply"]);

function quantile(sorted: number[], q: number): number | null {
  if (!sorted.length) return null;
  return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
}

/** Each store's health, from the ring. */
export function health(): Health[] {
  const by = new Map<string, Entry[]>();
  for (const e of entries) {
    const list = by.get(e.store) ?? [];
    list.push(e);
    by.set(e.store, list);
  }
  return [...by].map(([store, list]) => {
    let failures = 0;
    for (let i = list.length - 1; i >= 0 && FAIL.has(list[i].outcome); i--) failures++;
    const ok = list.filter((e) => e.outcome === "ok");
    const times = list
      .filter((e) => e.outcome === "ok" || e.outcome === "dropped")
      .slice(-WINDOW)
      .map((e) => e.ms)
      .sort((a, b) => a - b);
    return {
      store,
      lastOk: ok.length ? ok[ok.length - 1].t : null,
      failures,
      p50: quantile(times, 0.5),
      p95: quantile(times, 0.95),
      dropped: list.filter((e) => e.outcome === "dropped").length,
      count: list.length,
    };
  });
}
