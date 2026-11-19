// Security › Auth (docs/ops/DEMO.md §3.5; the owner, 2026-11-14): who may sign in, as a table
// that can be edited. Teams › People shows the same people as cards, read only.
//
// - The title "Users" and Add sit above the table. A save's result or a refusal shows between
//   them, as on Config. Copy invite is off for now; its link stays in the view's data.
// - One row per person: picture, first, last, email, phone and its mark, role, source, who
//   added them, when, when last seen, and Remove (never on the owner's row).
// - Clicking a row opens a panel under it (the owner's choice A): the picture, a text box for
//   each field, and Config's Discard | Save at its top right, with the result beside it. Each
//   greys when nothing changed; Discard puts back the stored values. A save keeps the panel
//   open, its boxes now the stored values. Clicking the row again closes it. One panel is open
//   at a time.
// - Add puts a blank row at the table's foot, every cell a placeholder, with its panel open
//   (the owner, 2026-11-14). Nothing is stored until Save; the first Save puts the person on
//   the list and the row becomes theirs, its panel still open (the picture can be added then).
//   Discard on a blank row takes it away. Opening another row takes it away too.
// - A card may be saved incomplete (the owner, 2026-11-14). A Save with an empty box does not
//   save: each empty box's title and Save turn red, and the status names the boxes. A second
//   Save saves the card as it is. A card with no email cannot sign in until one is added; a
//   saved email can be changed, not cleared.
// - Roles (docs/ideas/ACCESS-ROLES.md §5, §8.3): the Role cell names the person's roles. In the
//   panel, at Roles Edit, each role is a chip with ✕ (revoke) and a list adds one (grant); your
//   own row has neither (no one changes their own roles). ehr refuses what its rules do not
//   allow, and the panel shows its reason. A changed email asks "Confirm?" before it saves: it
//   moves who can sign in. Remove asks "Confirm?" too, as End call does.
// - People Edit to add, remove or save; at People View the table reads only, with a "View only"
//   mark on the title line (fork 3A). ehr also refuses a change to someone above you.
//
// Emails, names, phone numbers and pictures here are staff identities, not patients'. They show
// only in this view and on Teams › People: never in a toast, a hint or a log line.
import {
  addPerson,
  grantRole,
  removePerson,
  revokeRole,
  setPersonPhoto,
  updatePerson,
  type People,
  type Person,
  type Role,
} from "../agent/api";
import { people, roles } from "../agent/stores";
import { accessChanged, may, myId, onAccess } from "../access";
import { tokenMs } from "../tokens";
import { toolButton } from "./data-table";
import { el } from "./page";
import { fillPhoto, forgetPhotos, phoneMark, showPhone, squareWebp } from "./person";
import type { View } from "./subnav";

const COLUMNS = ["", "First", "Last", "Email", "Phone", "Role", "Source", "Added by", "Added", "Last seen", ""];

/** Each box's name in the "Missing:" line. */
const LABEL = { first_name: "first name", last_name: "last name", email: "email", phone: "phone" } as const;

/** The blank row's id: never a person's (theirs are uuids). */
const NEW = "new";

const BLANK: Person = {
  id: NEW,
  email: "",
  name: "",
  first_name: "",
  last_name: "",
  phone: null,
  phone_verified: false,
  source: "access",
  photo_at: null,
  owner: false,
  added_by: null,
  added_at: "",
  last_seen_at: null,
  role_ids: [],
};

/** The active roles, by id, when you may read them (Roles View); else none, and a role shows
 * by its id. */
let roleList: Role[] = [];
export const roleName = (id: string): string => roleList.find((r) => r.id === id)?.name ?? id;
/** The active roles, from any view that read them (Auth, Audit Logs). */
export const setRoleList = (list: Role[]): void => {
  roleList = list;
};

/** A person's roles as one line: "Deets, Intern", or null for none. */
const roleLine = (p: Person): string | null => (p.role_ids.length ? p.role_ids.map(roleName).join(", ") : null);

