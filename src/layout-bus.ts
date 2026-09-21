// Card-summon bus — lets a mounted card ask the layout manager to bring another card
// on-screen without importing layout.ts directly (that would close an import cycle:
// layout → cards → a card → layout).

import type { CardId } from "./cards";

/** A drill request opens one thing in the target card. From a grown card it swaps that card
 *  into the grown place (DeetsMusic's CARD-GROW.md §14). */
export type RequestHow = "summon" | "drill";
type RequestCb = (id: CardId, how: RequestHow) => void;
const subs = new Set<RequestCb>();

/**
 * Ask the layout to bring `id` on-screen. The layout mounts it into the
 * least-recently-touched content slot. A card already on screen stays where it is.
 */
export function requestCard(id: CardId, how: RequestHow = "summon"): void {
  subs.forEach((cb) => cb(id, how));
}

/** Ask for `id` because the user is drilling into something in the card they are looking at. */
export const requestDrillCard = (id: CardId): void => requestCard(id, "drill");

// The on-screen panel that shows a card. The layout installs the lookup; before it does,
// nothing is on screen.
let hostLookup: (id: CardId) => HTMLElement | null = () => null;
/** The drill swap: would a drill for `id` land in the grown card's own place? The layout
 *  installs the answer; before it does, nothing swaps. */
let swapCheck: (id: CardId) => boolean = () => false;
/** Layout-side: install the drill-swap check. */
export function setDrillSwapCheck(fn: (id: CardId) => boolean): void {
  swapCheck = fn;
}
/** True when a drill for `id` takes the grown card's place. */
export const drillSwapsInPlace = (id: CardId): boolean => swapCheck(id);

/** Layout-side: install the lookup. */
export function setCardHostLookup(fn: (id: CardId) => HTMLElement | null): void {
  hostLookup = fn;
}
/** The panel showing `id` on screen now, or null. */
export const cardHost = (id: CardId): HTMLElement | null => hostLookup(id);

/** Layout-side: subscribe to summon requests. Returns an unsubscribe fn. */
export function onCardRequest(cb: RequestCb): () => void {
  subs.add(cb);
  return () => subs.delete(cb);
}

// ── Settings at one row (a toast's [Settings] button, the quick panel) ──
// The request is held until a Settings card takes it: a card mounted by this request
// takes it on mount, a card already on-screen takes it through the subscription.
let pendingRow: string | null = null;
const rowSubs = new Set<() => void>();

/** Bring the Settings card on-screen, unfold the section holding `rowId`, scroll to the row, highlight it. */
export function requestSetting(rowId: string): void {
  pendingRow = rowId;
  requestCard("settings");
  rowSubs.forEach((cb) => cb());
}

/** Settings-card side: take the waiting row id (null when none). */
export function takeSettingRequest(): string | null {
  const id = pendingRow;
  pendingRow = null;
  return id;
}

/** Settings-card side: be told when a row request arrives. Returns an unsubscribe fn. */
export function onSettingRequest(cb: () => void): () => void {
  rowSubs.add(cb);
  return () => rowSubs.delete(cb);
}
