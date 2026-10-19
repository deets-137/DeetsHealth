---
status: built
desk_test: open
sources: [core/src/auth.rs, core/build.rs, core/src/net.rs, archive/src-tauri/src/signin.rs, archive/src-tauri/src/loopback.rs, archive/src-tauri/src/browser.rs, archive/src-tauri/src/lib.rs, archive/src-tauri/src/sites.rs]
updated: 2026-10-13
---
# DeetsHealth — Sign-in

> How the app signs in to a site, step by step, with every value that can be changed and
> where. For a user question, a bug, or a change. The design history is CORE-CRATE.md §6 and
> §12 (stage 4); the rules are PATIENT-APP-SECURITY.md §4.4 and §8.5. Settings › Sites starts a
> sign-in (stage 7, CORE-CRATE.md §12).

## 1. Terms
- **Site** — one health system's FHIR server, with its own base URL.
- **SMART** — SMART App Launch, the OAuth 2.0 profile that FHIR sites use for sign-in.
- **Standalone patient launch** — the app starts the sign-in (not an EHR), and the user
  picks or is the patient.
- **Public client** — an app with no secret. It proves itself with PKCE.
- **PKCE** — a random *verifier* kept in memory, and its SHA-256 *challenge* sent first. The
  code is useless without the verifier.
- **`state`** — a random value sent with the request and checked on the returning link.
- **The link** — `http://127.0.0.1:<port>/callback?code=…&state=…`: the site sends the
  browser here after sign-in.
- **Return listener** — a tiny HTTP listener in the app, on 127.0.0.1 and a registered port,
  open only while one sign-in waits (`loopback.rs`; RFC 8252 §7.3). It replaced the
  `deetshealth://` scheme on 2026-10-13: Epic's sign-in page cannot hand off to a custom
  scheme in Edge (integrations/EPIC.md §3).
- **Waiting sign-in** — the verifier, `state` and site facts held in memory between the
  browser opening and the link returning. One at a time.

## 2. What the user sees
1. Settings › **Sites** › **Connect a site** › **Choose**, then a pick from the menu. Or,
   on a row that says "Sign in again", its **Sign in** half (§3, step 1b).
2. The default browser opens at the site's sign-in page. For the SMART sandbox: a patient
   picker (synthetic patients), then sometimes a practice login with no real password.
3. The browser tab shows a short page from the app: "You're signed in. DeetsHealth has your
   sign-in. You can close this tab." No dialog, nothing to allow.
4. DeetsHealth comes to the front; the Sites row goes from "Finish signing in in your
   browser" to the counts as the fetch runs (stage 5).

