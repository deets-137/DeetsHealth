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
- **The Analytics tab** (the owner, 2026-11-12: "doc it when we eventually build out the
  analytics tab of DeetsHealth, we can build for this"). A console tab for analyzing calls
  across many calls. Its first source is already built: per-tool latency, `GET
  /agent/latency` (CHAT.md §10), a summary per tool and model (p50, p90, max of six spans) and
  one row per tool call, with names and numbers only. What the tab needs before it is built:
  its design on paper (where it sits in the nav, the charts, the filters: source, model, date
  range, exact only); the per-turn spans for turns with no tool (the 800 ms target per turn,
  DEBUGGING.md); and in production, the same rows in the store's call log, behind roles.
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
- **A CLI and an MCP server for the console** (the owner, 2026-11-12):
  [CONSOLE-MCP.md](CONSOLE-MCP.md). One staff API with three clients (the console, `deets` on
  the command line, a remote MCP server), under the same sign-in, roles and audit trail. The
  CLI first, for us to debug; the MCP server later, for a hospital's analysts. No protected
  data through MCP unless the hospital turns it on for one client its own BAA covers.
- **Access levels by role** (the owner, 2026-11-12): [ACCESS-ROLES.md](ACCESS-ROLES.md).
  Open ended; the owner designs and builds it. Each role is custom: a hospital creates and
  names it, built from levers that become clear as we build. The doc collects the levers.
- **Spanish and other languages.** Cartesia's Ink-2 is English only (VOICE-AGENT.md §8.6).
- **Running on Epic's Agent Factory** once Epic opens external connectors (VOICE-AGENT.md §5).
- **Other health products in this repo** (the owner, 2026-10-19: "if we ever get more health
  products / ideas can stay in this repo").
