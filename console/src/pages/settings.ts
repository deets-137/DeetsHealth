// Settings: blank until the console has its first setting (BUILD-ORDER.md §3.1).
import { el, pageHead, type Page } from "./page";

export function settingsPage(): Page {
  return {
    id: "settings",
    title: "Settings",
    bottom: true,
    build() {
      return el(`<section class="page">${pageHead("Settings", "No settings yet.")}</section>`);
    },
  };
}