## 3. The flow in the code
| Step | Where | What happens |
|---|---|---|
| 1 | `signin::sign_in_start(site)` (command) | Picks `auth::sandbox_sites()[site]`. |
| 1b | `signin::site_sign_in(site)` (command; `site` is the app's site key) | Opens the saved site, finds its menu entry by base URL (`auth::entry_for`), and remembers the site in the waiting sign-in. At the return (`signin::save`), `auth::renew_site` puts the new tokens into the saved site if the same patient came back; another patient becomes a new site. |
| 2 | `auth::begin` | GET `<base>/.well-known/smart-configuration`. Needs `authorization_endpoint` and `token_endpoint`, both `https://`. Makes the verifier (32 random bytes → 43 base64url characters), the challenge (base64url of SHA-256, both from CNG) and `state` (16 random bytes). Builds the authorize URL (§4). |
| 1c | `loopback::Bound::open` (before step 2) | Binds 127.0.0.1 on the first free port of `PORTS` (80, 42813, 46219). The redirect URI is `http://127.0.0.1/callback` on 80, else with the port. None free → `sign-in-port`. |
| 3 | `signin.rs` | Stores the waiting sign-in (replacing any older one, which closes its listener) and starts this one's listener thread. |
| 4 | `net::open_in_browser` → `browser::SystemBrowser` | `ShellExecuteW("open", url)`. `https://` only, checked twice. |
| 5 | the browser | The site sends the browser to the link. |
| 6 | `loopback.rs` | Reads one request head (8 KB at most, 5 s). The `Host` must be `127.0.0.1:<port>`, or `421`; only `GET`. `/callback?…` → shielded at once → `signin::on_link`. Our link answers `303 /done` (the code leaves the address bar), then `/done` shows the page for the outcome and the listener closes. A wrong `state` gets a `400` page, and the listener keeps waiting. |
| 7 | `signin::on_link` | No waiting sign-in → ignored. Older than 10 min → dropped. Then `auth::check_link`. Returns the outcome the page shows. |
| 8 | `auth::check_link` | `Ours(code)`: our `state` and a `code`. `Refused`: our `state`, but `error=` or no code. `NotOurs`: any other `state`, or not a URL — the waiting sign-in stays. |
| 9 | `auth::exchange` | POST the token endpoint: `grant_type=authorization_code`, `code`, `redirect_uri`, `client_id`, `code_verifier`. Needs `token_type` Bearer, an `access_token` and a `patient`. |
| 10 | `signin::save` → `sites::SiteFiles::create` + `SiteStore::put_site` | (A **Sign in** of a saved site with the same patient updates that site instead: step 1b.) A new site key (random name), a new DPAPI-wrapped data key, a new SQLite file; the site row sealed: name, base URL, client id, `Patient` id, SMART config, scopes, tokens, expiry. |
| 11 | events | `sign-in` = `connected`; `sites-changed`. The fetch starts (stage 5). |

**The authorize URL** (step 2): `response_type=code`, `client_id`, `redirect_uri`,
`scope`, `state`, `aud` (= the base URL), `code_challenge`, `code_challenge_method=S256`.

**Refresh** (`auth::refresh`, used by the fetch): when `auth::needs_refresh` says the token
runs out within 2 minutes, POST `grant_type=refresh_token`, `refresh_token`, `client_id` to
the token endpoint read from the sealed SMART config. `Renewed`: new access token, expiry,
and refresh token or scope when the site sends new ones. `SignInAgain`: no refresh token,
or the site answers 400 / 401. Any other failure is an error: try again later.

## 4. What to change, and where
| To change | Where | Notes |
|---|---|---|
| Scopes | `SiteEntry.scopes`: `auth::SCOPES` (SMART v1, `patient/*.read`) or `auth::SCOPES_V2` (`patient/*.rs`) | SMART uses v1. Epic uses the version its registration names (`scope_version` in `epic-client.json`). A site may grant less; the granted scope is sealed in `site.scopes`. Epic grants what the registration lists, whatever this string asks. |
| How long a sign-in waits | `auth::WAIT_LIMIT_S` | 10 minutes. |
| How early to refresh | `auth::REFRESH_MARGIN_S` | 2 minutes. |
| Expiry when a site sends none | `auth::read_tokens` | 1 hour (the SMART default). |
| Client id | `SiteEntry.client_id` in `auth::sandbox_sites` | `deetshealth` for the SMART sandbox (it accepts any). Epic's comes from `epic-client.json` at the repo root (git-ignored; template `epic-client.example.json`): `core/build.rs` reads it at build time into `DEETSHEALTH_EPIC_CLIENT_ID` (integrations/EPIC.md §2). A change to the file rebuilds the core. |
| Sites in the Connect menu | `auth::sandbox_sites` | A `SiteEntry` per site: name, base URL, client id, scopes. SMART is index 0. **Epic sandbox** is index 1, only in a build that found `epic-client.json` with a `nonprod_client_id` (`auth::EPIC_SANDBOX`). Real sites come from the directory (integrations/ENDPOINT-DIRECTORY.md). |
| The SMART sandbox's setup | `auth::SMART_LAUNCH` | §5. |
| Redirect URI | `loopback::PORTS`, `Bound::redirect_uri` | The first free port of 80, 42813, 46219: `http://127.0.0.1/callback` for 80, else `http://127.0.0.1:<port>/callback`. Epic matches the port exactly, so every listed port must be registered (PATIENT-APP-EPIC.md §3); a new port means a new registered URI. Debug and release are the same. |
| The return pages | `loopback::Page::words` | Four fixed pages: signed in, not shared, ended, not current. A draft for the owner's words. |
| HTTP rules | `net::Net::new` | HTTPS only, TLS 1.2+, no redirects, gzip, 30 s timeout, `User-Agent: DeetsHealth/<version>`. |
| What the Sites row shows | the `sign-in` event words | `waiting`, `connected`, `refused`, `expired`, `failed`, `cancelled`. Fixed words only: never a value. |

## 5. The SMART sandbox launcher
SMART Health IT's launcher (`launch.smarthealthit.org`) simulates an EHR. **A plain base URL
(`…/v/r4/fhir`) refuses a standalone launch** ("Invalid launch options"). It reads its setup
from a base64url JSON **array** in the URL: `https://launch.smarthealthit.org/v/r4/sim/<b64>/fhir`.

| Index | Field | Ours (`SMART_LAUNCH`) | Values |
|---|---|---|---|
| 0 | launch type | `3` | 0 provider-ehr · 1 patient-portal · 2 provider-standalone · **3 patient-standalone** · 4 backend-service |
| 1 | patient | `""` | a `Patient` id; empty = the picker shows all |
| 2 | provider | `""` | |
| 3 | encounter | `""` | |
| 4 | skip login | `0` | 1 = no login page (needs one patient) |
| 5 | skip auth | `0` | 1 = no approve page |
| 6 | simulated EHR | `0` | |
| 7 | scope | `""` | empty = what the app asks |
| 8 | redirect URIs | `""` | empty = any is accepted |
| 9 | client id | `""` | empty = any is accepted |
| 10 | client secret | `""` | |
| 11 | simulated error | `""` | e.g. `auth_invalid_scope`, `token_expired_refresh_token`: to test failure paths |
| 12 | JWKS URL | `""` | for `private_key_jwt` later |
| 13 | JWKS | `""` | |
| 14 | client type | `0` | **0 public** · 1 confidential-symmetric · 2 confidential-asymmetric · 3 backend-service |
| 15 | PKCE | `2` | 0 none · 1 auto · **2 always** (the launcher rejects a sign-in without a correct PKCE) |

The source of the format: `smart-launcher-v2`, `src/isomorphic/codec.ts`.

**Headless use** (the live test): patient set, skip login `1`, skip auth `1`. The authorize
step then answers `302` with the code in `Location` at once. **To simulate a failure** for a
desk test, put a code from index 11 into a copy of the entry.

## 6. The return listener
- **Why not a custom scheme** (2026-10-13): after the patient approves, Epic's page POSTs a
  form, and the reply redirects to the redirect URI. Epic's page sets `form-action *`, and
  Chromium checks `form-action` along the whole redirect chain; `*` matches only network
  schemes. So Edge blocked `deetshealth-dev://` with no prompt, and the page stalled.
  `http://127.0.0.1` is a network scheme, so it passes.
- **No registry, no installer step.** Nothing is registered with Windows, so the MSIX
  private-hive trap of the old scheme (CLAUDE.md › How to verify) no longer touches sign-in.
  A dev app started from a Claude session can sign in.
- **Safety:** 127.0.0.1 only (never `0.0.0.0`); open only while a sign-in waits; the `Host`
  check stops a web page that reaches the port through a DNS name; the code is useless without
  the PKCE verifier, which never leaves the app; `state` must match. The pages send
  `Content-Security-Policy: default-src 'none'`, `no-store` and `no-referrer`, and carry
  fixed words only.
- **Firewall:** a listener on 127.0.0.1 gets no Windows Firewall prompt (loopback traffic is
  not filtered).
- **Test the listener by hand:** while a sign-in waits, open
  `http://127.0.0.1:<port>/callback?state=x&code=y` (the port is in the authorize URL's
  `redirect_uri`). It shows "This link is not for the current sign-in", and the sign-in goes on.
- `tauri-plugin-single-instance` is still the first plugin, so a second launch only brings the
  running window forward.

## 7. Log lines
The log is `%APPDATA%\com.deetshealth.dev\deetshealth.log` (debug) or
`%APPDATA%\com.deetshealth.app\deetshealth.log` (installed). Every sign-in line starts
`sign-in:` and carries no value: never the link, the code, a token, the site or the patient.

| Line | Meaning | Next step |
|---|---|---|
| `started; waiting for the browser` | Steps 1–4 worked. | Finish in the browser. |
| `connected; the site is sealed` | Steps 9–10 worked. | — |
| `cancelled` | The user pressed Cancel. | — |
| `a link arrived, but no sign-in was waiting; ignored` | A stale link, or the app restarted mid sign-in (the waiting sign-in is memory only). | Connect again. |
| `a link arrived after the wait ran out; ignored` | More than 10 minutes passed. | Connect again. |
| `a link with the wrong state arrived; ignored` | An old tab, a second attempt, or a link from elsewhere. The waiting sign-in goes on. | Finish the newest attempt. |
| `the site did not grant access` | The user declined, or the site sent `error=`. | Connect again; check the site's scopes. |
| `the token trade failed: <kind>` | `network error` (no reply, or not 2xx: wrong client id, wrong redirect URI, bad PKCE, used code) · `sign-in error` (not Bearer, no token, no `patient`) · `unexpected reply` (not JSON). | See §8. |
| `could not seal the new site` | The key or the file could not be made. | Check the data folder is writable; see `keystore.rs`. |
| `unavailable: <kind>` | The HTTP client could not start. Sign-in commands answer "sign-in is not available". | A TLS or system problem. |
| `could not open a local port for the return: <kind>` | No port on 127.0.0.1 (very rare: a security tool, or every port taken). | Try again; check local security software. |
| `the return listener closed` | The sign-in ended (taken, cancelled, replaced, or 10 minutes). | — |
| `the return listener stopped: <kind>` | The listener failed while it waited. | Connect again. |

## 8. When it goes wrong
| Symptom | Likely cause | Fix |
|---|---|---|
| The site's page stalls after you approve, and the console says a form was blocked by `form-action` | The redirect URI is not `http` or `https` (§6). | Check the redirect URI the build sends. |
| The site shows an error before its sign-in page | The site does not know this redirect URI, or not with this port (PATIENT-APP-EPIC.md §3). | Check the site's registration. |
| The tab says "This site can't be reached" at `127.0.0.1` | The listener had closed: the sign-in ended (Cancel, a newer sign-in, 10 minutes). | Connect again. |
| The browser page says "Invalid launch options" | A plain launcher base URL, or a broken `SMART_LAUNCH`. | §5. |
| `token trade failed: network error` right after the link | The code was already used or is too old, the redirect URI differs from the one in the authorize URL, or PKCE did not match. | Connect again. If it repeats, compare the client id and redirect URI with the site's registration. |
| `token trade failed: sign-in error` | The site gave no `patient`: `launch/patient` was not granted. | Check the scopes the site allows. |
| The Sites row says "Sign in again" | Refresh refused (400 / 401) or no refresh token (`offline_access` not granted). | Sign in again; for Epic, see §9. |

## 9. Changing it later
- **Epic:** registered 2026-10-13 (integrations/EPIC.md). Next after the sign-in works: dynamic
  client registration with the JWT bearer grant (PATIENT-APP-EPIC.md §6, PATIENT-APP-SECURITY.md §5). The SMART
  launcher can test the key half first (index 12–14 and client type 2).
- **A real site list:** `sandbox_sites` gives way to the endpoint directory. Each site's
  `smart-configuration` is read at sign-in, so no endpoint is hard-coded.
- **SMART v2 scopes** (`patient/*.rs`): change `SCOPES` per site, when a site asks for them.
- **macOS:** the same return listener works as is. **iPhone:** `ASWebAuthenticationSession`
  instead of `ShellExecute` (PATIENT-APP-DESIGN.md §6.3); it catches the return itself, and whether Epic's
  `form-action` affects it is a check for that time. **Web:** the site redirects back to the
  web app's own `https://…/callback`, which `form-action *` allows.

## 10. Security, in one place
- PKCE S256 and `state` on every sign-in; one waiting sign-in; 10-minute limit (PATIENT-APP-SECURITY.md §4.4).
- The link is shielded on arrival and never logged. A wrong-`state` link cannot cancel the
  user's sign-in.
- The verifier, the code, the tokens, the `Patient` id and the site's URL go only to that
  site, through the network module's pass (`net.rs`, PATIENT-APP-SECURITY.md §4.2).
- No redirects are followed, so a token never reaches another host. HTTPS and TLS 1.2+ only.
- The browser opens `https://` only; Rust opens it, and no opener is exposed to the page.
- The tokens are sealed with the site's own key the moment the site row is written.
- The return listener: 127.0.0.1 only, one per sign-in, closed when the wait ends; the `Host`
  check; `303` so the code leaves the address bar; fixed pages with a `default-src 'none'`
  policy (§6).

## 11. Tests
- `cargo test -p deetshealth-core --features sqlite`: base64url (RFC 4648), the sandbox
  entry, link matching (ours, refused, not ours, not a URL), the wait limit, token replies
  (Bearer, no token, not JSON, no expiry), refresh timing.
- `cargo test -p deetshealth --lib -- --ignored`: **the live sign-in** against the SMART
  sandbox, about 5 calls: `begin`, the authorize redirect read without a browser, a
  wrong-`state` link refused, the token trade (PKCE checked by the launcher), a refresh,
  and a bad refresh token → `SignInAgain`.
- `cargo test -p deetshealth --lib loopback`: the listener binds 127.0.0.1 on a listed port, skips a taken one, and writes port 80 with no port;
  the pages carry fixed words and no script.

## 12. Desk test
> Open. Built 2026-09-24; the whole Sites test is CORE-CRATE.md §12, "Stage 7 desk test".
1. Start `npm run dev:app` (any terminal since 2026-10-13: no scheme to register).
2. Settings › Sites › Connect a site › SMART Health IT sandbox. Expect the browser at the
   patient picker.
3. Pick a patient. Expect the tab to say "You're signed in", the app in front, and the row
   counting up. The tab's address bar shows `127.0.0.1:<port>/done`, with no code.
4. Connect again and close the browser tab. After 10 minutes, a late link does nothing.
5. Read the log: only the lines of §7, no value in any.
