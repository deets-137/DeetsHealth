// The Chat page's Timeline tab (CHAT.md §18; the owner's fork, 2026-11-22: a waterfall per turn).
// One block per turn: the greeting, then each caller line. Each block draws, on its own time axis
// from the moment the turn began, where the wait went: Ink's end of turn (voice), each request to
// Claude (with its first token), each tool, the ehr and Epic calls inside it, and the moment the
// caller hears the answer. A line on each axis marks the target for a turn (TURN_TARGET_MS).
//
// Names, kinds and times only, from the call's events. The caller's line heads its block, as the
// Tools tab shows it; nothing else from the call's words. Made new for each call.
import type { CallEvent } from "../agent/api";

/** The target for one turn, from the caller's silence to the first word they hear
 * (DEBUGGING.md §1, "the 800 ms target per turn"). */
const TURN_TARGET_MS = 800;

type BarKind = "voice" | "claude" | "tool" | "ehr" | "epic";

type Bar = {
  kind: BarKind;
  label: string;
  from: number;
  /** Undefined while it runs. */
  to?: number;
  /** Inside a tool's run (an ehr or Epic call): drawn indented under it. */
  nested?: boolean;
  /** Claude's first token, on the call's clock. */
  first?: number;
  failed?: boolean;
};

type Mark = { label: string; at: number };

type Turn = {
  /** 0 for the greeting. */
  n: number;
  /** When the turn began, on the call's clock: the caller's silence (voice), their line (text),
   * or the call's start (the greeting). */
  t0: number;
  line?: string;
  bars: Bar[];
  marks: Mark[];
  /** When the caller hears the answer: the seat speaking (voice), else the first frame out. */
  end?: number;
  /** True when `end` is the seat speaking (a voice call's true end of the wait). */
  heard?: boolean;
  /** Opened by a caller event, not by Ink's final line. */
  fromCaller?: boolean;
  el: HTMLElement;
};

export class Timeline {
  private turns: Turn[] = [];
  /** A voice turn opened by Ink's final line, waiting for the caller event that carries it. */
  private awaitingLine = false;
  /** Tools still running, by name, oldest first. */
  private running = new Map<string, Bar[]>();
  private ended = false;

  constructor(private readonly box: HTMLElement) {
    box.replaceChildren(emptyNote("No call yet"));
  }

  add(e: CallEvent): void {
    const t = num(e.t_ms) ?? 0;
    switch (e.kind) {
      case "call_start":
        this.open(0);
        break;
      case "voice":
        this.voice(e, t);
        break;
      case "caller": {
        const line = typeof e.text === "string" ? e.text : undefined;
        if (this.awaitingLine) {
          const turn = this.current();
          turn.line = line;
          this.awaitingLine = false;
          this.redraw(turn);
        } else {
          const turn = this.open(t);
          turn.line = line;
          turn.fromCaller = true;
          this.redraw(turn);
        }
        break;
      }
      case "llm": {
        const total = num(e.total_ms);
        const from = total === undefined ? t : t - total;
        const firstIn = num(e.first_text_ms) ?? num(e.first_tool_ms);
        this.bar({
          kind: "claude",
          label: e.hit ? "Claude (replayed)" : "Claude",
          from,
          to: t,
          first: firstIn === undefined ? undefined : from + firstIn,
          failed: e.error !== undefined && e.error !== null,
        });
        break;
      }
      case "tool_call": {
        const name = typeof e.name === "string" ? e.name : "tool";
        const b: Bar = { kind: "tool", label: name, from: t };
        this.running.set(name, [...(this.running.get(name) ?? []), b]);
        this.bar(b);
        break;
      }
      case "tool_result": {
        const name = typeof e.name === "string" ? e.name : "tool";
        const b = this.running.get(name)?.shift();
        if (b) {
          const ms = num(e.ms);
          b.to = ms === undefined ? t : b.from + ms;
          b.failed = !!e.error;
          this.redraw(this.turnAt(b.from));
        }
        break;
      }
      case "ehr": {
        const ms = num(e.ms) ?? 0;
        const call = typeof e.call === "string" ? e.call : "call";
        this.bar({ kind: "ehr", label: `ehr ${call}`, from: t - ms, to: t, nested: true, failed: e.outcome !== "ok" });
        break;
      }
      case "epic": {
        // ehr's own requests, sent at the call's end (CloseCall's ledger). `at_ms` is on ehr's
        // clock, which starts at OpenCall, a few ms after the call: close enough to place it.
        const at = num(e.at_ms);
        if (at === undefined) break;
        const what = typeof e.what === "string" ? e.what : "request";
        const status = num(e.status);
        this.bar({
          kind: "epic",
          label: `Epic ${what}`,
          from: at,
          to: at + (num(e.ms) ?? 0),
          nested: true,
          failed: status !== undefined && status >= 400,
        });
        break;
      }
      case "agent": {
        // `first_ms` is on the call's clock; the event itself comes later (the turn grace), so
        // the mark goes to the turn its time falls in, not to the newest.
        const first = num(e.first_ms) ?? t;
        const turn = this.turnAt(first);
        if (!turn.marks.some((m) => m.label === "First words out")) {
          turn.marks.push({ label: "First words out", at: first });
          if (turn.end === undefined) turn.end = first;
          this.redraw(turn);
        }
        break;
      }
      case "call_end":
        this.ended = true;
        this.turns.forEach((x) => this.redraw(x));
        break;
    }
  }

