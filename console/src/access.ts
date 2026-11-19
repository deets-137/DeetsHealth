// What you may do in the console (docs/architecture/ACCESS-ROLES.md §5, §6 Hand-over 3). Every page and
// the nav ask here; none reads the `me` store itself. The server refuses what a role does not
// allow either way (dev_server.py › ROUTE_NEEDS, ehr's RPC_LEVERS): this only keeps the console
// from offering it.
//
// Fork 3A (the owner, 2026-11-15): a lever at None hides its page or view; at View the page shows
// with no Save, Add or Remove and a "View only" mark; at Edit, all of it.
import { me } from "./agent/stores";
import type { Level, Lever, Me } from "./agent/api";

const RANK: Record<Level, number> = { none: 0, view: 1, edit: 2 };

let current: Me | null = null;
/** `me` answered at least once (a value, or a refusal: not on the list). */
let answered = false;
const listeners = new Set<() => void>();

/** Starts reading `me`, shown by `el` (the nav): read on start and when the window comes back. */
export function startAccess(el: HTMLElement): void {
  me.subscribe(el, (s) => {
    if (s.status === "loading") return;
    const before = JSON.stringify(current);
    // A refusal (403: not on the list) is no access at all; the agent down keeps what it had.
    if (s.status === "ready" && s.value) current = s.value;
    else if (s.status === "refused") current = null;
    const first = !answered;
    answered = s.status === "ready" || s.status === "refused";
    if (first || JSON.stringify(current) !== before) listeners.forEach((fn) => fn());
  });
}

/** Whether `me` has answered: until it does, the console shows every page (the first-load wave
 * waits for it, so this is the briefest moment). */
export const known = (): boolean => answered;

/** Your level on a lever: None before `me` answers, or when it refused. */
export function level(lever: Lever): Level {
  return current?.levels[lever] ?? "none";
}

/** Whether you hold `lever` at `at` or above. Before `me` answers: yes (the server decides). */
export function may(lever: Lever, at: Level): boolean {
  if (!answered) return true;
  return RANK[level(lever)] >= RANK[at];
}

/** Whether any lever is above None: a person with no role sees only "No role yet". */
export function anyAccess(): boolean {
  if (!answered) return true;
  return current !== null && Object.values(current.levels).some((l) => l !== "none");
}

/** Your user id; null without a roles store or before `me` answers. */
export const myId = (): string | null => current?.user_id ?? null;

/** Your role ids. */
export const myRoles = (): string[] => current?.role_ids ?? [];

/** Runs `fn` each time what you may do changes (and once when `me` first answers). Returns the
 * way to stop. */
export function onAccess(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Read `me` again now: after a grant, a revoke or a roles save, which may change your own. */
export function accessChanged(): void {
  void me.refresh("user");
}
