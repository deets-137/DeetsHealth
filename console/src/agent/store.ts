// The read layer (docs/architecture/LOADING.md §2–§4): one store per kind of data from the
// agent. A page reads a store; it never calls api.ts for a read. The rules live here, so no
// page writes them again:
//  1. The newest asked wins: a read answering after a write was asked is dropped.
//  2. A write wins over reads: its answer is the new value; reads in flight are dropped.
//  3. Writes to one store go one at a time, in click order.
//  4. One read in flight per store; an ask meanwhile reads once more when it lands.
//  5. A poll reads again `every` ms after each answer, never on a fixed timer.
//  6. `focus: true`: read again when the window comes back, at most every --read-focus-gap.
//  7. The agent did not answer: try again (--read-retry-first, doubling to --read-retry-max)
//     while a page on screen shows the store; when any read reaches the agent again, every
//     store that was down reads again.
//  8. Values live in memory only: never in browser storage, gone on a reload.
// Every read, write and poll tick goes in the ledger (ledger.ts), with no value in it.

import { AgentDown } from "./api";
import { record, type Op, type Outcome, type Why } from "./ledger";
import { tokenMs } from "../tokens";

/** `loading`: no answer yet. `ready`: a value. `down`: the agent did not answer. `refused`: it
 * answered with an error (`reason`: fixed words, never a value). A store that has a value keeps
 * it when a later read fails, so a page may still show it. */
export type Status = "loading" | "ready" | "down" | "refused";
export type State<T> = { status: Status; value: T | null; reason: string | null };

export type StoreOptions = {
  /** Read again when the window comes back (rule 6). */
  focus?: boolean;
  /** The read leaves this machine (Epic): never on focus, never holds the first-load wave. */
  leaves?: boolean;
};

type Sub<T> = { el: HTMLElement; fn: (s: State<T>) => void };

/** What the module-wide signals call on each store. */
interface Waker {
  wake(focus: boolean): void;
  backNow(): void;
}

const all = new Set<Waker>();

export class Store<T> {
  private state: State<T> = { status: "loading", value: null, reason: null };
  private readonly subs = new Set<Sub<T>>();
  /** Bumped by each write, before and after: a read asked before the bump is dropped. */
  private era = 0;
  private inFlight: Promise<void> | null = null;
  private again: Why | null = null;
  private writes: Promise<unknown> = Promise.resolve();
  private lastFocus = 0;
  private retryTimer = 0;
  private attempt = 0;
  /** A retry came due while no page showed the store: it runs when one does (`shown`). */
  private paused = false;
  private readonly settled: Promise<void>;
  /** Its first read joined the first-load hold. */
  private held = false;
  private settle: () => void = () => {};

  constructor(
    readonly name: string,
    private readonly fetcher: () => Promise<T>,
    readonly options: StoreOptions = {},
    /** The ledger's key: only a fixed word (the store's declared keys). */
    private readonly ledgerKey?: string,
  ) {
    this.settled = new Promise((r) => (this.settle = r));
    all.add(this);
  }

  get(): State<T> {
    return this.state;
  }

  /** Resolves when the store first leaves `loading` (any answer, or none). */
  ready(): Promise<void> {
    return this.settled;
  }

  /** Draws `fn` now and on each change. `el` is the piece of the page that shows the store: while
   * it is not on screen the store does not retry or poll. The first subscription reads. */
  subscribe(el: HTMLElement, fn: (s: State<T>) => void): () => void {
    const sub = { el, fn };
    this.subs.add(sub);
    fn(this.state);
    if (this.state.status === "loading" && !this.inFlight) void this.read("open");
    return () => this.subs.delete(sub);
  }

  /** Read now (rule 4: joins a read in flight, and reads once more after it). */
  refresh(why: Why = "user"): Promise<void> {
    return this.read(why);
  }

  /** A write whose answer is the store's new value (rules 2, 3). Rejects as the request did;
   * the store keeps its value then. */
  write(fn: () => Promise<T>, why: Why = "user"): Promise<T> {
    return this.queue(async () => {
      let value: T;
      try {
        value = await this.timed("write", why, fn);
      } catch (e) {
        // The write voided the reads in flight; read once more so the store is current.
        void this.read("after-write");
        throw e;
      }
      this.era++;
      this.set({ status: "ready", value, reason: null });
      return value;
    });
  }

