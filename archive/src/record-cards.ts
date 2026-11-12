// The record cards, empty (SURFACES-AND-CARDS.md §2). The shell step: each card mounts its
// chrome — the header title the slot picker and card grow hang from, and an empty body — so
// the slots, the pickers, grow, swap and memory can be desk-tested before any data exists.
// No text in the bodies on purpose (the owner, 2026-09-21): a card's empty state is its own
// design, made with the card.
//
// The Patient card is Now Playing's twin: anchored, and with no header (a card with no
// `.panel__head` gets no picker and no grow button, as in DeetsMusic). The Timeline card is
// the Queue's twin: pickable in Midi, anchored under the Patient card in Max.
//
// **The PHI wall (SECURITY.md §4.3).** When the bodies fill, record text goes in through
// `textContent` only. The markup here is fixed app chrome.

import type { CardDef, CardId, CardInstance, MountOpts } from "./cards";
import { scrollSnapshot, applyScrollSnapshot } from "./card-memory";

/** A content card: the header title and an empty body that scrolls with the app's bar. */
function emptyCard(id: CardId, title: string): CardDef {
  return {
    id,
    title,
    mount(host: HTMLElement, opts?: MountOpts): CardInstance {
      host.dataset.card = id;
      host.innerHTML =
        `<header class="panel__head"><h2 class="panel__title">${title}</h2></header>` +
        `<div class="panel__body app-scroll"></div>`;
      const body = host.querySelector<HTMLElement>(".panel__body")!;
      applyScrollSnapshot(body, opts?.memory);
      return {
        snapshot: () => scrollSnapshot(body),
        destroy() {
          host.innerHTML = "";
          delete host.dataset.card;
        },
      };
    },
  };
}

export const patientCard: CardDef = {
  id: "patient",
  title: "Patient",
  mount(host: HTMLElement): CardInstance {
    host.dataset.card = "patient";
    host.innerHTML = `<div class="patient" aria-label="Patient"></div>`;
    return {
      destroy() {
        host.innerHTML = "";
        delete host.dataset.card;
      },
    };
  },
};

export const timelineCard = emptyCard("timeline", "Timeline");
export const labsCard = emptyCard("labs", "Labs");
export const medicationsCard = emptyCard("medications", "Medications");
export const allergiesCard = emptyCard("allergies", "Allergies");
export const conditionsCard = emptyCard("conditions", "Conditions");
