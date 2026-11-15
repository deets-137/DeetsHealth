---
status: built
desk_test: open
sources: [agent/deets_agent/flow.py, agent/deets_agent/call.py, agent/deets_agent/screen.py, agent/deets_agent/config.py, agent/prompts/AGENT-PROMPT.md, ehr/src/service.rs, ehr/src/session.rs, ehr/src/texts.rs]
updated: 2026-11-15
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
| menu | `CallFlow.menu` | `task.menu` | `find_slots`, `find_visit`, `read_instructions`, `change_visit` | schedule, after `find_slots` offers times; hand off after `change_visit` |
| schedule | `CallFlow.schedule` | `task.schedule` | `choose_option`, `book`, `find_slots`, `change_visit` | menu, after `book`; hand off after `change_visit` |
| goodbye | `CallFlow.goodbye` | none | none | the call ends |
| hand off | `CallFlow.handed_off` | none | none | the call ends (Phase 1 has no transfer) |

- **Global tools:** `CallFlow.global_functions` (`flow.py`), given to the FlowManager in
  `TypedCall.__init__` (`call.py`). `tool_steps()` and `tool_scope()` in `flow.py` read the
  nodes for Config › System and for the tool check (SYSTEM-PAGE.md §3.5).
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
| The offer of a person when the agent cannot help (a yes is `hand_off`, a no goes on) | `offer_staff_fn`, `OFFER_REASONS` | `tool.offer_staff`, `arg.offer_staff.why`, `line.offer_staff`, `line.offer_staff_private`; the rule: `role`, `task.menu` | FUTURE-SETTINGS.md §10 |
| What a visit is: its type, reason, online | `_find_visit_fn`, `_visit_lines`; the screen in `ehr/src/visit_about.rs` | `line.visit_online`, `line.visit_type`, `line.visit_reason`; the model's next step: `note.after_find_visit` | FUTURE-SETTINGS.md §9 |
| The visit is the one just booked: its time is not said again | `_visit_lines`, `_same_visit`, `CallFlow.last_booked` | `line.booked_visit_asked` | — |
| Which visit: the next, one named, or a past one; the narrowing question | `_find_visit_fn`, `_which_visit`; in ehr `visits.rs` › `narrow` (`NARROW_OVER`, `GROUPS_NAMED`) | `line.next_visit`, `line.visit_found`, `line.past_seen`, `line.past_missed`, `line.past_cancelled`, `line.one_more_visit`, `line.more_visits`, `line.no_visit_match`, `line.no_past_visits`, `line.which_visit`; `note.after_which_visit` | FUTURE-SETTINGS.md §13.3 |
| A visit's instructions: offered, or read at once when the caller named the visit | `_find_visit_fn`, `_read_instructions_fn` | `line.offer_instructions`, `line.visit_instructions`, `line.no_instructions`; `note.after_offer_instructions` | FUTURE-SETTINGS.md §13.4 |
| The hospital's time at the call's start | `CallFlow.start`, `clock_note`; ehr's `OpenCallReply.local_now` | `note.call_start` | FUTURE-SETTINGS.md §13.1 |
| In person, online or either (the caller's words) | `_find_slots_fn`, `VISIT_MODES` (`ehr_client.py`); the filter: `VisitMode` (`ehr/src/scheduler.rs`) | `arg.find_slots.visit_mode` | a hospital's visit types: FUTURE-SETTINGS.md › Booking |
| An odd reason: booked as said, flagged for clinical review | `FLAG_KINDS` › `visit_reason_review` | the rule: `task.menu` | FLAGS.md §3.1 |
| A staff summary never guesses a pronoun | `CallFlow._summary_back`, `guesses_a_pronoun` | the rule: `role`; `note.summary_pronoun` | FLAGS.md §3.2 |
| A caller for someone else goes to a person | the rule in `role` | — | — |
| A staff flag (silent to the caller) | `flag_for_staff_fn`, `CallFlow.raise_flag`, `FLAG_KINDS` (FLAGS.md) | `tool.flag_for_staff`, `arg.flag_for_staff.*`; the model's next step: `note.after_flag` | FUTURE-SETTINGS.md §5 |
| A hand-off after the code did not work (the model's `hand_off` with kind `verification_failed`; the same for every caller, matched or not) | `hand_off_fn` | `line.hand_off_code`; the rule: `task.verify` | FUTURE-SETTINGS.md §7 |
| A hand-off after a lock | `_check_code_fn` | `line.locked` | — |
| A hand-off when no code is left to send | `_resend_code_fn` | `line.no_more_codes` | — |
| When `ehr` refuses or is down (all hand off but `line.say_again`) | `_ehr_trouble` | `line.not_on_this_call`, `line.not_now`, `line.ehr_down`, `line.say_again` | — |
| The emergency line | `emergency_fn`, `EMERGENCY_LINE` | `line.emergency` | FUTURE-SETTINGS.md §1 |
| The safety reminder (after a warning, the same danger again; the call goes on) | `safety_reminder_fn` | `line.emergency_reminder`; the model's next step: `note.after_reminder` | FUTURE-SETTINGS.md §1 |
| The emergency phrase screen | `EMERGENCY_PHRASES`, `CallerScreen` (`agent/deets_agent/screen.py`) | the patterns are in the code, each with its examples | FUTURE-SETTINGS.md §3 |
| The medical-question line | `medical_question_fn` | `line.medical_question` | FUTURE-SETTINGS.md §2 |
| Verification lines | `_identify_fn`, `_confirm_details_fn`, `_check_code_fn`, `_resend_code_fn` | `line.read_back`, `line.ask_details`, `line.birth_not_possible`, `line.name_not_said`, `line.code_sent`, `line.code_length`, `line.code_wrong`, `line.code_resent`, `line.verified` | — |
| After the code, no second question when the caller already said what they need | `_identify_fn` (`caller_wants`, `CALLER_WANTS`), `_check_code_fn`, `CallFlow.wants` | `line.verified_go`; `note.after_verified` | FUTURE-SETTINGS.md §13.3 |
| A slow read sent once more, quietly | `_once_more` (`find_visit`, `find_slots`) | — | EPIC.md §7.10 |
| The text message with the code | `SECTION` (`ehr/src/texts.rs`) | `text.code` | — |
| Booking lines | `_find_slots_fn`, `_choose_option_fn`, `_book_fn`, `_find_visit_fn` | `line.slots`, `line.slot_option`, `line.no_slots`, `line.which_option`, `line.confirm_slot`, `line.slot_taken`, `line.booked`, `line.next_visit`, `line.no_visits`; after `book`, the model also gets `visit_is` and `reason_for_visit` | — |
| A change or cancel goes to staff, with a flag; the caller hears which visit stays booked | `_change_visit_fn` | `line.change_booked`, `line.change_visit` | — |
| The holding line when a reply is cut off by the token cap (with a review flag, FLAGS.md §8) | `_CutOffGuard` (`agent/deets_agent/llm.py`), `on_cut_off` (`call.py`) | `line.one_moment` | FUTURE-SETTINGS.md › What the agent may say |
| The clinic's name, and every other System variable | the call's `Words.variables` (`prompts.py`; the store's `system` service, or `agent/prompts/VARIABLES.md`), filled by `prompts.text` | fills `{clinic}` and the analyst's own `{name}`s in any section | Config › System (SYSTEM-PAGE.md) |
| Today's date | `CallFlow.today`; `FAKE_TODAY` for a typed call, the real date for an Epic call (`TypedCall` › `ehr_source`) | fills `{today}` | — |
| The EHR read | `CallFlow.ehr_source` → `OpenCall` › `ehr`; `Session.source` in `ehr` | — | — |

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
| The model, its effort, its longest reply, and the cap for the one retry of a cut-off reply | `MODEL`, `EFFORT`, `MAX_TOKENS`, `RETRY_MAX_TOKENS` (`agent/deets_agent/config.py`) |
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
| The reason sent as the booking's note is the caller's own words: the model's are kept only if the caller said them, else the caller's latest line goes | `CallFlow._reason_words` (uses `_caller_said`) |
| An online time says "online" in its words; an in-person one reads as before | `_visit_words` (`say_online`) |
| A staff summary with he, she, him, her, his or hers goes back to the model once; nothing is spoken and no flag is raised until it comes back | `CallFlow._summary_back`, `CallFlow.summary_retries` |
| The booked visit keeps the caller's reason, through the sensitive screen | `BookNote`, `VisitAbout::booked` (`ehr/src/visit_about.rs`); `Session.note` (`ehr/src/session.rs`) |
| A code flag of one kind is raised once per call | `CallFlow.raise_flag` |
| The details are read back before the lookup | `_identify_fn`, `_confirm_details_fn` |
| The chosen time is read back before it is booked; `book` books nothing without it | `_choose_option_fn`, `_book_fn`, `CallFlow.chosen` |
| An option number keeps its time for the whole call; a new search goes on from the last number | `CallFlow.offered`, `CallFlow._number_for` |
| The model may send only names the caller said | `CallFlow._was_said` (the name guard) |
| An impossible date of birth costs no try | `CallFlow._possible_birth` |
| Chart and booking tools exist only after a pass | the node table in §2; `ehr` refuses them too (`ehr/src/service.rs`) |
| The code check is constant time | `check_code` (`ehr/src/service.rs`) |
| A sensitive visit type or reason never leaves `ehr`; the agent learns only that it is private | `VisitAbout::from_record`, `SENSITIVE` (`ehr/src/visit_about.rs`) |
| A value the model says that no one gave it is flagged | `invented_values`, `TurnCollector._check_values` (`call.py`) |
| The model's words before a silent tool are flagged | `words_before_silent_tool` (`call.py`; `agent/deets_agent/llm.py`) |

## 6. Toward settings
Every row in §3 and §4 is a candidate for a console setting. Each will move the same way: the
code keeps the default, a per-hospital value comes from the settings store, and this doc
keeps pointing at the code that reads it. Which pieces a hospital may change, and who may change
them, is FUTURE-SETTINGS.md §6.
