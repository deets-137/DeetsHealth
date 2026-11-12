---
status: built
desk_test: open
sources: [agent/deets_agent/flow.py, agent/deets_agent/call.py, agent/deets_agent/screen.py, agent/deets_agent/config.py, agent/prompts/AGENT-PROMPT.md, ehr/src/service.rs, ehr/src/session.rs, ehr/src/texts.rs]
updated: 2026-11-12
---
# The default call flow — where each piece lives

The map of one call, as the code runs it today: each step, each fixed line, each limit and each
rule. **This doc points to the code. It never copies a word or a value** (the owner,
2026-11-12: every fixed piece will become a setting that a console page can change, so the
code is the one copy). Each row names a file and a symbol, or a section of AGENT-PROMPT.md. To
read the words, open the file at that symbol.

What a hospital will change, and how: FUTURE-SETTINGS.md. Why each rule exists: SECURITY.md,
REGULATIONS.md. How the parts were built and decided: AGENT.md.

## 1. Terms
- **Node:** one step of the call. Each has its own task and its own tools (Pipecat Flows).
- **Task:** the instructions for the model in one node: `task.<node>` in AGENT-PROMPT.md.
- **Fixed line:** words the code speaks to the caller word for word, never the model:
  `line.<name>` in AGENT-PROMPT.md, spoken by `_say` in `agent/deets_agent/flow.py`.
- **Note:** words for the model only, inside a tool's result, never spoken: `note.<name>` in
  AGENT-PROMPT.md.
- **Global tool:** a tool the model has in every node.
- **Closing node:** a node that ends the call after its fixed line (`CallFlow._closing_node`).

## 2. The steps
The shape, as a drawing: the docstring at the top of `agent/deets_agent/flow.py`.

| Node | Built by (`flow.py`) | Task | Its tools | Leads to |
|---|---|---|---|---|
| greet | `CallFlow.greet` | `task.greet` | `identify`, `confirm_details` | verify, after `confirm_details` |
| verify | `CallFlow.verify` | `task.verify` | `check_code`, `resend_code`, `identify`, `confirm_details` | menu on a pass; hand off on a lock or no more codes |
| menu | `CallFlow.menu` | `task.menu` | `find_slots`, `next_visit` | schedule, after `find_slots` offers times |
| schedule | `CallFlow.schedule` | `task.schedule` | `book`, `find_slots` | menu, after `book` |
| goodbye | `CallFlow.goodbye` | none | none | the call ends |
| hand off | `CallFlow.handed_off` | none | none | the call ends (Phase 1 has no transfer) |

- **Global tools:** the list `global_functions` in `TypedCall.__init__`
  (`agent/deets_agent/call.py`).
- **The role prompt** (every node): `role` in AGENT-PROMPT.md, filled by `role()` in `flow.py`.
- **Each tool's description** for the model: `tool.<name>` and `arg.<tool>.<arg>` in
  AGENT-PROMPT.md, attached by `_tool()` in `flow.py`.
- **How the call starts:** `TypedCall.start` and `TypedCall.serve` open the call in `ehr`
  (`CallFlow.start`) and enter greet.
- **How the call ends:** a closing node runs `end_conversation`. `TypedCall.serve` then
  closes the call in `ehr` (`CallFlow.close`).