  /** A write whose answer is not the value: the store reads again after it (rule 3). */
  act<R>(fn: () => Promise<R>, why: Why = "user"): Promise<R> {
    return this.queue(async () => {
      const r = await this.timed("write", why, fn);
      void this.read("after-write");
      return r;
    });
  }

  /** Read again `every` ms after each answer while `keep(value)` is true (rule 5). Returns a
   * function that stops it. Waits while no page on screen shows the store. */
  poll(every: number, keep: (v: T | null) => boolean): () => void {
    let stopped = false;
    let tick = 0;
    let timer = 0;
    const next = () => {
      if (stopped) return;
      timer = window.setTimeout(async () => {
        if (stopped) return;
        if (!this.onScreen()) return next(); // wait on, without a request
        await this.read("poll", ++tick);
        if (!stopped && keep(this.state.value)) next();
      }, every);
    };
    next();
    return () => {
      stopped = true;
      window.clearTimeout(timer);
    };
  }

  // ---- inside

  private queue<R>(fn: () => Promise<R>): Promise<R> {
    const run = this.writes.catch(() => {}).then(() => {
      this.era++; // reads asked before this write may hold the value from before it
      return fn();
    });
    this.writes = run;
    return run;
  }

  private onScreen(): boolean {
    if (document.hidden) return false;
    for (const s of this.subs) if (s.el.isConnected) return true;
    return false;
  }

  private set(s: State<T>): void {
    this.state = s;
    if (s.status !== "loading") this.settle();
    for (const sub of this.subs) {
      try {
        sub.fn(s);
      } catch (e) {
        console.error("[store]", this.name, e instanceof Error ? e.name : "error");
      }
    }
  }

  private log(op: Op, why: Why, t0: number, outcome: Outcome, extra: { status?: number; poll?: number } = {}): void {
    record({
      t: Math.round(performance.now()),
      store: this.name,
      ...(this.ledgerKey ? { key: this.ledgerKey } : {}),
      op,
      why,
      ms: Math.round(performance.now() - t0),
      outcome,
      ...(extra.status !== undefined ? { status: extra.status } : {}),
      ...(why === "retry" ? { attempt: this.attempt + 1 } : {}),
      ...(extra.poll !== undefined ? { poll: extra.poll } : {}),
    });
  }

  /** Runs a write and logs it; rethrows its error. */
  private async timed<R>(op: Op, why: Why, fn: () => Promise<R>): Promise<R> {
    const t0 = performance.now();
    try {
      const r = await fn();
      this.log(op, why, t0, "ok");
      if (wasDown.size) back();
      return r;
    } catch (e) {
      const { outcome, status } = classify(e);
      this.log(op, why, t0, outcome, { status });
      throw e;
    }
  }

  private read(why: Why, poll?: number): Promise<void> {
    if (this.inFlight) {
      this.again ??= why;
      return this.inFlight;
    }
    // A first read while the first-load hold is open joins the hold (LOADING.md §6), unless
    // it leaves this machine.
    if (holding && this.state.status === "loading" && !this.options.leaves && !this.held) {
      this.held = true;
      holding.push(this.settled);
    }
    const era = this.era;
    const t0 = performance.now();
    const op: Op = why === "poll" ? "poll" : "read";
    window.clearTimeout(this.retryTimer);
    this.inFlight = (async () => {
      try {
        const value = await this.fetcher();
        if (era !== this.era) {
          this.log(op, why, t0, "dropped", { poll });
          return;
        }
        this.log(op, why, t0, "ok", { poll });
        this.attempt = 0;
        wasDown.delete(this);
        this.set({ status: "ready", value, reason: null });
        if (wasDown.size) back();
      } catch (e) {
        const { outcome, status } = classify(e);
        this.log(op, why, t0, era !== this.era ? "dropped" : outcome, { status, poll });
        if (era !== this.era) return;
        if (outcome === "down") {
          this.set({ status: "down", value: this.state.value, reason: null });
          wasDown.add(this);
          this.retryLater();
        } else {
          const reason = e instanceof Error && e.message ? e.message : "refused";
          this.set({ status: "refused", value: this.state.value, reason });
        }
      } finally {
        this.inFlight = null;
        if (why === "focus" || why === "open") this.lastFocus = performance.now();
        const w = this.again;
        this.again = null;
        if (w) void this.read(w);
      }
    })();
    return this.inFlight;
  }

