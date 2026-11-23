// The left nav and the page beside it (DeetsMotion's nav.ts, 2026-10-29, less its status
// lines). The pages with `bottom` sit at the nav's foot.
import { el, type Page } from "./pages/page";
import { reducedMotion, tokenMs } from "./tokens";
import { shown } from "./agent/store";
import { picker, type Picker } from "./picker";
import { anyAccess, known, may, onAccess, startAccess } from "./access";
import { noRolePage } from "./pages/no-role";
import { logLook } from "./agent/api";
import { onOpenCall } from "./open-call";

const LAST_PAGE = "deets.console.page";

export function initNav(allPages: Page[]) {
  const nav = document.querySelector(".nav") as HTMLElement;
  const top = document.getElementById("nav-list") as HTMLElement;
  const bottom = document.getElementById("nav-list-bottom") as HTMLElement;
  const highlight = document.getElementById("nav-highlight") as HTMLElement;
  const host = document.getElementById("page-host") as HTMLElement;
  const views = document.getElementById("compact-views") as HTMLElement;
  const built = new Map<string, HTMLElement>();
  let current: Page | null = null;
  let swap = 0; // the latest click wins when clicks come quickly
  // Shown when you hold no role at all (deny by default, ACCESS-ROLES.md §5), with Settings.
  const noRole = noRolePage();
  let pages: Page[] = [];
  let buttons: HTMLButtonElement[] = [];
  let pagePick: Picker | null = null;

  const item = (p: Page) => {
    const li = el(`<li><button class="nav-item" type="button" data-page=""><span></span></button></li>`);
    const b = li.querySelector("button")!;
    b.dataset.page = p.id;
    b.querySelector("span")!.textContent = p.title;
    return li;
  };

  /** The pages you may see (access.ts): each with any of its levers at View; with no role at
   * all, "No role yet" and the pages that need none. Drawn again when your access changes. */
  const render = () => {
    const allowed = allPages.filter((p) => !p.levers || p.levers.some((l) => may(l, "view")));
    pages = anyAccess() ? allowed : [noRole, ...allPages.filter((p) => !p.levers)];
    top.replaceChildren(...pages.filter((p) => !p.bottom).map(item));
    bottom.replaceChildren(...pages.filter((p) => p.bottom).map(item));
    buttons = [...nav.querySelectorAll<HTMLButtonElement>(".nav-item")];
    // The same list as a dropdown, for a narrow screen (picker.ts). The foot's pages come after
    // a divider, as they sit apart in the nav. `go` is defined below; a pick runs only after it is.
    const ordered = [...pages.filter((p) => !p.bottom), ...pages.filter((p) => p.bottom)];
    pagePick = picker(
      "Pages",
      ordered.map((p, i) => ({ id: p.id, title: p.title, apart: p.bottom && !ordered[i - 1]?.bottom })),
      "left",
      (id) => go(id),
    );
    document.getElementById("compact-pages")!.replaceChildren(pagePick.el);
    if (current && !pages.includes(current)) {
      // The page shown is no longer yours: the first page left.
      go(pages[0].id);
    } else if (current) {
      buttons.find((b) => b.dataset.page === current!.id)?.setAttribute("aria-current", "page");
      pagePick.set(current.id);
      place(false);
    }
  };

  /** Moves the pill onto the current item, in whole pixels. */
  const place = (animate: boolean) => {
    const b = buttons.find((x) => x.dataset.page === current?.id);
    if (!b) return;
    highlight.classList.toggle("no-anim", !animate);
    highlight.style.width = `${b.offsetWidth}px`;
    highlight.style.height = `${b.offsetHeight}px`;
    highlight.style.transform = `translate(${b.offsetLeft}px, ${b.offsetTop}px)`;
  };
  // The bottom list moves with the window's height; the pill follows it without sliding.
  new ResizeObserver(() => place(false)).observe(nav);

  const show = (page: Page) => {
    if (!built.has(page.id)) built.set(page.id, page.build());
    host.replaceChildren(built.get(page.id)!);
    // The page's own dropdown at the compact bar's right end; none for a page with no own nav.
    const own = page.picker?.();
    if (own) views.replaceChildren(own);
    else views.replaceChildren();
    page.shown?.();
    shown(); // a store paused while its page was off screen reads now (store.ts)
  };

  const go = (id: string) => {
    const page = pages.find((p) => p.id === id) ?? pages[0];
    if (page === current) return;
    const first = current === null;
    current?.hidden?.();
    current = page;
    buttons.forEach((b) => {
      if (b.dataset.page === page.id) b.setAttribute("aria-current", "page");
      else b.removeAttribute("aria-current");
    });
    place(!first);
    pagePick?.set(page.id);
    // Opening a page is a look that changes nothing, kept in the audit trail (ACCESS-ROLES.md
    // §5.1, point 4): the page's id, the first lever you hold it by.
    if (known()) {
      const lever = page.levers?.find((l) => may(l, "view"));
      if (lever) void logLook(lever, "view", "page.open", "page", page.id);
    }
    try {
      localStorage.setItem(LAST_PAGE, page.id);
    } catch {
      /* storage blocked: the console opens on the first page next time */
    }

    // The old page fades out, then the new one fades in (its own .page animation).
    const mine = ++swap;
    if (first || reducedMotion()) return show(page);
    host.classList.add("leaving");
    window.setTimeout(() => {
      if (mine !== swap) return;
      host.classList.remove("leaving");
      show(page);
    }, tokenMs("--dur-leave"));
  };

  nav.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>(".nav-item");
    if (b) go(b.dataset.page!);
  });

  // Up and down move between pages, as in a list.
  nav.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    if (!(e.target as HTMLElement).closest(".nav-item")) return;
    e.preventDefault();
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = buttons[(at + (e.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length];
    next.focus();
    go(next.dataset.page!);
  });

  // Open in Chat (open-call.ts): the Chat page takes the call once it shows.
  onOpenCall(() => go("chat"));

  // A hidden tab pauses the visible page's work.
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) current?.hidden?.();
    else current?.shown?.();
  });

  render();
  onAccess(render);
  startAccess(nav);
  let saved: string | null = null;
  try {
    saved = localStorage.getItem(LAST_PAGE);
  } catch {
    /* ignore */
  }
  go(saved ?? pages[0].id);
}
