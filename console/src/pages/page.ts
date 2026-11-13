// A page in the nav (DeetsMotion's page.ts). Each page is built once, on its first visit,
// and kept, so what it holds survives a switch away and back.

export interface Page {
  id: string;
  title: string;
  /** Sits at the foot of the nav (Security, Audit Logs, Settings). */
  bottom?: boolean;
  /** Builds the page's element. Called once. */
  build(): HTMLElement;
  /** The page became visible, or stopped being visible: start or pause its work here. */
  shown?(): void;
  hidden?(): void;
}

export function el(html: string): HTMLElement {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content.firstElementChild as HTMLElement;
}
