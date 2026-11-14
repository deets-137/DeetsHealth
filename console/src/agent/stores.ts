// Every store the console reads (docs/architecture/LOADING.md §3), in one place. Each name is a
// fixed word: it is what the ledger shows (ledger.ts).

import * as api from "./api";
import { keyed, store } from "./store";

/** Security › Auth and Teams › People: one list for both. */
export const people = store("people", () => api.people(), { focus: true });

/** Config's sections, by version (0: the active one). A version is a number the store made,
 * not protected data, but the ledger leaves it out all the same. */
export const cartesiaSettings = keyed("settings.cartesia", (v: number) => api.cartesiaSettings(v));
export const anthropicSettings = keyed("settings.anthropic", (v: number) => api.anthropicSettings(v));
export const systemSettings = keyed("settings.system", (v: number) => api.systemSettings(v));
export const promptSettings = keyed("settings.agent", (v: number) => api.promptSettings(v));

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
