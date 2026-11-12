---
status: idea
desk_test: none
sources: []
updated: 2026-11-12
---
# Ideas — not built

The docs in this folder describe features that do not exist in DeetsHealth. Do not tell a
hospital or a patient that the agent can do any of these things. The patient app's ideas are
[archive/PATIENT-APP-IDEAS.md](../archive/PATIENT-APP-IDEAS.md).

- **Outbound calls** (decided for after inbound, VOICE-AGENT.md §7): reminders, confirmations,
  check-in calls after discharge. Epic reports outbound answer rates of about 15–30 percent.
  **Medication check-ins** (the owner, 2026-11-11: inbound first, then outbound): the agent
  calls on a schedule and asks whether the patient took their medicines. Needs its own design on
  paper first: consent, call times, who sees the answers, and where they are filed (the held
  `DocumentReference.Create (Clinical Notes)` API, EPIC.md §2.1). Inbound, the caller asks
  "when do I take my pills?" and the agent reads `MedicationRequest`'s dose instructions.
- **A live demo for a hospital's analyst team** (the owner, 2026-11-11: "doc this idea for
  later, can plan to utilize a cloudflare tunnel"). The owner is the caller in the Chat box on
  his own PC; each analyst watches the same call from their own PC. The plan: a Cloudflare
  Tunnel to a name under `deets.solutions`, with Cloudflare Access in front (an emailed
  one-time code, an allow list of emails), so nothing on his PC is open to the internet. What
  it needs: a watch-only view for the analysts (only the caller's seat starts a call or spends
  tokens); a built console, not the dev server, with the page, the caller connection and the
  watch stream all on one address; a test from the hospital's own network first, because many
  block WebSockets to unknown sites; synthetic callers only, and the page says so. A real call
  in front of analysts is the production watch instead (sign-in, roles, an audit row per view;
  CONSOLE.md).
- **Spanish and other languages.** Cartesia's Ink-2 is English only (VOICE-AGENT.md §8.6).
- **Running on Epic's Agent Factory** once Epic opens external connectors (VOICE-AGENT.md §5).
- **Other health products in this repo** (the owner, 2026-10-19: "if we ever get more health
  products / ideas can stay in this repo").
