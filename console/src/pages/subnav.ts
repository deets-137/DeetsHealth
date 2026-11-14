// A page with its own nav (ANALYTICS.md §2; Analytics, Teams, Config, Security, Audit Logs and Settings use it): one panel with a
// nav stacked inside it, at the left. The nav works like the big one (nav.ts): a pill slides
// to the chosen item, Up and Down move through it, and the console remembers the last
// choice. Each choice fills the view beside the nav with its own panels.
import { el, type Page } from "./page";
import { reducedMotion, tokenMs } from "../tokens";
import { shown } from "../agent/store";

/** One choice in a page's own nav, and what it shows. */
export interface View {
  id: string;
  title: string;
  /** Builds the view's element. Called once, on its first visit; then kept. */
  build(): HTMLElement;
}

/** An empty view: "Empty for now", till it has a design. */
export const blankView = (id: string, title: string): View => ({
  id,
  title,
  build: () => el(`<div class="subnav-blank"><p class="empty">Empty for now</p></div>`),
});

export interface SubnavPageSpec {
  id: string;
  title: string;
  /** The `localStorage` key that keeps the last view. */
  lastViewKey: string;
  /** In the owner's order; the first one opens when nothing is saved. */
  views: View[];
}

/** Moves a nav's pill onto its chosen item, in whole pixels. Also used by the Chat page's own
 * nav (chat.ts). */
export function placePill(highlight: HTMLElement, b: HTMLElement | undefined, animate: boolean): void {
  if (!b || !b.offsetWidth) return; // the page is not on screen yet
  highlight.classList.toggle("no-anim", !animate);
  highlight.style.width = `${b.offsetWidth}px`;
  highlight.style.height = `${b.offsetHeight}px`;
  highlight.style.transform = `translate(${b.offsetLeft}px, ${b.offsetTop}px)`;
}

export function subnavPage(spec: SubnavPageSpec): Page {
  return {
    id: spec.id,
    title: spec.title,
    build() {
      const page = el(`
        <section class="page">
          <div class="subnav-page glass">
            <header class="stage-head"><h1></h1></header>
            <div class="subnav-body">
              <nav class="subnav">
                <span class="nav-highlight" aria-hidden="true"></span>
                <ul class="nav-list"></ul>
              </nav>
              <div class="subnav-view"></div>
            </div>
          </div>
        </section>`);
      page.querySelector("h1")!.textContent = spec.title;
      page.querySelector(".subnav")!.setAttribute("aria-label", spec.title);
      initViews(page, spec);
      return page;
    },
  };
}

function initViews(page: HTMLElement, spec: SubnavPageSpec) {
  const views = spec.views;
  const nav = page.querySelector(".subnav") as HTMLElement;
  const list = nav.querySelector(".nav-list") as HTMLElement;
  const highlight = nav.querySelector(".nav-highlight") as HTMLElement;
  const host = page.querySelector(".subnav-view") as HTMLElement;
  const built = new Map<string, HTMLElement>();
  let current: View | null = null;
  let swap = 0; // the latest click wins when clicks come quickly

  list.replaceChildren(
    ...views.map((v) => {
      const li = el(`<li><button class="nav-item" type="button"><span></span></button></li>`);
      const b = li.querySelector("button")!;
      b.dataset.view = v.id;
      b.querySelector("span")!.textContent = v.title;
      return li;
    }),
  );
  const buttons = [...list.querySelectorAll<HTMLButtonElement>(".nav-item")];

  const place = (animate: boolean) =>
    placePill(highlight, buttons.find((x) => x.dataset.view === current?.id), animate);
  // Also places the pill the first time the page is shown, when the nav first has a size.
  new ResizeObserver(() => place(false)).observe(nav);

  const show = (v: View) => {
    if (!built.has(v.id)) built.set(v.id, v.build());
    host.replaceChildren(built.get(v.id)!);
    shown(); // a store paused while its view was off screen reads now (store.ts)
  };

  const go = (id: string) => {
    const v = views.find((x) => x.id === id) ?? views[0];
    if (v === current) return;
    const first = current === null;
    current = v;
    buttons.forEach((b) => {
      if (b.dataset.view === v.id) b.setAttribute("aria-current", "page");
      else b.removeAttribute("aria-current");
    });
    place(!first);
    try {
      localStorage.setItem(spec.lastViewKey, v.id);
    } catch {
      /* storage blocked: the page opens on its first view next time */
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
    saved = localStorage.getItem(spec.lastViewKey);
  } catch {
    /* ignore */
  }
  go(saved ?? views[0].id);
}
