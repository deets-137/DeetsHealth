---
status: idea
desk_test: none
sources: []
updated: 2026-11-12
---
# DeetsHealth — Staff sign-in and access control (idea)

> Not built. Nothing here exists: no sign-in, no staff API, no roles. Do not tell a hospital
> that the console signs in through its directory yet. The owner and Claude design this
> together (the owner, 2026-11-12: "keep it in ideas/ until we build it out"). The roles
> themselves are the owner's (ACCESS-ROLES.md). This doc is the sign-in, the checks and the
> records around them.

## 1. Terms
- **IdP (identity provider):** the system that holds the hospital's staff accounts. A hospital
  with Active Directory almost always syncs it to **Microsoft Entra ID**. Others use Okta or
  Ping. We connect to the IdP, never to the on-site directory.
- **SSO (single sign-on):** staff sign in to the console with their hospital account. We never
  see or keep a staff password.
- **OIDC / SAML:** the two standard ways an IdP tells us who signed in. OIDC fits a web console.
  Some hospitals require SAML.
- **Claim:** one fact in the sign-in token (the user's stable id, their roles, how and when they
  signed in).
- **SCIM:** a standard by which the IdP tells us when a user is added, changed or disabled.
- **Role, lever:** as in ACCESS-ROLES.md. The hospital names a role and builds it from levers.
- **Step-up:** a request for a fresh sign-in, or fresh MFA, before one sensitive action.
- **Break-the-glass:** a view of a restricted record that needs a stated reason and is reviewed
  later. Epic uses this for employee and VIP charts.
- **Restricted call:** a call whose caller the hospital marks as sensitive (an employee, a VIP,
  a patient who asked for privacy). Not built (§7, F2).

## 2. What is decided
- Staff sign in through the hospital's own SSO; MFA is the hospital IdP's (SECURITY.md §4.10,
  the owner, S1 A).
- Each transcript view, recording play and export is an audit row (SECURITY.md §4.10).
- Our staff have no standing access to a hospital's protected data. Support access is
  time-limited, approved by the hospital, and audited (SECURITY.md §4.10).
- Each role is custom, named by the hospital, built from levers. The owner designs the roles
  (ACCESS-ROLES.md).
- One staff API serves the console, the CLI and the MCP server, with the same checks for each
  (CONSOLE-MCP.md §3).

