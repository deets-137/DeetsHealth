// Audit Logs (CONSOLE.md §3; docs/ideas/ACCESS-ROLES.md §5.1): one panel with its own nav
// (subnav.ts), at the nav's foot between Security and Settings. The owner's three views
// (2026-11-13): Config, Calls, Security. Each is the audit trail's rows of its kind, newest first,
// as a plain table in Auth's look: the page's own look is a fork for the owner (§6, part 3,
// step 6), and this is the stand-in until he picks.
//
// - Config: settings saved and seeded (`settings.*`). Calls: a call's transcript read, a live
//   call watched, a Test run. Security: the People list, roles, page opens and every refusal.
// - One page of 100 rows at a time (Older, Newest), so no more than 100 rows are ever drawn
//   (CONSOLE.md §12).
// - Who: the person's name from the People list when your role shows it, else the start of
//   their id. A row holds ids and field names only (audit.rs); the names come from the list.
// - Check chain walks the whole trail (audit::verify) and says whether every row is as written.
// - Audit logs View (ROUTE_NEEDS); the page is hidden without it.
import { audit, auditVerify, leverName, type AuditRow, type Person } from "../agent/api";
import { audit as auditStore, people, roles as rolesStore } from "../agent/stores";
import { once } from "../agent/store";
import { may } from "../access";
import { roleName, setRoleList } from "./auth";
import { el, type Page } from "./page";
import { subnavPage, type View } from "./subnav";

const PAGE = 100;
const COLUMNS = ["When", "Who", "Roles", "Action", "What", "Fields", "Versions", "Result"];

type Kind = "config" | "calls" | "security";

/** Which view a row belongs to. */
function kindOf(r: AuditRow): Kind {
  if (r.action.startsWith("settings.") || r.action === "rpc.save_settings") return "config";
  if (r.object_kind === "call" || r.object_kind === "test_run" || r.action === "call.watch") return "calls";
  return "security";
}

/** An action's words: "settings.save" → "Settings saved". */
const ACTION: Record<string, string> = {
  "settings.save": "Settings saved",
  "settings.seed": "Default seeded",
  "person.add": "Person added",
  "person.remove": "Person removed",
  "person.card": "Card saved",
  "person.photo": "Picture changed",
  "person.seed": "Person brought in",
  "role.grant": "Role granted",
  "role.revoke": "Role revoked",
  "page.open": "Page opened",
  "transcript.view": "Transcript read",
  "call.watch": "Live calls watched",
  "test.run": "Test run",
};
const actionWords = (a: string) => ACTION[a] ?? (a.startsWith("rpc.") ? `Asked: ${a.slice(4).replace(/_/g, " ")}` : a);

export function auditPage(): Page {
  return {
    ...subnavPage({
      id: "audit",
      title: "Audit Logs",
      lastViewKey: "deets.console.audit",
      lever: "audit_logs",
      // The order is the owner's (2026-11-13): Config, Calls, Security.
      views: [trailView("config", "Config"), trailView("calls", "Calls"), trailView("security", "Security")],
    }),
    bottom: true,
  };
}

