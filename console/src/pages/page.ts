// A page in the nav (DeetsMotion's page.ts). Each page is built once, on its first visit,
// and kept, so what it holds survives a switch away and back.
import type { Lever } from "../agent/api";

export interface Page {
  id: string;
  title: string;
  /** In the nav while you hold any of these levers at View (access.ts; ACCESS-ROLES.md §6).
   * None: always (Settings). */
  levers?: Lever[];
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

// Trusted Types (CONSOLE.md §5.1): the page's policy refuses a plain string at an HTML sink, and
// `deets-html` is the one policy that may make HTML. Every string it takes is fixed app markup
// (a template, an icon), never a value from a call or a record: those go in through
// textContent. `npm run check` fails on an HTML sink outside this file
// (scripts/html-sinks.mjs). A browser without Trusted Types takes the string as it is.
interface HtmlPolicy {
  createHTML(markup: string): unknown;
}
const trustedTypes = (globalThis as {
  trustedTypes?: { createPolicy(name: string, rules: { createHTML(markup: string): string }): HtmlPolicy };
}).trustedTypes;
const policy: HtmlPolicy = trustedTypes?.createPolicy("deets-html", { createHTML: (markup) => markup }) ?? {
  createHTML: (markup) => markup,
};

/** Sets an element's content from fixed app markup, through the one policy. */
export function setHtml(node: Element, markup: string): void {
  node.innerHTML = policy.createHTML(markup) as string;
}

export function el(html: string): HTMLElement {
  const t = document.createElement("template");
  setHtml(t, html.trim());
  return t.content.firstElementChild as HTMLElement;
}
