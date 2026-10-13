---
status: built
desk_test: open
sources: [src-tauri/src/errors.rs, src/app-errors.ts, src-tauri/src/signin.rs, src-tauri/src/fetch.rs, src-tauri/src/sites.rs, src-tauri/src/lib.rs]
updated: 2026-09-24
---
# DeetsHealth — App errors and their toasts

> Every failure route in sign-in, the fetch and the store sends one event with a fixed code,
> and the page shows a toast for it (the owner, 2026-09-21: "all failures and error routes
> should have errors we can have toasts for"; one `app-error` event, fork chosen the same
> day). **The toast wording in §3 is a draft for the owner's review.**

## 1. Terms
- **Code** — a fixed word for what failed, e.g. `fetch-site-unreachable`. The page turns it
  into a toast.
- **Kind** — the core's error kind (`network`, `storage`, …). For diag and the log only;
  never shown.
- **Site key** — the app's own random name for a site (STORE.md §1). Not protected data.

## 2. How it works
```
a failure in Rust ──► errors::report(app, AppError::new(code).kind(e).site(key).count(n))
                         │ before the page listens: queued
                         ▼
                      app-error { code, kind?, site?, count? }
                         ▼
src/app-errors.ts ──► COPY[code] ──► toast({ kind, text })     diag: code + kind
```
- **One route.** Every failure calls `errors::report` (`src-tauri/src/errors.rs`). A command
  that fails also returns the same code as its error, but its caller does not toast it: the
  event already did.
- **The queue.** An error at setup comes before the page listens. It waits in `AppErrors`
  until the page calls `app_errors_ready`, which returns the queue and switches to live
  events (`initAppErrors`, called from `main.ts` after the layout).
- **The wall.** The event carries only fixed words, the random site key and a count. The
  toast text is ours; the only thing put into it is the count (SECURITY.md §4.3).
- **Toast tiers** (toast.ts): `error` is sticky; `warn` is timed; `info` shows only under
  the "all" tier.
- A code with no entry in `COPY` still toasts, with the fallback text.

## 3. Every code
Draft wording. "Where" is the route that sends it.

| Code | Toast | Where, and when |
|---|---|---|
| `sign-in-unavailable` | error: "Sign-in is not available. Restart DeetsHealth." | `lib.rs` setup: the HTTP client did not start. `sign_in_start` with no sign-in state. |
| `sign-in-no-site` | error: "This site is not in the list. Pick the site again." | `sign_in_start`: no such entry. |
| `sign-in-site-unreachable` | error: "Could not reach the site to start sign-in. Check your connection and try again." | `auth::begin`: `smart-configuration` did not answer 2xx. |
| `sign-in-site-unreadable` | error: "The site's sign-in settings cannot be used. Try again later." | `auth::begin`: the reply is not JSON, or its endpoints are not `https://`. |
| `sign-in-failed` | error: "Could not start sign-in. Try again." | `sign_in_start`: any other failure (random bytes, a lock). |
| `sign-in-browser` | error: "Could not open your browser for sign-in." | `open_in_browser` failed. |
| `sign-in-stray-link` | warn: "A sign-in link arrived, but no sign-in was waiting. Connect again." | `on_link`: no sign-in waiting (an old tab, or the app restarted mid sign-in). |
| `sign-in-wrong-link` | warn: "An old sign-in link arrived. Finish the newest sign-in in your browser." | `on_link`: the `state` is not ours. The waiting sign-in goes on. |
| `sign-in-expired` | warn: "The sign-in took more than 10 minutes. Connect again." | `on_link`: the wait ran out. |
| `sign-in-refused` | warn: "The site did not give access. Connect again to try again." | `on_link`: `error=` or no code. |
| `sign-in-trade-failed` | error: "The site did not finish the sign-in. Connect again." | `auth::exchange`: no 2xx, or not JSON. |
| `sign-in-incomplete` | error: "The site's sign-in did not give access to a record. Connect again." | `auth::exchange`: not Bearer, no token, or no `patient`. |
| `site-seal-failed` | error: "Could not save the new site securely. Connect again." | `on_link`: the key or the file could not be made. |
| `link-register-failed` | error: "Sign-in links cannot reach DeetsHealth. Reinstall the app." | `lib.rs` setup: the scheme write failed. |
| `site-unknown` | error: "This site is not connected." | `site_refresh`, `site_sign_in`, `site_sign_out`: the site key is not in the right form. |
| `site-key-missing` | error: "The key for this site is gone, so its data cannot be opened. Sign out and connect again." | Opening a site: no key file. |
| `site-unreadable` | error: "This site's saved data cannot be opened. Sign out and connect again." | Opening a site: DPAPI, the file, or the schema 2 migration failed. |
| `site-newer` | error: "This site's data was saved by a newer version of DeetsHealth. Update the app." | Opening a site: a newer schema (`CoreError::Version`). |
| `reindex-failed` | warn: "Could not update the record index. Your data is safe. The app tries again at the next start." | `reindex_if_stale` failed. The store still works; the old index stays. |
| `sites-list-failed` | error: "Could not read your saved sites. Restart DeetsHealth." | `sites_list`: the task stopped. (One site that will not open gets its own row instead.) |
| `sign-out-busy` | info: "This site is updating. Sign out when the update ends." | `site_sign_out`: the site is fetching; the fetch holds its file open. |
| `sign-out-failed` | error: "Could not erase all of this site's data. Sign out again." | `site_sign_out`: the key or a file could not be deleted. The key may be gone already, so the row then reads "cannot be opened"; Sign out again finishes the erase. |
| `fetch-unavailable` | error: "Updating is not available. Restart DeetsHealth." | A fetch or the start-up check with no sign-in state. |
| `fetch-busy` | info: "This site is already updating." | `fetch::start`: work on this site runs already. |
| `fetch-site-unreachable` | error: "Could not reach the site. Refresh to try again." | The whole fetch: `/metadata` or the token refresh did not answer. |
| `fetch-site-unreadable` | error: "The site sent a reply that the app cannot read. Refresh to try again." | The whole fetch: the capability or the patient could not be read. |
| `fetch-store-failed` | error: "Could not save the records. Refresh to try again." | The whole fetch: a seal, key or SQLite failure. |
| `fetch-failed` | error: "Could not update this site. Refresh to try again." | The whole fetch: any other kind. |
| `fetch-sign-in-again` | warn: "Sign in again to update this site." | The refresh token was refused. |
| `fetch-partial` | warn: "Some records could not be updated (N types). The rest are up to date. Refresh to try again." | The fetch ended, but N types had a job that stopped early (FETCH.md §6–§7). |

**Not an app error:** the user's own Cancel; a warning the site puts in a page (such as
`suppressed`: it is stored for a card to show, FETCH.md §7); a stray second launch with no
link.

## 4. Adding a route
1. Call `errors::report(app, AppError::new("<code>")…)` at the failure, next to its log line.
2. Add the code and its wording to `COPY` in `src/app-errors.ts` and to §3. The wording is
   the owner's call.
3. No value from the record, no name, no URL and no site name goes into the event or the
   text.

## 5. Desk test
> Open.
1. Start `npm run dev:app`. Expect no toast at start.
2. Settings › Sites › Connect a site › Choose › SMART Health IT sandbox, then close the
   browser tab. Wait 10 minutes, then open the old link. Expect "The sign-in took more than
   10 minutes. Connect again."
3. With the network off, press Refresh. Expect "Could not reach the site…".
4. Press Refresh, then press Sign out twice while the row counts up. Expect "This site is
   updating. Sign out when the update ends."
5. Read each toast in §3 against the wording you want, and mark the changes.
