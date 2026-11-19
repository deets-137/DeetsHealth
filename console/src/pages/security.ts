// Security (CONSOLE.md §3): one panel with its own nav (subnav.ts), like Analytics. It sits at
// the nav's foot, above Audit Logs. Auth is who may sign in, as a table that can be edited
// (auth.ts; the owner, 2026-11-14). Roles is the role templates as a draft table (roles.ts).
import { authView } from "./auth";
import type { Page } from "./page";
import { rolesView } from "./roles";
import { subnavPage } from "./subnav";

export function securityPage(): Page {
  return {
    ...subnavPage({
      id: "security",
      title: "Security",
      lastViewKey: "deets.console.security",
      // The order is the owner's (2026-11-13): Auth, then Roles.
      // Auth lists and edits the People list, so it needs People; Roles needs Roles.
      views: [
        { ...authView(), lever: "people" },
        { ...rolesView(), lever: "roles" },
      ],
    }),
    bottom: true,
  };
}
