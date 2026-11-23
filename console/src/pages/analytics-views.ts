// Analytics' views on the sums (ANALYTICS.md §3.2; the owner, 2026-11-22): Home, Outcome and Cost
// for an executive (few numbers, large, each with its change since the period before, each
// opening the view under it); Speed and Flags for an analyst (breakdowns, the slowest and the
// worst, every row leading to Logs). The sums come from ehr (`CallStats`); every chart is the
// kit's (analytics-kit.ts). Hints and lines name counts, times and dollars, never a word of a
// call. Flags shows a flag's own words only to PHI View, each read audited in ehr (`flag.view`).
import { flagWords, type FlagWords, type LoggedCall } from "../agent/api";
import { logs } from "../agent/stores";
import { may, onAccess } from "../access";
import { once } from "../agent/store";
import { filters, flagged as isFlagged, onFilters, passes, setFilters, TURN_TARGET_MS } from "./analytics-filter";
import {
  allDays,
  allUsd,
  bars,
  voiceCredits,
  voiceText,
  voiceUsd,
  change,
  columns,
  count,
  dollars,
  histogram,
  list,
  OUTCOMES,
  pct,
  rangeStart,
  section,
  share,
  span,
  split,
  statsView,
  tiles,
  toLogs,
  words,
  type Drawn,
} from "./analytics-kit";
import { outcomeWords, stepWord } from "./chat";
import { findInLogs } from "./logs";
import { el } from "./page";
import { goView, type View } from "./subnav";

/** The Flow's steps in the order a call meets them (flow.py › STEPS); others after. */
const STEP_ORDER = ["greet", "verify", "menu", "schedule", "goodbye", "hand_off"];

const outcomeSeries = OUTCOMES.map((o) => ({ name: o.word, cls: o.cls }));

/** Calls per day, stacked by outcome; a part opens Logs on that outcome. */
function outcomeDays(d: Drawn): HTMLElement {
  const days = allDays(
    d.stats.days.map((x) => ({ day: x.day, parts: x.outcomes })),
    rangeStart(d.f),
    (day) => ({ day, parts: OUTCOMES.map(() => 0) }),
  );
  return columns("Calls per day, by outcome", days, outcomeSeries, (n) => count(n, "call"), (i) => toLogs(d.view, { outcome: OUTCOMES[i].key }));
}

// ---- Home (executive)

export function homeView(): View {
  return statsView("home", "Home", (d) => {
    const s = d.stats;
    const [t, p] = [s.totals, s.prev];
    const booked = t.outcomes[0];
    const usd = allUsd(t);
    const flaggedShare = share(t.flagged_calls, t.calls);
    return [
      tiles([
        { label: "Calls", value: t.calls.toLocaleString(), sub: "answered by the agent", delta: change(d.f, t.calls, p?.calls, null, "count") },
        {
          label: "Booked",
          value: pct(booked, t.calls),
          sub: `${booked} of ${count(t.calls, "call")}`,
          delta: change(d.f, share(booked, t.calls) ?? 0, p ? share(p.outcomes[0], p.calls) : null, true, "points"),
          open: () => goView(d.view, "outcome"),
        },
        {
          label: "Cost per call",
          value: dollars(usd / t.calls),
          sub: t.voice.priced ? "Claude and the voice" : "Claude only: a voice plan has no price set",
          delta: change(d.f, usd / t.calls, p && p.calls ? allUsd(p) / p.calls : null, false, "dollars"),
          open: () => goView(d.view, "cost"),
        },
        {
          label: "Typical wait",
          value: t.turns_answered ? span(t.wait_p50_ms) : "—",
          sub: `the median of ${count(t.turns_answered, "turn")}`,
          delta: t.turns_answered ? change(d.f, t.wait_p50_ms, p?.turns_answered ? p.wait_p50_ms : null, false, "ms") : null,
          open: () => goView(d.view, "speed"),
        },
        {
          label: "Flagged calls",
          value: t.flagged_calls.toLocaleString(),
          sub: flaggedShare === null ? "—" : `${Math.round(flaggedShare)}% of calls`,
          delta: change(d.f, t.flagged_calls, p?.flagged_calls, false, "count"),
          open: () => goView(d.view, "flags"),
        },
      ]),
      section("Calls per day", "Each column is a day, split by how its calls ended. A part opens those calls in Logs.", outcomeDays(d)),
    ];
  });
}

// ---- Outcome (executive)

