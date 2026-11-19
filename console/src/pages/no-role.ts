// "No role yet" (docs/ideas/ACCESS-ROLES.md §5: deny by default). A person on the list who holds
// no role sees this page and Settings only, until someone with Roles Edit grants one on
// Security › Auth. The nav moves here, and away again, on its own (nav.ts, access.ts).
import { el, type Page } from "./page";

export function noRolePage(): Page {
  return {
    id: "no-role",
    title: "No role yet",
    build: () =>
      el(`
        <section class="page">
          <div class="subnav-page glass">
            <header class="stage-head"><h1>No role yet</h1></header>
            <div class="subnav-blank">
              <p class="empty">You are signed in, and you hold no role yet. Ask the person who invited you to give you one on Security › Auth. This page changes by itself once they do.</p>
            </div>
          </div>
        </section>`),
  };
}
