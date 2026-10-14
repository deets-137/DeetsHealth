// App errors → toasts (docs/architecture/ERRORS.md). The Rust side sends one `app-error`
// event for every failure, with a fixed code (src-tauri/src/errors.rs); this module turns
// each code into a toast.
//
// **The PHI wall (SECURITY.md §4.3).** The event carries a code, a core error kind, the app's
// own random site key and a count: never a value from the record, a name or a site. The text
// below is ours and fixed; the only thing put into it is `count`, a number.
//
// The wording is a DRAFT for the owner's review (ERRORS.md §3).

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast, type ToastKind } from "./toast";
import * as diag from "./diag";

interface AppError {
  code: string;
  kind?: string | null;
  site?: string | null;
  count?: number | null;
}

interface Copy {
  kind: ToastKind;
  text: (count: number) => string;
}

const c = (kind: ToastKind, text: string): Copy => ({ kind, text: () => text });

/** Every code the Rust side sends. A code missing here still toasts (`fallback`). */
export const COPY: Record<string, Copy> = {
  // Sign-in
  "sign-in-unavailable": c("error", "Sign-in is not available. Restart DeetsHealth."),
  "sign-in-no-site": c("error", "This site is not in the list. Pick the site again."),
  "sign-in-site-unreachable": c("error", "Could not reach the site to start sign-in. Check your connection and try again."),
  "sign-in-site-unreadable": c("error", "The site's sign-in settings cannot be used. Try again later."),
  "sign-in-failed": c("error", "Could not start sign-in. Try again."),
  "sign-in-browser": c("error", "Could not open your browser for sign-in."),
  "sign-in-stray-link": c("warn", "A sign-in link arrived, but no sign-in was waiting. Connect again."),
  "sign-in-wrong-link": c("warn", "An old sign-in link arrived. Finish the newest sign-in in your browser."),
  "sign-in-expired": c("warn", "The sign-in took more than 10 minutes. Connect again."),
  "sign-in-refused": c("warn", "The site did not give access. Connect again to try again."),
  "sign-in-trade-failed": c("error", "The site did not finish the sign-in. Connect again."),
  "sign-in-incomplete": c("error", "The site's sign-in did not give access to a record. Connect again."),
  "site-seal-failed": c("error", "Could not save the new site securely. Connect again."),
  "sign-in-port": c("error", "Could not start sign-in on this PC. Try again."),
  // A site's saved data
  "site-unknown": c("error", "This site is not connected."),
  "site-key-missing": c("error", "The key for this site is gone, so its data cannot be opened. Sign out and connect again."),
  "site-unreadable": c("error", "This site's saved data cannot be opened. Sign out and connect again."),
  "site-newer": c("error", "This site's data was saved by a newer version of DeetsHealth. Update the app."),
  "reindex-failed": c("warn", "Could not update the record index. Your data is safe. The app tries again at the next start."),
  // Settings › Sites
  "sites-list-failed": c("error", "Could not read your saved sites. Restart DeetsHealth."),
  "sign-out-busy": c("info", "This site is updating. Sign out when the update ends."),
  "sign-out-failed": c("error", "Could not erase all of this site's data. Sign out again."),
  // The fetch
  "fetch-unavailable": c("error", "Updating is not available. Restart DeetsHealth."),
  "fetch-busy": c("info", "This site is already updating."),
  "fetch-site-unreachable": c("error", "Could not reach the site. Refresh to try again."),
  "fetch-site-unreadable": c("error", "The site sent a reply that the app cannot read. Refresh to try again."),
  "fetch-store-failed": c("error", "Could not save the records. Refresh to try again."),
  "fetch-failed": c("error", "Could not update this site. Refresh to try again."),
  "fetch-sign-in-again": c("warn", "Sign in again to update this site."),
  "fetch-partial": {
    kind: "warn",
    text: (n) =>
      `Some records could not be updated (${n} ${n === 1 ? "type" : "types"}). The rest are up to date. Refresh to try again.`,
  },
};

const fallback: Copy = c("error", "Something went wrong. Try again.");

function show(e: AppError): void {
  // diag gets the code and the kind: both fixed words, never a value.
  diag.warn("app-error", { code: e.code, kind: e.kind ?? undefined });
  const copy = COPY[e.code] ?? fallback;
  toast({ kind: copy.kind, text: copy.text(e.count ?? 0) });
}

/** Listen for `app-error`, then take the errors that came before the page was ready. */
export async function initAppErrors(): Promise<void> {
  try {
    await listen<AppError>("app-error", (ev) => show(ev.payload));
    const waiting = await invoke<AppError[]>("app_errors_ready");
    waiting.forEach(show);
  } catch (err) {
    diag.error("app-errors: init", { kind: err instanceof Error ? err.name : typeof err });
  }
}
