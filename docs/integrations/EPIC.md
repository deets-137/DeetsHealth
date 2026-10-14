---
status: project
desk_test: none
sources: [core/src/auth.rs, core/build.rs, epic-client.example.json, scripts/epic-redirect-check.mjs, src-tauri/src/signin.rs, src-tauri/src/lib.rs, .gitignore]
updated: 2026-10-13
---
# DeetsHealth — Epic: the registration and the sandbox sign-in

> The owner has a fhir.epic.com account (2026-10-13). This doc is the sheet he fills in from
> the app registration (§2), the sandbox facts (§3), and the plan for the Epic sandbox
> sign-in (§4). Forks are §5. How sign-in works today: [SIGN-IN.md](../architecture/SIGN-IN.md).

## 1. Terms
- **fhir.epic.com** — Epic's developer site, where an app is registered.
- **Registration** — the app's record on fhir.epic.com: its audience, its APIs, its redirect
  URIs. Epic gives it two client IDs.
- **Non-production client ID** — works against Epic's sandbox only. Not a secret for a public
  client, but it lives in a git-ignored file all the same (SECURITY.md §4.8).
- **Production client ID** — works against real health systems once Epic syncs it to them.
  Not used until a real record is approved (stop rule 4).
- **Epic sandbox** — Epic's test FHIR server with fake patients:
  `https://fhir.epic.com/interconnect-fhir-oauth/api/FHIR/R4/`.
- **DCR** — dynamic client registration. Each install registers its own key with the site
  after its first sign-in, so the site can renew access with no new sign-in (SECURITY.md §5,
  decided 2026-09-21).

## 2. The sheet: what the owner fills in
Copy each value from the registration. **Paste the IDs into `epic-client.json` at the repo
root, not into this doc or the chat.** The file is git-ignored already (`.gitignore`). Copy
its shape from `epic-client.example.json` (§5a).

The rows follow the "Create an App" form, top to bottom (seen 2026-10-13).

| # | Field on fhir.epic.com | What to pick | Where it goes in the app |
|---|---|---|---|
| 0 | Application Name | `DeetsHealth` | MyChart shows it on the consent page. |
| 1 | Application Audience | **Patients** | Fixes the sign-in as a patient sign-in (MyChart). |
| 1a | Automatic Client Distribution | **USCDI v3** (fork E0, §5; chosen 2026-10-13). | Decides whether real Epic sites get the app with no per-site request. No effect on the sandbox. |
| 1b | Public Documentation URL | Blank for now, if the form lets you save. Otherwise a page on deets.solutions. | Needed before production. |
| 2 | Incoming APIs | **Every R4 `.Read` and `.Search`** the form offers, both **(Patient Chart)** and **(Outside Record)** (2026-10-13: the portal shows the whole record, and the fetch already maps every FHIR type, `core/src/fhir.rs` `table_for`). Skip DSTU2 and STU3; every `.Create`, `.Update` and other write (stop rule 1); **(Premium Billing)** (Epic charges for it); and any API the form refuses for a Patients audience. If fork E0 is USCDI v3, take what the form still allows. | Sets the scopes Epic grants. The fetch asks only for the types that the site's capability statement and the grant allow (FETCH.md). |
| 3 | Endpoint URI (the redirect URI) | **`http://127.0.0.1/callback`** (added 2026-10-13), **`http://127.0.0.1:42813/callback`** and **`http://127.0.0.1:46219/callback`** (the fallback ports, §3). The two `deetshealth(-dev)://callback` entries from the first try are no longer used; they can go. The form also says production needs at least one `https` URI: the web app's callback can be that one, later. | `loopback::PORTS`: the first free port of 80, 42813, 46219 (§3). |
| 4 | Is Confidential Client | **Unchecked.** A desktop app cannot keep a secret. PKCE proves it. | Already the design (SIGN-IN.md §1). |
| 5 | Can Register Dynamic Clients | **Checked** (SECURITY.md §5, decided). | Step E-5 (§4). Without it, a public app gets no long-lived access, and the user signs in again about every hour. |
| 5a | How will dynamic clients request an access token | **JWT bearer grant type** (§6). | Step E-5. |
| 6 | SMART on FHIR version | **R4** | |
| 7 | SMART scope version | Tell me **v1 or v2**. | v1: `patient/*.read`. v2: `patient/*.rs`. Today the app asks with v1 words (`auth::SCOPES`). |
| 8 | FHIR ID generation scheme | The default. Tell me which it is. | The store keeps IDs of any length (sealed). |
| 9 | Non-production client ID | Copy it to `epic-client.json`. | Fork E1. |
| 10 | Production client ID | Copy it to `epic-client.json`, unused for now. | Later, with the endpoint directory (ENDPOINT-DIRECTORY.md). |
| 11 | Privacy policy and terms URLs | Leave blank if allowed. Needed before production. | Later, with the release path. |