function trailView(kind: Kind, title: string): View {
  return {
    id: kind,
    title,
    build() {
      const root = el(`
        <div class="auth audit">
          <div class="auth-top">
            <h2 class="auth-title"></h2>
            <p class="cfg-status audit-status" role="status"></p>
            <div class="tabs" role="group" aria-label="The trail">
              <button class="tab" type="button" data-act="newest" disabled>Newest</button>
              <button class="tab" type="button" data-act="older">Older</button>
              <button class="tab" type="button" data-act="verify">Check chain</button>
            </div>
          </div>
          <div class="auth-scroll own-scrollbars">
            <table class="dt dt-plain auth-table audit-table">
              <thead><tr></tr></thead>
              <tbody><tr><td class="empty" colspan="${COLUMNS.length}">Loading</td></tr></tbody>
            </table>
          </div>
        </div>`);
      root.querySelector("h2")!.textContent = title;
      const q = <T extends HTMLElement>(s: string) => root.querySelector<T>(s)!;
      const body = q<HTMLElement>("tbody");
      const status = q<HTMLElement>(".audit-status");
      const newest = q<HTMLButtonElement>('[data-act="newest"]');
      const older = q<HTMLButtonElement>('[data-act="older"]');
      const verify = q<HTMLButtonElement>('[data-act="verify"]');
      q<HTMLElement>("thead tr").append(
        ...COLUMNS.map((c) => {
          const th = document.createElement("th");
          th.scope = "col";
          th.textContent = c;
          return th;
        }),
      );
      const say = (text: string) => (status.textContent = text);

      let rows: AuditRow[] = []; // the page shown
      let first: AuditRow[] = []; // the newest page, from the store
      let names = new Map<string, string>();
      let paging = false; // an older page is shown: the store's answers wait

      const draw = () => {
        const mine = rows.filter((r) => kindOf(r) === kind);
        if (!mine.length) {
          body.replaceChildren(message(paging ? "No rows of this kind on this page: try Older" : "No rows yet"));
          return;
        }
        body.replaceChildren(...mine.map((r) => line(r, names)));
      };

      auditStore("all").subscribe(body, (s) => {
        if (s.value) {
          first = s.value.rows;
          if (!paging) {
            rows = first;
            older.disabled = rows.length < PAGE;
            draw();
          }
          return;
        }
        if (s.status === "down") body.replaceChildren(message("The agent's dev server is not running"));
        else if (s.status === "refused") body.replaceChildren(message(s.reason ?? "The agent's dev server refused"));
      });
      // The roles' names, when your role shows them.
      if (may("roles", "view")) {
        rolesStore(0).subscribe(body, (s) => {
          if (!s.value) return;
          setRoleList(s.value.active);
          draw();
        });
      }
      // The people's names, when your role shows the People list.
      if (may("people", "view")) {
        people.subscribe(body, (s) => {
          if (!s.value) return;
          names = new Map(s.value.people.map((p: Person) => [p.id, personName(p)]));
          draw();
        });
      }

      older.addEventListener("click", async () => {
        const last = rows[rows.length - 1];
        if (!last) return;
        older.disabled = true;
        say("Reading");
        try {
          const got = await once("audit", () => audit(last.seq));
          rows = got.rows;
          paging = true;
          newest.disabled = false;
          older.disabled = rows.length < PAGE;
          say("");
          draw();
        } catch (err) {
          older.disabled = false;
          say(err instanceof Error && err.message ? `Not read: ${err.message}` : "Not read");
        }
      });
      newest.addEventListener("click", () => {
        paging = false;
        rows = first;
        newest.disabled = true;
        older.disabled = rows.length < PAGE;
        void auditStore("all").refresh("user");
        draw();
      });
      verify.addEventListener("click", async () => {
        verify.disabled = true;
        say("Checking every row");
        try {
          const r = await once("audit.verify", () => auditVerify());
          say(r.ok ? `Every row is as written (${r.rows})` : `Row ${r.first_bad} is not as written: the trail was changed there`);
        } catch (err) {
          say(err instanceof Error && err.message ? `Not checked: ${err.message}` : "Not checked");
        } finally {
          verify.disabled = false;
        }
      });
      return root;
    },
  };
}

/** "First L.", or the email, or the start of the id. */
function personName(p: Person): string {
  if (p.first_name) return p.last_name ? `${p.first_name} ${p.last_name.charAt(0)}.` : p.first_name;
  return p.email || p.id.slice(0, 8);
}

function line(r: AuditRow, names: Map<string, string>): HTMLElement {
  const tr = document.createElement("tr");
  tr.dataset.allowed = String(r.allowed);
  const when = new Date(r.at);
  const who = r.actor ? names.get(r.actor) ?? r.actor.slice(0, 8) : r.source === "system" ? "System" : "Unknown";
  const what = r.object_kind ? `${r.object_kind}${r.object_id ? ` ${r.object_kind === "person" ? names.get(r.object_id) ?? r.object_id.slice(0, 8) : r.object_id}` : ""}` : "";
  const fields = r.fields.map((f) => (f.startsWith("role:") ? roleName(f.slice(5)) : f)).join(", ");
  const versions = r.before_version || r.after_version ? `${r.before_version ?? "none"} → ${r.after_version ?? "none"}` : "";
  const lever = r.lever ? ` (${leverName(r.lever)} ${r.level === "edit" ? "Edit" : "View"})` : "";
  const cells = [
    Number.isFinite(when.getTime()) ? when.toLocaleString() : r.at,
    who,
    r.roles.map(roleName).join(", "),
    actionWords(r.action),
    what,
    fields,
    versions,
    r.allowed ? "Allowed" : `Refused${lever}`,
  ];
  for (const c of cells) {
    const td = document.createElement("td");
    td.textContent = c;
    tr.append(td);
  }
  return tr;
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
