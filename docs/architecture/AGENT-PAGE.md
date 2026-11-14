---
status: built
desk_test: open
sources: [console/src/pages/agent.ts, console/src/pages/system.ts, console/src/styles/config.css, agent/prompts/GUIDE.md, agent/deets_agent/flow.py, agent/deets_agent/call.py, agent/deets_agent/prompts.py, agent/deets_agent/dev_server.py, agent/tests/test_prompts.py, agent/prompts/AGENT-PROMPT.md, agent/prompts/TOOLS.md, docs/architecture/images/system-test-variable-2026-11-13.png]
updated: 2026-11-14
---
# DeetsHealth — Config › Agent, arranged by the call

The owner, 2026-11-13, after a desk test in which `{test}` in "Did not hear clearly" never
played: "I evidently got confused and I imagine an analyst most definitely would." The page
said what each section says, never when it plays. This design arranges the sections by the
call's steps, puts each line under the tool that speaks it, and gives every section a "Plays
when" line. It replaces CONSOLE.md §9's five tabs.

**Terms.** A **section** is one block of AGENT-PROMPT.md (78). A **step** is a node of the call
(`flow.py` › `TOOL_STEPS`). A **line** is a fixed line the caller hears. A **field** is one
input of a tool (`arg.*`). The **reader** is a hospital IT analyst, new to building an agent.

## 1. What the owner decided (2026-11-13)
> **Part:** designed · 2026-11-13
- **2A:** the tabs are the call's steps; each line sits under the tool that speaks it.
- **The tab for lines no tool speaks is "Failure".** Every tab name is one word.
- **Semantic | Flow stays**, to frame the page for two readers: one new to this, one a power
  user. "Imagine a hospital IT vet who isn't particularly tech savvy and building with an agent
  for the first time."

## 2. The tabs
`Greet · Verify · Menu · Schedule · Anytime · Failure`, in the call's order.
- **Greet** — from the call's start until the code is sent.
- **Verify** — until the caller is verified.
- **Menu** — what the verified caller wants.
- **Schedule** — choosing and booking a time.
- **Anytime** — the role prompt and the seven tools the model has at every step.
- **Failure** — the lines no tool chooses: the scheduling system refused or did not answer, or
  a reply was cut off.

A tool offered in two steps sits once, in its first step, and the other step shows a one-line
link to it ("Find open times · in Menu"). One section, one place to edit.

## 3. Semantic and Flow
The same sections in each tab, read two ways. The bar stays at the right of the tab bar.

**Semantic (the default: for the reader new to this).** Three plain headings, by what a
section is:
- **What the agent is told** — the step's instructions, and the notes it reads after a tool.
- **What the agent can do** — each tool, by its plain name, with its fields indented under it.
- **What the caller hears** — each line, and the text message.

Every section shows its name and, under it, one grey line: **Plays when …** (§5). No keys, no
tool names, no tags.

**Flow (for the power user).** The step as the code runs it: the step's instructions first,
then each tool as a block: the tool's words, its fields, then every line and note it can
produce, in the order of its outcomes. The block's head shows the tool's name as a chip
(`book`); each section's key shows under its name (`line.booked`). "Plays when" stays.

```
Semantic, Schedule tab                         Flow, Schedule tab
─────────────────────────────────────────      ─────────────────────────────────────────
What the agent is told  1                      Step 4: choose and book      task.schedule
  Step 4: choose and book    [words…]            Plays when: …               [words…]
    Plays when: after open times are offered   ┌ `choose_option` Read back a time
What the agent can do  2                       │   Read back a time         tool.choose_option
  Read back a time           [words…]          │   Option number            arg.….option
    Option number            [words…]          │   Which time?              line.which_option
  Book the time              [words…]          │   Confirm the time         line.confirm_slot
What the caller hears  4                       └
  Which time?                [words…]          ┌ `book` Book the time
  Confirm the time           [words…]          │   Book the time            tool.book
  Time just taken            [words…]          │   Time just taken          line.slot_taken
  Booked                     [words…]          │   Booked                   line.booked
                                               └
```

## 4. What follows (one option was left each time)
1. **"Plays when" lives with the names**, in one file the page and the tests read
   (`agent/prompts/GUIDE.md`, the shape of TOOLS.md: per section its tab, its name, its tool and
   its "plays when"). The page's `SECTIONS` list in `agent.ts` goes; a test checks that every
   section has an entry and that each one's tab and tool agree with `tool_scope`.
