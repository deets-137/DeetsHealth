# The guide to the agent's words, for Config › Agent

Where each section of AGENT-PROMPT.md shows on Config › Agent, its name there, and when it plays
(AGENT-PAGE.md). The model never reads this file. The owner edits the names and the "plays when"
lines here; a test holds every section to one entry, and each entry's tab and tool to the code
(`flow.py` › `tool_steps`).

Each section: a `## key` heading, then:
- `tab:` Greet, Verify, Menu, Schedule, Anytime or Failure.
- `name:` its title on the page. The title alone says what the row is (the owner,
  2026-11-13); every title is different. A tool's title says what it does ("Book the time");
  a line's says when the caller hears it ("Booked"); a field's names what the model fills in.
- `under:` the tool it belongs to (a field, or a line or note the tool can produce), or `-`.
- `plays:` when it plays, in plain words. The page shows it under the row's "i".

The order here is the page's order: Flow shows it as written; Semantic groups it by kind.

## line.greeting
tab: Greet
name: Greeting
under: -
plays: The call starts. Spoken before the model runs.

## task.greet
tab: Greet
name: Greet instructions
under: -
plays: The model reads it from the call's start until the code is sent.

## tool.identify
tab: Greet
name: Read back the details
under: identify
plays: The model calls it when the caller has said a full name and a date of birth.

## arg.identify.given_name
tab: Greet
name: First name
under: identify
plays: The model fills it from the caller's words.

## arg.identify.family_name
tab: Greet
name: Last name
under: identify
plays: The model fills it from the caller's words.

## arg.identify.birth_date
tab: Greet
name: Date of birth
under: identify
plays: The model fills it from the caller's words.

## line.birth_not_possible
tab: Greet
name: Impossible date of birth
under: identify
plays: The date cannot be true (before 1900 or in the future). No try is spent.

## line.name_not_said
tab: Greet
name: Name not said
under: identify
plays: The model sent a name the caller did not say. No try is spent.

## line.read_back
tab: Greet
name: Details read back
under: identify
plays: Both checks pass: the caller hears the details and says yes or no.

## tool.confirm_details
tab: Greet
name: Look up and send a code
under: confirm_details
plays: The model calls it when the caller says yes to the read-back.

## line.ask_details
tab: Greet
name: No details yet
under: confirm_details
plays: Nothing was read back yet.

## line.code_sent
tab: Greet
name: Code sent
under: confirm_details
plays: The details went to the record. The same words whether or not a patient matched.

## text.code
tab: Greet
name: Code text message
under: confirm_details
plays: Sent by text to the number on file, when exactly one patient matches.

## task.verify
tab: Verify
name: Verify instructions
under: -
plays: The model reads it after the code is sent, until the caller is verified.

## tool.check_code
tab: Verify
name: Check the code
under: check_code
plays: The model calls it when the caller reads a code.

## arg.check_code.code
tab: Verify
name: Code the caller read
under: check_code
plays: The model fills it with the digits the caller read.

## line.code_length
tab: Verify
name: Code not 6 digits
under: check_code
plays: The code is not 6 digits. No try is spent.

## line.code_wrong
tab: Verify
name: Wrong code
under: check_code
plays: The code is wrong, and tries are left.

## line.locked
tab: Verify
name: Too many wrong codes
under: check_code
plays: Too many wrong codes. The call goes to a person.

## line.verified
tab: Verify
name: Verified
under: check_code
plays: The code is right. The chart and booking open.

## tool.resend_code
tab: Verify
name: Send a new code
under: resend_code
plays: The model calls it when the caller asks for a new code.

## line.code_resent
tab: Verify
name: New code sent
under: resend_code
plays: A new code went out.

## line.no_more_codes
tab: Verify
name: No codes left
under: resend_code
plays: No code is left to send. The call goes to a person.

## task.menu
tab: Menu
name: Menu instructions
under: -
plays: The model reads it once the caller is verified, until they choose a time.

## tool.next_visit
tab: Menu
name: Tell the next visit
under: next_visit
plays: The model calls it when the caller asks about their next visit.

## line.no_visits
tab: Menu
name: No upcoming visit
under: next_visit
plays: No visit is coming.

## line.next_visit
tab: Menu
name: Next visit
under: next_visit
plays: The next visit: its time, clinician, place and length.

## line.visit_online
tab: Menu
name: Online visit (added)
under: next_visit
plays: Added after Next visit when the visit is online.

## line.visit_type
tab: Menu
name: Visit type (added)
under: next_visit
plays: Added after Next visit when the visit is in person and the record names its type. Never for a private visit.

## line.visit_reason
tab: Menu
name: Visit reason (added)
under: next_visit
plays: Added after Next visit when the record holds a reason. Never for a private visit.

## note.after_next_visit
tab: Menu
name: Note after the next visit
under: next_visit
plays: Sent to the model after the visit is spoken. Never heard.

## tool.find_slots
tab: Menu
name: Find open times
under: find_slots
plays: The model calls it when the caller wants to book, or another day.

## arg.find_slots.reason
tab: Menu
name: Reason for the visit
under: find_slots
plays: The model fills it from the caller's words.

## arg.find_slots.earliest_day
tab: Menu
name: Earliest day
under: find_slots
plays: The model fills it when the caller names a day.

## line.slots
tab: Menu
name: Open times offered
under: find_slots
plays: Open times were found. Holds each One open time.

## line.slot_option
tab: Menu
name: One open time (in the list)
under: find_slots
plays: One time, inside Open times offered.

## line.no_slots
tab: Menu
name: No open times
under: find_slots
plays: No open time from that day.

