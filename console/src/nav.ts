// The left nav and the page beside it (DeetsMotion's nav.ts, 2026-10-29, less its status
// lines). The pages with `bottom` sit at the nav's foot.
import { el, type Page } from "./pages/page";

const LAST_PAGE = "deets.console.page";

/** A duration token from skin.css, in ms ("120ms" or "0.12s"). */
function tokenMs(name: string): number {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  const n = parseFloat(v);
  if (!Number.isFinite(n)) return 0;
  return v.endsWith("ms") ? n : n * 1000;
}

export function initNav(pages: Page[]) {
  const nav = document.querySelector(".nav") as HTMLElement;
  const top = document.getElementById("nav-list") as HTMLElement;
  const bottom = document.getElementById("nav-list-bottom") as HTMLElement;
  const highlight = document.getElementById("nav-highlight") as HTMLElement;
  const host = document.getElementById("page-host") as HTMLElement;
  const built = new Map<string, HTMLElement>();
  let current: Page | null = null;
  let swap = 0; // the latest click wins when clicks come quickly

  const item = (p: Page) => {
    const li = el(`<li><button class="nav-item" type="button" data-page=""><span></span></button></li>`);
    const b = li.querySelector("button")!;
    b.dataset.page = p.id;
    b.querySelector("span")!.textContent = p.title;
    return li;
  };
  top.replaceChildren(...pages.filter((p) => !p.bottom).map(item));
  bottom.replaceChildren(...pages.filter((p) => p.bottom).map(item));
  const buttons = [...nav.querySelectorAll<HTMLButtonElement>(".nav-item")];

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
    page.shown?.();
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
    try {
      localStorage.setItem(LAST_PAGE, page.id);
    } catch {
      /* storage blocked: the console opens on the first page next time */
    }

    // The old page fades out, then the new one fades in (its own .page animation).
    const mine = ++swap;
    if (first || matchMedia("(prefers-reduced-motion: reduce)").matches) return show(page);
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

  // A hidden tab pauses the visible page's work.
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) current?.hidden?.();
    else current?.shown?.();
  });

  let saved: string | null = null;
  try {
    saved = localStorage.getItem(LAST_PAGE);
  } catch {
    /* ignore */
  }
  go(saved ?? pages[0].id);
}