2. **Read | Edit, versions, Record and test, the `{` and " ." lists, the problem lines and the
   glow** stay as built (CONSOLE.md §9, SYSTEM-PAGE.md).
3. **A tab with a problem or a change shows its dot**, as now.
4. **The remembered tab** maps from the old names: General → Anytime, Open and close → Anytime,
   Verify → Verify, Visits → Menu, Safety → Anytime; unknown → Greet.
5. **Config › System's links** ("Used in", "Says", "Its words") open the right tab, as now.

## 5. Draft: every section's place, name and "plays when" (for the owner to edit)
> **Part:** designed · 2026-11-13 — a draft list; the names are today's unless marked *new*.

**Greet**
| Section | Name | Under | Plays when |
|---|---|---|---|
| `line.greeting` | Greeting | — | The call starts. Spoken before the model runs. |
| `task.greet` | Step 1: name and date of birth | — | The model reads it from the call's start until the code is sent. |
| `tool.identify` | Read back details | — | The model calls it when the caller has said a full name and a date of birth. |
| `arg.identify.given_name` | First name | identify | The model fills it from the caller's words. |
| `arg.identify.family_name` | Last name | identify | The model fills it from the caller's words. |
| `arg.identify.birth_date` | Date of birth | identify | The model fills it from the caller's words. |
| `line.birth_not_possible` | Date of birth not real | identify | The date cannot be true (before 1900 or in the future). No try is spent. |
| `line.name_not_said` | Full name not heard | identify | The model sent a name the caller did not say. No try is spent. |
| `line.read_back` | Read back details | identify | Both checks pass; the caller hears the details and says yes or no. |
| `tool.confirm_details` | Look up and send code | — | The model calls it when the caller says yes to the read-back. |
| `line.ask_details` | Ask for details | confirm_details | Nothing was read back yet. |
| `line.code_sent` | Code sent | confirm_details | The details went to the record. The same words whether or not a patient matched. |
| `text.code` | Code text | confirm_details | Sent by text to the number on file, when exactly one patient matches. |

**Verify**
| Section | Name | Under | Plays when |
|---|---|---|---|
| `task.verify` | Step 2: check the code | — | The model reads it after the code is sent, until the caller is verified. |
| `tool.check_code` | Check the code | — | The model calls it when the caller reads a code. |
| `arg.check_code.code` | Code | check_code | The model fills it with the digits the caller read. |
| `line.code_length` | Code not 6 digits | check_code | The code is not 6 digits. No try is spent. |
| `line.code_wrong` | Code does not match | check_code | The code is wrong, and tries are left. |
| `line.locked` | Cannot verify | check_code | Too many wrong codes. The call goes to a person. |
| `line.verified` | Verified | check_code | The code is right. The chart and booking open. |
| `tool.resend_code` | Send a new code | — | The model calls it when the caller asks for a new code. |
| `line.code_resent` | New code sent | resend_code | A new code went out. |
| `line.no_more_codes` | No more codes | resend_code | No code is left to send. The call goes to a person. |
| (link) | Read back details · in Greet | — | — |

**Menu**
| Section | Name | Under | Plays when |
|---|---|---|---|
| `task.menu` | Step 3: what the caller needs | — | The model reads it once the caller is verified, until they choose a time. |
| `tool.next_visit` | Tell the next visit | — | The model calls it when the caller asks about their next visit. |
| `line.no_visits` | No visits coming up | next_visit | No visit is coming. |
| `line.next_visit` | Next visit | next_visit | The next visit: its time, clinician, place and length. |
| `line.visit_online` | Online visit (part) | next_visit | Added after Next visit when the visit is online. |
| `line.visit_type` | Visit type (part) | next_visit | Added when the visit is in person and the record names its type. Never for a private visit. |
| `line.visit_reason` | Visit reason (part) | next_visit | Added when the record holds a reason. Never for a private visit. |
| `note.after_next_visit` | After the next visit | next_visit | Sent to the model after the visit is spoken. Never heard. |
| `tool.find_slots` | Find open times | — | The model calls it when the caller wants to book, or another day. Also in Schedule. |
| `arg.find_slots.reason` | Reason for the visit | find_slots | The model fills it from the caller's words. |
| `arg.find_slots.earliest_day` | Earliest day | find_slots | The model fills it when the caller names a day. |
| `line.slots` | Open times | find_slots | Open times were found. Holds each "One open time". |
| `line.slot_option` | One open time (part) | find_slots | One time, inside Open times. |
| `line.no_slots` | No open times | find_slots | No open time from that day. |
| `tool.change_visit` | Change or cancel a visit | — | The model calls it when the caller asks to move or cancel a visit. The call goes to a person, with a flag. Also in Schedule. |
| `line.change_booked` | Visit stays booked | change_visit | A visit was booked on this call: the caller hears it stays booked. |
| `line.change_visit` | Change or cancel a visit | change_visit | No visit was booked on this call. |

