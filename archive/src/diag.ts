// Lightweight diagnostics — an in-memory ring buffer of timestamped events plus a
// console handle (window.__diag). The buffer ALWAYS records (bounded, cheap); the
// console echo is opt-in via localStorage "deets.debug" = "1" (or __diag.echo(true)).
//
// It reaches disk through `flush()` → the Rust `diag_flush` command, which appends
// `report()` to the rolling app log (LOGGING.md in DeetsMusic). The triggers are an uncaught
// error (throttled), `beforeunload`, and a five-minute timer — so a crash still leaves a trace.
//
// **The PHI wall (SECURITY.md §4.5).** Everything here can reach the log file, so every
// `tag` and every `data` object carries UI words, counts, kinds and durations only. Never a
// value from a record, a name, an id, a code, a date of care, a URL with a query string or
// a real site's name. "3 lab results", never what they say. The auto-captured errors below
// keep the error's KIND and where it happened, never its message: a message can quote the
// value that caused it.

import { invoke } from "@tauri-apps/api/core";

export interface DiagEvent {
  n: number; // sequence number, monotonic for the session — what a flush counts from
  t: number; // ms since page load (monotonic)
  tag: string; // e.g. "grow", "boot:ready"
  data?: unknown; // small JSON-able snapshot — counts and kinds only (header)
}

const CAP = 300;
const buffer: DiagEvent[] = [];
let seq = 0; // the last number handed out; `flushedSeq` is how far the file has it
let flushedSeq = 0;

let echo = false;
try {
  echo = localStorage.getItem("deets.debug") === "1";
} catch {
  /* storage disabled */
}

export function log(tag: string, data?: unknown): void {
  buffer.push({ n: ++seq, t: Math.round(performance.now()), tag, data });
  if (buffer.length > CAP) buffer.shift();
  if (echo) console.debug(`[diag] ${tag}`, data ?? "");
}

// ── Warnings and errors reach the log file as they happen ─────────────────────────
// `log()` only fills the ring. `warn()` / `error()` also write one line through the Rust
// `log_event` command (rate-limited there). Same tag + data within DEDUPE_MS is sent once.
type Level = "warn" | "error";
const DEDUPE_MS = 2000;
let lastSent = { key: "", at: -Infinity };
function send(level: Level, tag: string, data?: unknown): void {
  let text: string | undefined;
  try {
    text = data === undefined ? undefined : JSON.stringify(data);
  } catch {
    text = "[not serializable]";
  }
  const key = `${level}|${tag}|${text ?? ""}`;
  const now = performance.now();
  if (key === lastSent.key && now - lastSent.at < DEDUPE_MS) return;
  lastSent = { key, at: now };
  invoke("log_event", { level, tag, data: text }).catch(() => {
    /* not under Tauri */
  });
}

export function warn(tag: string, data?: unknown): void {
  log(tag, data);
  send("warn", tag, data);
}

export function error(tag: string, data?: unknown): void {
  log(tag, data);
  send("error", tag, data);
}

export function events(): DiagEvent[] {
  return buffer.slice();
}

export function setEcho(on: boolean): void {
  echo = on;
  try {
    localStorage.setItem("deets.debug", on ? "1" : "0");
  } catch {
    /* storage disabled */
  }
}

/** A text report of the recent events. `since` (a sequence number) keeps only the events
 *  after it, which is what a flush writes; the default is the whole ring. */
export function report(since = 0): string {
  const events = since ? buffer.filter((e) => e.n > since) : buffer;
  const header = `DeetsHealth diag — ${new Date().toISOString()} — ${events.length} events`;
  const lines = events.map(
    (e) => `${String(e.t).padStart(8)}ms  ${e.tag}${e.data !== undefined ? "  " + JSON.stringify(e.data) : ""}`,
  );
  return [header, ...lines].join("\n");
}

/**
 * Append the events the file does not have yet to the app log. Fire-and-forget; never
 * throws. It writes only what is NEW: nothing new means nothing written, so the timer
 * below stays silent on an idle app. `all` re-writes the whole ring (`__diag.flush(true)`).
 */
export function flush(all = false): Promise<void> {
  if (!all && seq === flushedSeq) return Promise.resolve();
  const since = all ? 0 : flushedSeq;
  flushedSeq = seq;
  return invoke<void>("diag_flush", { text: report(since) }).catch((e) => {
    console.warn("[diag] flush failed", e); // not under Tauri, or the command is missing
  });
}

// ── The timer: the ring lands every 5 minutes, gated on new events ─────────────
const FLUSH_EVERY_MS = 5 * 60 * 1000;
window.setInterval(() => void flush(), FLUSH_EVERY_MS);

// One uncaught error tends to bring friends; write the buffer once per burst.
const FLUSH_GAP_MS = 5000;
let lastErrorFlush = -Infinity;
function flushOnError(): void {
  const now = performance.now();
  if (now - lastErrorFlush < FLUSH_GAP_MS) return;
  lastErrorFlush = now;
  flush();
}

/** The script file and line only: a path in our own bundle, never a query string. */
const where = (src: string | undefined, line: number | undefined): string => {
  const file = (src ?? "").split(/[?#]/)[0].split("/").pop() ?? "";
  return file ? `${file}:${line ?? 0}` : "";
};
/** An error's kind (TypeError, RangeError…), never its message. */
const kindOf = (e: unknown): string => (e instanceof Error ? e.name : typeof e);

// Auto-capture uncaught errors and promise rejections. Checked on the next task so a later
// listener that swallows a KNOWN benign case (preventDefault) keeps it out of the file.
window.addEventListener("error", (e) => {
  setTimeout(() => {
    const data = { kind: kindOf(e.error), at: where(e.filename, e.lineno) };
    if (e.defaultPrevented) return log("window:error", { ...data, swallowed: true });
    error("window:error", data);
    flushOnError();
  }, 0);
});
window.addEventListener("unhandledrejection", (e) => {
  setTimeout(() => {
    const data = { kind: kindOf(e.reason) };
    if (e.defaultPrevented) return log("window:unhandledrejection", { ...data, swallowed: true });
    error("window:unhandledrejection", data);
    flushOnError();
  }, 0);
});
// On unload, whatever the timer has not written yet.
window.addEventListener("beforeunload", () => {
  flush();
});

// Console handle (available in release too, so a desk test can read the ring).
(window as unknown as { __diag: unknown }).__diag = {
  events,
  report,
  flush,
  dump: () => console.table(buffer),
  /** The text a flush would write now — the events the log file does not have yet. */
  pending: () => report(flushedSeq),
  clear: () => {
    buffer.length = 0;
  },
  echo: setEcho,
};
