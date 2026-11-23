// Analytics › Logs (ANALYTICS.md §3.1; the owner, 2026-11-22): every call, for the analyst. The
// filter bar (analytics-filter.ts, shared by the Analytics views), then a count line, then the
// one table (data-table.ts), newest first, windowed past 200 rows (list-window.ts), with a
// search box and Ctrl+F. A click on a row opens a panel under it (his A): each turn's wait as a
// strip of bars with a line at the target, the tools and their errors, the flags, the steps, the
// versions, and Open in Chat, which plays the call in Chat › Demo (open-call.ts).
//
// The rows come from ehr's ListCalls through GET /agent/logs (Analytics View): names and numbers,
// and a synthetic call's own label. No word of the call is here; the words are in Chat, where
// each look is audited. Hints and status lines name counts, never a value.
import type { LoggedCall } from "../agent/api";
import { logs } from "../agent/stores";
import { may } from "../access";
import { openCall } from "../open-call";
import { filterBar, filters, flagged, onFilters, passes, replayed, slowest, TURN_TARGET_MS } from "./analytics-filter";
import { outcomeWords, stepWord } from "./chat";
import { row, table, type Column } from "./data-table";
import { fillTable, heightsOf } from "./list-window";
import { el } from "./page";
import type { View } from "./subnav";

const COLUMNS: Column[] = [
  { head: "Started", kind: "short" },
  { head: "Origin", kind: "num" },
  { head: "Seat", kind: "num" },
  { head: "EHR", kind: "num" },
  { head: "Outcome", kind: "num" },
  { head: "Last step", kind: "num" },
  { head: "Turns", kind: "num" },
  { head: "Length", kind: "num" },
  { head: "Typical wait", kind: "num" },
  { head: "Slowest turn", kind: "num" },
  { head: "Tool errors", kind: "num" },
  { head: "Flags", kind: "num" },
  { head: "$", kind: "num" },
  { head: "Versions" },
  { head: "Label" },
  { head: "Rating", kind: "num" },
];

const WORD: Record<string, string> = { test: "Test", kept: "Kept", typed: "Typed", live: "Live", text: "Text", voice: "Voice", local: "Local", epic: "Epic" };
const word = (v: string | null, fallback: string) => (v ? (WORD[v] ?? v) : fallback);

/** A span in words: whole ms under a second, else seconds to the hundredth. */
const span = (ms: number) => (ms < 1000 ? `${Math.round(ms)} ms` : `${(ms / 1000).toFixed(2)} s`);
/** A call's length: m:ss. */
const clock = (ms: number) => {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};
const median = (xs: number[]) => {
  if (!xs.length) return null;
  const s = [...xs].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2;
};
/** "11-22 16:45", or the id when the record has no start. */
const started = (c: LoggedCall) => (c.started ? c.started.replace("T", " ").slice(5, 16) : c.id);
const dollars = (n: number) => (n < 0.1 ? `$${n.toFixed(3)}` : `$${n.toFixed(2)}`);