export function outcomeView(): View {
  return statsView("outcome", "Outcome", (d) => {
    const s = d.stats;
    const [t, p] = [s.totals, s.prev];
    const at = (k: string) => OUTCOMES.findIndex((o) => o.key === k);
    const tile = (k: string, up: boolean | null) => {
      const i = at(k);
      return {
        label: OUTCOMES[i].word,
        value: pct(t.outcomes[i], t.calls),
        sub: `${t.outcomes[i]} of ${count(t.calls, "call")}`,
        delta: change(d.f, share(t.outcomes[i], t.calls) ?? 0, p ? share(p.outcomes[i], p.calls) : null, up, "points"),
        open: () => toLogs(d.view, { outcome: k }),
      };
    };
    const steps = [...s.reached].sort((a, b) => {
      const ia = STEP_ORDER.indexOf(a.name);
      const ib = STEP_ORDER.indexOf(b.name);
      return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib) || a.name.localeCompare(b.name);
    });
    return [
      tiles([tile("booked", true), tile("handed_off", null), tile("abandoned", false), tile("crashed", false)]),
      section("Outcomes per day", "A part opens those calls in Logs.", outcomeDays(d)),
      section(
        "Where calls reach",
        "How many calls reached each step of the call. A drop between two steps is where callers stop.",
        bars(steps.map((r) => ({ label: stepWord(r.name), value: r.count, text: `${r.count} · ${pct(r.count, t.calls)}` }))),
      ),
      section(
        "How calls end",
        "The end reasons, most first (at most 10).",
        bars(s.end_reasons.map((r) => ({ label: r.name, value: r.count, text: String(r.count), cls: "ch-quiet" }))),
      ),
    ];
  });
}

// ---- Cost (executive)

export function costView(): View {
  return statsView("cost", "Cost", (d) => {
    const s = d.stats;
    const [t, p] = [s.totals, s.prev];
    const priced = t.voice.priced;
    const usd = allUsd(t);
    const booked = t.outcomes[0];
    const claude = { name: "Claude", cls: "ch-claude" };
    const voice = { name: "Voice", cls: "ch-voice" };
    const days = allDays(
      s.days.map((x) => ({ day: x.day, parts: priced ? [x.cost_usd, voiceUsd(x.voice) ?? 0] : [x.cost_usd] })),
      rangeStart(d.f),
      (day) => ({ day, parts: priced ? [0, 0] : [0] }),
    );
    const voiceWords = voiceText(s, t.voice, dollars);
    return [
      tiles([
        {
          label: "Total",
          value: dollars(usd),
          sub: `Claude ${dollars(t.cost_usd)} · Voice ${voiceWords}`,
          delta: change(d.f, usd, p ? allUsd(p) : null, false, "dollars"),
        },
        {
          label: "Per call",
          value: dollars(usd / t.calls),
          sub: `over ${count(t.calls, "call")}`,
          delta: change(d.f, usd / t.calls, p && p.calls ? allUsd(p) / p.calls : null, false, "dollars"),
        },
        {
          label: "Per booking",
          value: booked ? dollars(usd / booked) : "—",
          sub: `over ${count(booked, "booking")}`,
          delta: booked ? change(d.f, usd / booked, p && p.outcomes[0] ? allUsd(p) / p.outcomes[0] : null, false, "dollars") : null,
        },
        {
          label: "Voice",
          value: voiceWords,
          sub: priced ? voiceCredits(s, t.voice) : "Set each plan's price in Config › Voice for dollars",
        },
      ]),
      section(
        "Dollars per day",
        priced ? "Claude and the voice, each day." : "Claude only: the voice shows in dollars once each plan has its price in Config › Voice.",
        columns("Dollars per day", days, priced ? [claude, voice] : [claude], dollars),
      ),
      section(
        "By model",
        "Claude's dollars by the model the calls ran on.",
        bars(s.models.map((m) => ({ label: m.model || "Not recorded", value: m.cost_usd, text: `${dollars(m.cost_usd)} · ${count(m.calls, "call")}`, cls: "ch-claude" }))),
      ),
      section(
        "The most costly calls",
        "A row opens the call in Logs.",
        list(
          ["Started", "Outcome", "Claude", "Voice"],
          s.costliest.map((c) => ({
            cells: [
              c.started ? c.started.replace("T", " ").slice(5, 16) : c.id,
              outcomeWords(c.outcome),
              dollars(c.cost_usd),
              voiceText(s, c.voice, dollars),
            ],
            open: () => findInLogs(d.view, c.id),
          })),
        ),
      ),
    ];
  });
}

// ---- Speed (analyst)

