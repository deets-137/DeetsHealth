// The card registry — the foundation of the swappable-slot system (SURFACES-AND-CARDS.md).
// Every card is a self-contained, mountable module: it builds its own markup into a host
// element and returns a handle that tears it down (so a slot can swap cards without leaking
// listeners). The layout manager (layout.ts) mounts cards into slots from a persisted
// assignment.

import { patientCard, timelineCard, labsCard, medicationsCard, allergiesCard, conditionsCard } from "./record-cards";
import { settingsCard } from "./settings-card";

export type CardId = "patient" | "timeline" | "labs" | "medications" | "allergies" | "conditions" | "settings";

export interface CardInstance {
  /** Tear down: drop every listener and clear the host. Called when a slot swaps cards. */
  destroy(): void;
  /**
   * Optional — drilling cards report header state so a slot can show the picker only at
   * root and mirror the live title. Non-drilling cards omit it (always "at root").
   * Returns an unsubscribe fn.
   */
  onHeaderChange?(cb: (h: { title: string; atRoot: boolean }) => void): () => void;
  /** Card memory (card-memory.ts): where the card is — view state, never a record key or a
   *  value. The layout reads it just before `destroy()` and gives it back on the next mount. */
  snapshot?(): unknown;
}

/** What the layout hands a card at mount. */
export interface MountOpts {
  /** The snapshot this card left at its last destroy. A held request wins over it. */
  memory?: unknown;
  /**
   * The drill swap: the next level this card opens is the one a grown card's drill asked
   * for. Back on that level calls this; true = the layout brought the first card back (the
   * card is being destroyed), false = do a plain Back.
   */
  onReturn?: () => boolean;
  /** The card the return goes to, for the Back button's hint. */
  returnTitle?: string;
}

export interface CardDef {
  id: CardId;
  /** Default header label (used by the slot chrome + the picker). */
  title: string;
  /** Build markup into `host` and wire it up; return a handle to tear it down. */
  mount(host: HTMLElement, opts?: MountOpts): CardInstance;
}

/** Cards available to slots. The order is the pickers' order. */
export const registry: Partial<Record<CardId, CardDef>> = {
  patient: patientCard,
  labs: labsCard,
  medications: medicationsCard,
  allergies: allergiesCard,
  conditions: conditionsCard,
  timeline: timelineCard,
  settings: settingsCard,
};