/** The words a search looks in: names, words of the page, the label. Never read out. */
function haystack(c: LoggedCall): string {
  return [
    c.id,
    c.label,
    c.scenario,
    outcomeWords(c.outcome),
    ...c.reached.map(stepWord),
    ...c.tools,
    ...c.staff_flags,
    ...c.review_flags,
    c.prompt_version,
    c.system_version,
    c.hospital_version,
    c.model,
    c.end_reason,
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();
}

/** "find_slots ×2 · book", and the failed ones after. */
function toolWords(c: LoggedCall): string {
  if (!c.tools.length) return "None";
  const count = new Map<string, number>();
  for (const t of c.tools) count.set(t, (count.get(t) ?? 0) + 1);
  const all = [...count].map(([t, n]) => (n > 1 ? `${t} ×${n}` : t)).join(" · ");
  const failed = c.tools.filter((_, i) => c.tool_failed[i]);
  return failed.length ? `${all}. Failed: ${failed.join(", ")}` : all;
}

const versions = (c: LoggedCall) =>
  [c.prompt_version && `Words ${c.prompt_version}`, c.system_version && `System ${c.system_version}`, c.hospital_version && `Hospital ${c.hospital_version}`, c.model]
    .filter(Boolean)
    .join(" · ");

/** Each turn's wait as a bar, as tall as its share of the slowest (or of the target), with a
 * line across at the target. A bar over the target is in the error color. */
function waitStrip(c: LoggedCall): HTMLElement {
  const strip = el(`<span class="lg-strip" role="img"></span>`);
  const waits = c.turn_waits_ms;
  if (!waits.length || replayed(c)) {
    strip.textContent = waits.length ? "Replayed: the waits are the replay cache's, not a caller's" : "No answered turn";
    strip.classList.add("is-empty");
    return strip;
  }
  const top = Math.max(TURN_TARGET_MS, ...waits);
  const line = el(`<span class="lg-target" aria-hidden="true"></span>`);
  line.style.bottom = `${(TURN_TARGET_MS / top) * 100}%`;
  strip.append(line);
  waits.forEach((w, i) => {
    const bar = el(`<span class="lg-bar"></span>`);
    bar.style.height = `${Math.max(4, (w / top) * 100)}%`;
    bar.classList.toggle("is-over", w > TURN_TARGET_MS);
    bar.title = `Turn ${i + 1}: ${span(w)}`;
    strip.append(bar);
  });
  const over = waits.filter((w) => w > TURN_TARGET_MS).length;
  strip.setAttribute("aria-label", `${waits.length} turns, ${over} over ${TURN_TARGET_MS} ms`);
  return strip;
}

function cells(c: LoggedCall): (string | Node)[] {
  const again = replayed(c) && c.turn_waits_ms.length > 0;
  const typical = again ? null : median(c.turn_waits_ms);
  const worst = slowest(c);
  const slow = document.createElement("span");
  slow.textContent = again ? "Replayed" : worst === null ? "—" : span(worst);
  if (worst !== null && worst > TURN_TARGET_MS) slow.className = "lg-over";
  const errors = c.tool_failed.filter(Boolean).length;
  const flags = c.staff_flags.length + c.review_flags.length;
  const rating = c.ratings_passed + c.ratings_failed ? `${c.ratings_passed} ✓ · ${c.ratings_failed} ✕` : "—";
  return [
    started(c),
    word(c.origin, "—"),
    word(c.seat, "Text"),
    word(c.ehr, "Local"),
    outcomeWords(c.outcome),
    c.reached.length ? stepWord(c.reached[c.reached.length - 1]) : "—",
    String(c.turns),
    clock(c.duration_ms),
    again ? "Replayed" : typical === null ? "—" : span(typical),
    slow,
    String(errors),
    String(flags),
    dollars(c.cost_usd),
    versions(c) || "—",
    c.label ?? "—",
    rating,
  ];
}

/** The panel under a row: the facts, then Open in Chat. */
function facts(c: LoggedCall): [string, string | Node][] {
  const out: [string, string | Node][] = [["Turns", waitStrip(c)]];
  out.push(["Tools", toolWords(c)]);
  const flagWords = [...c.staff_flags.map((f) => f.replace(/_/g, " ")), ...c.review_flags.map((r) => `review: ${r.replace(/_/g, " ")}`)];
  out.push(["Flags", flagWords.length ? flagWords.join(" · ") : "None"]);
  if (c.reached.length) out.push(["Steps", c.reached.map(stepWord).join(" → ")]);
  if (c.end_reason) out.push(["End", c.end_reason]);
  if (c.scenario) out.push(["Script", c.scenario]);
  out.push(["Versions", [versions(c), c.mode].filter(Boolean).join(" · ") || "—"]);
  if (may("testing", "view") && !c.record) {
    // A kept-script draw is filed in the store, but its record file is not kept here.
    out.push(["", "No record on this machine to play in Chat"]);
  } else if (may("testing", "view")) {
    const open = el(`<div class="tabs lg-open" role="group"><button class="tab" type="button">Open in Chat</button></div>`);
    open.querySelector("button")!.addEventListener("click", (e) => {
      e.stopPropagation();
      openCall(c.id);
    });
    open.querySelector("button")!.title = "Play this call in Chat › Demo, with its Timeline, Tools and Review";
    out.push(["", open]);
  }
  return out;
}

export function logsView(): View {
  return {
    id: "logs",
    title: "Logs",
    build() {
      const searchWrap = el(`
        <div class="cfg-search an-search" role="search">
          <input class="cfg-field cfg-search-box" type="search" spellcheck="false" autocomplete="off"
            placeholder="Search the calls" aria-label="Search the calls" aria-keyshortcuts="Control+F Meta+F" />
        </div>`);
      const search = searchWrap.querySelector<HTMLInputElement>("input")!;
      search.title = "Ctrl+F. Press Ctrl+F again for the browser's own find. Esc clears.";
      const bar = filterBar(searchWrap);
      const root = el(`
        <div class="auth logs">
          <div class="auth-top">
            <h2 class="auth-title">Logs</h2>
            <p class="cfg-status lg-count" role="status"></p>
          </div>
          <div class="auth-scroll own-scrollbars lg-scroll"></div>
        </div>`);
      root.insertBefore(bar.el, root.querySelector(".auth-scroll"));
      const count = root.querySelector<HTMLElement>(".lg-count")!;
      const scroll = root.querySelector<HTMLElement>(".lg-scroll")!;
      scroll.replaceChildren(el(`<p class="empty">Loading</p>`));

      let calls: LoggedCall[] = [];
      let cut = false; // the read stopped at its limit: older calls are not listed
      let range = filters().range;
      const heights = heightsOf("logs");

      const draw = () => {
        const q = search.value.trim().toLowerCase();
        const shown = calls.filter((c) => passes(c) && (!q || haystack(c).includes(q)));
        const flaggedN = shown.filter(flagged).length;
        const slowN = shown.filter((c) => (slowest(c) ?? 0) > TURN_TARGET_MS).length;
        count.textContent =
          `${shown.length} ${shown.length === 1 ? "call" : "calls"} · ${flaggedN} flagged · ${slowN} over ${TURN_TARGET_MS} ms` +
          (cut ? ` · the newest ${calls.length} only: choose fewer dates` : "");
        const t = table("Calls", COLUMNS);
        t.wrap.dataset.empty = calls.length ? "No call fits these filters" : "No calls in these dates yet";
        t.table.classList.add("lg-table");
        fillTable(
          t,
          shown.map((c) => ({
            item: c,
            draw: () => {
              const [tr, help] = row(c, `the call of ${started(c)}`, cells(c), facts(c));
              // A click anywhere on the row opens its panel, as the "i" does.
              tr.addEventListener("click", (e) => {
                if ((e.target as HTMLElement).closest("button, a")) return;
                tr.querySelector<HTMLButtonElement>(".cfg-info")?.click();
              });
              return [tr, help];
            },
          })),
          { heights },
        );
        scroll.replaceChildren(t.wrap);
      };

      // The store for the range chosen: a new range reads its own list.
      let unsubscribe: (() => void) | null = null;
      const watch = () => {
        unsubscribe?.();
        unsubscribe = logs(range).subscribe(scroll, (s) => {
          if (s.value) {
            calls = s.value.calls;
            cut = calls.length >= s.value.limit;
            bar.setVersions(calls);
            draw();
            return;
          }
          if (s.status === "down") scroll.replaceChildren(el(`<p class="empty">The agent's dev server is not running</p>`));
          else if (s.status === "refused") {
            const p = el(`<p class="empty"></p>`);
            p.textContent = s.reason ?? "The agent's dev server refused";
            scroll.replaceChildren(p);
          }
        });
      };
      watch();
      onFilters(() => {
        if (filters().range !== range) {
          range = filters().range;
          watch();
          void logs(range).refresh("user");
        } else if (root.isConnected) draw();
      });

      // One redraw a frame, however fast the typing.
      let queued = false;
      search.addEventListener("input", () => {
        if (queued) return;
        queued = true;
        requestAnimationFrame(() => {
          queued = false;
          draw();
        });
      });
      search.addEventListener("keydown", (e) => {
        if (e.key !== "Escape") return;
        e.preventDefault();
        if (!search.value) return search.blur();
        search.value = "";
        draw();
      });
      // Ctrl+F (⌘F) while this view shows: the first press goes to the box; a second, while the
      // box has the cursor, is the browser's own find (TABLES.md §6.3).
      document.addEventListener("keydown", (e) => {
        if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey || e.key.toLowerCase() !== "f") return;
        if (!search.isConnected || !search.offsetParent || document.activeElement === search) return;
        e.preventDefault();
        search.focus();
        search.select();
      });
      // Each time the view opens again (it is kept while hidden), the list is read again: a
      // call may have ended since.
      // The first showing is the subscription's own read.
      let open: boolean | null = null;
      new ResizeObserver(() => {
        const now = root.isConnected && root.offsetParent !== null;
        if (now && open === false) void logs(range).refresh("user");
        open = now;
      }).observe(root);
      return root;
    },
  };
}