export function authView(): View {
  return {
    id: "auth",
    title: "Auth",
    build() {
      const root = el(`
        <div class="auth">
          <div class="auth-top">
            <h2 class="auth-title">Users</h2>
            <span class="view-only" hidden>View only</span>
            <p class="cfg-status auth-status" role="status"></p>
            <div class="dt-add"><div class="tabs" role="group"><button class="tab auth-add" type="button" aria-label="Add a user">Add</button></div></div>
          </div>
          <div class="auth-scroll own-scrollbars">
            <table class="dt dt-plain auth-table">
              <thead><tr></tr></thead>
              <tbody><tr><td class="empty" colspan="${COLUMNS.length}">Loading</td></tr></tbody>
            </table>
          </div>
        </div>`);
      const q = <T extends HTMLElement>(s: string) => root.querySelector<T>(s)!;
      const add = q<HTMLButtonElement>(".auth-add");
      const status = q<HTMLElement>(".auth-status");
      const body = q<HTMLElement>(".auth-table tbody");
      q<HTMLElement>(".auth-table thead tr").append(
        ...COLUMNS.map((c) => {
          const th = document.createElement("th");
          th.scope = "col";
          th.textContent = c;
          if (c === "Phone") th.className = "auth-col-phone";
          return th;
        }),
      );

      let view: People | undefined;
      let open: string | null = null; // the row whose panel is open; NEW for the blank row
      let drafting = false; // the blank row is there
      let confirming: string | null = null; // "remove:<id>" or "email:<id>": the button that says "Confirm?"
      let confirmTimer = 0;
      const say = (text: string) => (status.textContent = text);

      const confirm = (key: string) => {
        window.clearTimeout(confirmTimer);
        confirming = key;
        confirmTimer = window.setTimeout(() => {
          confirming = null;
          draw();
        }, tokenMs("--confirm-hold"));
      };

      const viewOnly = q<HTMLElement>(".view-only");
      const draw = () => {
        if (!view) return;
        const editing = may("people", "edit");
        viewOnly.hidden = editing;
        add.closest<HTMLElement>(".dt-add")!.hidden = !editing;
        add.disabled = !view.store;
        if (!view.store) {
          body.replaceChildren(message("No store: start Postgres (npm run db:start), then restart npm run dev"));
          return;
        }
        // A panel with unsaved words keeps them: only its row's facts are drawn again.
        const kept = open ? body.querySelector<HTMLElement>(`.auth-panel[data-id="${CSS.escape(open)}"]`) : null;
        const rows: HTMLElement[] = [];
        for (const p of view.people) {
          rows.push(row(p, view.you, open === p.id, confirming === `remove:${p.id}`, editing));
          if (open === p.id) rows.push(kept ?? panel(p));
        }
        if (drafting && editing) rows.push(row(BLANK, view.you, true, false, editing), kept ?? panel(BLANK));
        // The open panel's roles follow the list (a grant or a revoke redraws them).
        if (kept && open) {
          const now = view.people.find((x) => x.id === open);
          if (now) drawRoles(kept, now);
        }
        body.replaceChildren(...rows);
        const save = kept?.querySelector<HTMLButtonElement>('[data-act="save"]');
        if (save) save.textContent = confirming === `email:${open}` ? "Confirm?" : "Save";
      };

      /** Draws a list from the people store. The store drops a read that answers after a save
       * (LOADING.md §2), so this never draws the list from before one. */
      const take = (next: People) => {
        view = next;
        forgetPhotos(next.people);
        if (open && open !== NEW && !next.people.some((p) => p.id === open)) open = null;
        draw();
        if (!next.linked) say("Cloudflare is not linked: the list is saved, but sign-in follows Cloudflare's own group");
      };
      const failed = (what: string, err: unknown, tell: (text: string) => void = say) =>
        tell(err instanceof Error && err.message ? `${what}: ${err.message}` : what);
      /** The result of a save that reached Cloudflare's group, or why it did not. */
      const outcome = (next: People, what: string) =>
        next.warning ? `${what}. Cloudflare not updated: ${next.warning}` : what;

      /** Opens a row's panel, or closes it. Any other panel closes, and the blank row goes. */
      const toggle = (id: string) => {
        if (id === NEW) return; // the blank row's panel stays open: Discard takes it away
        open = open === id ? null : id;
        drafting = false;
        if (confirming?.startsWith("email:")) confirming = null;
        // A new panel starts from the stored values.
        body.querySelector(".auth-panel")?.remove();
        draw();
        body.querySelector<HTMLInputElement>(".auth-panel input")?.focus();
      };

      add.addEventListener("click", () => {
        if (!drafting) {
          drafting = true;
          open = NEW;
          if (confirming?.startsWith("email:")) confirming = null;
          body.querySelector(".auth-panel")?.remove();
          draw();
        }
        const first = body.querySelector<HTMLInputElement>(`.auth-panel[data-id="${NEW}"] input[name="first_name"]`);
        first?.scrollIntoView({ block: "nearest" });
        first?.focus();
      });

      body.addEventListener("click", async (e) => {
        const t = e.target as HTMLElement;
        const remove = t.closest<HTMLButtonElement>(".auth-remove");
        if (remove) {
          const id = remove.closest<HTMLElement>(".auth-row")?.dataset.id;
          if (!id || remove.disabled) return;
          if (confirming !== `remove:${id}`) {
            confirm(`remove:${id}`);
            draw();
            return;
          }
          window.clearTimeout(confirmTimer);
          confirming = null;
          say("Removing");
          try {
            const next = await people.write(() => removePerson(id));
            say(outcome(next, "Removed"));
          } catch (err) {
            draw();
            failed("Not removed", err);
          }
          return;
        }
        const r = t.closest<HTMLElement>(".auth-row");
        if (r?.dataset.id) toggle(r.dataset.id);
      });

      body.addEventListener("keydown", (e) => {
        const r = (e.target as HTMLElement).closest<HTMLElement>(".auth-row");
        if (!r?.dataset.id || e.target !== r || (e.key !== "Enter" && e.key !== " ")) return;
        e.preventDefault();
        toggle(r.dataset.id);
      });

      /** The panel under a row: the picture, a text box for each field, Discard | Save. */
      const panel = (p: Person): HTMLElement => {
        const isNew = p.id === NEW;
        const tr = document.createElement("tr");
        tr.className = "auth-panel";
        tr.dataset.id = p.id;
        const td = document.createElement("td");
        td.colSpan = COLUMNS.length;
        const box = el(`
          <form class="auth-edit" autocomplete="off">
            <div class="auth-edit-photo">
              <div class="auth-photo-big" aria-hidden="true"></div>
              <label class="cfg-button auth-photo-pick">Choose picture
                <input type="file" accept="image/*" hidden />
              </label>
              <button class="cfg-button auth-photo-clear" type="button">Remove picture</button>
            </div>
            <div class="auth-edit-main">
              <div class="auth-edit-bar">
                <p class="cfg-status auth-edit-status" role="status"></p>
                <div class="tabs cfg-actions" role="group" aria-label="Changes">
                  <button class="tab" type="button" data-act="discard" disabled>Discard</button>
                  <button class="tab cfg-save" type="submit" data-act="save" disabled>Save</button>
                </div>
              </div>
              <div class="auth-edit-fields">
                <label class="auth-field"><span>First name</span>
                  <input class="cfg-field" name="first_name" type="text" maxlength="49" placeholder="First" /></label>
                <label class="auth-field"><span>Last name</span>
                  <input class="cfg-field" name="last_name" type="text" maxlength="49" placeholder="Last" /></label>
                <label class="auth-field"><span>Email</span>
                  <input class="cfg-field" name="email" type="email" maxlength="254" placeholder="Email" /></label>
                <label class="auth-field"><span>Phone</span>
                  <input class="cfg-field" name="phone" type="tel" maxlength="24" placeholder="XXX-XXX-XXXX" /></label>
                <div class="auth-field auth-roles"><span>Role</span>
                  <div class="role-chips"></div></div>
              </div>
            </div>
          </form>`) as HTMLFormElement;
        const field = (n: string) => box.querySelector<HTMLInputElement>(`[name="${n}"]`)!;
        const names = ["first_name", "last_name", "email", "phone"] as const;
        const stored = (): Record<(typeof names)[number], string> => {
          const now = view?.people.find((x) => x.id === p.id) ?? p;
          return {
            first_name: now.first_name,
            last_name: now.last_name,
            email: now.email,
            phone: now.phone ? showPhone(now.phone) : "",
          };
        };
        const big = box.querySelector<HTMLElement>(".auth-photo-big")!;
        const pick = box.querySelector<HTMLElement>(".auth-photo-pick")!;
        const clear = box.querySelector<HTMLButtonElement>(".auth-photo-clear")!;
        const file = box.querySelector<HTMLInputElement>('input[type="file"]')!;
        const discard = box.querySelector<HTMLButtonElement>('[data-act="discard"]')!;
        const save = box.querySelector<HTMLButtonElement>('[data-act="save"]')!;
        const panelStatus = box.querySelector<HTMLElement>(".auth-edit-status")!;
        const tell = (text: string) => (panelStatus.textContent = text);
        let saving = false;
        /** A Save found an empty box: their titles and Save are red, and the next Save saves. */
        let warned = false;
        const missing = () => names.filter((k) => !field(k).value.trim());
        /** The red titles follow the boxes as they are typed in, once a Save has asked. */
        const showMissing = () => {
          const gone = warned ? missing() : [];
          for (const k of names) field(k).closest(".auth-field")!.classList.toggle("is-missing", gone.includes(k));
          save.classList.toggle("is-warn", gone.length > 0);
        };
        /** Discard | Save grey when the boxes hold the stored values (Config's rule). On the
         * blank row Discard always works: it takes the row away. */
        const showChanges = () => {
          const was = stored();
          const n = names.filter((k) => field(k).value.trim() !== was[k]).length;
          discard.disabled = (n === 0 && !isNew) || saving;
          save.disabled = n === 0 || saving;
          if (n) tell("");
          showMissing();
        };
        const fill = () => {
          const was = stored();
          for (const k of names) field(k).value = was[k];
          warned = false;
          showChanges();
        };
        fill();
        fillPhoto(big, p);
        clear.disabled = !p.photo_at;
        // A picture needs the person on the list first: it is stored under their id.
        if (isNew) {
          file.disabled = true;
          pick.classList.add("is-disabled");
          pick.title = "Save first: then a picture can be added";
        }
        box.addEventListener("input", showChanges);

        discard.addEventListener("click", () => {
          window.clearTimeout(confirmTimer);
          if (confirming === `email:${p.id}`) confirming = null;
          save.textContent = "Save";
          if (isNew) {
            drafting = false;
            open = null;
            tr.remove();
            draw();
            return;
          }
          fill();
        });

        /** The blank row's first Save: on the list, then the row is theirs, its panel open. */
        const create = async () => {
          const first = field("first_name").value.trim();
          const last = field("last_name").value.trim();
          const email = field("email").value.trim().toLowerCase();
          let next = await people.write(() => addPerson(email, [first, last].filter(Boolean).join(" ")));
          const added = next.people.find((x) => x.id === next.added);
          // The list splits a new name at its last space; a last name with a space in it ("de la
          // Cruz") and a phone number are saved as typed with one more call.
          const phone = field("phone").value;
          if (added && (added.first_name !== first || added.last_name !== last || phone.trim())) {
            next = await people.write(() => updatePerson(added.id, { first_name: first, last_name: last, phone, email }));
          }
          drafting = false;
          open = added?.id ?? null;
          tr.remove();
          draw(); // the store drew the list already; now the row is theirs, its panel open
          const fresh = body.querySelector<HTMLElement>(".auth-panel .auth-edit-status");
          if (fresh) fresh.textContent = outcome(next, "Added");
          else say(outcome(next, "Added"));
        };

        box.addEventListener("submit", async (e) => {
          e.preventDefault();
          if (save.disabled) return;
          const gone = missing();
          if (gone.length && !warned) {
            warned = true;
            showMissing();
            const no = gone.map((k) => LABEL[k]).join(", ");
            tell(gone.includes("email")
              ? `Missing: ${no}. Save again to save it; no sign-in until an email is added`
              : `Missing: ${no}. Save again to save it as it is`);
            return;
          }
          const person = view?.people.find((x) => x.id === p.id) ?? p;
          const email = field("email").value.trim().toLowerCase();
          if (!isNew && email !== person.email && confirming !== `email:${p.id}`) {
            confirm(`email:${p.id}`);
            save.textContent = "Confirm?";
            tell("The email is who signs in");
            return;
          }
          window.clearTimeout(confirmTimer);
          confirming = null;
          saving = true;
          showChanges();
          tell(isNew ? "Adding" : "Saving");
          try {
            if (isNew) {
              await create();
              return;
            }
            const next = await people.write(() =>
              updatePerson(p.id, {
                first_name: field("first_name").value,
                last_name: field("last_name").value,
                phone: field("phone").value,
                email: field("email").value,
              }),
            );
            save.textContent = "Save";
            saving = false;
            fill(); // the boxes now hold the stored values (a phone number as the store keeps it)
            tell(outcome(next, "Saved"));
          } catch (err) {
            save.textContent = "Save";
            failed(isNew ? "Not added" : "Not saved", err, tell);
          } finally {
            saving = false;
            if (box.isConnected) showChanges();
          }
        });

        file.addEventListener("change", async () => {
          const f = file.files?.[0];
          file.value = "";
          if (!f || isNew) return;
          tell("Saving the picture");
          try {
            const webp = await squareWebp(f);
            const next = await people.write(() => setPersonPhoto(p.id, webp));
            const fresh = next.people.find((x) => x.id === p.id);
            if (fresh) {
              fillPhoto(big, fresh);
              clear.disabled = !fresh.photo_at;
            }
            tell("Picture saved");
          } catch (err) {
            failed("Picture not saved", err, tell);
          }
        });

        clear.addEventListener("click", async () => {
          tell("Removing the picture");
          try {
            await people.write(() => setPersonPhoto(p.id, null));
            big.replaceChildren();
            delete big.dataset.photoAt;
            clear.disabled = true;
            tell("Picture removed");
          } catch (err) {
            failed("Picture not removed", err, tell);
          }
        });

        // People View (fork 3A): the card reads only; no Save, no picture change.
        if (!may("people", "edit")) {
          for (const k of names) field(k).readOnly = true;
          box.querySelector<HTMLElement>(".cfg-actions")!.hidden = true;
          pick.hidden = true;
          clear.hidden = true;
        }
        td.append(box);
        tr.append(td);
        drawRoles(tr, p);
        return tr;
      };

      /** A panel's roles: a chip per role, with ✕ and an Add list at Roles Edit (never on your
       * own row, nor the blank one). */
      function drawRoles(tr: HTMLElement, p: Person): void {
        const holder = tr.querySelector<HTMLElement>(".role-chips");
        if (!holder) return;
        const can = may("roles", "edit") && p.id !== NEW && p.id !== myId();
        const chips: HTMLElement[] = p.role_ids.map((id) => {
          const chip = el(`<span class="role-chip"><span></span></span>`);
          chip.firstElementChild!.textContent = roleName(id);
          if (can) {
            const x = el(`<button class="role-chip-remove" type="button">✕</button>`);
            x.dataset.revoke = id;
            x.setAttribute("aria-label", `Revoke ${roleName(id)}`);
            chip.append(x);
          }
          return chip;
        });
        if (!chips.length) chips.push(dim(p.id === NEW ? "Save first, then add a role" : "No role"));
        const free = roleList.filter((r) => !p.role_ids.includes(r.id));
        if (can && free.length) {
          const add = el(`<select class="cfg-field role-add" aria-label="Add a role"><option value="">Add a role</option></select>`) as HTMLSelectElement;
          for (const r of free) {
            const o = document.createElement("option");
            o.value = r.id;
            o.textContent = r.name;
            add.append(o);
          }
          chips.push(add);
        }
        holder.replaceChildren(...chips);
      }

      /** A grant or a revoke from a panel: the list again from the answer, your own access read
       * again (a change to the roles can change it), and ehr's reason when it refused. */
      const changeRole = async (tr: HTMLElement, person: string, role: string, grant: boolean) => {
        const tell = (text: string) => (tr.querySelector<HTMLElement>(".auth-edit-status")!.textContent = text);
        tell(grant ? "Granting" : "Revoking");
        try {
          await people.write(() => (grant ? grantRole(person, role) : revokeRole(person, role)));
          accessChanged();
          tell(`${roleName(role)} ${grant ? "granted" : "revoked"}`);
        } catch (err) {
          failed(grant ? "Not granted" : "Not revoked", err, tell);
          draw();
        }
      };
      body.addEventListener("click", (e) => {
        const x = (e.target as HTMLElement).closest<HTMLButtonElement>("[data-revoke]");
        const tr = x?.closest<HTMLElement>(".auth-panel");
        if (x && tr?.dataset.id) void changeRole(tr, tr.dataset.id, x.dataset.revoke!, false);
      });
      body.addEventListener("change", (e) => {
        const add = (e.target as HTMLElement).closest<HTMLSelectElement>(".role-add");
        const tr = add?.closest<HTMLElement>(".auth-panel");
        if (add && add.value && tr?.dataset.id) void changeRole(tr, tr.dataset.id, add.value, true);
      });

      // The roles' names, when you may read them (Roles View); read once you may.
      let rolesRead = false;
      const readRoles = () => {
        if (rolesRead || !may("roles", "view")) return;
        rolesRead = true;
        roles(0).subscribe(root, (s) => {
          if (!s.value) return;
          setRoleList(s.value.active);
          draw();
        });
      };
      readRoles();
      onAccess(() => {
        readRoles();
        draw();
      });

      // The people store (LOADING.md §3), shared with Teams › People. It reads again when the
      // window comes back (someone else may have changed the list), and on its own when the
      // agent comes back. A list already drawn stays while a later read fails.
      people.subscribe(body, (s) => {
        if (s.value) return take(s.value);
        if (s.status === "down") body.replaceChildren(message("The agent's dev server is not running"));
        else if (s.status === "refused") body.replaceChildren(message("The agent's dev server refused"));
      });
      return root;
    },
  };
}

