---
status: idea
desk_test: none
sources: []
updated: 2026-11-12
---
# DeetsHealth — Access levels by role (idea)

> Not built, and open ended on purpose. **The owner designs and builds this** (2026-11-12).
> This doc holds his direction and a place to collect the levers as they appear. It holds no
> design of ours: no role list, no permission table, no forks.

## 1. Terms
- **Role** — a set of access that a hospital creates and names itself.
- **Lever** — one thing a role can be allowed or refused. The levers are found as we build.

## 2. The direction (the owner, 2026-11-12)
- **Each role is custom.** A hospital creates a role and gives it its own name.
- **Each role is built from levers.** Which levers exist becomes clear as we build, and as the
  product gets closer to finished.
- **Open ended for now.** Nothing more is decided.

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
