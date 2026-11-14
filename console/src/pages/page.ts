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
  /** The page's own nav as a dropdown, for the compact bar's right end (picker.ts). Read
   * after build(). A page with no own nav leaves it out. */
  picker?(): HTMLElement;
}

export function el(html: string): HTMLElement {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content.firstElementChild as HTMLElement;
}
