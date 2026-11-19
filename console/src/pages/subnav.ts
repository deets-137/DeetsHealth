// A page with its own nav (ANALYTICS.md §2; Analytics, Teams, Config, Security, Audit Logs and Settings use it): one panel with a
// nav stacked inside it, at the left. The nav works like the big one (nav.ts): a pill slides
// to the chosen item, Up and Down move through it, and the console remembers the last
// choice. Each choice fills the view beside the nav with its own panels.
import { el, type Page } from "./page";
import { reducedMotion, tokenMs } from "../tokens";
import { shown } from "../agent/store";
import { picker, type Picker } from "../picker";
import { may, onAccess } from "../access";
import type { Lever } from "../agent/api";

/** One choice in a page's own nav, and what it shows. */
export interface View {
  id: string;
  title: string;
  /** The lever this view needs at View (ACCESS-ROLES.md §6): hidden at None. None: always. */
  lever?: Lever;
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
  /** The lever of every view that names none of its own. */
  lever?: Lever;
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
  // Each view's lever: its own, or the page's.
  const views = spec.views.map((v) => (v.lever || !spec.lever ? v : { ...v, lever: spec.lever }));
  spec = { ...spec, views };
  // The page is in the nav while any of its views may be: none when a view needs no lever.
  const levers = views.every((v) => v.lever) ? [...new Set(views.map((v) => v.lever!))] : undefined;
  // The dropdown's holder: its content is drawn again when the views shown change.
  const pickHolder = el(`<div class="subnav-pick"></div>`);
  return {
    id: spec.id,
    title: spec.title,
    levers,
    picker: () => pickHolder,
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
      initViews(page, spec, pickHolder);
      return page;
    },
  };
}

/** Wires the page's own nav, and draws the same list as a dropdown for a narrow screen into
 * `pickHolder`. Only the views you may see are listed, again each time that changes. */
function initViews(page: HTMLElement, spec: SubnavPageSpec, pickHolder: HTMLElement): void {
  const nav = page.querySelector(".subnav") as HTMLElement;
  const list = nav.querySelector(".nav-list") as HTMLElement;
  const highlight = nav.querySelector(".nav-highlight") as HTMLElement;
  const host = page.querySelector(".subnav-view") as HTMLElement;
  const built = new Map<string, HTMLElement>();
  let current: View | null = null;
  let swap = 0; // the latest click wins when clicks come quickly
  let views: View[] = [];
  let buttons: HTMLButtonElement[] = [];
  let pick: Picker | null = null;

  /** Lists the views you may see; moves off a view you no longer may. */
  const render = () => {
    views = spec.views.filter((v) => !v.lever || may(v.lever, "view"));
    list.replaceChildren(
      ...views.map((v) => {
        const li = el(`<li><button class="nav-item" type="button"><span></span></button></li>`);
        const b = li.querySelector("button")!;
        b.dataset.view = v.id;
        b.querySelector("span")!.textContent = v.title;
        return li;
      }),
    );
    buttons = [...list.querySelectorAll<HTMLButtonElement>(".nav-item")];
    // `go` is defined below; a pick runs only after it is.
    pick = picker(spec.title, views, "right", (id) => go(id));
    pickHolder.replaceChildren(pick.el);
    if (current && !views.includes(current)) {
      current = null;
      if (views.length) go(views[0].id);
      else host.replaceChildren();
    } else if (current) {
      buttons.find((b) => b.dataset.view === current!.id)?.setAttribute("aria-current", "page");
      pick.set(current.id);
      place(false);
    }
  };

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
    if (!v || v === current) return;
    const first = current === null;
    current = v;
    buttons.forEach((b) => {
      if (b.dataset.view === v.id) b.setAttribute("aria-current", "page");
      else b.removeAttribute("aria-current");
    });
    place(!first);
    pick?.set(v.id);
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

  render();
  onAccess(render);
  let saved: string | null = null;
  try {
    saved = localStorage.getItem(spec.lastViewKey);
  } catch {
    /* ignore */
  }
  if (views.length) go(saved ?? views[0].id);
}