const STAGES = [
  { key: "ink_ms", name: "Ink: end of turn", cls: "ch-ink" },
  { key: "claude_ms", name: "Claude", cls: "ch-claude" },
  { key: "tools_ms", name: "Tools", cls: "ch-tools" },
  { key: "ehr_ms", name: "EHR", cls: "ch-ehr" },
  { key: "voice_ms", name: "Voice out", cls: "ch-voice" },
  { key: "other_ms", name: "Other", cls: "ch-quiet" },
] as const;

const VERSION_KINDS: [string, string][] = [
  ["prompt", "Words"],
  ["system", "System"],
  ["hospital", "Hospital"],
  ["model", "Model"],
];
let compareBy = "prompt";

export function speedView(): View {
  return statsView("speed", "Speed", (d) => {
    const s = d.stats;
    const [t, p] = [s.totals, s.prev];
    const within = share(t.turns_within, t.turns_answered);
    const out: HTMLElement[] = [
      tiles([
        {
          label: "Median wait",
          value: t.turns_answered ? span(t.wait_p50_ms) : "—",
          sub: `of ${count(t.turns_answered, "turn")}`,
          delta: t.turns_answered ? change(d.f, t.wait_p50_ms, p?.turns_answered ? p.wait_p50_ms : null, false, "ms") : null,
        },
        {
          label: "90th percentile",
          value: t.turns_answered ? span(t.wait_p90_ms) : "—",
          sub: "9 turns in 10 are faster",
          delta: t.turns_answered ? change(d.f, t.wait_p90_ms, p?.turns_answered ? p.wait_p90_ms : null, false, "ms") : null,
        },
        {
          label: `Within ${span(TURN_TARGET_MS)}`,
          value: within === null ? "—" : `${Math.round(within)}%`,
          sub: `${t.turns_within} of ${count(t.turns_answered, "turn")}`,
          delta: within === null ? null : change(d.f, within, p ? share(p.turns_within, p.turns_answered) : null, true, "points"),
          open: () => toLogs(d.view, { slow: true }),
        },
      ]),
    ];
    if (!t.turns_answered) {
      out.push(el(`<p class="empty">No answered turn to time: replayed calls are left out</p>`));
      return out;
    }
    out.push(
      section(
        "The spread of waits",
        `Each column is ${span(s.wait_bucket_ms)} of wait; the line is ${span(TURN_TARGET_MS)}, and columns past it are over.`,
        histogram(s.wait_hist, s.wait_bucket_ms),
      ),
    );
    const st = s.stages;
    if (st && st.turns) {
      out.push(
        section(
          "Where the wait goes",
          `The mean of each stage over ${count(st.turns, "turn")}. Ink and Voice out are measured on voice calls since 2026-11-22 only.`,
          split(
            "Where the wait goes",
            STAGES.map((x) => ({ series: { name: x.name, cls: x.cls }, value: st[x.key], text: span(st[x.key]) })),
          ),
        ),
      );
    }
    out.push(
      section(
        "The slowest tools",
        "Each tool's own run. A row opens the calls that used it in Logs.",
        list(
          ["Tool", "Calls", "Failed", "Median", "90th"],
          s.tools.map((x) => ({
            cells: [x.tool, String(x.calls), String(x.failed), span(x.p50_ms), span(x.p90_ms)],
            cls: x.failed ? "is-worse" : undefined,
            open: () => findInLogs(d.view, x.tool),
          })),
        ),
      ),
      section(
        "The slowest turns",
        "A row opens its call in Logs; its panel's Open in Chat shows the Timeline.",
        list(
          ["Started", "Turn", "Wait"],
          s.slowest.map((x) => ({
            cells: [x.started ? x.started.replace("T", " ").slice(5, 16) : x.call_id, String(x.turn), span(x.wait_ms)],
            cls: x.wait_ms > TURN_TARGET_MS ? "is-worse" : undefined,
            open: () => findInLogs(d.view, x.call_id),
          })),
        ),
      ),
    );
    // Compare by version: the same numbers for each label of one kind. A row filters every view.
    const pick = document.createElement("select");
    pick.className = "cfg-field an-select";
    pick.setAttribute("aria-label", "Compare by");
    for (const [k, w] of VERSION_KINDS) pick.add(new Option(`Compare by ${w.toLowerCase()}`, k, false, k === compareBy));
    const table = el(`<div></div>`);
    const fill = () => {
      const rows = s.versions.filter((v) => v.kind === compareBy).sort((a, b) => b.label.localeCompare(a.label));
      table.replaceChildren(
        list(
          ["Version", "Calls", "Booked", "Median", "90th", `Within ${span(TURN_TARGET_MS)}`, "$ per call"],
          rows.map((v) => ({
            cells: [
              v.label,
              String(v.calls),
              pct(v.booked, v.calls),
              v.turns_answered ? span(v.wait_p50_ms) : "—",
              v.turns_answered ? span(v.wait_p90_ms) : "—",
              pct(v.turns_within, v.turns_answered),
              dollars(v.calls ? v.cost_usd / v.calls : 0),
            ],
            open: () => setFilters({ version: `${v.kind}:${v.label}` }),
          })),
        ),
      );
    };
    pick.addEventListener("change", () => {
      compareBy = pick.value;
      fill();
    });
    fill();
    out.push(section("Compare versions", "A row sets the Version filter for every view; Any version clears it.", pick, table));
    return out;
  });
}

