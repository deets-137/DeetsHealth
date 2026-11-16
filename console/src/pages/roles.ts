// Security › Roles (docs/ideas/ACCESS-ROLES.md; the owner, 2026-11-14): the role templates as
// tables, in Auth's look (auth.css). A draft to see the roles, not wired to any store: a reload
// puts back the draft below.
//
// - Two tables, each with its own title (the owner's sketch, 2026-11-14): "Customer" (the
//   hospital's roles), then "DeetsHealth Team" (ours). Ours get into a hospital's console only
//   when that hospital approves it (STAFF-SIGN-IN.md M6).
// - One row per role; one column per lever. Each cell is None, View or Edit (the owner's
//   choice, 2026-11-14). Clicking a cell moves it to the next level (None → View → Edit → None).
// - Add, at the right of each title, puts a new role at that table's foot: its name a
//   placeholder, every lever None, its panel open (the owner, 2026-11-14; as Auth's Add).
//   Discard on a new role takes it away; so does opening another role.
// - Clicking a role's name opens a panel under its row, as on Auth: the name in a text box as
//   tall as Discard | Save, on one row with them (no label; the owner's sketch, 2026-11-14), a
//   result line under them only when there is one, and a table of the users who have the role (read only: a user's role is
//   set elsewhere). One panel is open at a time, across both tables.
import { el } from "./page";
import type { View } from "./subnav";

type Level = "none" | "view" | "edit";

const LEVERS = ["PHI", "Auth", "Roles", "People", "Agent", "Vendors", "Rules", "Testing", "Analytics", "Flags", "Audit logs"];

const WORD: Record<Level, string> = { none: "None", view: "View", edit: "Edit" };
const NEXT: Record<Level, Level> = { none: "view", view: "edit", edit: "none" };
const LETTER: Record<string, Level> = { n: "none", v: "view", e: "edit" };

/** The columns of the users table in a role's panel: Auth's, less what is edited there. */
const USER_COLUMNS = ["", "First", "Last", "Email", "Last seen"];

type Role = {
  id: string;
  name: string;
  levels: Level[];
  /** Added with Add and never saved: Discard takes it away. */
  fresh: boolean;
};

// The draft of 2026-11-14 (ACCESS-ROLES.md §4). The letters are in LEVERS' order:
// n = None, v = View, e = Edit.
const CUSTOMER: [string, string][] = [
  ["Director", "veeeeeeevvv"],
  ["Manager", "vvveeveevvv"],
  ["Senior Analyst", "vnnneveevvn"],
  ["Junior Analyst", "nnnnvnvevnn"],
  ["Clinician", "vnnnnnnnnen"],
  ["Compliance", "vvvvvvvnvvv"],
];
const TEAM: [string, string][] = [
  ["Engineer", "nvvnvvvevnv"],
  ["Intern", "nnnnvnvevnn"],
];

let nextId = 0;
const newId = () => `role-${++nextId}`;

const draft = (rows: [string, string][]): Role[] =>
  rows.map(([name, letters]) => ({
    id: newId(),
    name,
    levels: [...letters].map((c) => LETTER[c] ?? "none"),
    fresh: false,
  }));

export function rolesView(): View {
  return {
    id: "roles",
    title: "Roles",
    build() {
      const root = el(`
        <div class="auth roles">
          <div class="auth-scroll own-scrollbars roles-scroll"></div>
        </div>`);
      const tables = [table("Customer", draft(CUSTOMER)), table("DeetsHealth Team", draft(TEAM))];
      // One panel open at a time: opening one closes the other table's.
      for (const t of tables) t.onOpen = () => tables.filter((o) => o !== t).forEach((o) => o.close());
      root.querySelector<HTMLElement>(".roles-scroll")!.append(...tables.map((t) => t.section));
      return root;
    },
  };
}

type Table = { section: HTMLElement; close: () => void; onOpen: () => void };

