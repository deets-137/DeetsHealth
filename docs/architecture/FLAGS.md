---
status: project
desk_test: closed 2026-11-22
sources: [agent/deets_agent/flow.py, agent/deets_agent/screen.py, agent/deets_agent/call.py, agent/deets_agent/catalog.py, agent/prompts/AGENT-PROMPT.md]
updated: 2026-11-16
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

### 3.1 A visit reason to review (decided, not built)
> **Part:** built · 2026-11-14 · desk test closed 2026-11-22 — the owner, after the Priya Raman test call
> (`agent/calls/20261113-232112-test.jsonl`): fork 1A, "silent flag we can throw for nursing /
> clinical staff to review and call back for". Built: the kind in `FLAG_KINDS`, the rule in
> `task.menu`; the kept script `booked_reason_asked.txt`.
- **The agent books the visit as the caller said it,** whatever the reason ("strawberry water
  distillation" in that call). It never asks the caller to justify a reason, so no caller with
  a rare but real need hears doubt.
- **When the reason does not read as a health need, it raises a flag, silent to the caller,**
  for nursing or clinical staff to review and call back. The evidence is the caller's own words
  for the reason, checked by code as for every flag (§2).
- Kind name: a draft, `visit_reason_review`, in the kinds list (§3) for the owner to edit.
- Whether a reason "reads as a health need" is the model's call; nothing in code judges it. The
  booking never waits on the flag.
- Built with the booking work that carries the reason into the visit (EHR-SERVICE.md §4, the
  owner's 1A of 2026-11-13).

### 3.2 No guessed pronouns in a summary (decided, not built)
> **Part:** built · 2026-11-14 · desk test closed 2026-11-22 — the owner's option A, after the Priya Raman
> call, whose hand-off summary said "She asks…" from the name alone. Built:
> `guesses_a_pronoun`, `CallFlow._summary_back` (`flow.py`), the rule in `role`,
> `note.summary_pronoun`; tests `test_a_summary_that_guesses_a_pronoun_goes_back_once`
> (`test_gate.py`) and `test_booked_reason_asked` (`test_typed_call.py`).
- A flag's or a hand-off's summary names the caller by name or as "the caller", never "he",
  "she", "him", "her", "his" or "hers". ehr reads no gender, sex or pronoun field for it: staff
  see the chart's own pronouns in the EHR, and no gender identity data goes to the model.
- Code checks each summary for those words. A summary that has one goes back to the model once
  with a fixed instruction to rewrite it (`note.summary_pronoun`); nothing is spoken and no flag
  is raised until it comes back. Each tool gets one chance per call; a second summary with a
  pronoun is kept as it is, so the call never loops. The caller hears no extra words; the
  hand-off's own line waits one model turn (about a second). The quotes in the evidence are
  the caller's own words and are never checked for pronouns.

### 3.3 The visit's CSN in a flag about a visit (built)
> **Part:** built · 2026-11-14 · desk test closed 2026-11-22 — `StaffFlag.csn`, `change_visit`'s `visit`
> argument (`flow.py`); the CSN from `Visit.csn` (`ehr.proto`). Designed — the owner: "include the identifier / CSN in flags for nurses
> as applicable (ie changing/cancelling, notifying, etc)". EPIC.md §7.8.
- **Why:** staff find the visit at once in the EHR, with no search by name and date.
- **Which flags:** a flag about one visit. Today that is `booking_change`, raised by code on
  `change_visit`. Later: a cancel, and a caller who says they will be late or cannot come (the
  kinds are the owner's to name, §3).
- **Which visit:** the one this call booked (`last_booked`), else the last one `find_visit` read, or the one whose number the model gives `change_visit`. Code
  picks it. The model never sees a CSN and never names a visit.
- **Where it goes:** in the flag's notes, `Shielded`, beside the summary and the evidence. The
  CSN is an identifier, so it is protected data (§4): never in the kind, a toast or a log line.
- **Needs:** `ehr` reads `Appointment.identifier` and sends it with the visit (EPIC.md §7.3).
  The fake gives each visit a synthetic CSN.

### 3.4 The Booking rules' flags (built 2026-11-16)
> **Part:** built · 2026-11-16 · desk test closed 2026-11-22 — BOOKING-RULES.md §5.5. Both kind names are
> drafts for the owner to edit.
- **`referral_check`**, by code, when the caller books a time whose Booking rule says Book and
  flag: staff check the referral before the visit. It carries the visit's CSN (§3.3); no
  summary, no evidence. The caller hears nothing.
- **`booking_rule_hand_off`**, by code, when a Booking rule says Hand off: the call goes to a
  person. Its evidence is the caller's reason, kept only if the caller said it (§2).

## 4. Protected data
- **The kind leaves the wall; the notes do not.** An alert, a toast, a log line or a catalog
  label names the kind only ("a possible emergency on line 3"). The summary and the evidence
  hold names and symptoms, so they are protected data.
- **In memory:** `StaffFlag` holds the summary and the evidence as `Shielded`.
- **In test mode:** the dev call record keeps them in full (`StaffFlag.test_record`), like the
  rest of a test call (REGULATIONS.md §8). The catalog's label is the kind only.
- **In the store (built for synthetic calls, 2026-11-22):** sealed, like a transcript; read in the
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
and the confused caller's hand-off (`verification_failed`). Desk test closed 2026-11-22.

## 7. Open
- The kinds list: the owner's to edit (§3).
- The nurse's view of a flag and their controls (FUTURE-SETTINGS.md §5). An analyst's view is built: Analytics › Flags counts flags by kind and source, and shows a flag's summary and evidence with PHI View, each read audited (`flag.view`; ANALYTICS.md §3.2, 2026-11-22).
- Whether the model over-flags. The kept scripts show a flag in most calls that go wrong; a
  measure across simulated callers comes with the voice work.

## 8. Review flags (for an analyst or an engineer)
> **Part:** built · 2026-11-12

A review flag marks a call for the people who build and tune the agent, not for a nurse. Code
raises it; the model never does. It carries a reason, never a value.
- **`token_cap`** (the owner, 2026-11-12): a reply ran through the token cap with nothing to
  say. The caller hears the holding line (`line.one_moment`), and the request goes once more
  with the retry cap. `final` is true when the retry was cut off too. Code: `_CutOffGuard` in
  `agent/deets_agent/llm.py`; `on_cut_off` in `TypedCall` (`agent/deets_agent/call.py`); the caps:
  `MAX_TOKENS`, `RETRY_MAX_TOKENS` (`agent/deets_agent/config.py`).
- **`voice_failed`** (2026-11-16): Sonic was marked unusable during a voice call (the voice of
  the call of 2026-11-15, 22:38, was gone from Cartesia), so the caller heard nothing. The call
  ends at once. Code: `serve` in `TypedCall` (`agent/deets_agent/call.py`). Since 2026-11-23 the
  hearing service raises it too, and so does a vendor's refusal (auth, quota, retention), not only
  an unusable service (`voice_failure`; VOICE-VENDORS.md §11.3).
- **Where it goes:** a `review_flag` event in the call record, and a `review_<reason>` label in
  the catalog. In production, a review queue that only roles with transcript access can open,
  each view audited (SECURITY.md §4.10).
