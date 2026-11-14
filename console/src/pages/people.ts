// Teams › People (docs/ops/DEMO.md §3.5; the owner's draft, 2026-11-14): everyone who may sign
// in. An email and a name, then Add; each row with who added them, when, and when they were last
// seen; Remove asks "Confirm?" first, like End call. Copy invite puts the link and how to sign in
// on the clipboard. The store is the record; the dev server keeps Cloudflare's group in step.
//
// Emails and names here are staff identities, not patients'. They show only in this view: never
// in a toast, a hint or a log line.
import { addPerson, people, removePerson, type People, type Person } from "../agent/api";
import { tokenMs } from "../tokens";
import { el } from "./page";
import type { View } from "./subnav";

export function peopleView(): View {
  return {
    id: "people",
    title: "People",
    build() {
      const root = el(`
        <div class="people">
          <div class="people-top">
            <form class="people-add" autocomplete="off">
              <input class="cfg-field people-email" type="email" maxlength="254" placeholder="Email"
                aria-label="Their email" required />
              <input class="cfg-field people-name" type="text" maxlength="100" placeholder="Name"
                aria-label="Their name" required />
              <button class="cfg-button cfg-save people-go" type="submit">Add</button>
            </form>
            <button class="cfg-button people-invite" type="button">Copy invite</button>
          </div>
          <p class="people-status" role="status"></p>
          <div class="people-list own-scrollbars" role="list" aria-label="People"><p class="empty">Loading</p></div>
        </div>`);
      const form = root.querySelector<HTMLFormElement>(".people-add")!;
      const email = root.querySelector<HTMLInputElement>(".people-email")!;
      const name = root.querySelector<HTMLInputElement>(".people-name")!;
      const go = root.querySelector<HTMLButtonElement>(".people-go")!;
      const invite = root.querySelector<HTMLButtonElement>(".people-invite")!;
      const status = root.querySelector<HTMLElement>(".people-status")!;
      const list = root.querySelector<HTMLElement>(".people-list")!;
      let view: People | undefined;
      let confirming: string | null = null; // the row whose Remove says "Confirm?"
      let confirmTimer = 0;
      const say = (text: string) => (status.textContent = text);

      const draw = () => {
        if (!view) return;
        invite.disabled = !view.invite;
        go.disabled = !view.store;
        if (!view.store) {
          list.replaceChildren(empty("No store: start Postgres (npm run db:start), then restart npm run dev"));
          return;
        }
        list.replaceChildren(...view.people.map((p) => row(p, view!.you, confirming === p.id)));
      };
      const take = (next: People) => {
        view = next;
        draw();
        if (!next.linked) say("Cloudflare is not linked: the list is saved, but sign-in follows Cloudflare's own group");
      };
      const load = async () => {
        try {
          take(await people());
        } catch {
          list.replaceChildren(empty("The agent's dev server is not running"));
        }
      };

      form.addEventListener("submit", async (e) => {
        e.preventDefault();
        if (go.disabled) return;
        go.disabled = true;
        say("Adding");
        try {
          const next = await addPerson(email.value, name.value);
          email.value = "";
          name.value = "";
          take(next);
          say(next.warning ? `Added. Cloudflare not updated: ${next.warning}` : "Added");
          email.focus();
        } catch (err) {
          say(err instanceof Error && err.message ? `Not added: ${err.message}` : "Not added");
        } finally {
          go.disabled = !view?.store;
        }
      });

      list.addEventListener("click", async (e) => {
        const b = (e.target as HTMLElement).closest<HTMLButtonElement>(".people-remove");
        const id = b?.closest<HTMLElement>(".people-row")?.dataset.id;
        if (!b || !id || b.disabled) return;
        if (confirming !== id) {
          window.clearTimeout(confirmTimer);
          confirming = id;
          draw();
          confirmTimer = window.setTimeout(() => {
            confirming = null;
            draw();
          }, tokenMs("--confirm-hold"));
          return;
        }
        window.clearTimeout(confirmTimer);
        confirming = null;
        say("Removing");
        try {
          const next = await removePerson(id);
          take(next);
          say(next.warning ? `Removed. Cloudflare not updated: ${next.warning}` : "Removed");
        } catch (err) {
          draw();
          say(err instanceof Error && err.message ? `Not removed: ${err.message}` : "Not removed");
        }
      });

      invite.addEventListener("click", async () => {
        if (!view?.invite) return;
        const text =
          `You're invited to try the DeetsHealth console: ${view.invite}\n` +
          "Open the link and sign in with this email address. Cloudflare emails you a one-time code.";
        try {
          await navigator.clipboard.writeText(text);
          say("Invite copied");
        } catch {
          say("The browser did not allow copying");
        }
      });

      // Read again each time the window comes back: someone else may have changed the list.
      window.addEventListener("focus", () => {
        if (root.isConnected) void load();
      });
      void load();
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

/** One person: name and email; who added them and when; when last seen; Remove (not for the
 * owner). The asker's own row says "You". */
function row(p: Person, you: string, confirming: boolean): HTMLElement {
  const r = document.createElement("div");
  r.className = "people-row";
  r.setAttribute("role", "listitem");
  r.dataset.id = p.id;
  const mine = you === p.email || (you === "owner" && p.owner);
  const who = document.createElement("span");
  who.className = "people-who";
  who.textContent = `${mine ? "You · " : ""}${p.name === p.email ? p.email : `${p.name} · ${p.email}`}`;
  const added = document.createElement("span");
  added.className = "people-meta";
  added.textContent = p.owner
    ? "owner"
    : [p.added_by ? `added by ${p.added_by}` : "from Cloudflare's group", day(p.added_at)].join(" · ");
  const seen = document.createElement("span");
  seen.className = "people-meta";
  seen.textContent = p.last_seen_at ? `seen ${ago(p.last_seen_at)}` : "never seen";
  r.append(who, added, seen);
  if (!p.owner) {
    const b = document.createElement("button");
    b.type = "button";
    b.className = "cfg-button people-remove";
    b.textContent = confirming ? "Confirm?" : "Remove";
    b.setAttribute("aria-label", confirming ? `Confirm: remove ${p.name}` : `Remove ${p.name}`);
    r.append(b);
  }
  return r;
}

/** "11-14" from an RFC 3339 time. */
function day(iso: string): string {
  return iso.slice(5, 10);
}

/** "now", "5 min ago", "3 h ago", or the day. */
function ago(iso: string): string {
  const s = (Date.now() - Date.parse(iso)) / 1000;
  if (!Number.isFinite(s) || s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return day(iso);
}