### 2.1 The APIs to pick (read and search only)
Row 2 takes every R4 read. These are the ones the portal's first hubs need
(features/HOME-AND-HUBS.md), so check that each one is in the Selected list: `Patient`, `AllergyIntolerance`,
`Condition` (every category the list offers: problems, health concerns, encounter diagnoses),
`Encounter`, `Observation` (labs, vitals, social history, and the rest of the categories),
`DiagnosticReport`, `MedicationRequest`, `MedicationStatement` (if listed), `Medication`,
`Immunization`, `Procedure`, `DocumentReference`, `Binary` (the note's text),
`CarePlan`, `CareTeam`, `Goal`, `Device`, `Practitioner`, `PractitionerRole`,
`Organization`, `Location`, `Provenance`, `Appointment` (read: upcoming visits).
The fetch already handles each of these (INDEX-AND-LINKS.md). If the form offers more read
APIs, pick them too: an unused scope costs no calls.

## 3. The Epic sandbox
- Base URL: `https://fhir.epic.com/interconnect-fhir-oauth/api/FHIR/R4/`. Its SMART
  configuration is at `.well-known/smart-configuration`, as the sign-in reads it today
  (SIGN-IN.md §3, step 2).
- Test patients and their MyChart logins: fhir.epic.com › Documentation › **Test Data**.
  Camila Lopez first (HANDOFF.md › Decided 3). These are public fakes, so they may appear in
  docs and in chat (DOCS-ORG.md §1, rule 7).
  - **Camila Lopez:** MyChart login `fhircamila`, password `epicepic1` (Epic's published
    sandbox login; worked 2026-10-13).
- **Epic's sign-in page cannot hand off to a custom scheme in Edge** (found 2026-10-13, E-3).
  After the patient approves, the page POSTs a form to
  `https://fhir.epic.com/mychart-fhir/Authentication/OAuth/Authorize`, and that POST is
  redirected to our `deetshealth-dev://callback`. Epic's page sets the Content Security Policy
  `form-action *`. Chromium checks `form-action` along a form's whole redirect chain, and `*`
  matches only network schemes (http, https, ws, wss). So Edge blocks the last hop, and the
  page stalls: "Sending form data to '…/OAuth/Authorize' violates … 'form-action *' … The
  request has been blocked." We cannot change Epic's page. A sign-in return that Epic's page
  allows must be an `http` or `https` URL (HANDOFF.md › Open now 5).
- **What MyChart shows at sign-in** (2026-10-13): "Share your data with DeetsHealth", a notice
  that "DeetsHealth, offered by Deets Solutions, is not affiliated with your healthcare provider
  and might not be obligated by HIPAA", and a list of five practices that Deets Solutions "has
  not shared if it" does (sells data, shares without permission, marketing, insights about the
  family, stores health data permanently). Then the patient picks the data to share and for how
  long (one hour was offered). The list comes from Epic's app attestation. Answering it on
  fhir.epic.com is later work, before production.
- A new or changed registration can take a while to reach the sandbox. A sign-in refused as
  "invalid client" in the first hour may only mean that.
- **`error=4` on the authorize step** (a `302` to `…/OAuth/Start?error=4`) means the sandbox does
  not know that redirect URI. Proved 2026-10-13: an `https://example.com` URI that was never
  registered gets it, and a registered one does not. A redirect URI added on fhir.epic.com
  still got `error=4` for a while after it was saved and listed: the change had not synced.
  It synced in about 25 minutes (saved near 17:15, accepted at 17:41).