**Schedule**
| Section | Name | Under | Plays when |
|---|---|---|---|
| `task.schedule` | Step 4: choose and book | — | The model reads it after open times are offered. |
| `tool.choose_option` | Read back a time | — | The model calls it when the caller picks an option number. |
| `arg.choose_option.option` | Option number | choose_option | The model fills it with the number the caller chose. |
| `line.which_option` | Which time? | choose_option | The number is not one the caller heard. Book says it too, when no time was read back. |
| `line.confirm_slot` | Confirm the time | choose_option | The chosen time is read back for a yes. |
| `tool.book` | Book the time | — | The model calls it when the caller says yes to the read-back time. |
| `line.slot_taken` | Time just taken | book | Someone else took the time first. |
| `line.booked` | Booked | book | The visit is booked. |
| (links) | Find open times · Change or cancel a visit · in Menu | — | — |

**Anytime**
| Section | Name | Under | Plays when |
|---|---|---|---|
| `role` | Role and rules | — | The model reads it at every step. |
| `tool.end_call` | End the call | — | The model calls it when the caller says goodbye or has nothing else. |
| `line.goodbye` | Goodbye | end_call | The call ends after it. |
| `tool.hand_off` | Connect to a person | — | The model calls it when the caller asks for a person, says yes to an offer, or the code did not work. |
| `arg.hand_off.kind` | Reason for a person | hand_off | The model chooses a kind of flag. |
| `arg.hand_off.summary` | Summary for staff | hand_off | The model writes it for staff. |
| `arg.hand_off.evidence` | Caller's words for staff | hand_off | The model quotes the caller; the code keeps only words the caller said. |
| `line.hand_off` | Connecting to a person | hand_off | Any hand-off but one after the code did not work. |
| `line.hand_off_code` | Code does not work | hand_off | A hand-off because the code did not work. |
| `tool.offer_staff` | Offer a person | — | The model calls it when it cannot do or see what was asked. |
| `arg.offer_staff.why` | Reason for the offer | offer_staff | Cannot help, or private. |
| `line.offer_staff` | Cannot help, offer a person | offer_staff | The agent cannot do or see what was asked. |
| `line.offer_staff_private` | Private reason, offer a person | offer_staff | The record holds it, but it is private. |
| `tool.emergency` | Emergency | — | The model calls it when the caller may be in danger. |
| `line.emergency` | Call 911 | emergency | The emergency tool; the phrase screen hearing an emergency phrase; or a 911 reminder before any warning. No one can cut it off. |
| `tool.safety_reminder` | 911 reminder | — | The model calls it when the same danger comes up again. |
| `line.emergency_reminder` | 911 reminder | safety_reminder | The caller has heard the full 911 line already. |
| `note.after_reminder` | After the 911 reminder | safety_reminder | Sent to the model after the reminder. Never heard. |
| `tool.medical_question` | Medical question | — | The model calls it when the caller asks for medical advice. |
| `line.medical_question` | Cannot answer medical questions | medical_question | Every time the tool is called. |
| `tool.flag_for_staff` | Flag for a nurse | — | The model calls it to mark the call for staff. Silent to the caller. |
| `arg.flag_for_staff.kind` | Kind of flag | flag_for_staff | The model chooses a kind. |
| `arg.flag_for_staff.summary` | Summary for the nurse | flag_for_staff | The model writes it for staff. |
| `arg.flag_for_staff.evidence` | Caller's words for the nurse | flag_for_staff | The model quotes the caller; the code keeps only words the caller said. |
| `note.after_flag` | After a flag | flag_for_staff | Sent to the model after a flag. Never heard. |

**Failure**
| Section | Name | Under | Plays when |
|---|---|---|---|
| `line.one_moment` | Reply cut off *(new)* | — | The model's reply ran past its length cap. The agent tries again after it. |
| `line.say_again` | Could not use a value *(new)* | — | The scheduling system refused a value as unreadable (a date it cannot read). Not for speech the agent did not catch: the model says that in its own words. |
| `line.not_on_this_call` | Not allowed on this call *(new)* | — | The scheduling system says this call may not do that (not verified, or the call closed). The call goes to a person. |
| `line.not_now` | Refused for now *(new)* | — | The scheduling system refused for now (too many tries). The call goes to a person. |
| `line.ehr_down` | Scheduling system down | — | The scheduling system did not answer. The call goes to a person. |

