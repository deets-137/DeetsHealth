// Teams › People (docs/ops/DEMO.md §3.5): everyone who may sign in, one card per person. Read
// only: the cards are edited on Security › Auth (the owner, 2026-11-14).
//
// The owner's sketch, 2026-11-14 (docs/architecture/images/people-owner-card-sketch-2026-11-14.png):
// a round picture slot, then the name, the role, the phone number and the email, each on its
// own row. The band above the picture is the theme's own color; everything is left aligned.
// The owner's card comes first; the rest follow as they are added. Where the store holds no
// value yet, the row shows a placeholder in the dim text color, so it never reads as data.
//
// Emails, names, phone numbers and pictures here are staff identities, not patients'. They show
// only in this view and on Security › Auth: never in a toast, a hint or a log line.
import type { Person } from "../agent/api";
import { people } from "../agent/stores";
import { el } from "./page";
import { fillPhoto, forgetPhotos, phoneMark, showPhone } from "./person";
import type { View } from "./subnav";

/** What a row shows while the store has no value for it. */
const PLACEHOLDER = {
  name: "Name",
  role: "Role", // the owner, 2026-11-14: "Role" until the role system exists
  phone: "XXX-XXX-XXXX",
  email: "Email",
} as const;

export function peopleView(): View {
  return {
    id: "people",
    title: "People",
    build() {
      const root = el(`
        <div class="people">
          <div class="people-cards own-scrollbars" role="list" aria-label="People"><p class="empty">Loading</p></div>
        </div>`);
      const cards = root.querySelector<HTMLElement>(".people-cards")!;

      // The people store (LOADING.md §3), shared with Security › Auth: a save there draws here.
      // It reads again on focus, and on its own when the agent comes back.
      people.subscribe(cards, (s) => {
        const view = s.value;
        if (!view) {
          if (s.status === "down") cards.replaceChildren(empty("The agent's dev server is not running"));
          else if (s.status === "refused") cards.replaceChildren(empty("The agent's dev server refused"));
          return;
        }
        if (!view.store) {
          cards.replaceChildren(empty("No store: start Postgres (npm run db:start), then restart npm run dev"));
          return;
        }
        forgetPhotos(view.people);
        cards.replaceChildren(
          ...(view.people.length
            ? view.people.map(card)
            : [empty("No one yet: the list fills from Cloudflare's group at start")]),
        );
      });
      return root;
    },
  };
}

function empty(text: string): HTMLElement {
  const p = document.createElement("p");
  p.className = "empty";
  p.textContent = text;
  return p;
}

/** One person's card: the band, the picture, then name, role, phone and email. */
function card(p: Person): HTMLElement {
  const c = document.createElement("article");
  c.className = "people-card";
  c.setAttribute("role", "listitem");
  c.dataset.id = p.id;

  const band = document.createElement("div");
  band.className = "people-band";
  const photo = document.createElement("div");
  photo.className = "people-photo";
  photo.setAttribute("aria-hidden", "true");
  fillPhoto(photo, p);
  const body = document.createElement("div");
  body.className = "people-body";

  const name = [p.first_name, p.last_name].filter(Boolean).join(" ") || null;
  const phone = line("people-phone", p.phone ? showPhone(p.phone) : null, PLACEHOLDER.phone);
  if (p.phone) phone.append(phoneMark(p.phone_verified));
  body.append(
    line("people-name", name, PLACEHOLDER.name),
    line("people-role", null, PLACEHOLDER.role),
    phone,
    line("people-email", p.email || null, PLACEHOLDER.email),
  );
  c.append(band, photo, body);
  c.setAttribute("aria-label", name ?? p.email);
  return c;
}

/** One row of a card: the value, or the placeholder marked as one. */
function line(cls: string, value: string | null, placeholder: string): HTMLElement {
  const d = document.createElement("div");
  d.className = `people-line ${cls}`;
  const text = document.createElement("span");
  text.className = "people-text";
  text.textContent = value ?? placeholder;
  d.append(text);
  if (value === null) d.classList.add("is-placeholder");
  return d;
}
