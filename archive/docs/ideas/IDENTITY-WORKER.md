---
status: idea
desk_test: none
sources: []
updated: 2026-10-13
---
# Idea — an identity worker for tiered access

> Not built, not designed: forks open (§7). The owner (2026-10-13): "setup cloudflare workers to
> do better identity and security verification for a tiered access setup (imagine parent /
> child, or read-only vs read-write permissions, etc)". Do not tell a user the app does this.
> It changes DESIGN.md §5 ("No cloud copy") only under option W1-C, which is not recommended.

## 1. Terms
- **Worker** — a Cloudflare Worker: our code on Cloudflare's servers. Created and deployed
  with the `cf` CLI.
- **Deets account** — a person's identity in DeetsHealth, apart from any health system.
- **Device** — one install (the PC app, a browser, a phone). An account has one or more.
- **Owner** — the person whose record it is, or who is signed in to the site for them.
- **Delegate** — a person the owner lets see some or all of the record in DeetsHealth.
- **Grant** — the owner's permission for one delegate: which person's record, which tier,
  until when.
- **Tier** — how much a grant allows (§4).
- **Relay** — the worker passes a sealed bundle from one device to another. It cannot open it.

## 2. What the health system already does
**Epic has proxy access at the source.** A parent with proxy access signs in to MyChart as
themselves, picks the child in Epic's patient picker, and the token is for the child only
(EPIC.md §3). The site applies its own rules, including each state's limits on a parent's
view of a teen's record. DeetsHealth already makes one site per patient, so this works with
no worker.

**Proposed rule:** the site decides who may read a record at the source. DeetsHealth never
widens that. A grant only lets the owner pass on what the owner can already see.

So a worker adds what the site cannot do:
1. **One person, several devices:** the PC app and, later, the web and the phone.
2. **Sharing between people inside DeetsHealth**, where the site has no proxy: a spouse, an
   adult child helping a parent, a caregiver who needs only medicines and allergies.
3. **Stronger identity** for each of these: who is this account, and is this the same person
   as at the site?

## 3. Parent and child, case by case
| Case | Path | Worker needed? |
|---|---|---|
| Young child | The parent's own MyChart proxy sign-in. The child is a person under the parent's switcher (HOME-AND-HUBS.md fork H5). | No |
| Teen (about 12–17) | The site often limits the parent's proxy view by state law. A teen with their own MyChart may choose to share some of it. | Yes, for the teen's own share. **Sensitive:** the app must never let a parent get past the site's limits. |
| Adult helping a parent | The parent's proxy grant in MyChart, or the parent shares in DeetsHealth. | Only for the DeetsHealth share |
| Caregiver, babysitter | A partial share (medicines, allergies), with a time limit. | Yes |

## 4. Tiers (a draft for the owner to edit)
| Tier | Sees | Can do |
|---|---|---|
| Owner | everything the site gives | sign in, fetch, share, revoke |
| Full read | every hub | read only |
| Partial read | the hubs the owner picks (for example Medicines and Allergies) | read only |
| Act (later, with writes) | as Full read | writes only through **their own** site sign-in (Epic proxy). A DeetsHealth tier never lends the owner's token. |

Each grant has an end date, and the owner can revoke it at once. A revoke stops new bundles.
It cannot erase what a delegate's device already opened: the app says that plainly.

## 5. Architectures
**W1-A. Zero-knowledge relay (the choice; §7).** The worker holds accounts, devices, each
device's **public** key, the grants, and an audit list (who opened which share, when, with
no record data). The owner's device builds the share, seals it to each delegate device's
public key, and sends it through the relay. The worker never holds a key that opens a record.
- On Cloudflare: one **Durable Object per account** (its devices, grants, audit). DeetsEngine
  already runs on a Durable Object (`DeetsEngine/src/do_store.rs`), so grants can be globals
  (`^GRANT`, `^DEVICE`). **R2** holds sealed bundles with an expiry. **Turnstile** guards sign-up.
- Still PII: the worker knows that account A shares with account B. That is a relationship, not
  a health fact, but Washington's My Health My Data Act and the FTC's Health Breach
  Notification Rule may still count it (SECURITY.md §6). It needs its own privacy notice.

**W1-B. No sharing worker.** Each person signs in to the site through Epic proxy, on their own
device. A worker only links one person's devices (a sealed backup and settings). Strongest
privacy. Covers no caregiver without a MyChart proxy.

**W1-C. A cloud copy of the record (not recommended).** The worker stores the record and checks
tiers on each read. This breaks DESIGN.md §5 and SECURITY.md §4.1, brings breach duties, and
puts every user's record in one place to attack.

## 6. Identity: how strong
- **Passkeys** (WebAuthn) for every Deets account: no password to steal. Windows Hello on
  the PC, Face ID on the phone, the browser on the web. WebView2's passkey support needs a
  check. If it falls short, the ceremony runs in the system browser, as sign-in does
  (SECURITY.md §4.4).
- **Linking a delegate face to face:** the two devices show a short code or a QR code, and
  each person compares it, as Signal's safety numbers work. This stops a relay that lies about
  whose key is whose.
- **Site-verified owner (optional):** at sign-in, the site gives a signed `id_token`. The worker
  can check its signature against the site's published keys and keep only a salted hash of
  "this account is a patient at this site", never the patient id. This proves the owner is
  real, with the health system's own identity check behind it. It needs the outside security
  review (SECURITY-BRIEF.md).
- A paid ID check (ID.me, CLEAR or similar) is possible. It is not needed while the site's
  sign-in already proves the owner.

## 7. Forks (for the owner, when this comes up)
The owner asked how many of the first six forks were real (2026-10-13). Four were not:
- **The architecture is the zero-knowledge relay** (§5, W1-A). The cloud copy breaks a decided
  non-goal (DESIGN.md §5). No sharing worker does not give the tiered access he asked for.
- **The worker's store is Durable Objects with DeetsEngine globals.** The engine already has a
  Durable Object backend, and he built it to be the data structure of new work.
- **When:** after the Epic sign-in and the first hubs. The one-feature rule decides it
  (CLAUDE.md › Working style).
- **The tiers** (§4) are a draft for him to edit, not a fork.

The real forks:
- **W2. Site-verified owner** (§6): A yes, the worker checks the site's signed `id_token` and
  keeps only a salted hash (recommended; it needs the outside security review) · B no, passkeys
  and face-to-face linking only. Passkeys and the face-to-face check are in either way. A paid
  ID check is not needed while the site's sign-in proves the owner.
- **W3. Share shape:** A a snapshot the owner sends, and sends again after a fetch
  (recommended first) · B a live share that the owner's device updates after each fetch, by
  itself.

## 8. Every platform
- **Windows:** device keys from CNG, wrapped by DPAPI, as the site keys are (SECURITY.md §4.6).
- **Web:** WebCrypto non-extractable keys. The web app talks to the same worker. The worker
  also serves the web app's static files, so the code the browser runs and the relay share one
  deploy. DESIGN.md §6 names the risk: a changed host could serve changed code.
- **macOS and iPhone:** the Keychain and the Secure Enclave hold the device key. Passkeys are
  native.