## tool.change_visit
tab: Menu
name: Hand a change to staff
under: change_visit
plays: The model calls it when the caller asks to move or cancel a visit. The call goes to a person, with a flag.

## line.change_booked
tab: Menu
name: Booked visit stays
under: change_visit
plays: A visit was booked on this call: the caller hears it stays booked.

## line.change_visit
tab: Menu
name: Change goes to staff
under: change_visit
plays: No visit was booked on this call.

## task.schedule
tab: Schedule
name: Schedule instructions
under: -
plays: The model reads it after open times are offered.

## tool.choose_option
tab: Schedule
name: Read back the chosen time
under: choose_option
plays: The model calls it when the caller picks an option number.

## arg.choose_option.option
tab: Schedule
name: Option number
under: choose_option
plays: The model fills it with the number the caller chose.

## line.which_option
tab: Schedule
name: Unknown option
under: choose_option
plays: The number is not one the caller heard. Book the time says it too, when no time was read back.

## line.confirm_slot
tab: Schedule
name: Chosen time read back
under: choose_option
plays: The chosen time is read back for a yes.

## tool.book
tab: Schedule
name: Book the time
under: book
plays: The model calls it when the caller says yes to the read-back time.

## line.slot_taken
tab: Schedule
name: Time already taken
under: book
plays: Someone else took the time first.

## line.booked
tab: Schedule
name: Booked
under: book
plays: The visit is booked.

## role
tab: Anytime
name: Role and rules
under: -
plays: The model reads it at every step.

## tool.end_call
tab: Anytime
name: End the call
under: end_call
plays: The model calls it when the caller says goodbye or has nothing else.

## line.goodbye
tab: Anytime
name: Goodbye
under: end_call
plays: The call ends after it.

## tool.hand_off
tab: Anytime
name: Connect to a person
under: hand_off
plays: The model calls it when the caller asks for a person, says yes to an offer, or the code did not work.

## arg.hand_off.kind
tab: Anytime
name: Hand-off reason
under: hand_off
plays: The model chooses a kind of flag.

## arg.hand_off.summary
tab: Anytime
name: Summary for staff
under: hand_off
plays: The model writes it for staff.

## arg.hand_off.evidence
tab: Anytime
name: Caller's words for staff
under: hand_off
plays: The model quotes the caller; the code keeps only words the caller said.

## line.hand_off
tab: Anytime
name: Connecting to a person
under: hand_off
plays: Any hand-off but one after the code did not work.

## line.hand_off_code
tab: Anytime
name: Connecting after a failed code
under: hand_off
plays: A hand-off because the code did not work.

## tool.offer_staff
tab: Anytime
name: Offer a person
under: offer_staff
plays: The model calls it when it cannot do or see what was asked.

## arg.offer_staff.why
tab: Anytime
name: Reason for the offer
under: offer_staff
plays: The model chooses: cannot help, or private.

## line.offer_staff
tab: Anytime
name: Offer: cannot help
under: offer_staff
plays: The agent cannot do or see what was asked.

## line.offer_staff_private
tab: Anytime
name: Offer: private
under: offer_staff
plays: The record holds it, but it is private.

## tool.emergency
tab: Anytime
name: Send to 911
under: emergency
plays: The model calls it when the caller may be in danger.

## line.emergency
tab: Anytime
name: 911 line
under: emergency
plays: The emergency tool; the phrase screen hearing an emergency phrase; or a 911 reminder before any warning. No one can cut it off.

## tool.safety_reminder
tab: Anytime
name: Remind about 911
under: safety_reminder
plays: The model calls it when the same danger comes up again.

## line.emergency_reminder
tab: Anytime
name: 911 reminder
under: safety_reminder
plays: The caller has heard the full 911 line already.

## note.after_reminder
tab: Anytime
name: Note after the 911 reminder
under: safety_reminder
plays: Sent to the model after the reminder. Never heard.

## tool.medical_question
tab: Anytime
name: Decline medical advice
under: medical_question
plays: The model calls it when the caller asks for medical advice.

## line.medical_question
tab: Anytime
name: Medical advice declined
under: medical_question
plays: Every time the tool is called.

## tool.flag_for_staff
tab: Anytime
name: Flag for a nurse
under: flag_for_staff
plays: The model calls it to mark the call for staff. Silent to the caller.

## arg.flag_for_staff.kind
tab: Anytime
name: Flag kind
under: flag_for_staff
plays: The model chooses a kind.

## arg.flag_for_staff.summary
tab: Anytime
name: Summary for the nurse
under: flag_for_staff
plays: The model writes it for staff.

## arg.flag_for_staff.evidence
tab: Anytime
name: Caller's words for the nurse
under: flag_for_staff
plays: The model quotes the caller; the code keeps only words the caller said.

## note.after_flag
tab: Anytime
name: Note after a flag
under: flag_for_staff
plays: Sent to the model after a flag. Never heard.

## line.one_moment
tab: Failure
name: Reply cut off
under: -
plays: The model's reply ran past its length cap. The agent tries again after it.

## line.say_again
tab: Failure
name: Could not use a value
under: -
plays: The scheduling system refused a value as unreadable (a date it cannot read). Not for speech the agent did not catch: the model says that in its own words.

## line.not_on_this_call
tab: Failure
name: Not allowed on this call
under: -
plays: The scheduling system says this call may not do that (not verified, or the call closed). The call goes to a person.

## line.not_now
tab: Failure
name: Refused for now
under: -
plays: The scheduling system refused for now (too many tries). The call goes to a person.

## line.ehr_down
tab: Failure
name: Scheduling system down
under: -
plays: The scheduling system did not answer. The call goes to a person.
