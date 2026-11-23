// Every store the console reads (docs/architecture/LOADING.md §3), in one place. Each name is a
// fixed word: it is what the ledger shows (ledger.ts).

import * as api from "./api";
import { keyed, store } from "./store";

/** Security › Auth and Teams › People: one list for both. */
export const people = store("people", () => api.people(), { focus: true });

/** Who you are and what you may do (docs/architecture/ACCESS-ROLES.md §6): read on start and when the
 * window comes back; the nav and every page ask access.ts, never this store. */
export const me = store("me", () => api.me(), { focus: true });
/** Security › Roles, by version (0: the active one). */
export const roles = keyed("roles", (v: number) => api.roles(v));
/** Audit Logs' first page: every row, or the changes and refusals only. Older rows are read
 * with `once` as the page asks (More). */
export const audit = keyed("audit", (rows: "all" | "changes") => api.audit(0, rows === "changes"), {
  words: ["all", "changes"],
});

/** Config's sections, by version (0: the active one). A version is a number the store made,
 * not protected data, but the ledger leaves it out all the same. */
export const cartesiaSettings = keyed("settings.cartesia", (v: number) => api.cartesiaSettings(v));
export const anthropicSettings = keyed("settings.anthropic", (v: number) => api.anthropicSettings(v));
export const systemSettings = keyed("settings.system", (v: number) => api.systemSettings(v));
export const hospitalSettings = keyed("settings.hospital", (v: number) => api.hospitalSettings(v));
export const promptSettings = keyed("settings.agent", (v: number) => api.promptSettings(v));

/** Config › Cartesia's voice list. It leaves this machine (Cartesia): never on focus, never
 * holds the first-load wave. Refresh is a write whose answer is the new list. */
export const cartesiaVoices = store("cartesia.voices", () => api.cartesiaVoices(), { leaves: true });

/** Analytics › Logs (ANALYTICS.md §3.1), by the filter bar's range, a fixed word. The dev server
 * counts the days on its own clock at each read, so "7d" stays the last seven days. */
export type Range = "today" | "7d" | "30d" | "90d" | "all";
export const RANGES: Range[] = ["today", "7d", "30d", "90d", "all"];
const RANGE_DAYS: Record<Range, number | null> = { today: 0, "7d": 6, "30d": 29, "90d": 89, all: null };
export const logs = keyed("logs", (r: Range) => api.logs(RANGE_DAYS[r]), { words: RANGES });

/** Chat. */
export const health = store("health", () => api.health());
export const catalog = store("catalog", () => api.catalog());
/** By EHR. Epic's list leaves this machine: never on focus, never holds the wave. */
export const sandbox = keyed("sandbox", (ehr: api.EhrSource) => api.sandbox(ehr), {
  words: ["local", "epic"],
  options: (ehr) => ({ leaves: ehr === "epic" }),
});
/** One call's use, or this cycle's alone (`null`). The call id never reaches the ledger. */
export const resources = keyed("resources", (call: string | null) => api.resources(call));
// A call's events are its transcript: never kept in a store. Chat reads them with `once`
// (store.ts), which only times the read into the ledger as "events".
