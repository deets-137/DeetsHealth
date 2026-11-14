---
status: idea
desk_test: none
sources: [console/src/pages/roles.ts]
updated: 2026-11-14
---
# DeetsHealth — Access levels by role (idea)

> Not built, and open ended on purpose. **The owner designs and builds this** (2026-11-12).
> This doc holds his direction, his decisions, and a place to collect the levers as they
> appear. Since 2026-11-14 it also holds his first role templates (§4), as a draft.

## 1. Terms
- **Role** — a set of access that a hospital creates and names itself.
- **Lever** — one thing a role can be allowed or refused. The levers are found as we build.

## 2. The direction (the owner, 2026-11-12)
- **Each role is custom.** A hospital creates a role and gives it its own name.
- **Each role is built from levers.** Which levers exist becomes clear as we build, and as the
  product gets closer to finished.
- **Open ended for now.** Nothing more is decided.

**Staff sign-in is built with the roles** (the owner, 2026-11-12): STAFF-SIGN-IN.md, its plan
and sizes in §10.

SECURITY.md §4.10's four roles (admin, supervisor, viewer, auditor) are examples, not a fixed
list.

## 3. Levers seen so far
A list to collect, not a design. Add a line when a feature shows a new thing to allow or
refuse, with a link to where it appears.
- Read the call log (DEBUGGING.md §1).
- Read a transcript (SECURITY.md §4.10; CHAT.md §2, PHI mode).
- Play a recording (VOICE-AGENT.md §8.7).
- Change the settings, and the recording rules (FUTURE-SETTINGS.md).
- Read the audit trail (SECURITY.md §4.10).
- The tools of the CLI and the MCP server (ideas/CONSOLE-MCP.md §5.2).
- Map the IdP's groups or app roles to roles; change the IdP settings (STAFF-SIGN-IN.md §5,
  M1 and M3).
- Approve our support access; hold the local emergency account (STAFF-SIGN-IN.md §5, M6).
- Export the audit trail; read the access review (STAFF-SIGN-IN.md §5, M5 and M7).
- Open a restricted call (STAFF-SIGN-IN.md §7, F2).
- Approve a change to what callers hear (STAFF-SIGN-IN.md §5, M8).

## 4. The templates (a draft, 2026-11-14)
> **Part:** idea · 2026-11-14 — shown on Security › Roles (`console/src/pages/roles.ts`),
> not wired to a store.

**Decided (the owner, 2026-11-14):** each lever has three levels: **None**, **View**, **Edit**.
(Not chosen: on / off, and a fourth level, Propose, where a change waits for approval.)

The owner's starting roles, 2026-11-14: Director, Manager, Senior Analyst, Junior Analyst,
Clinician; on our side, Engineer and Intern. **Compliance** is Claude's addition (a privacy
officer who reviews access, so the Director does not audit their own changes): the owner to
keep or drop it. A template is a starting role: a hospital copies, renames or changes it
(§2).

The levers in the table: the owner's nine (PHI, Auth, Roles, Agent config, vendor config,
analytics, testing, rules, audit logs) and Claude's two (People: add and remove users; Flags:
work the flag list). The levels are Claude's draft for the owner to edit.

| Role | PHI | Auth | Roles | People | Agent | Vendors | Rules | Testing | Analytics | Flags | Audit logs |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Director | View | Edit | Edit | Edit | Edit | Edit | Edit | Edit | View | View | View |
| Manager | View | View | View | Edit | Edit | View | Edit | Edit | View | View | View |
| Senior Analyst | View | None | None | None | Edit | View | Edit | Edit | View | View | None |
| Junior Analyst | None | None | None | None | View | None | View | Edit | View | None | None |
| Clinician | View | None | None | None | None | None | None | None | None | Edit | None |
| Compliance | View | View | View | View | View | View | View | None | View | View | View |
| Engineer (ours) | None | View | View | None | View | View | View | Edit | View | None | View |
| Intern (ours) | None | None | None | None | View | None | View | Edit | View | None | None |

Notes on the draft, not decided:
- **Our side** gets into a hospital's console only when that hospital approves it, for a set
  time, in the audit log (STAFF-SIGN-IN.md M6). Engineer's PHI is None except through that
  emergency access. Intern never gets PHI.
- **No one gives more access than they have.** Edit on Roles never covers one's own role. A
  hospital always keeps one Director. Every lever change goes in the audit log.
- **PHI has parts** (a transcript, the audio, the caller's number, a restricted call) and a
  scope (a Clinician sees the flagged calls only). Both wait for the work that needs them.
- Testing is with synthetic patients only.
