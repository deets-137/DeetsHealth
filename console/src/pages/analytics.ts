// Analytics (ANALYTICS.md): one panel with its own nav stacked inside it, at the left. The
// nav works like the big one (nav.ts): a pill slides to the chosen item, Up and Down move
// through it, and the console remembers the last choice. Each choice fills the view beside
// the nav with its own panels. Every view is empty until the owner designs it.
import { el, type Page } from "./page";
import { reducedMotion, tokenMs } from "../tokens";

const LAST_VIEW = "deets.console.analytics";

/** One choice in the Analytics nav, and what it shows. */
interface View {
  id: string;
  title: string;
  /** Builds the view's element. Called once, on its first visit; then kept. */
  build(): HTMLElement;
}

/** An empty view: its name and "Empty for now", till it has a design. */
const blank = (id: string, title: string): View => ({
  id,
  title,
  build: () => el(`<div class="analytics-blank"><p class="empty">Empty for now</p></div>`),
});

// The order is the owner's (2026-11-12): Home first, Flags last.
const VIEWS: View[] = [
  blank("home", "Home"),
  blank("outcome", "Outcome"),
  blank("cost", "Cost"),
  blank("logs", "Logs"),
  blank("speed", "Speed"),
  blank("flags", "Flags"),
];

export function analyticsPage(): Page {
  return {
    id: "analytics",
    title: "Analytics",
    build() {
      const page = el(`
        <section class="page">
          <div class="analytics glass">
            <header class="stage-head"><h1>Analytics</h1><p>Nothing here yet.</p></header>
            <div class="analytics-body">
              <nav class="analytics-nav" aria-label="Analytics">
                <span class="nav-highlight" aria-hidden="true"></span>
                <ul class="nav-list"></ul>
              </nav>
              <div class="analytics-view"></div>
            </div>
          </div>
        </section>`);
      initViews(page);
      return page;
    },
  };
}

function initViews(page: HTMLElement) {
  const nav = page.querySelector(".analytics-nav") as HTMLElement;
  const list = nav.querySelector(".nav-list") as HTMLElement;
  const highlight = nav.querySelector(".nav-highlight") as HTMLElement;
  const host = page.querySelector(".analytics-view") as HTMLElement;
  const built = new Map<string, HTMLElement>();
  let current: View | null = null;
  let swap = 0; // the latest click wins when clicks come quickly

  list.replaceChildren(
    ...VIEWS.map((v) => {
      const li = el(`<li><button class="nav-item" type="button"><span></span></button></li>`);
      const b = li.querySelector("button")!;
      b.dataset.view = v.id;
      b.querySelector("span")!.textContent = v.title;
      return li;
    }),
  );
  const buttons = [...list.querySelectorAll<HTMLButtonElement>(".nav-item")];

  /** Moves the pill onto the current item, in whole pixels. */
  const place = (animate: boolean) => {
    const b = buttons.find((x) => x.dataset.view === current?.id);
    if (!b || !b.offsetWidth) return; // the page is not on screen yet
    highlight.classList.toggle("no-anim", !animate);
    highlight.style.width = `${b.offsetWidth}px`;
    highlight.style.height = `${b.offsetHeight}px`;
    highlight.style.transform = `translate(${b.offsetLeft}px, ${b.offsetTop}px)`;
  };
  // Also places the pill the first time the page is shown, when the nav first has a size.
  new ResizeObserver(() => place(false)).observe(nav);

  const show = (v: View) => {
    if (!built.has(v.id)) built.set(v.id, v.build());
    host.replaceChildren(built.get(v.id)!);
  };

  const go = (id: string) => {
    const v = VIEWS.find((x) => x.id === id) ?? VIEWS[0];
    if (v === current) return;
    const first = current === null;
    current = v;
    buttons.forEach((b) => {
      if (b.dataset.view === v.id) b.setAttribute("aria-current", "page");
      else b.removeAttribute("aria-current");
    });
    place(!first);
    try {
      localStorage.setItem(LAST_VIEW, v.id);
    } catch {
      /* storage blocked: Analytics opens on Home next time */
    }

    // The same swap as the big nav: the old view fades out, then the new one fades in.
    const mine = ++swap;
    if (first || reducedMotion()) return show(v);
    host.classList.add("leaving");
    window.setTimeout(() => {
      if (mine !== swap) return;
      host.classList.remove("leaving");
      show(v);
    }, tokenMs("--dur-leave"));
  };

  nav.addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>(".nav-item");
    if (b) go(b.dataset.view!);
  });

  // Up and down move between views, as in a list.
  nav.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    if (!(e.target as HTMLElement).closest(".nav-item")) return;
    e.preventDefault();
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = buttons[(at + (e.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length];
    next.focus();
    go(next.dataset.view!);
  });

  let saved: string | null = null;
  try {
    saved = localStorage.getItem(LAST_VIEW);
  } catch {
    /* ignore */
  }
  go(saved ?? VIEWS[0].id);
}