// ---- Flags (analyst)

export function flagsView(): View {
  return statsView("flags", "Flags", (d) => {
    const s = d.stats;
    const [t, p] = [s.totals, s.prev];
    const unverified = s.staff_flags.reduce((a, f) => a + f.unverified, 0);
    const staff = { name: "Staff flags", cls: "ch-worse" };
    const review = { name: "Review flags", cls: "ch-quiet" };
    const days = allDays(
      s.days.map((x) => ({ day: x.day, parts: [x.staff_flags, x.review_flags] })),
      rangeStart(d.f),
      (day) => ({ day, parts: [0, 0] }),
    );
    const flaggedShare = share(t.flagged_calls, t.calls);
    return [
      tiles([
        {
          label: "Flagged calls",
          value: t.flagged_calls.toLocaleString(),
          sub: flaggedShare === null ? "—" : `${Math.round(flaggedShare)}% of ${count(t.calls, "call")}`,
          delta: change(d.f, t.flagged_calls, p?.flagged_calls, false, "count"),
          open: () => toLogs(d.view, { flagged: true }),
        },
        { label: "Staff flags", value: t.staff_flags.toLocaleString(), sub: "for a nurse", delta: change(d.f, t.staff_flags, p?.staff_flags, false, "count") },
        { label: "Review flags", value: t.review_flags.toLocaleString(), sub: "for an analyst", delta: change(d.f, t.review_flags, p?.review_flags, false, "count") },
        { label: "Unverified evidence", value: unverified.toLocaleString(), sub: "words not found as the caller's" },
      ]),
      section("Flags per day", "A part opens the flagged calls in Logs.", columns("Flags per day", days, [staff, review], (n) => count(n, "flag"), () => toLogs(d.view, { flagged: true }))),
      section(
        "Staff flags by kind",
        "Each kind, and who raised it: the code's own checks or the model.",
        bars(s.staff_flags.map((f) => ({ label: `${words(f.kind)} (${f.source})`, value: f.count, text: `${f.count}${f.unverified ? ` · ${f.unverified} unverified` : ""}`, cls: "ch-worse" }))),
      ),
      section(
        "Review flags by reason",
        null,
        bars(s.review_flags.map((r) => ({ label: words(r.name), value: r.count, text: String(r.count), cls: "ch-quiet" }))),
      ),
      section(
        "The flagged calls",
        may("phi", "view")
          ? "Open a call to read its flags' summary and evidence. Each opening is kept in the audit trail."
          : "Kinds only: reading a flag's summary and evidence needs PHI View.",
        flaggedList(d.view),
      ),
    ];
  });
}

/** The Flags view's one list, built once with its listeners and put back in each redraw. */
let flaggedBox: HTMLElement | null = null;
function flaggedList(view: HTMLElement): HTMLElement {
  flaggedBox ??= flaggedCalls(view);
  return flaggedBox;
}

/** The flagged calls of the range, from Logs' list (the same filters). A row opens its flags:
 * their words with PHI View (each read audited in ehr), else their kinds. Built once: its
 * listeners never multiply, and it reads only while it is on screen. The words close when the
 * view hides or your access changes, so none stay in a kept view. */