  private voice(e: CallEvent, t: number): void {
    if (e.stage === "heard") {
      const last = num(e.last_words_ms);
      // Ink's final line and the screen's caller event come together, in either order. When the
      // caller event came first (within a second), this is its turn: it begins at the silence.
      const cur = this.current();
      let turn: Turn;
      if (cur.fromCaller &&!cur.bars.some((b) => b.kind === "voice") && t - cur.t0 < 1000) {
        turn = cur;
        turn.t0 = Math.min(turn.t0, last ?? t);
      } else {
        turn = this.open(last ?? t);
        this.awaitingLine = true;
      }
      if (last !== undefined) {
        turn.bars.push({ kind: "voice", label: "Ink: end of turn", from: last, to: t });
        this.redraw(turn);
      }
      return;
    }
    const turn = this.current();
    if (e.stage === "sonic_audio" && !turn.marks.some((m) => m.label === "Sonic: first audio")) {
      turn.marks.push({ label: "Sonic: first audio", at: t });
    } else if (e.stage === "speaking" && !turn.heard) {
      turn.marks.push({ label: "Caller hears", at: t });
      turn.end = t;
      turn.heard = true;
    } else {
      return;
    }
    this.redraw(turn);
  }

  /** The newest turn; the greeting when a record starts without `call_start`. */
  private current(): Turn {
    return this.turns[this.turns.length - 1] ?? this.open(0);
  }

  /** The turn a time falls in: the last that began at or before it. */
  private turnAt(at: number): Turn {
    for (let i = this.turns.length - 1; i >= 0; i--) if (this.turns[i].t0 <= at) return this.turns[i];
    return this.turns[0] ?? this.open(0);
  }

  private open(t0: number): Turn {
    const el = document.createElement("section");
    el.className = "tl-turn";
    const turn: Turn = { n: this.turns.length, t0, bars: [], marks: [], el };
    this.turns.push(turn);
    const b = this.scroller();
    const atEnd = b.scrollHeight - b.scrollTop - b.clientHeight < 8;
    this.box.querySelector(":scope > .empty")?.remove();
    this.box.append(el);
    this.redraw(turn);
    if (atEnd) b.scrollTop = b.scrollHeight;
    return turn;
  }

  private bar(b: Bar): void {
    const turn = this.turnAt(b.from);
    // An ehr or Epic call is indented only inside a tool's run (OpenCall, say, is in none).
    if (b.nested) {
      b.nested = turn.bars.some((x) => x.kind === "tool" && x.from <= b.from && (x.to === undefined || x.to >= (b.to ?? b.from)));
    }
    turn.bars.push(b);
    this.redraw(turn);
  }

  /** The list scrolls in its tab's body (CHAT.md §15). */
  private scroller(): HTMLElement {
    return this.box.closest<HTMLElement>(".tab-body") ?? this.box;
  }