- **Epic matches the redirect URI's port exactly** (2026-10-13). Once `http://127.0.0.1/callback`
  was accepted, the same URI with any other port still got `error=4`. RFC 8252 §7.3 asks a
  server to accept any port on a loopback URI; Epic does not. So the app tries a short fixed list
  (`loopback::PORTS`: 80, then 42813, then 46219), and **every port in it must be registered**:
  `http://127.0.0.1/callback` (port 80), `http://127.0.0.1:42813/callback`,
  `http://127.0.0.1:46219/callback`.
  Check before a desk test: `node scripts/epic-redirect-check.mjs` ("accepted" or `error=4` for each
  port in `loopback::PORTS`).
- Epic's sandbox data is small and tidy (HANDOFF.md › Gotchas). It proves the path, not the mess.
- **Epic's sign-in has a patient picker for proxies.** A person with proxy access in MyChart
  picks whose record to share, and the token is for that one patient. A parent who signs in
  for two children makes two sites. The app already works this way: another patient becomes
  a new site (SIGN-IN.md §3, step 1b).

## 4. The plan
| Step | What | Who | Calls to Epic |
|---|---|---|---|
| E-1 | Fill the sheet (§2) and write `epic-client.json`. **Form done 2026-10-13; the file is next.** | Owner | 0 |
| E-2 | The Epic sandbox becomes a second entry in Settings › Sites › Connect, with its client ID and its own scope words. **Built 2026-10-13** (§5a). | Claude | 0 |
| E-3 | Live sign-in as Camila Lopez. First try 2026-10-13: blocked at the return (§3); the loopback return is built for the second try. | Owner, desk test | 2 (the SMART configuration, the token) |
| E-4 | The full fetch against Epic. Then FHIR-SHAPES.md gets Epic's real shapes. The probe (`scripts/fhir-probe.mjs`) reads only open servers and holds no token, so for Epic the shapes come from the sealed store through the core, printing shapes only, never a value. That tool is part of this step. | Claude, then the owner | about 70–100 for the fetch, a guess (SMART took 70), measured after as FETCH.md does; 0 for the shapes |
| E-5 | DCR: the install's key pair, registered after the first sign-in, then renewal with no sign-in (SECURITY.md §5). Its own paper design first. | Claude, design to the owner | 1 to register, then 1 per renewal |

E-2 to E-4 change no schema, and they do not touch the wall. The store, the fetch and the
seal are the ones built for SMART.

## 5. Forks (for the owner)
**E0. Automatic Client Distribution** (sheet row 1a). **Chosen: USCDI v3** (the owner,
2026-10-13). The options as they were brought:
- **A (recommended). USCDI v3.** Real Epic sites can get the app with no per-site request.
  This matters for a consumer app that can reach about 1,278 sites (ENDPOINT-DIRECTORY.md). The
  cost: the API list may be limited to the USCDI v3 set, so an API outside it (some
  Outside Record ones, perhaps) may have to go.
- **B. None.** Any API can be picked. Each real site then needs Epic's manual client
  distribution request before its patients can use the app. Fine for the sandbox, and it can
  change later ("You'll also be able to make changes to this information later").
- USCDI v1 is the smaller older set. CMS Patient Access API fits payer data (insurance claims).
  Neither fits better than v3.

**E1–E3 were not real forks** (the fork test, LESSONS.md §5; 2026-10-13). Each had one
option that passes:
- **E1. The client ID is read at build time** (`core/build.rs`). Read at run time, a release
  has no file to read.
- **E2. Each site entry carries its own scope words.** One v2 string for every site risks
  SMART's launcher, and gives nothing in return.
- **E3. E-4 (the fetch) is desk-tested before E-5 (DCR).** The one-feature rule decides it.
  An Epic DCR starts from a normal sign-in anyway, so E-3 is its first half. (§6 explains E-5.)

## 5a. As built: E-2 (2026-10-13)
> **Part:** built · 2026-10-13

- `epic-client.json` at the repo root (git-ignored): `nonprod_client_id`, `prod_client_id`
  (unused), `scope_version` (`"v1"` or `"v2"`). The template is `epic-client.example.json`.
- `core/build.rs` reads it at build time into `DEETSHEALTH_EPIC_CLIENT_ID` and
  `DEETSHEALTH_EPIC_SCOPES`. It prints nothing. With no file, or an empty ID, there is no Epic
  entry. A `scope_version` that is not `v1` or `v2` stops the build with a plain message.
