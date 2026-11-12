---
status: idea
desk_test: none
sources: []
updated: 2026-11-12
---
# DeetsHealth — A CLI and an MCP server for the console (idea)

> Not built. Nothing here exists, and the staff API it needs does not exist yet. Do not tell a
> hospital that the console has these tools. Forks 1 and 2 are the owner's (2026-11-12, §6).

## 1. Terms
- **The staff API** — the server that the console calls for the call log, the settings, the
  audit trail and the rest. Not built.
- **The CLI** — a command-line tool, `deets`, that calls the staff API.
- **The MCP server** — the same calls, given as tools to an AI client (Claude Code, Claude
  Desktop, Copilot, others) over the Model Context Protocol.
- **PHI-free tools** — tools that return only the call log's plain columns: call ids, times,
  lengths, outcomes, Flow paths, latencies, error kinds and counts (DEBUGGING.md §1).

## 2. Who it is for
1. **Us, while we build.** Claude Code (or a helper's agent) reads the call log, a call's
   timeline and its EHR ledger, replays a call, and starts a synthetic call, all from the
   terminal. This shortens the recipes in DEBUGGING.md §4.
2. **A hospital's analysts, later.** They ask their own AI client questions about the call
   log ("how many calls failed verification this week, and at which Flow node?"), draft
   setting changes, and run demo and training calls with synthetic callers.

## 3. The rule: one API, three clients
The console, the CLI and the MCP server all call the same staff API.
- **The same sign-in.** The hospital's own single sign-on (SECURITY.md §4.10, S1 A). The MCP
  server's OAuth step goes through it.
- **The same roles.** A tool exists for a user only if their role allows the same action in
  the console. An analyst with the viewer role gets the viewer's tools.
- **The same audit trail.** Each tool call is a row, marked "via MCP" or "via CLI".
- **No back door.** The MCP server has no access that the console does not have.

## 4. Protected data through MCP (fork 1 A, the owner, 2026-11-12)
**Off by default.** The MCP server gives the PHI-free tools only. A transcript, a recording
or any other protected data never goes through it. We cannot see which model is at the other
end of an MCP connection, or whether its vendor signed a BAA.

**A hospital can turn it on** when the hospital has its own BAA with the AI vendor of its
client (for example, Claude Enterprise under the hospital's BAA with Anthropic).
- An admin turns it on per client, not for all clients: the hospital registers the one MCP
  client its BAA covers, and only that client gets the protected tools.
- Turning it on is an audited setting change, with the admin's name and a note that names the
  BAA.
- Each transcript the MCP server returns is an audit row, the same as a view in the console.
- The protected tools stay inside the user's role: a viewer never gets a transcript.
- Synthetic calls are not protected data. Their transcripts are open to the PHI-free tools.

**Open:** how the staff API knows that a call comes from the registered client and not
another one. An OAuth client id alone can be copied by a public client. Options to study when
this is built: a confidential client with its own key, or the vendor's own connector
registration for the hospital's organization.

**When built,** SECURITY.md §4 changes in the same commit: today it lists "an agent / MCP
route" as a place protected data never goes, with no exception.

## 5. What the tools do
### 5.1 The CLI first, for us
In Claude Code a shell is enough, so the CLI comes before the MCP server. A draft of the
commands:
```
deets calls --since 1d --outcome verify-failed
deets call <call-id> --timeline --ledger
deets replay <call-id>
deets demo --persona <name>
deets settings get
```
Until the staff API exists, the CLI can read the local dev pieces: the dev call record
(AGENT.md §1) and the text call harness (`agent/harness/typed_call.py`).

To debug the console's own UI, no new tool is needed: Claude in Chrome drives the console at
localhost, reads the page and reads its console errors.

### 5.2 The MCP server later, for analysts
A remote MCP server in our deploy (Streamable HTTP), not a local one: the hospital installs
nothing (CONSOLE.md §2). It wraps the same calls as the CLI. A draft of the tools, for the
owner to edit:

| Tool | What it does | Role |
|---|---|---|
| `list_calls` | Calls by time, outcome, Flow node | viewer |
| `get_call` | One call's row, timeline and EHR ledger | viewer |
| `call_stats` | Counts and latencies, grouped | viewer |
| `replay_call` | Re-run a synthetic call from its cached replies | supervisor |
| `get_transcript` | A call's transcript (only when §4 is on) | supervisor |
| `get_settings` | The hospital's settings (FUTURE-SETTINGS.md) | admin |
| `draft_setting_change` | Makes a draft change (§5.3) | admin |
| `start_demo_call` | A call with a synthetic caller | supervisor |
| `audit_trail` | The audit rows | auditor |

### 5.3 Setting changes are drafts
A tool never changes a setting directly. It makes a **draft change**, and a person with the
admin role applies it in the console. Some settings control the patient's safety (the
verification floor, SECURITY.md §4.4; the emergency phrase list, FUTURE-SETTINGS.md). We
cannot make every MCP client ask its user to confirm before a write. A draft that a person
approves in the console keeps the edit safe in every client.

### 5.4 Demos and trainings
Synthetic callers only (VOICE-AGENT.md §9), and the call says so. Each demo call has a token
cap, and the hospital sees what each one spent. This joins the live demo for an analyst team
(ideas/README.md).

## 6. Forks
- **1. Protected data through MCP:** A off by default, a hospital turns it on for one client
  its BAA covers (decided 2026-11-12) · B never.
- **2. How analysts reach the tools:** A an external MCP server and, later, an assistant in
  the console (decided 2026-11-12) · B the external MCP server only · C the in-console
  assistant only.
  - **The in-console assistant waits** (the owner: hold off on it unless we find it useful).
    If built, its requests go to the Anthropic API under our own BAA, it uses the same tools,
    and it can read transcripts inside the user's role, each view audited. It spends our
    tokens.

## 7. Before it is built
- The staff API, with single sign-on and the roles (SECURITY.md §4.10).
- The call log and the call timeline (DEBUGGING.md §2).
- Record and replay (VOICE-AGENT.md §9).
