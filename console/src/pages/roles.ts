// Security › Roles (docs/ideas/ACCESS-ROLES.md; the owner, 2026-11-14 for the look, 2026-11-15
// for the rules): the roles as tables, in Auth's look (auth.css), wired to the `roles` store
// (Hand-over 3, 2026-11-18).
//
// - Two tables, each with its own title (the owner's sketch, 2026-11-14): "Customer" (the
//   hospital's roles), then "DeetsHealth Team" (ours). Ours get into a hospital's console only
//   when that hospital approves it (STAFF-SIGN-IN.md M6).
// - One row per role; one column per lever. Each cell is None, View or Edit (the owner's
//   choice, 2026-11-14). Clicking a cell moves it to the next level (None → View → Edit → None).
// - The changes are a draft of the whole list. Discard | Save sit on the page's title row, as on
//   Config: Save makes a new version of the roles, active at once for everyone (ehr checks it:
//   access.rs › roles_from, may_save_roles). A refusal names the role's place or the rule.
// - Add, at the right of each title, puts a new role at that table's foot: its name a
//   placeholder, every lever None, its panel open (as Auth's Add). Its id comes from its name
//   at Save. Remove role, in a role's panel, takes it out of the draft; ehr refuses a role
//   someone still holds.
// - Clicking a role's name opens a panel under its row: the name in a text box, Remove role, and
//   the users who hold it (from the People list; grants are on Security › Auth).
// - Roles View (fork 3A): the tables read only, a "View only" mark, no Add, no Discard | Save.
import { LEVERS, leverName, saveRoles, type Level, type Lever, type Person, type Role, type Roles } from "../agent/api";
import { people, roles as rolesStore } from "../agent/stores";
import { accessChanged, may, onAccess } from "../access";
import { el } from "./page";
import type { View } from "./subnav";

const WORD: Record<Level, string> = { none: "None", view: "View", edit: "Edit" };
const NEXT: Record<Level, Level> = { none: "view", view: "edit", edit: "none" };

/** The columns of the users table in a role's panel: Auth's, less what is edited there. */
const USER_COLUMNS = ["First", "Last", "Email", "Last seen"];

/** A role in the draft: `fresh` until saved (its id is made from its name at Save). */
type Draft = Role & { fresh?: boolean };

let nextFresh = 0;

/** A role id from its name (access.rs › role_id_ok): lower case, a letter first, at most 32,
 * unique in the list. */
function idFrom(name: string, taken: Set<string>): string {
  let base = name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "");
  if (!/^[a-z]/.test(base)) base = `role_${base}`;
  base = base.slice(0, 28) || "role";
  let id = base;
  for (let n = 2; taken.has(id); n++) id = `${base}_${n}`;
  return id;
}