function message(text: string): HTMLElement {
  const tr = document.createElement("tr");
  const td = document.createElement("td");
  td.className = "empty";
  td.colSpan = COLUMNS.length;
  td.textContent = text;
  tr.append(td);
  return tr;
}

/** One person's row, or the blank row (every cell a placeholder). The whole row opens its
 * panel; Remove does not. */
function row(p: Person, you: string, isOpen: boolean, confirming: boolean, editing: boolean): HTMLElement {
  const tr = document.createElement("tr");
  tr.className = "auth-row";
  tr.dataset.id = p.id;
  tr.tabIndex = 0;
  tr.setAttribute("aria-expanded", String(isOpen));
  const blank = p.id === NEW;
  const mine = !blank && (you === p.email || (you === "owner" && p.owner));

  const photo = document.createElement("div");
  photo.className = "auth-photo";
  photo.setAttribute("aria-hidden", "true");
  fillPhoto(photo, p);

  const phone = document.createElement("span");
  phone.className = "auth-phone";
  if (p.phone) {
    phone.textContent = showPhone(p.phone);
    phone.append(phoneMark(p.phone_verified));
  } else {
    phone.textContent = "XXX-XXX-XXXX";
    phone.classList.add("is-placeholder");
  }

  const cells: (string | HTMLElement)[] = blank
    ? [photo, dim("First"), dim("Last"), dim("Email"), phone, dim("Role"), dim("Source"), dim("Added by"), dim("Added"), dim("Last seen")]
    : [
        photo,
        p.first_name || dim("First"),
        p.last_name || dim("Last"),
        !p.email ? dim("Email") : mine ? `${p.email} (you)` : p.email,
        phone,
        roleLine(p) ?? dim("No role"),
        p.source === "idp" ? "IdP" : "Access",
        p.owner ? "Deets" : p.added_by ?? "Cloudflare's group",
        day(p.added_at),
        p.last_seen_at ? ago(p.last_seen_at) : "never",
      ];
  for (const c of cells) {
    const td = document.createElement("td");
    td.append(c);
    // The phone column keeps its width with or without the mark, so a save that adds the
    // mark does not move the columns after it.
    if (c === phone) td.className = "auth-col-phone";
    tr.append(td);
  }
  // ✕ in the table's tools column; a first press turns it into "Confirm?" in the error colors,
  // in the same column, for --confirm-hold (TABLES.md §3.12): a remove takes sign-in away at once.
  const last = document.createElement("td");
  last.className = "dt-tools-cell";
  if (!p.owner && !blank && editing) {
    const who = p.email || [p.first_name, p.last_name].filter(Boolean).join(" ") || "this person";
    const b = toolButton("remove", `Remove ${who}`);
    b.classList.add("auth-remove");
    if (confirming) {
      b.classList.add("dt-confirm");
      b.textContent = "Confirm?";
      b.setAttribute("aria-label", `Confirm: remove ${who}`);
    }
    const tools = el(`<div class="dt-tools"></div>`);
    tools.append(b);
    last.append(tools);
  }
  tr.append(last);
  return tr;
}

/** A cell's placeholder, dimmer than any value. */
function dim(text: string): HTMLElement {
  const s = document.createElement("span");
  s.className = "is-placeholder";
  s.textContent = text;
  return s;
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