## 3. The fixed pieces
| Piece | Where in code | Its words | Setting later |
|---|---|---|---|
| The greeting, with the disclosures | `CallFlow.greet` (its `pre_actions`), `greeting()` | `line.greeting` | FUTURE-SETTINGS.md §4 |
| The goodbye (the disconnect) | `end_call_fn` → `goodbye()` | `line.goodbye` | not yet listed |
| The hand-off (it may carry a staff flag) | `hand_off_fn` → `handed_off()` | `line.hand_off`; its flag fields: `arg.hand_off.*` | the transfer number: FUTURE-SETTINGS.md › Booking |
| A caller for someone else goes to a person | the rule in `role` | — | — |
| A staff flag (silent to the caller) | `flag_for_staff_fn`, `CallFlow.raise_flag`, `FLAG_KINDS` (FLAGS.md) | `tool.flag_for_staff`, `arg.flag_for_staff.*`; the model's next step: `note.after_flag` | FUTURE-SETTINGS.md §5 |
| A hand-off after a lock | `_check_code_fn` | `line.locked` | — |
| A hand-off when no code is left to send | `_resend_code_fn` | `line.no_more_codes` | — |
| When `ehr` refuses or is down (all hand off but `line.say_again`) | `_ehr_trouble` | `line.not_on_this_call`, `line.not_now`, `line.ehr_down`, `line.say_again` | — |
| The emergency line | `emergency_fn`, `EMERGENCY_LINE` | `line.emergency` | FUTURE-SETTINGS.md §1 |
| The safety reminder (after a warning, the same danger again; the call goes on) | `safety_reminder_fn` | `line.emergency_reminder`; the model's next step: `note.after_reminder` | FUTURE-SETTINGS.md §1 |
| The emergency phrase screen | `EMERGENCY_PHRASES`, `CallerScreen` (`agent/deets_agent/screen.py`) | the patterns are in the code, each with its examples | FUTURE-SETTINGS.md §3 |
| The medical-question line | `medical_question_fn` | `line.medical_question` | FUTURE-SETTINGS.md §2 |
| Verification lines | `_identify_fn`, `_confirm_details_fn`, `_check_code_fn`, `_resend_code_fn` | `line.read_back`, `line.ask_details`, `line.birth_not_possible`, `line.name_not_said`, `line.code_sent`, `line.code_length`, `line.code_wrong`, `line.code_resent`, `line.verified` | — |
| The text message with the code | `SECTION` (`ehr/src/texts.rs`) | `text.code` | — |
| Booking lines | `_find_slots_fn`, `_book_fn`, `_next_visit_fn` | `line.slots`, `line.slot_option`, `line.no_slots`, `line.which_option`, `line.slot_taken`, `line.booked`, `line.next_visit`, `line.no_visits` | — |
| The clinic's name | `CLINIC` (`flow.py`) | fills `{clinic}` | the hospital's settings |
| Today's date | `CallFlow.today`; `FAKE_TODAY` for a typed call | fills `{today}` | — |

## 4. Limits and timers
| What | Where in code |
|---|---|
| Tries at name and date of birth, per call | `MAX_IDENTIFIES` (`ehr/src/service.rs`) |
| New codes, per call | `MAX_RESENDS` (`ehr/src/service.rs`) |
| Wrong codes before the lock, per call | `MAX_CODE_FAILURES` (`ehr/src/service.rs`) |
| How long a code works | `CODE_LIFETIME` (`ehr/src/service.rs`) |
| How long the text service may take | `SEND_LIMIT` (`ehr/src/texts.rs`) |
| Open calls in `ehr`, and the longest a call lives there | `MAX_SESSIONS`, `MAX_AGE` (`ehr/src/session.rs`) |
| The earliest date of birth | `EARLIEST_BIRTH` (`flow.py`); `ehr` checks too (`ehr/src/edge.rs`) |
| The pause that ends the agent's turn; the longest turn | `TURN_GRACE_S`, `TURN_LIMIT_S` (`call.py`) |
| The model, its effort, its longest reply | `MODEL`, `EFFORT`, `MAX_TOKENS` (`agent/deets_agent/config.py`) |
| The tools that speak for the model; the tools that end the call | `SILENT_TOOLS`, `CLOSING_TOOLS` (`config.py`) |

The verification limits are a floor: a hospital may make them stricter, never looser
(SECURITY.md §4.4).

## 5. Rules held by code, not by words
| Rule | Where in code |
|---|---|
| A line that ends in a question waits for the caller; any other lets the model go on | `CallFlow._say` |
| The reminder only after the full 911 line; before that, the full line plays | `CallFlow.warned`, set by `emergency_fn` and `CallerScreen`; read by `safety_reminder_fn` |
| After a fixed line that ends in a statement, a note in the tool result tells the model what comes next | `note.*` sections; `safety_reminder_fn` |
| A flag keeps only the quotes the caller really said | `CallFlow._caller_said`; the count of the rest: `StaffFlag.unverified` |
| A code flag of one kind is raised once per call | `CallFlow.raise_flag` |
| The details are read back before the lookup | `_identify_fn`, `_confirm_details_fn` |
| The model may send only names the caller said | `CallFlow._was_said` (the name guard) |
| An impossible date of birth costs no try | `CallFlow._possible_birth` |
| Chart and booking tools exist only after a pass | the node table in §2; `ehr` refuses them too (`ehr/src/service.rs`) |
| The code check is constant time | `check_code` (`ehr/src/service.rs`) |
| A value the model says that no one gave it is flagged | `invented_values`, `TurnCollector._check_values` (`call.py`) |
| The model's words before a silent tool are flagged | `words_before_silent_tool` (`call.py`; `agent/deets_agent/llm.py`) |

## 6. Toward settings
Every row in §3 and §4 is a candidate for a console setting. Each will move the same way: the
code keeps the default, a per-hospital value comes from the settings store, and this doc
keeps pointing at the code that reads it. Which pieces a hospital may change, and who may change
them, is FUTURE-SETTINGS.md §6.