  private redraw(turn: Turn | undefined): void {
    if (!turn) return;
    const latest = Math.max(
      turn.end ?? 0,
      ...turn.bars.map((b) => b.to ?? b.from),
      ...turn.marks.map((m) => m.at),
    );
    // Each axis runs from the turn's start to its last moment, or the target if that is later.
    const span = Math.max(latest - turn.t0, TURN_TARGET_MS);
    const at = (x: number) => `${(Math.min(Math.max(x - turn.t0, 0), span) / span) * 100}%`;

    const head = document.createElement("header");
    head.className = "tl-head";
    const title = document.createElement("span");
    title.className = "tl-title";
    title.textContent = turn.n === 0 ? "Greeting" : `Turn ${turn.n}`;
    const total = document.createElement("span");
    total.className = "tl-total";
    if (turn.end === undefined) {
      total.textContent = this.ended ? "no answer" : "waiting…";
    } else if (turn.n === 0) {
      // The greeting waits on the call's set-up, not on a caller's line: no verdict.
      total.textContent = msWords(turn.end - turn.t0);
    } else {
      const wait = turn.end - turn.t0;
      const over = wait > TURN_TARGET_MS;
      total.textContent = `${msWords(wait)} · ${over ? "over" : "within"} ${msWords(TURN_TARGET_MS)}`;
      total.classList.toggle("is-over", over);
    }
    head.append(title, total);
    const parts: HTMLElement[] = [head];
    if (turn.line) {
      const line = document.createElement("p");
      line.className = "tl-line";
      line.textContent = `“${turn.line}”`;
      parts.push(line);
    }

    const rows = document.createElement("div");
    rows.className = "tl-rows";
    const row = (label: string, nested: boolean, time: string, draw: (track: HTMLElement) => void) => {
      const r = document.createElement("div");
      r.className = `tl-row${nested ? " is-nested" : ""}`;
      const l = document.createElement("span");
      l.className = "tl-label";
      l.textContent = label;
      l.title = label;
      const track = document.createElement("span");
      track.className = "tl-track";
      track.setAttribute("aria-hidden", "true");
      const target = document.createElement("span");
      target.className = "tl-target";
      target.style.left = at(turn.t0 + TURN_TARGET_MS);
      track.append(target);
      draw(track);
      const tm = document.createElement("span");
      tm.className = "tl-time";
      tm.textContent = time;
      r.append(l, track, tm);
      rows.append(r);
    };
    const rel = (x: number) => msWords(Math.max(x - turn.t0, 0));
    for (const b of [...turn.bars].sort((x, y) => x.from - y.from)) {
      const time = b.to === undefined ? `from ${rel(b.from)}` : `${rel(b.from)} – ${rel(b.to)}`;
      row(b.label, !!b.nested, b.failed ? `${time} · failed` : time, (track) => {
        const bar = document.createElement("span");
        bar.className = `tl-bar tl-bar--${b.kind}${b.failed ? " is-failed" : ""}${b.to === undefined ? " is-running" : ""}`;
        bar.style.left = at(b.from);
        bar.style.right = `calc(100% - ${at(b.to ?? latest)})`;
        track.append(bar);
        if (b.first !== undefined) {
          const tick = document.createElement("span");
          tick.className = "tl-tick";
          tick.style.left = at(b.first);
          track.append(tick);
        }
      });
    }
    for (const m of [...turn.marks].sort((x, y) => x.at - y.at)) {
      row(m.label, false, rel(m.at), (track) => {
        const pin = document.createElement("span");
        pin.className = "tl-pin";
        pin.style.left = at(m.at);
        track.append(pin);
      });
    }
    if (!turn.bars.length && !turn.marks.length) rows.append(emptyNote(this.ended ? "Nothing timed" : "waiting…"));
    parts.push(rows);
    turn.el.replaceChildren(...parts);
  }
}

function num(v: unknown): number | undefined {
  return typeof v === "number" && Number.isFinite(v) ? v : undefined;
}

/** A span in words: whole ms under a second ("610 ms"), else seconds to the hundredth ("1.42 s"). */
function msWords(ms: number): string {
  return ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(2)} s`;
}

function emptyNote(text: string): HTMLElement {
  const p = document.createElement("p");
  p.className = "empty";
  p.textContent = text;
  return p;
}