- `auth::sandbox_sites` adds **Epic sandbox** at index 1, after SMART, with
  `auth::EPIC_SANDBOX` as its base URL. `SiteEntry.scopes` carries each site's words;
  `auth::begin` sends them.
- Test: `auth::tests::epic_entry_follows_the_build`. Run with a dummy file (two entries, v2
  words), with no file (SMART only), and with a bad `scope_version` (the build stops).

**Desk test (E-3):**
1. Copy `epic-client.example.json` to `epic-client.json`. Paste the non-production client ID,
   and set `scope_version` to the form's SMART scope version.
2. `npm run dev:app`, from any terminal (since 2026-10-13 there is no scheme to register).
3. Settings › Sites › Connect a site › **Choose** › **Epic sandbox**.
4. The browser opens Epic's MyChart sign-in. Sign in as Camila Lopez (`fhircamila`, §3). Allow
   the data. The tab then says "You're signed in", and DeetsHealth comes to the front.
5. The Sites row turns from "Finish signing in in your browser" to the counts, as the fetch
   runs. The fetch (E-4) runs at once, because the fetch code is already built.
6. If a step fails: the toast's words, and what the browser showed. An "invalid client" in the
   first hour may only mean the registration has not reached the sandbox yet (§3).

## 6. Dynamic clients (step E-5)
> **Part:** project · 2026-10-13 · the key's algorithm and home wait for E-5's paper design

**The problem it solves.** A public client (an app with no secret) gets an access token for
about an hour. Epic does not give a public app a long-lived refresh token, because anyone who
copied it could use it. Without DCR, the patient signs in again every hour.

**What DCR does.** It turns each install into its own confidential client, with a secret that
cannot be copied:
1. **A normal sign-in** (E-3): authorization code + PKCE. The token reply has a short access
   token and the `patient` id.
2. **The install makes a key pair** in its own key store. The private key never leaves it.
3. **Register:** `POST https://fhir.epic.com/interconnect-fhir-oauth/oauth2/register`, with
   the sign-in's access token as `Authorization: Bearer`, and a body of `software_id` (our
   non-production client ID) and `jwks` (the **public** key). Epic answers `201` with a **new
   client ID for this install only**, bound to that public key and to this patient's grant.
4. **Each new access token, from then on (the JWT bearer grant):** the install builds a short
   JWT: `iss` and `sub` = its dynamic client ID, `aud` = the token endpoint, a unique `jti`,
   and an `exp` a few minutes away. It signs the JWT with its private key, then POSTs
   `grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer`, `assertion=<the JWT>`,
   `client_id=<its dynamic client ID>`. Epic checks the signature against the registered
   public key and gives a new access token for the same patient. There is no browser and no
   sign-in.
5. **It ends when** the patient revokes the app in MyChart (connected apps), the site ends the
   registration, or the user signs out here: the sign-out erase destroys the key (SECURITY.md
   §4.7), so the registration can never be used again.

**JWT bearer or refresh token** (the form's question). Not a fork, by the fork test (LESSONS.md
§5). Epic offers both (`grant_types_supported` in its SMART configuration lists
`refresh_token` and `jwt-bearer`).
| | JWT bearer | Refresh token |
|---|---|---|
| Long-lived secrets at rest | One: the private key, which cannot be exported | Two: the key and a refresh token |
| What a copied data folder gives an attacker | Nothing, when the key lives in a store that cannot export it | A refresh token, which still needs the key |
| How long access lasts | As long as the registration | Until the refresh token runs out, by each site's own policy, then a new sign-in |
| State to keep | None | The refresh token, sealed, and replaced on each rotation |

JWT bearer is more secure and gives fewer sign-ins, so it follows.

**Where the key lives, per platform** (E-5's paper design picks the algorithm; Epic's docs
name the JWT algorithms it accepts. Check them then: they are not in its published
configuration):
- **Windows:** a CNG key that cannot be exported. If the algorithm allows it, the TPM (the
  Microsoft Platform Crypto Provider), so the key is tied to this PC's hardware.
- **Web:** a WebCrypto key that cannot be extracted, kept in IndexedDB.
- **macOS and iPhone:** the Keychain. The Secure Enclave holds only P-256 keys, so whether it
  can hold this key depends on Epic's accepted algorithms.

**What each site sees:** one more registration per install and per patient. It costs one call to
register, then one call per hour of use. A full fetch already costs about 70.
