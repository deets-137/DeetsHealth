---
status: project
desk_test: open
sources: [agent/deets_agent/flow.py, agent/deets_agent/screen.py, agent/deets_agent/call.py, agent/deets_agent/catalog.py, agent/prompts/AGENT-PROMPT.md]
updated: 2026-11-12
---
# Staff flags — the agent marks a call for a person

A staff flag marks a call for a nurse or a member of staff to review and act on. This doc holds
the design, the decisions and the as-built state. What a hospital will configure (kinds,
criteria, routing, the rules engine, who reads the notes) is FUTURE-SETTINGS.md §5. Every word
and value lives in the code; this doc points to it (DOCS-ORG.md §1, rule 8).

## 1. Decisions (the owner, 2026-11-12)
- **Fork 1A — the notes are the model's claim and its evidence.** The model writes a short
  summary, and it gives the caller's own words that support it. The code checks each quote
  against what the caller said. A nurse sees the evidence, not only the model's reading of it.
- **Fork 2A — silent to the caller.** A flag starts work in the background: a rules engine may
  act on a kind, and a nurse may choose to join the call. The caller hears nothing, because a
  promise such as "I've let our nurse know" may not be kept.
- **Both code and the model raise flags.** Code raises flags on events it knows. The model
  raises flags on what only it can hear. A flag that only code could raise could not carry the
  model's notes.

## 2. How a flag is raised
- **By code**, with no model: the 911 line (the screen keeps the caller's line as evidence), a
  locked verification, no code left to send. Each kind once per call. Where: the code-flag row
  in FUTURE-SETTINGS.md §5, and `CallFlow.raise_flag`.
- **By the model:** `flag_for_staff`, which keeps the call going, and `hand_off`, whose optional
  fields carry a flag. A hand-off ends the call, so a flag in a later reply would never run.
  The hand-off flag also tells the staff member why the caller is coming to them.
- **The evidence check:** `CallFlow._caller_said` matches the quote's words, in order, inside one
  caller line; case and punctuation do not count. A quote that fails is dropped and counted
  (`StaffFlag.unverified`), never kept: it is the model's words, not evidence.
- **The caller hears nothing:** the tool speaks no line, and `note.after_flag` tells the model to
  go on and never to mention it.

## 3. The kinds
`FLAG_KINDS` in `agent/deets_agent/flow.py`. A draft list for the owner to edit. An unknown kind
becomes `other`. A hospital will add its own (FUTURE-SETTINGS.md §5).

## 4. Protected data
- **The kind leaves the wall; the notes do not.** An alert, a toast, a log line or a catalog
  label names the kind only ("a possible emergency on line 3"). The summary and the evidence
  hold names and symptoms, so they are protected data.
- **In memory:** `StaffFlag` holds the summary and the evidence as `Shielded`.
- **In test mode:** the dev call record keeps them in full (`StaffFlag.test_record`), like the
  rest of a test call (REGULATIONS.md §8). The catalog's label is the kind only.
- **In the store (not built):** sealed under the hospital's key, like a transcript; read in the
  console only by the roles the hospital allows, with one audit row for each read
  (SECURITY.md §4.10).

## 5. Where a flag goes
- **Today:** `CallFlow.on_flag` writes a `staff_flag` event to the dev call record
  (`agent/deets_agent/call.py`), so the Chat page's watch stream receives it; the catalog labels
  the call `staff_<kind>` (`agent/deets_agent/catalog.py`).
- **Later:** the store, a flag queue in the console, a live alert, and the rules engine's
  actions (a nurse joins the call, an in-basket message, a call-back task). FUTURE-SETTINGS.md §5
  lists them. The Chat page's view of a flag is the console's to design (CHAT.md).

## 6. As built (2026-11-12)
Built in `agent/`: the kinds, code flags, the model's tool, flags on a hand-off, the evidence
check, the record event and the catalog label. Tests: `test_a_flag_keeps_only_what_the_caller_said`,
`test_an_unknown_flag_kind_becomes_other_and_code_flags_raise_once` (`tests/test_gate.py`), and
the kept script `flag_for_someone_else.txt`. In the kept scripts the model also flagged a
medical question (`clinical_question_waiting`), chest pain that still hurt (`safety_concern`),
and the confused caller's hand-off (`verification_failed`). Desk test open.

## 7. Open
- The kinds list: the owner's to edit (§3).
- The console's view of a flag and the nurse's controls (FUTURE-SETTINGS.md §5).
- Whether the model over-flags. The kept scripts show a flag in most calls that go wrong; a
  measure across simulated callers comes with the voice work.