## 3. Two identity domains
1. **The hospital's staff.** Their IdP, their groups, their MFA. Everything in §5 is for them.
2. **Our own staff** (engineers, support). Our own IdP, our own MFA, no standing access to
   production. SOC 2 and ISO 27001 audit this domain as closely as the first. In deploy B (the
   hospital's cloud account) our access is also subject to the hospital's own cloud rules.

The two domains never mix. A hospital role never grants one of our staff access, and our
support access never comes from a hospital group.

## 4. What the standards ask of staff access
**This is a map, not legal advice.** *confirm* marks a fact to check before the first hospital.
REGULATIONS.md holds the wider list; this section holds only what touches staff access.

### 4.1 HIPAA Security Rule (binds us as a business associate)
| Section | What it asks | Module (§5) |
|---|---|---|
| 164.308(a)(1)(ii)(D) | Review system activity: audit logs, access reports | M5 audit, M7 reviews |
| 164.308(a)(3)(ii)(A–C) | Authorize and supervise the workforce; clearance; **end access at termination** | M2 sign-in, M4 provisioning |
| 164.308(a)(4)(ii)(B–C) | Grant access by policy; record each grant and change | M3 roles, M5 audit |
| 164.308(a)(5)(ii)(C) | Monitor sign-in attempts | M5 audit |
| 164.312(a)(2)(i) | **Unique user id** (required) | M2: the IdP's stable id |
| 164.312(a)(2)(ii) | **Emergency access procedure** (required) | M6 break-glass |
| 164.312(a)(2)(iii) | Automatic logoff | M2 sessions |
| 164.312(b) | Audit controls | M5 |
| 164.312(c) | Integrity: show a record was not changed | M5 tamper evidence (REGULATIONS.md R6) |
| 164.312(d) | Person authentication | M2, with the IdP's MFA |
| 164.316(b)(2) | Keep policies and records of actions 6 years | M5 retention. *confirm* whether audit rows fall under it |
| Privacy Rule 164.514(d)(2) | Minimum necessary for the workforce: each class of staff sees only the PHI it needs | M3 levers |
| Proposed Security Rule update (January 2025) | MFA mandatory; notify within 24 hours when a workforce member's access changes or ends | M2, M4. *confirm* whether it is final and the exact terms |

### 4.2 The Joint Commission
The Joint Commission surveys the **hospital**, not us. The hospital must show that it controls
what its vendors do with its information. So their survey questions become our requirements.
- **Information Management (IM) chapter:** the hospital protects the privacy, security and
  integrity of health information (IM.02.01.01, IM.02.01.03). *confirm* the current numbers.
- **Contracted services (LD.04.03.09):** the hospital oversees the quality and safety of a
  vendor's service. It will want our access reports and audit export. *confirm*.
- **Responsible Use of AI in Healthcare (with the Coalition for Health AI, 2025):** a
  governance body for each AI tool, local monitoring of quality, reporting of safety events,
  staff training. For access this means: a record of **who changed the agent's behavior and who
  approved it**, and roles for the people who monitor quality. *confirm* its current form.

### 4.3 ISO/IEC 27001:2022 Annex A (if we certify)
| Control | What it asks | Module |
|---|---|---|
| 5.3 | Segregation of duties | M3: no one approves their own access |
| 5.15, 8.3 | Access control policy; restrict access to information | M3 |
| 5.16 | Identity management across the life cycle | M2, M4 |
| 5.17, 8.5 | Authentication information; secure authentication | M2 (the IdP's) |
| 5.18 | Grant, review and remove access rights | M4, M7 |
| 8.2 | Privileged access rights | M6, M3 (admin levers) |
| 8.15, 8.16, 8.17 | Logging, monitoring, synchronized clocks | M5 |

ISO 27799 applies the same controls to health data. ISO 27701 adds privacy records.

### 4.4 SOC 2 and HITRUST
- **SOC 2:** CC6.1 (logical access), CC6.2 (registration and removal of users), CC6.3 (access
  by role, least privilege, segregation of duties), CC7.2 (monitoring). The auditor tests a
  sample of leavers: was access removed on time? M4 and M5 must prove it.
- **HITRUST CSF, domain 01 Access Control:** user registration, privilege management, review of
  access rights, MFA for remote access.

### 4.5 NIST and HHS practices
- **NIST SP 800-63B:** access to PHI at AAL2 at least. Prefer phishing-resistant MFA (FIDO2,
  Windows Hello, a badge with a certificate). The hospital's IdP enforces it; we read the claim.
- **NIST CSF 2.0, PR.AA** (identity, authentication, access control) and **HHS 405(d) HICP,
  practice 3 (access management):** the "recognized security practices" that lower OCR
  penalties (REGULATIONS.md R7).

### 4.6 State and other rules
- **New York, 10 NYCRR 405.46** (hospital cybersecurity, 2024): MFA for remote access, and
  hospitals hold vendors to it. *confirm* scope.
- **Texas HB 300:** staff training on PHI within 90 days of hire. The hospital trains its
  staff; a role may need a "trained" check later. *confirm*.
- **42 CFR Part 2:** a caller can say something about substance-use treatment. A transcript
  can then hold Part 2 data. *confirm* whether it applies to what a caller says, and whether a
  lever must limit those transcripts.

## 5. The modules
Each module lives behind the staff API (not built). Each check runs on the server, never only
in the console's screen.

- **M1 · Hospital identity settings.** Per hospital: the IdP's issuer and metadata, the client
  id, the protocol (OIDC or SAML), which claim carries roles, the session lengths. Changed only
  by a lever, and each change is audited.
- **M2 · Sign-in and sessions.** OIDC with PKCE (SAML where the hospital requires it). The
  user's id is the IdP's stable id (Entra's object id), never the email. Server-side sessions,
  an idle logoff, and a full session limit. A new sign-in on the same shared workstation (tap
  badge, for example Imprivata) replaces the session at once. On each refresh the IdP is asked
  again, so a disabled user is out within minutes even without M4.
- **M3 · Roles and levers.** The owner's design (ACCESS-ROLES.md). What this module must give
  every design: deny by default; one check function for every API call; the role input taken
  from Entra app roles or a filtered groups claim (both work with Okta and Ping too, and both
  avoid the groups-overage limit); no one may grant or approve their own access.
- **M4 · Provisioning.** SCIM from the IdP: a disabled user's sessions end at once. Where a
  hospital does not turn on SCIM, M2's refresh check is the floor. Every add, change and removal
  is an audit row with its source (SCIM, sign-in, an admin).
- **M5 · The audit trail.** Append-only. Each row: who (stable id), what (the lever used),
  which object (a call id, never a patient name), when (synchronized clock), from where, and the
  reason when one is asked (§7, F2). Tamper evident: a hash chain, and a copy in write-once
  storage (REGULATIONS.md R6). Export to the hospital's SIEM (Splunk, Sentinel) and to its
  privacy monitoring tool (the tools that already watch Epic's access log). What an export row
  may carry is open (§8).
- **M6 · Emergency and support access.** Two paths.
  - *The hospital's IdP is down:* one sealed local account per hospital, held by the hospital's
    security team, usable only with a hardware key, alerted on every use. Without it no one can
    stop a misbehaving agent during an IdP outage.
  - *Our support:* a request, the hospital's approval, a time limit, every action audited
    (SECURITY.md §4.10). In Entra this can be a guest account that the hospital turns on and off.
- **M7 · Access reviews.** A report for the hospital: each user, their roles, their last
  sign-in, their last protected-data view. The hospital fixes membership in its own groups.
- **M8 · Change control of the agent.** A setting or prompt change that alters what callers
  hear is recorded with who made it and who approved it (the Joint Commission's AI governance).
  Whether a second person must approve is the roles design (ACCESS-ROLES.md).

## 6. Working with the hospital's security team
1. **They review us first:** a questionnaire (HECVAT or SIG), the BAA, a data-flow diagram,
   SOC 2 or a pen test. SECURITY-BRIEF.md starts the answer. What we need from their directory:
   sign-in through their IdP only. No LDAP, no service account, no firewall opening.
2. **They register the console** in their IdP (in Entra, an Enterprise application). We give
   the redirect and logout URLs and the claims we need. They give the issuer and client id.
3. **Their directory decides who has a role; we decide what a role can do.** They create the
   groups or app roles and fill them. The hospital's admin maps each to a role in the console.
4. **They choose the MFA and the conditional access** (managed device, hospital network).
5. **They agree the emergency paths** (M6) and who holds the local account's key.
6. **They connect the audit export** to their SIEM and their privacy monitoring tool.
7. **They test in their test tenant** with test users: the role mapping and the leaver path,
   before any real staff sign in.
8. **They review access** on their schedule, with M7's report.

## 7. Forks
Brought to the owner 2026-11-12. "No step-up, trust the session" was dropped: it is weaker for
our convenience (LESSONS.md §5).

**Decided (the owner, 2026-11-12):**
- **F1: A, configurable by the hospital.** A protected-data lever needs a recent IdP sign-in
  with MFA. The hospital sets how recent. Fresh MFA on each open (B) is not built.
- **F2: C.** A hospital setting with two values: a reason only for restricted calls (the
  default) or a reason on every open. Every open is an audit row under both values.

The defaults' numbers and any floor are §8. The options as brought: 
- **F1 · Step-up before protected data.** When must a staff member prove themselves again before
  a transcript, a recording or an export?
  - **A (recommended): a recent sign-in.** A protected-data lever needs an IdP sign-in no older
    than a set time (OIDC `max_age`), and MFA in the token's claims. Cost: at most one extra
    sign-in per shift; seconds with a tap badge. A left-open screen is covered by the idle logoff.
  - **B: fresh MFA on each open.** Cost: many prompts per shift. Staff learn to approve prompts
    without reading them, or leave one person signed in for a team. Stronger on paper, weaker
    in practice.
- **F2 · A reason for access.** When must a staff member type why they open a transcript?
  Every open is an audit row in all options.
  - **A (recommended): only for restricted calls** (break-the-glass, as in Epic). Protects the
    callers most at risk (coworkers, VIPs) and keeps the reason meaningful. Cost: a way to mark
    a restricted call (§8).
  - **B: on every open.** Cost: many reasons per shift; they become empty words, and the
    privacy tool gets noise.
  - **C: a hospital setting, A by default** (FUTURE-SETTINGS.md). Cost: one more setting to
    build and test.

## 8. Later (not forks now)
- F1's default age of a sign-in, and whether there is a ceiling a hospital may not pass. The
  idle logoff's default, and its ceiling. Both go in FUTURE-SETTINGS.md when M2 is built.
- What an exported audit row may carry, and whether a call id is enough for the hospital's
  privacy tool to match a patient (it would need the hospital's own lookup).
- How a hospital marks a restricted call, if F2 needs it (from the chart's own flag, or the
  console).
- Our support access in deploy B.
- The full lever list (ACCESS-ROLES.md §3).

## 9. Where it runs
- **The call:** no staff sign-in. Caller verification is separate (SECURITY.md §4.4).
- **The console (a browser):** OIDC with PKCE; server-side sessions.
- **Deploy A2 (our cloud) and B (the hospital's cloud):** the same sign-in, because the IdP is
  reached over the internet in both. B also lets the hospital put the console behind its own
  network rules.
- **The EHR:** not affected. Epic access uses the backend-services key, not a staff identity.
