// Open in Chat (ANALYTICS.md §3.1): a page asks for one recorded call to play in Chat › Demo.
// The nav goes to Chat; the Chat page, once built, takes the call and plays it. The id waits
// here until Chat takes it, since Chat may not be built yet when the ask comes. A call id is
// an origin and a start time, never protected data.
const OPEN_CALL = "deets:open-call";

let pending: string | null = null;

/** Asks for `id` to play in Chat › Demo. */
export function openCall(id: string): void {
  pending = id;
  window.dispatchEvent(new CustomEvent(OPEN_CALL));
}

/** The call waiting to be opened, once: null when none. */
export function takeCall(): string | null {
  const id = pending;
  pending = null;
  return id;
}

/** Runs `fn` on each ask. */
export function onOpenCall(fn: () => void): void {
  window.addEventListener(OPEN_CALL, fn);
}