export function rolesView(): View {
  return {
    id: "roles",
    title: "Roles",
    build() {
      const root = el(`
        <div class="auth roles">
          <div class="auth-top roles-top">
            <span class="view-only" hidden>View only</span>
            <p class="cfg-status roles-status" role="status"></p>
            <div class="tabs cfg-actions" role="group" aria-label="Changes">
              <button class="tab" type="button" data-act="discard" disabled>Discard</button>
              <button class="tab cfg-save" type="button" data-act="save" disabled>Save</button>
            </div>
          </div>
          <div class="auth-scroll own-scrollbars roles-scroll"><p class="empty">Loading</p></div>
        </div>`);
      const scroll = root.querySelector<HTMLElement>(".roles-scroll")!;
      const status = root.querySelector<HTMLElement>(".roles-status")!;
      const viewOnly = root.querySelector<HTMLElement>(".view-only")!;
      const actions = root.querySelector<HTMLElement>(".cfg-actions")!;
      const discard = root.querySelector<HTMLButtonElement>('[data-act="discard"]')!;
      const save = root.querySelector<HTMLButtonElement>('[data-act="save"]')!;
      const say = (text: string) => (status.textContent = text);

      let stored: Role[] = []; // the active version
      let draft: Draft[] = [];
      let holders: Person[] | null = null; // null: you may not read the People list
      let open: string | null = null; // the role whose panel is open
      let saving = false;

      const canEdit = () => may("roles", "edit");
      const changed = () => JSON.stringify(draft.map(({ fresh: _f, ...r }) => r)) !== JSON.stringify(stored);
      const showBar = () => {
        const editing = canEdit();
        viewOnly.hidden = editing;
        actions.hidden = !editing;
        discard.disabled = !changed() || saving;
        save.disabled = !changed() || saving;
      };

      const draw = () => {
        const editing = canEdit();
        // A panel's unsaved name stays: the draft holds it as it is typed.
        const sections = (["hospital", "team"] as const).map((domain) =>
          table(domain === "hospital" ? "Customer" : "DeetsHealth Team", domain, editing),
        );
        scroll.replaceChildren(...sections);
        showBar();
      };

      /** One titled table: the title and Add (Auth's "Users" row), then a row per role. */
      const table = (title: string, domain: Role["domain"], editing: boolean): HTMLElement => {
        const section = el(`
          <section class="roles-section">
            <div class="auth-top">
              <h2 class="auth-title"></h2>
              <div class="dt-add"><div class="tabs" role="group"><button class="tab auth-add roles-add" type="button">Add</button></div></div>
            </div>
            <table class="dt dt-plain auth-table roles-table">
              <thead><tr><th scope="col">Role</th></tr></thead>
              <tbody></tbody>
            </table>
          </section>`);
        section.querySelector("h2")!.textContent = title;
        section.querySelector("table")!.setAttribute("aria-label", title);
        const add = section.querySelector<HTMLButtonElement>(".roles-add")!;
        add.setAttribute("aria-label", `Add a role to ${title}`);
        section.querySelector<HTMLElement>(".dt-add")!.hidden = !editing;
        add.addEventListener("click", () => {
          const r: Draft = {
            id: `new-${++nextFresh}`,
            name: "",
            domain,
            levels: Object.fromEntries(LEVERS.map((l) => [l, "none"])) as Role["levels"],
            fresh: true,
          };
          draft.push(r);
          open = r.id;
          draw();
          focusName();
        });
        const head = section.querySelector<HTMLElement>("thead tr")!;
        for (const l of LEVERS) {
          const th = document.createElement("th");
          th.scope = "col";
          th.textContent = leverName(l);
          head.append(th);
        }
        const body = section.querySelector<HTMLElement>("tbody")!;
        const rows: HTMLElement[] = [];
        for (const r of draft.filter((x) => x.domain === domain)) {
          rows.push(row(r, open === r.id, editing));
          if (open === r.id) rows.push(panel(r, editing));
        }
        body.replaceChildren(...rows);
        return section;
      };

      const focusName = () => {
        const box = scroll.querySelector<HTMLInputElement>('.roles-panel input[name="name"]');
        box?.scrollIntoView({ block: "nearest" });
        box?.focus();
      };

      scroll.addEventListener("click", (e) => {
        const t = e.target as HTMLElement;
        const level = t.closest<HTMLButtonElement>(".roles-level");
        if (level) {
          if (!canEdit()) return;
          const r = draft.find((x) => x.id === level.closest<HTMLElement>("tr")?.dataset.id);
          const lever = level.dataset.lever as Lever;
          if (!r || !lever) return;
          r.levels[lever] = NEXT[r.levels[lever]];
          setLevel(level, r, lever);
          showBar();
          say("");
          return;
        }
        const name = t.closest<HTMLButtonElement>(".roles-open");
        const id = name?.closest<HTMLElement>("tr")?.dataset.id;
        if (!id) return;
        open = open === id ? null : id;
        draw();
        if (open) focusName();
      });

      /** The panel under a role: its name, Remove role, and the users who hold it. */
      const panel = (r: Draft, editing: boolean): HTMLElement => {
        const tr = document.createElement("tr");
        tr.className = "auth-panel roles-panel";
        tr.dataset.id = r.id;
        const td = document.createElement("td");
        td.colSpan = LEVERS.length + 1;
        const box = el(`
          <form class="auth-edit" autocomplete="off">
            <div class="auth-edit-main">
              <div class="auth-edit-bar">
                <input class="cfg-field roles-name-box" name="name" type="text" maxlength="40"
                  placeholder="Role name" aria-label="Role name" />
                <div class="tabs" role="group"><button class="tab roles-remove" type="button">Remove role</button></div>
              </div>
              <div class="roles-users">
                <h3 class="roles-users-title">Users with this role</h3>
                <table class="dt dt-plain roles-users-table">
                  <thead><tr></tr></thead>
                  <tbody></tbody>
                </table>
              </div>
            </div>
          </form>`) as HTMLFormElement;
        const field = box.querySelector<HTMLInputElement>('input[name="name"]')!;
        field.value = r.name;
        field.readOnly = !editing;
        const remove = box.querySelector<HTMLElement>(".roles-remove")!.parentElement!;
        remove.hidden = !editing;
        box.addEventListener("submit", (e) => e.preventDefault());
        field.addEventListener("input", () => {
          r.name = field.value;
          const name = tr.previousElementSibling?.querySelector<HTMLElement>(".roles-open");
          if (name) name.textContent = r.name || "Role name";
          showBar();
          say("");
        });
        box.querySelector(".roles-remove")!.addEventListener("click", () => {
          draft.splice(draft.indexOf(r), 1);
          open = null;
          draw();
          say(r.fresh ? "" : "Removed from the draft. Save to remove it");
        });
        box.querySelector<HTMLElement>(".roles-users-table thead tr")!.append(
          ...USER_COLUMNS.map((c) => {
            const th = document.createElement("th");
            th.scope = "col";
            th.textContent = c;
            return th;
          }),
        );
        const users = box.querySelector<HTMLElement>(".roles-users-table tbody")!;
        const who = holders?.filter((p) => p.role_ids.includes(r.id)) ?? null;
        if (who === null || !who.length) {
          users.append(message(who === null ? "Your role does not show the People list" : "No users hold this role", USER_COLUMNS.length));
        } else {
          for (const p of who) {
            const row = document.createElement("tr");
            for (const v of [p.first_name, p.last_name, p.email, p.last_seen_at ? p.last_seen_at.slice(5, 10) : "never"]) {
              const c = document.createElement("td");
              c.textContent = v || "";
              row.append(c);
            }
            users.append(row);
          }
        }
        td.append(box);
        tr.append(td);
        return tr;
      };

      discard.addEventListener("click", () => {
        draft = structuredClone(stored);
        open = null;
        draw();
        say("");
      });

      save.addEventListener("click", async () => {
        if (save.disabled) return;
        const unnamed = draft.find((r) => !r.name.trim());
        if (unnamed) return say("Not saved: every role needs a name");
        // A new role's id from its name; the rest keep theirs (a grant names the id).
        const taken = new Set(draft.filter((r) => !r.fresh).map((r) => r.id));
        const list: Role[] = draft.map(({ fresh, ...r }) => {
          if (!fresh) return { ...r, name: r.name.trim() };
          const id = idFrom(r.name, taken);
          taken.add(id);
          return { ...r, id, name: r.name.trim() };
        });
        saving = true;
        showBar();
        say("Saving");
        try {
          const next = await rolesStore(0).write(() => saveRoles(list));
          take(next);
          // A role you hold may have changed: your access, and the People list's names.
          accessChanged();
          void people.refresh("user");
          say(next.version ? `Saved as ${next.version.label}. Active now` : "Saved");
        } catch (err) {
          say(err instanceof Error && err.message ? `Not saved: ${err.message}` : "Not saved");
        } finally {
          saving = false;
          showBar();
        }
      });

      const take = (next: Roles) => {
        stored = next.active;
        draft = structuredClone(next.active);
        if (open && !draft.some((r) => r.id === open)) open = null;
        draw();
      };

      rolesStore(0).subscribe(scroll, (s) => {
        // An edit not saved stays while a later read lands.
        if (s.value && !changed()) return take(s.value);
        if (s.value) {
          stored = s.value.active;
          return showBar();
        }
        if (s.status === "down") scroll.replaceChildren(empty("The agent's dev server is not running"));
        else if (s.status === "refused") scroll.replaceChildren(empty(s.reason ?? "The agent's dev server refused"));
      });
      // The holders, when you may read the People list.
      let peopleRead = false;
      const readPeople = () => {
        if (peopleRead || !may("people", "view")) return;
        peopleRead = true;
        people.subscribe(scroll, (s) => {
          if (!s.value) return;
          holders = s.value.people;
          if (open) draw();
        });
      };
      readPeople();
      onAccess(() => {
        readPeople();
        draw();
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

function message(text: string, span: number): HTMLElement {
  const tr = document.createElement("tr");
  const td = document.createElement("td");
  td.className = "empty";
  td.colSpan = span;
  td.textContent = text;
  tr.append(td);
  return tr;
}

/** One role's row: its name (a button that opens the panel), then a level per lever. */
function row(r: Draft, isOpen: boolean, editing: boolean): HTMLElement {
  const tr = document.createElement("tr");
  tr.dataset.id = r.id;
  const th = document.createElement("th");
  th.scope = "row";
  th.className = "roles-name";
  const b = document.createElement("button");
  b.type = "button";
  b.className = "roles-open";
  b.setAttribute("aria-expanded", String(isOpen));
  if (r.name) b.textContent = r.name;
  else {
    const dim = document.createElement("span");
    dim.className = "is-placeholder";
    dim.textContent = "Role name";
    b.append(dim);
  }
  th.append(b);
  tr.append(th);
  for (const lever of LEVERS) {
    const td = document.createElement("td");
    const level = document.createElement("button");
    level.type = "button";
    level.className = "roles-level";
    level.dataset.lever = lever;
    level.disabled = !editing;
    setLevel(level, r, lever);
    td.append(level);
    tr.append(td);
  }
  return tr;
}

function setLevel(b: HTMLButtonElement, r: Role, lever: Lever) {
  const level = r.levels[lever] ?? "none";
  b.dataset.level = level;
  b.textContent = WORD[level];
  b.setAttribute("aria-label", `${r.name || "New role"}, ${leverName(lever)}: ${WORD[level]}`);
}
