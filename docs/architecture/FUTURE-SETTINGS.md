---
status: project
desk_test: none
sources: [agent/deets_agent/flow.py]
updated: 2026-11-12
---
# Future settings — what a hospital will configure

The agent's behavior that each hospital (or clinic) will set for itself. Today each one is a
fixed default in code. Each entry says what it controls, what the default is now, the choices
a hospital needs, and where the value will live. None of this is built: the settings store
waits on Postgres (VOICE-AGENT.md §8.7), and the console page to edit them waits on the
console's design (BUILD-ORDER.md, Phase 4). The global values that DeetsEngine carries are
drafted in [GLOBALS.md](../archive/GLOBALS.md).

**How this list works** (from DeetsMusic's ledger of the same name, the owner, 2026-11-12):
- **One central list.** Every setting a hospital will configure is on the index below, built
  or not. A setting that another doc names goes on the index the day that doc names it, with
  a link to the place it is described.
- **A full entry when its default is built.** It gets a numbered section with what it
  controls, the default now, the choices a hospital needs, and every place it runs. Its
  index line then points to that section.
- **Section numbers are stable.** Code comments and other docs refer to them (`flow.py`,
  AGENT.md, VOICE-AGENT.md). A new entry goes at the end with the next number. Never
  renumber. Read the index by topic, not by number.
- **When the settings store is built,** a SETTINGS.md becomes the live table of what is
  built, and the entries here stay as the design record.

## The index, by topic
> **Part:** project · 2026-11-12

**Safety on the call**
- After the emergency line, and the emergency flag's actions — §1 (default built).
- The medical-question line — §2 (default built).
- The emergency phrase list — §3 (default built).

**Caller verification**
- Stronger checks above the floor: a code on every call, a call back only. Never below two
  factors for a chart answer. Not built (SECURITY.md §4.4).
- The random chart questions to ask, from a list whose questions reveal nothing. Not built
  (SECURITY.md §4.4).

**What the agent may say**
- The sensitive-results list: a hospital may add to it, never remove from it. Not built
  (VOICE-AGENT.md §7.1, DESIGN.md K2).
- The pronunciation dictionary: drug names, provider names, the hospital's own place names.
  Not built (CARTESIA.md).

**Booking**
- Hours, visit types and transfer numbers. Not built (DESIGN.md K2).
- Which providers and locations are open to booking. Not built (OTHER-EHRS.md).

**Records kept**
- Retention: transcripts (30 days, a draft), recordings, the call log's plain columns, within
  the law. The audit trail stays 6 years. Not built (SECURITY.md §4.7, VOICE-AGENT.md §8.7).
- Call audio: off by default; on for the whole hospital or by rule (a share of calls,
  hand-offs and errors, one line or Flow node, a debug window that ends by itself). Not built
  (VOICE-AGENT.md §8.7).

**Access and keys**
- The staff roles: custom, named by the hospital, built from levers. Open ended; the owner
  designs it. Not built (ideas/ACCESS-ROLES.md; SECURITY.md §4.10).
- Bring your own key: the hospital holds its key and may revoke it. Not built (SECURITY.md
  §4.6).
- Emergency support access for our staff, which the hospital approves. How it approves is
  open. Not built (SECURITY.md §4.10, §7).

**Ideas only** (not planned; see ideas/README.md)
- Protected data through MCP, for one client the hospital's BAA covers. Off by default
  (ideas/CONSOLE-MCP.md §4).
- The token cap for each demo call (ideas/CONSOLE-MCP.md §5.4).

## 1. After the emergency line
> **Part:** built (the default only) · 2026-11-11

**What it controls:** what the agent does after it tells a caller to hang up and call 911.

**The default now** (the owner's option A, 2026-11-11): the agent speaks a fixed line ("If
this might be an emergency, please hang up and call 911 now. If you're safe to go on, would
you like me to keep helping with your call?"). The call stays open, and the task goes on: a
caller who was only worried keeps the time they chose. Code: `flow.py` `emergency_fn`.

**The choices a hospital needs:**
- **Stay and go on** (the default).
- **Transfer to a nurse line** after the 911 line, where the hospital staffs one (C5,
  emergency transfer).
- **End the call** after the 911 line, where the hospital's counsel wants the strongest
  message.
- **The words of the line**: the hospital may need its own (a local emergency number outside
  the US, a crisis line such as 988, its own nurse line's name).

**The emergency flag: several things at once** (the owner, 2026-11-11). The 911 line need not
be the only thing that happens. The `emergency` tool can also raise a flag on the call, and
the flag starts the hospital's chosen actions while the agent goes on with the task:
- **Pull in a nurse:** join a nurse to the call (a conference), so the nurse hears the end of
  the booking and takes over at once. This is a warm hand-off: the caller never waits in a
  queue or repeats their story.
- **Transfer when the task ends:** the agent finishes the booking, then transfers at once.
- **Send a message** to the care team or a nurse inbox (an EHR in-basket message, a page).
- **Add a note to the chart** that the caller raised a possible emergency, with the time.
- **Show it in the console:** the call is marked, so staff can listen in or take over.

A hospital chooses one or more of these. The flag carries no protected data out of the wall:
the alert says "a caller on line N raised a possible emergency"; the details stay in the call
record and the chart (SECURITY.md §4). Open: the words that raise the flag (the model decides
today, from the prompt), and whether a staff member must acknowledge the alert.

**Every place it runs:**
- **The call:** a transfer or a joined nurse needs Twilio conferencing and a staffed number.
- **Each EHR (actions):** a chart note and an in-basket message are EHR writes (Epic first:
  `Communication` or a note, depending on what the hospital allows).
- **The console:** one control on the hospital's settings page, with an audit entry for each
  change.
- **The deploy:** A2 (our cloud) and B (the hospital's cloud) read it from the settings store
  the same way.
- **Each EHR (the line):** the line itself needs no EHR data.

## 2. The medical-question line
> **Part:** built (the default only) · 2026-11-11

**What it controls:** what the agent says when a caller asks a medical question, and what
follows.

**The default now** (the owner's option 1A, 2026-11-11): the `medical_question` tool speaks a
fixed line ("I can't answer medical questions, but a clinician can talk with you about that at
your visit. Shall we go on with your visit?"), and the task goes on. Code: `flow.py` `medical_question_fn`.

**The choices a hospital needs:** the words; an offer to transfer to a nurse line, where the
hospital staffs one; a message to the care team that the caller asked (an EHR in-basket
message), so a clinician sees the question before the visit. Never a symptom script unless the
hospital supplies one it has approved.

**Every place it runs:** the call (a transfer needs Twilio and a staffed number); the console
(the words and the follow-up, audited); the deploy (A2 and B the same); each EHR (only the
in-basket message writes to it).

## 3. The emergency phrase list
> **Part:** built (the default only) · 2026-11-11

**What it controls:** the phrases that make the code speak the 911 line itself, before the
model runs (the owner's option 2A, 2026-11-11: code reads the caller's words live, as a point
of customization). Code: `screen.py` `EMERGENCY_PHRASES`.

**The default now:** the English list below. It fires once per call. Each entry is a pattern
matched against the caller's line in lower case (curly apostrophes made straight); `\b` marks a
word edge, `'?` an apostrophe that may be missing, `(a|b)` one of several words. **This list
is checked against the code:** `test_screen.py` fails if it and `screen.py` differ, so change
both together.

- `\bchest pains?\b` — chest pain, chest pains
- `\bheart attack\b` — heart attack
- `\b(can'?t|cannot|can not|couldn'?t) breathe?\b` — I can't breathe, couldn't breathe
- `\b(trouble|difficulty|hard time) breathing\b` — trouble breathing, a hard time breathing
- `\bnot breathing\b` — he's not breathing
- `\bam i dying\b` — am I dying
- `\bi'?m dying\b(?! to\b)` — I'm dying (but not "I'm dying to get in")
- `\bgoing to die\b` — I'm going to die
- `\boverdos(e|ed|ing)\b` — overdose, overdosed, overdosing
- `\bkill(ing)? myself\b` — kill myself, killing myself
- `\bend my life\b` — end my life
- `\bwant to die\b` — I want to die
- `\bsuicid(e|al)\b` — suicide, suicidal
- `\b(having a |had a )?stroke\b` — a stroke (also "a stroke of luck": over-calling is the safe side)
- `\bseizures?\b` — a seizure, seizures
- `\bunconscious\b` — unconscious
- `\bpassed out\b` — she passed out
- `\b(bleeding a lot|heavy bleeding|won'?t stop bleeding)\b` — heavy bleeding, it won't stop bleeding
- `\b911\b` — should I call 911

**The choices a hospital needs:** phrases to add or remove; a list for each language it
serves; whether a match also raises the emergency flag (§1); whether it fires more than once
per call.

**Every place it runs:** the call (typed text today; speech-to-text words in Phase 3); the
console (the list, audited, with a test box that shows what a line would match); the deploy
(A2 and B the same); each EHR (none).

## 4. Open questions
- Which settings a clinic inside a hospital may override, and which stay with the hospital.
- Who in the hospital may change a setting that touches safety (the emergency line), and
  whether a change needs a second person to approve it.