  private retryLater(): void {
    const first = tokenMs("--read-retry-first") || 500;
    const max = tokenMs("--read-retry-max") || 8000;
    const wait = Math.min(max, first * 2 ** this.attempt);
    window.clearTimeout(this.retryTimer);
    this.retryTimer = window.setTimeout(() => {
      if (!this.onScreen()) {
        this.paused = true;
        return;
      }
      this.attempt++;
      void this.read("retry");
    }, wait);
  }

  /** @internal: a page came on screen, or the window came back. */
  wake(focus: boolean): void {
    if (this.paused && this.onScreen()) {
      this.paused = false;
      void this.read("retry");
      return;
    }
    if (!focus || !this.options.focus || this.options.leaves || !this.onScreen()) return;
    const gap = tokenMs("--read-focus-gap") || 5000;
    if (performance.now() - this.lastFocus < gap) return;
    this.lastFocus = performance.now();
    void this.read("focus");
  }

  /** @internal: the agent is back; read now if this store was down. */
  backNow(): void {
    if (this.state.status !== "down") return;
    window.clearTimeout(this.retryTimer);
    this.paused = false;
    this.attempt = 0;
    if (this.subs.size) void this.read("back");
  }
}

/** The first-load hold (LOADING.md §6): the first answer of each store read while the screen
 * waits for its wave. Open from the page's load until reveal.ts closes it. */
let holding: Promise<void>[] | null = [];

/** The hold's answers so far; the list may grow while they are awaited (reveal.ts). */
export const heldReads = (): readonly Promise<void>[] => holding ?? [];

/** The wave starts: later first reads no longer join the hold. */
export function closeHold(): void {
  holding = null;
}

/** The stores that found the agent down, until one of them reads again. */
const wasDown = new Set<Waker>();

/** The agent answered: every store that was down reads again (rule 7). */
function back(): void {
  const stores = [...wasDown];
  wasDown.clear();
  for (const s of stores) s.backNow();
}

function classify(e: unknown): { outcome: Outcome; status?: number } {
  if (e instanceof AgentDown) return e.status === undefined ? { outcome: "down" } : { outcome: "refused", status: e.status };
  if (e instanceof TypeError) return { outcome: "down" }; // fetch's own "failed to fetch"
  return { outcome: "bad-reply" };
}

/** One store. `name` is a fixed word for the ledger. */
export function store<T>(name: string, read: () => Promise<T>, options: StoreOptions = {}): Store<T> {
  return new Store(name, read, options);
}

/** One store per key, made on first use. `words`: the keys the ledger may name (fixed words
 * only: an EHR's name, never a call id or a person id). `options` may differ per key. */
export function keyed<K extends string | number | null, T>(
  name: string,
  read: (k: K) => Promise<T>,
  opts: { words?: readonly string[]; options?: (k: K) => StoreOptions } = {},
): (k: K) => Store<T> {
  const map = new Map<string, Store<T>>();
  return (k: K) => {
    const id = String(k);
    let s = map.get(id);
    if (!s) {
      s = new Store(name, () => read(k), opts.options?.(k) ?? {}, opts.words?.includes(id) ? id : undefined);
      map.set(id, s);
    }
    return s;
  };
}

/** A one-off read that fills no store (a person's picture, cached by its own module): only
 * timed into the ledger under `name`. Rejects as the request did. */
export async function once<R>(name: string, fn: () => Promise<R>): Promise<R> {
  const t0 = performance.now();
  const log = (outcome: Outcome, status?: number) =>
    record({
      t: Math.round(performance.now()),
      store: name,
      op: "read",
      why: "open",
      ms: Math.round(performance.now() - t0),
      outcome,
      ...(status !== undefined ? { status } : {}),
    });
  try {
    const r = await fn();
    log("ok");
    return r;
  } catch (e) {
    const c = classify(e);
    log(c.outcome, c.status);
    throw e;
  }
}

/** A page or a view came on screen: paused retries run (nav.ts, subnav.ts). */
export function shown(): void {
  for (const s of all) s.wake(false);
}

window.addEventListener("focus", () => {
  for (const s of all) s.wake(true);
});
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) for (const s of all) s.wake(false);
});

/** The agent answered on another path (the watch socket opened): the down stores read again. */
export function agentUp(): void {
  if (wasDown.size) back();
}