function flaggedCalls(view: HTMLElement): HTMLElement {
  const box = el(`<div class="an-flags"><p class="empty">Loading</p></div>`);
  let last: LoggedCall[] = [];
  const draw = (calls: LoggedCall[]) => {
    last = calls;
    const mine = calls.filter((c) => isFlagged(c) && passes(c)).slice(0, 200);
    if (!mine.length) return box.replaceChildren(el(`<p class="empty">No flagged calls in these dates</p>`));
    box.replaceChildren(
      ...mine.map((c) => {
        const item = el(`<div class="an-flag"><button class="ch-list-row an-flag-head" type="button" aria-expanded="false"></button><div class="an-flag-body" hidden></div></div>`);
        const head = item.querySelector<HTMLButtonElement>("button")!;
        const kinds = [...c.staff_flags.map(words), ...c.review_flags.map((r) => `Review: ${words(r)}`)].join(" · ");
        for (const text of [c.started ? c.started.replace("T", " ").slice(5, 16) : c.id, outcomeWords(c.outcome), kinds]) {
          const s = el(`<span></span>`);
          s.textContent = text;
          head.append(s);
        }
        const body = item.querySelector<HTMLElement>(".an-flag-body")!;
        head.addEventListener("click", () => {
          const open = body.hidden;
          body.hidden = !open;
          head.setAttribute("aria-expanded", String(open));
          if (!open || body.dataset.read) return;
          body.dataset.read = "1";
          void fillFlag(body, c, view);
        });
        return item;
      }),
    );
  };
  // One subscription at a time, to the store of the range chosen.
  let range = filters().range;
  let unsubscribe = logs(range).subscribe(box, (s) => {
    if (s.value) draw(s.value.calls);
  });
  onFilters(() => {
    if (!box.isConnected) return; // read again when it shows, below
    if (filters().range !== range) {
      range = filters().range;
      unsubscribe();
      unsubscribe = logs(range).subscribe(box, (s) => {
        if (s.value) draw(s.value.calls);
      });
    } else draw(last);
  });
  // Shown again: the list is read again; hidden: every opened flag's words are taken away.
  let shown: boolean | null = null;
  new ResizeObserver(() => {
    const now = box.isConnected && box.offsetParent !== null;
    if (now && shown === false) {
      if (filters().range !== range) {
        range = filters().range;
        unsubscribe();
        unsubscribe = logs(range).subscribe(box, (s) => {
          if (s.value) draw(s.value.calls);
        });
      }
      void logs(range).refresh("user");
    }
    if (!now && shown) closeAll(box);
    shown = now;
  }).observe(view);
  onAccess(() => closeAll(box));
  return box;
}

/** Closes every opened flag and drops its words from the page. */
function closeAll(box: HTMLElement): void {
  box.querySelectorAll<HTMLElement>(".an-flag").forEach((item) => {
    const body = item.querySelector<HTMLElement>(".an-flag-body")!;
    body.hidden = true;
    body.replaceChildren();
    delete body.dataset.read;
    item.querySelector("button")!.setAttribute("aria-expanded", "false");
  });
}

/** A flagged call's panel: each staff flag's words (PHI View; the read is audited in ehr), the
 * review flags, and a way to the call in Logs. The words are protected data: shown here only. */
async function fillFlag(body: HTMLElement, c: LoggedCall, view: HTMLElement): Promise<void> {
  const findButton = el(`<div class="tabs lg-open" role="group"><button class="tab" type="button">Find in Logs</button></div>`);
  findButton.querySelector("button")!.addEventListener("click", () => findInLogs(view, c.id));
  if (!c.staff_flags.length || !may("phi", "view")) {
    const p = el(`<p class="an-note"></p>`);
    p.textContent = c.staff_flags.length ? "Reading a flag's summary and evidence needs PHI View." : "Review flags only: they hold no words.";
    body.replaceChildren(p, findButton);
    return;
  }
  body.replaceChildren(el(`<p class="an-note">Reading</p>`));
  let flags: FlagWords[];
  try {
    flags = (await once("flags", () => flagWords(c.id))).flags;
  } catch (e) {
    const p = el(`<p class="an-note"></p>`);
    p.textContent = e instanceof Error && e.message ? `Not read: ${e.message}` : "Not read";
    body.replaceChildren(p, findButton);
    return;
  }
  body.replaceChildren(
    ...flags.map((f) => {
      const card = el(`<div class="an-flag-words"><p class="an-flag-kind"></p><p class="an-flag-summary"></p><ul class="bullets"></ul></div>`);
      card.querySelector(".an-flag-kind")!.textContent = `${words(f.kind)} · raised by the ${f.source}${f.unverified ? ` · ${f.unverified} unverified` : ""}`;
      card.querySelector(".an-flag-summary")!.textContent = f.summary || "No summary";
      const ul = card.querySelector("ul")!;
      for (const e of f.evidence) {
        const li = document.createElement("li");
        li.textContent = `“${e}”`;
        ul.append(li);
      }
      return card;
    }),
    findButton,
  );
}