Counts: Greet 13, Verify 10, Menu 17, Schedule 8, Anytime 25, Failure 5 = 78.

## 5a. As built and the desk test (2026-11-13)
> **Part:** built · 2026-11-13
- **The guide** is `agent/prompts/GUIDE.md` (§5's table, as a file): `prompts.py ›
  load_guide`, sent by `/agent/prompt` and `/agent/system` as `guide`; `/agent/prompt`'s tools
  also carry their `steps`. `agent.ts` reads it; its old `SECTIONS` list is gone.
  `test_the_guide_places_every_section_once_where_the_code_runs_it` holds every section to one
  entry, each tool and what sits under it to the tool's first step, each task to its step, each
  note to its tool.
- **Semantic's headings:** What the agent is told · What the agent can do · What the caller
  hears (the text message is under the last). The links to a tool in another step ("Also
  here: Find open times · in Menu") sit under "What the agent can do", and at the end in Flow.
- **Flow:** sections with no tool first, in GUIDE.md's order; then a block per tool (a bar at
  its left; its chip and name at its head). Each section shows its key under its name.
- **The title alone says what the row is; the rest is under the "i"** (the owner, 2026-11-13,
  after seeing it). Each section has a Config row's "i" before its name; it rolls down "Plays
  when", the code values it keeps ("Keeps: {visit}") and, in Flow, its key. The titles were
  made unique and plain (a tool by what it does, a line by when the caller hears it): GUIDE.md
  is now the list, and §5's names are the first draft. A test refuses two alike.
- **Kept:** Read | Edit, the versions, Record and test, the `{` and " ." lists, the problem
  lines, the dots on the tabs, the glow, System's links.

**Desk test.** Restart `npm run dev` first (the dev server reads GUIDE.md).
1. Config, Agent: the tabs read Greet · Verify · Menu · Schedule · Anytime · Failure; the page
   opens on the tab it last showed (Visits → Menu, the other old groups → Anytime).
2. Failure, Semantic: "What the caller hears 5". Each row: an "i", the title alone, the words.
   Press the "i" of Could not use a value: "Plays when: …" rolls down; press again, it rolls up.
   It works in Read and in Edit. No keys, no chips.
3. Schedule, Flow: Schedule instructions first, then a block `choose_option` Read back the
   chosen time (Option number, Unknown option, Chosen time read back) and a block `book` Book
   the time. An "i" here also shows "Key: …". At the end: "Also here: Find open times · in Menu
   · Hand a change to staff · in Menu". Press one: Menu opens on that tool, and it glows once.
4. Verify: "Also here: Read back the details · in Greet · Look up and send a code · in Greet".
   Menu › Next visit's "i" shows "Keeps: {visit}".
5. Edit a line in Menu so it fails (delete `{visit}` from Next visit): Menu's tab shows the red
   dot; Save stays off and its hint names "Menu › Next visit".
6a. Narrow the window below one Cartesia card: the "i" and the title sit on one line, the words
   under them, the help under the words.
6. Config, System, a variable's "Used in" link: Agent opens on the right tab, the section glows.
7. Change a "plays when" line in GUIDE.md and restart: the page shows the new words.

**Three new sections (2026-11-14, the booking work; EHR-SERVICE.md §4).** No page code changed:
the page lists `PLACEHOLDERS` and reads GUIDE.md, so each new section needed only its words, its
two list entries (`prompts.py`, `ehr/src/settings.rs`) and its GUIDE.md entry
(PROMPT-EDITOR.md §9). Where each shows: Menu › Find open times › **In person or online**
(`arg.find_slots.visit_mode`); Menu › Tell the next visit › **Next visit, just booked**
(`line.booked_visit_asked`); Anytime › Flag for a nurse › **Note when a summary guessed a
pronoun** (`note.summary_pronoun`). Two existing sections gained a rule: Anytime › Role and
rules (no guessed pronoun in a staff summary) and Menu › Menu instructions (an odd reason is
booked and flagged). Desk test: open each and check its "i".

## 6. Every place it runs
The console only (a browser). No change to the call, the store, `ehr` or any EHR: the
sections' keys and words are the same; only where the page shows them changes.

## 7. Not now
- A section for the model's own "didn't catch that" (today it is the model's words, not a
  line). It would be a new fixed line and a new rule; its own design.
- Searching sections by name or words.
