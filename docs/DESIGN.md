---
status: project
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — Design

## 1. Intent
Give a patient a better way to read and use their own health record than a portal page.
The record comes from their health system through the SMART on FHIR patient API that US
law requires (21st Century Cures Act). The app reads it; it does not change it.

Real records are messy: missing ranges, missing reactions, no dose text, one code system
spelled two ways. The owner has seen this data from the Epic side. **Working through that
mess well, and not hiding it, is what sets this app apart from other patient chart apps**
(the owner, 2026-09-21). A missing value is a designed state, never an error.

Priority (the owner, 2026-09-21): patient experience first, privacy and security second,
everything else after. See [SECURITY.md](architecture/SECURITY.md).

## 2. Who it is for
One patient, on their own PC, looking at their own record. Proxy access (a parent, a
caregiver) is a later idea, not the first build.

## 3. Same as DeetsMusic, and what differs
| Area | DeetsMusic | DeetsHealth |
|---|---|---|
| Shell | Tauri v2 + WebView2 | same |
| Front end | vanilla TS, Vite | same |
| Back end | Rust, normalizes Apple JSON | Rust, normalizes FHIR JSON |
| Tokens | palette → theme → skin | same |
| Surfaces | Mini, Midi, Max | **Midi and Max only** |
| Cards | collection-card engine, slots | same engine; health cards |
| Docs | front matter, HANDOFF/WORKLOG, area folders | same |
| Sign-in | MusicKit token via DeetsSupport | SMART on FHIR, PKCE, public client |
| Servers | one Apple API | one FHIR base URL per health system |
| Agent / MCP / SQL export | yes | **no, until the PHI wall is designed** |
| Telemetry, reports, diag | rich | PHI-free by rule (DATA-ARCHITECTURE.md §5) |
| Playback, queue, AirPlay, rooms | yes | not applicable |

## 4. First cards (ideas, not decided)
- **Labs** — results over time, with the reference range.
- **Medications** — active list, with the prescriber and dates.
- **Allergies** — substance, reaction, severity.
- **Timeline** — encounters in date order.

## 5. Non-goals
- No diagnosis or medical advice. The app shows what the record says.
- No writes back to the health system.
- No cloud copy of the record.

## 6. Web-ready (the owner, 2026-09-21)
The app is Windows-native for now. The owner expects most people would use a web version,
so every design assumes the app may become a web app later.
- A web version keeps the same promise: **no server holds a record.** The browser does the
  sign-in, the fetch and the sealed store; the host serves only static files.
- What differs per platform goes behind a small interface: the key store (DPAPI / WebCrypto
  non-extractable keys), the sign-in return (deep link / web redirect), the cache (SQLite /
  IndexedDB), opening a browser.
- Decided: the normalizer, the model and the sealing live in a Rust crate with no Tauri
  dependency, so it can also compile to WebAssembly.
- If a web version needs a server, the owner accepts a secure cloud provider (Azure, AWS or
  similar). A server that holds records changes SECURITY.md §4.1 and §6 (a provider
  agreement, breach duties), so it is a fork of its own at that time.
- A web version adds one risk the desktop app does not have: the code is served fresh on
  each visit, so a changed host could serve changed code. That is a later design question.

### 6.1 Every platform, from here on (the owner, 2026-09-21)
Windows is the only build today. A web version, a macOS app and an iPhone app may follow.
**Every fork from now on weighs every platform:** an option names what it costs, or rules
out, on each. Platform facts go behind the core's platform traits (CORE-CRATE.md §3); a
rule that holds on one platform only says so.

### 6.2 macOS (notes, 2026-09-21)
| Area | Windows today | macOS |
|---|---|---|
| Crypto (`Cipher`) | CNG, FIPS 140-validated | Apple's validated module (corecrypto). Its AES-GCM is in CryptoKit, Swift only: a small Swift bridge, or a validated library such as `aws-lc-rs` in FIPS mode |
| Keys (`KeyStore`) | DPAPI blob in a file | The Keychain. Stronger: the key never sits in our folder |
| TLS | SChannel | Apple's Security framework, through the same `native-tls` |
| Sign-in return | The link arrives as a launch argument (single-instance) | The link arrives as an "open URL" event from the deep-link plugin; `lib.rs` needs a second path |
| **Uninstall erase** | The uninstaller deletes data and keys (SECURITY.md §4.7) | **A Mac app has no uninstaller.** The Trash leaves the data folder and Keychain items. It needs an in-app "Delete all data", and the rule becomes: sign-out erases; uninstall cannot |
| Web view | WebView2 (Chromium) | WKWebView (Safari engine): small CSS and script differences; every skin, and Glass's blur cost, need a look |
| Window and keys | Our title bar; Ctrl shortcuts | Traffic-light buttons; Cmd shortcuts. Design forks |
| Windows-only code | the `windows` crate, WebView2 key blocking, `cng.rs`, `keystore.rs` | Needs `cfg(windows)` gates; today these are unconditional |
| Release | Signed installer, updater | An Apple Developer account, code signing and notarization; built on a Mac |

### 6.3 iPhone (notes, 2026-09-21)
- Tauri v2 builds for iOS, and the core crate compiles for it. The Keychain (with the Secure
  Enclave) is the key store; crypto as on macOS.
- Sign-in uses `ASWebAuthenticationSession`, iOS's system browser sheet, which keeps the
  rule "never an in-app web view" (SECURITY.md §4.4).
- Midi and Max are desktop shapes. A phone needs its own surface: a design of its own.
- iOS limits background work, so the fetch runs while the app is open.
- **An option unique to iOS:** Apple Health's Health Records already pulls FHIR records
  from many health systems. With the user's permission, HealthKit gives an app those records
  as FHIR JSON (`HKClinicalRecord`), with no sign-in of our own. A fork for that time.
- App Store review has rules for apps that hold health data.
