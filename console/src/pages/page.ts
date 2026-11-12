// A page in the nav (DeetsMotion's page.ts). Each page is built once, on its first visit,
// and kept, so what it holds survives a switch away and back.

export interface Page {
  id: string;
  title: string;
  /** Sits at the foot of the nav (Settings). */
  bottom?: boolean;
  /** Builds the page's element. Called once. */
  build(): HTMLElement;
  /** The page became visible, or stopped being visible: start or pause its work here. */
  shown?(): void;
  hidden?(): void;
}

/** The shared page panel: the title and one line on what the page is for. It fills the page
 *  until the page has more to show. */
export function pageHead(title: string, lede: string): string {
  return `<header class="page-head glass"><h1>${title}</h1><p>${lede}</p></header>`;
}

export function el(html: string): HTMLElement {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content.firstElementChild as HTMLElement;
}