/** One titled table: the title and Add (Auth's "Users" row), then a row per role. */
function table(title: string, roles: Role[]): Table {
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
  const head = section.querySelector<HTMLElement>("thead tr")!;
  for (const l of LEVERS) {
    const th = document.createElement("th");
    th.scope = "col";
    th.textContent = l;
    head.append(th);
  }
  const body = section.querySelector<HTMLElement>("tbody")!;

  let open: string | null = null; // the role whose panel is open

  const self: Table = {
    section,
    close: () => {
      if (open === null && !roles.some((r) => r.fresh)) return;
      open = null;
      dropFresh();
      body.querySelector(".roles-panel")?.remove();
      draw();
    },
    onOpen: () => {},
  };

  /** A new role never saved goes when another role opens. */
  const dropFresh = () => {
    for (let i = roles.length - 1; i >= 0; i--) if (roles[i].fresh && roles[i].id !== open) roles.splice(i, 1);
  };

  const draw = () => {
    // A panel with unsaved words keeps them: only the rows are drawn again.
    const kept = open ? body.querySelector<HTMLElement>(`.roles-panel[data-id="${CSS.escape(open)}"]`) : null;
    const rows: HTMLElement[] = [];
    for (const r of roles) {
      rows.push(row(r, open === r.id));
      if (open === r.id) rows.push(kept ?? panel(r));
    }
    body.replaceChildren(...rows);
  };

  const focusName = () => {
    const box = body.querySelector<HTMLInputElement>('.roles-panel input[name="name"]');
    box?.scrollIntoView({ block: "nearest" });
    box?.focus();
  };

  add.addEventListener("click", () => {
    const fresh = roles.find((r) => r.fresh);
    if (!fresh) {
      self.onOpen();
      const r: Role = { id: newId(), name: "", levels: LEVERS.map(() => "none"), fresh: true };
      roles.push(r);
      open = r.id;
      body.querySelector(".roles-panel")?.remove();
      draw();
    }
    focusName();
  });

  body.addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    const level = t.closest<HTMLButtonElement>(".roles-level");
    if (level) {
      const r = roles.find((x) => x.id === level.closest<HTMLElement>("tr")?.dataset.id);
      const i = Number(level.dataset.index);
      if (!r) return;
      r.levels[i] = NEXT[r.levels[i]];
      setLevel(level, r, i);
      return;
    }
    const name = t.closest<HTMLButtonElement>(".roles-open");
    const id = name?.closest<HTMLElement>("tr")?.dataset.id;
    if (!id) return;
    const r = roles.find((x) => x.id === id);
    if (r?.fresh) return; // a new role's panel stays open: Discard takes it away
    self.onOpen();
    open = open === id ? null : id;
    dropFresh();
    // A new panel starts from the stored name.
    body.querySelector(".roles-panel")?.remove();
    draw();
    if (open) focusName();
  });

  /** The panel under a role: its name in a text box, Discard | Save, and its users. */
  const panel = (r: Role): HTMLElement => {
    const tr = document.createElement("tr");
    tr.className = "auth-panel roles-panel";
    tr.dataset.id = r.id;
    const td = document.createElement("td");
    td.colSpan = LEVERS.length + 1;
    const box = el(`
      <form class="auth-edit" autocomplete="off">
        <div class="auth-edit-main">
          <div class="auth-edit-bar">
            <input class="cfg-field roles-name-box" name="name" type="text" maxlength="49"
              placeholder="Role name" aria-label="Role name" />
            <div class="tabs cfg-actions" role="group" aria-label="Changes">
              <button class="tab" type="button" data-act="discard" disabled>Discard</button>
              <button class="tab cfg-save" type="submit" data-act="save" disabled>Save</button>
            </div>
          </div>
          <p class="cfg-status auth-edit-status roles-edit-status" role="status"></p>
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
    const discard = box.querySelector<HTMLButtonElement>('[data-act="discard"]')!;
    const save = box.querySelector<HTMLButtonElement>('[data-act="save"]')!;
    const status = box.querySelector<HTMLElement>(".auth-edit-status")!;
    const tell = (text: string) => (status.textContent = text);

    box.querySelector<HTMLElement>(".roles-users-table thead tr")!.append(
      ...USER_COLUMNS.map((c) => {
        const th = document.createElement("th");
        th.scope = "col";
        th.textContent = c;
        return th;
      }),
    );
    // No user has a role yet: roles are not stored, so the list is always empty for now.
    const empty = document.createElement("td");
    empty.className = "empty";
    empty.colSpan = USER_COLUMNS.length;
    empty.textContent = "No users yet: roles are not stored";
    const emptyRow = document.createElement("tr");
    emptyRow.append(empty);
    box.querySelector<HTMLElement>(".roles-users-table tbody")!.append(emptyRow);

    /** Discard | Save grey when the box holds the stored name (Config's rule). On a new role
     * Discard always works: it takes the role away. */
    const showChanges = () => {
      const same = field.value.trim() === r.name;
      discard.disabled = same && !r.fresh;
      save.disabled = same;
      if (!same) tell("");
    };
    field.value = r.name;
    showChanges();
    field.addEventListener("input", showChanges);

    discard.addEventListener("click", () => {
      if (r.fresh) {
        roles.splice(roles.indexOf(r), 1);
        open = null;
        tr.remove();
        draw();
        return;
      }
      field.value = r.name;
      showChanges();
    });

    box.addEventListener("submit", (e) => {
      e.preventDefault();
      if (save.disabled) return;
      const name = field.value.trim();
      if (!name) return tell("Not saved: a name");
      if (roles.some((x) => x !== r && x.name.toLowerCase() === name.toLowerCase())) {
        return tell("Not saved: that name is taken");
      }
      const added = r.fresh;
      r.name = name;
      r.fresh = false;
      field.value = name;
      showChanges();
      draw(); // the row shows the new name; this panel is kept
      tell(added ? "Added on this page only" : "Saved on this page only");
    });

    td.append(box);
    tr.append(td);
    return tr;
  };

  draw();
  return self;
}

/** One role's row: its name (a button that opens the panel), then a level per lever. */
function row(r: Role, isOpen: boolean): HTMLElement {
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
  r.levels.forEach((_, i) => {
    const td = document.createElement("td");
    const level = document.createElement("button");
    level.type = "button";
    level.className = "roles-level";
    level.dataset.index = String(i);
    setLevel(level, r, i);
    td.append(level);
    tr.append(td);
  });
  return tr;
}

function setLevel(b: HTMLButtonElement, r: Role, i: number) {
  const level = r.levels[i];
  b.dataset.level = level;
  b.textContent = WORD[level];
  b.setAttribute("aria-label", `${r.name || "New role"}, ${LEVERS[i]}: ${WORD[level]}`);
}
