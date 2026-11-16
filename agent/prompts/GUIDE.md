# The guide to the agent's words, for Config › Agent

Where each section of AGENT-PROMPT.md shows on Config › Agent, its name there, and when it plays
(AGENT-PAGE.md). The model never reads this file. The owner edits the names and the "plays when"
lines here; a test holds every section to one entry, and each entry's tab and tool to the code
(`flow.py` › `tool_steps`).

Each section: a `## key` heading, then:
- `tab:` Core, Greet, Verify, Menu, Schedule, Anytime or Failure. Core holds the core prompt
  (Role and rules) and the two 911 tools with their lines (the owner, 2026-11-14).
- `name:` its title on the page. The title alone says what the row is (the owner,
  2026-11-13); every title is different. Titles are active and start with a verb (the owner,
  2026-11-14). A tool's title says what the agent does ("Book the time"). A line's says what
  the agent says ("Say the visit is booked", "Ask which option", "Offer the open times"; "Add
  …" for a part added to another line). A field's names what the model fills in. A note's
  starts "Note". On the page a tool's title is its block's head, and its own words show as
  "When to use it".
- `under:` the tool it belongs to (a field, or a line or note the tool can produce), or `-`.
- `plays:` when it plays, in plain words. The page shows it under the row's "i".

The order here is the page's order: the sections with no tool first, then a block per tool.

## role
tab: Core
name: Role and rules
under: -
plays: The model reads it at every step.

## note.call_start
tab: Core
name: Note the time the call started
under: -
plays: Sent to the model when the call starts, after Role and rules: the hospital's time, in its time zone. Never heard.

## note.repeat_dropped
tab: Anytime
name: Note after a repeated line
under: -
plays: Sent to the model when its reply only repeated a fixed line the caller just heard: the reply was not spoken, and the model is asked once to act. Never heard.

## tool.emergency
tab: Core
name: Send to 911
under: emergency
plays: The model calls it when the caller may be in danger.

## line.emergency
tab: Core
name: Say to call 911
under: emergency
plays: The emergency tool; the phrase screen hearing an emergency phrase; or a 911 reminder before any warning. No one can cut it off.

## tool.safety_reminder
tab: Core
name: Remind about 911
under: safety_reminder
plays: The model calls it when the same danger comes up again.

## line.emergency_reminder
tab: Core
name: Say the short 911 reminder
under: safety_reminder
plays: The caller has heard the full 911 line already.

## note.after_reminder
tab: Core
name: Note after the 911 reminder
under: safety_reminder
plays: Sent to the model after the reminder. Never heard.

## line.greeting
tab: Greet
name: Greet the caller
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

## arg.identify.caller_wants
tab: Greet
name: What the caller wants
under: identify
plays: The model fills it when the caller has already said they want to book, hear about a visit, or both; the agent then does not ask again after the code.

## line.birth_not_possible
tab: Greet
name: Ask for the date of birth again
under: identify
plays: The date cannot be true (before 1900 or in the future). No try is spent.

## line.name_not_said
tab: Greet
name: Ask for the full name again
under: identify
plays: The model sent a name the caller did not say. No try is spent.

## line.read_back
tab: Greet
name: Ask if the details are right
under: identify
plays: Both checks pass: the caller hears the details and says yes or no.

## tool.confirm_details
tab: Greet
name: Look up and send a code
under: confirm_details
plays: The model calls it when the caller says yes to the read-back.

## line.ask_details
tab: Greet
name: Ask for the details first
under: confirm_details
plays: Nothing was read back yet.

## line.code_sent
tab: Greet
name: Say a code is on its way
under: confirm_details
plays: The details went to the record. The same words whether or not a patient matched.

## text.code
tab: Greet
name: Text the code
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
name: Ask for all 6 digits
under: check_code
plays: The code is not 6 digits. No try is spent.

## line.code_wrong
tab: Verify
name: Say the code did not match
under: check_code
plays: The code is wrong, and tries are left.

## line.locked
tab: Verify
name: Say the caller cannot be verified
under: check_code
plays: Too many wrong codes. The call goes to a person.

## line.verified
tab: Verify
name: Say the caller is verified
under: check_code
plays: The code is right and the caller has not said what they need: the chart and booking open, and the caller is asked.

## line.verified_go
tab: Verify
name: Say verified and go on
under: check_code
plays: In place of Say the caller is verified when the caller already said what they need: no question.

## note.after_verified
tab: Verify
name: Note after verification
under: check_code
plays: Sent to the model with Say verified and go on: do what the caller asked. Never heard.

## tool.resend_code
tab: Verify
name: Send a new code
under: resend_code
plays: The model calls it when the caller asks for a new code.

## line.code_resent
tab: Verify
name: Say a new code is on its way
under: resend_code
plays: A new code went out.

## line.no_more_codes
tab: Verify
name: Say no more codes can be sent
under: resend_code
plays: No code is left to send. The call goes to a person.

## task.menu
tab: Menu
name: Menu instructions
under: -
plays: The model reads it once the caller is verified, until they choose a time.

## tool.find_visit
tab: Menu
name: Tell a visit
under: find_visit
plays: The model calls it when the caller asks about a visit they have: the next one, or one they name by day, month, clinician, kind or past.

## arg.find_visit.when
tab: Menu
name: The day or month
under: find_visit
plays: Filled when the caller names a day or a month for the visit.

## arg.find_visit.clinician
tab: Menu
name: The clinician named
under: find_visit
plays: Filled when the caller names the clinician.

## arg.find_visit.past
tab: Menu
name: A visit that is over
under: find_visit
plays: Set when the caller asks about a past visit.

## arg.find_visit.group
tab: Menu
name: The kind of visit
under: find_visit
plays: Filled when the caller names a kind of visit (an x-ray, a checkup).

## arg.find_visit.place
tab: Menu
name: The place of the visit
under: find_visit
plays: Filled when the caller names the visit's place; ehr matches it to Config › Hospital › Places.

## line.no_visits
tab: Menu
name: Say no visit is coming
under: find_visit
plays: No visit is coming.

## line.no_visit_match
tab: Menu
name: Say no visit matches
under: find_visit
plays: The caller named a visit and none matches.

## line.no_past_visits
tab: Menu
name: Say no past visit is on record
under: find_visit
plays: The caller asked about a past visit and the record has none.

## line.which_visit
tab: Menu
name: Ask which visit
under: find_visit
plays: The caller named a visit and too many match: the agent asks which kind they mean (a hospital setting, FUTURE-SETTINGS.md §13.3).

## line.visit_group
tab: Menu
name: Name one kind of visit
under: find_visit
plays: One choice inside Ask which visit, for each visit group.

## line.another_visit
tab: Menu
name: Name the other visits
under: find_visit
plays: The last choice inside Ask which visit, when visits outside the named groups match.

## line.choice_or
tab: Menu
name: Join the choices
under: find_visit
plays: The word between the choices of Ask which visit.

## line.next_visit
tab: Menu
name: Say the next visit
under: find_visit
plays: The next visit: its time, clinician, place and length.

## line.visit_found
tab: Menu
name: Say the visit named
under: find_visit
plays: In place of Say the next visit when the caller named the visit.

## line.past_seen
tab: Menu
name: Say the caller was seen
under: find_visit
plays: A past visit the caller came to.

## line.past_missed
tab: Menu
name: Say the visit was missed
under: find_visit
plays: A past visit marked as missed.

## line.past_cancelled
tab: Menu
name: Say the visit was cancelled
under: find_visit
plays: A visit that was cancelled.

## line.booked_visit_asked
tab: Menu
name: Say it is the visit just booked
under: find_visit
plays: In place of Say the next visit when the next visit is the one booked on this call: the caller heard its time a moment ago.

## line.visit_online
tab: Menu
name: Add that the visit is online
under: find_visit
plays: Added after the visit when it is online.

## line.visit_type
tab: Menu
name: Add the visit type
under: find_visit
plays: Added after the visit when it is in person and the record names its type. Never for a private visit.

## line.visit_reason
tab: Menu
name: Add the visit reason
under: find_visit
plays: Added after the visit when the record holds a reason. Never for a private visit.

## line.one_more_visit
tab: Menu
name: Add one more visit
under: find_visit
plays: Added after the next visit when one more visit is coming.

## line.more_visits_within
tab: Menu
name: Add how many more visits soon
under: find_visit
plays: In place of Add how many more visits when the search counted a window (7, 30 or 90 days).

## line.one_more_visit_within
tab: Menu
name: Add one more visit soon
under: find_visit
plays: In place of Add one more visit when the search counted a window.

## line.more_visits
tab: Menu
name: Add how many more visits
under: find_visit
plays: Added after the next visit when two or more visits are coming after it.

## line.offer_instructions
tab: Menu
name: Offer the instructions
under: find_visit
plays: Added last when the visit to come has instructions and the caller did not name the visit.

## note.after_find_visit
tab: Menu
name: Note after a visit
under: find_visit
plays: Sent to the model after the visit is spoken. Never heard.

## note.after_offer_instructions
tab: Menu
name: Note after offering instructions
under: find_visit
plays: Sent to the model after Offer the instructions. Never heard.

## note.after_which_visit
tab: Menu
name: Note after asking which visit
under: find_visit
plays: Sent to the model after Ask which visit. Never heard.

## tool.read_instructions
tab: Menu
name: Read the instructions
under: read_instructions
plays: The model calls it when the caller says yes to the instructions, or asks how to prepare.

## line.visit_instructions
tab: Menu
name: Say the instructions
under: read_instructions
plays: The visit's instructions. Also added after Say the visit named, at once, when the caller named the visit. Never for a private visit.

## line.no_instructions
tab: Menu
name: Say no instructions are on record
under: read_instructions
plays: The visit has no instructions to read.

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

## arg.find_slots.visit_mode
tab: Menu
name: In person or online
under: find_slots
plays: The model fills it when the caller says how they want to be seen; empty when they did not say, and the code then asks once (the owner's 1A).

## arg.find_slots.latest_day
tab: Menu
name: The last day
under: find_slots
plays: The model fills it when the caller names a last day they can come.

## arg.find_slots.time_of_day
tab: Menu
name: The time of day
under: find_slots
plays: The model picks one of the hospital's times of day when the caller names one (FUTURE-SETTINGS.md §13.1).

## arg.find_slots.days
tab: Menu
name: The days of the week
under: find_slots
plays: The model fills it when the caller names days of the week.

## arg.find_slots.clinician
tab: Menu
name: The clinician asked for
under: find_slots
plays: The model fills it when the caller asks for a clinician; the times are narrowed to that clinician.

## arg.find_slots.any_time
tab: Menu
name: Any day or time is fine
under: find_slots
plays: The model sets it when the caller says any day or time is fine; the code then does not ask when.

## arg.find_slots.more
tab: Menu
name: More of the same search
under: find_slots
plays: Set when none of the times suit the caller and more are waiting.

## arg.find_slots.any_place
tab: Menu
name: Any place
under: find_slots
plays: Set when the caller wants a place other than their usual one.

## arg.find_slots.place
tab: Menu
name: The place
under: find_slots
plays: Filled when the caller names a clinic, a building or a town; ehr matches it to Config › Hospital › Places.

## arg.find_slots.specialty
tab: Menu
name: The kind of care
under: find_slots
plays: Filled only when the caller names a kind of care or doctor (the owner's 2A); ehr matches it to Config › Hospital › Specialties. Empty is primary care.

## line.slots
tab: Menu
name: Offer the open times
under: find_slots
plays: Open times were found. Holds each One open time (in the list).

## line.slots_usual_place
tab: Menu
name: Name the usual place
under: find_slots
plays: Inside Offer the open times, first, when a new search was held to the caller's usual place (their last visit with their PCP, or at a primary care clinic).

## line.slot_sooner
tab: Menu
name: Say one time is sooner
under: find_slots
plays: Inside Offer the open times, last, when one time is with another clinician at the same place and sooner than the PCP's first (the hospital's setting).

## line.slot_option
tab: Menu
name: One open time (in the list)
under: find_slots
plays: One time, inside Offer the open times.

## line.ask_reason
tab: Menu
name: Ask what the visit is for
under: find_slots
plays: The first search of the call, when the caller has not said what the visit is for. Asked once, before how and when; no search yet.

## note.after_ask_reason
tab: Menu
name: Note after asking the reason
under: find_slots
plays: Sent to the model with the reason question. Never heard.

## line.ask_preference_both
tab: Menu
name: Ask how and when
under: find_slots
plays: The first search of the call, when the caller has said neither how nor when, and the hospital books both in person and online. Asked once; no search yet.

## line.ask_preference_mode
tab: Menu
name: Ask in person or online
under: find_slots
plays: The first search of the call, when the caller said when but not how, and the hospital books both. Asked once.

## line.ask_preference_when
tab: Menu
name: Ask a day or time
under: find_slots
plays: The first search of the call, when the caller has said no day, time of day, days or "any time". Asked once.

## note.after_ask_preference
tab: Menu
name: Note after asking how and when
under: find_slots
plays: Sent to the model with a preference question. Never heard.

## line.ask_place
tab: Menu
name: Ask where
under: find_slots
plays: The caller named no place and has no usual place, and the kind of care is at more places than one question names. Asked once a call; after the preference question when that is asked too. No search yet.

## line.which_place
tab: Menu
name: Ask which place
under: find_slots
plays: The caller's words name several places, or the kind of care is at a few places and they have no usual one. Holds each place's spoken name, and Another place for the rest. No search yet.

## line.another_place
tab: Menu
name: Another place
under: find_slots
plays: Inside Ask which place, last, when more places fit than one question names.

## note.after_ask_place
tab: Menu
name: Note after asking where
under: find_slots
plays: Sent to the model with Ask where or Ask which place. Never heard.

## line.unknown_place
tab: Menu
name: Say the place is not known
under: find_slots
plays: The caller's words for a place match no department on Config › Hospital › Places (also for a visit asked about by its place). No search.

## line.unknown_specialty
tab: Menu
name: Say the kind of care is not known
under: find_slots
plays: The caller's words for a kind of care match no specialty on Config › Hospital › Specialties, or no department has it. No search.

## line.not_at_place
tab: Menu
name: Say that care is not at that place
under: find_slots
plays: The place the caller named has no department of the kind of care they named. No search.

## line.referral_needed
tab: Menu
name: Say a referral is needed
under: find_slots
plays: A Booking rule (Config › Hospital › Rules) says Referral needed for every time the search could offer. No time is offered.

## note.after_referral_needed
tab: Menu
name: Note after a referral is needed
under: find_slots
plays: Sent to the model with Say a referral is needed. Never heard.

## line.rule_hand_off
tab: Menu
name: Say staff book this visit
under: find_slots
plays: A Booking rule says Hand off for every time the search could offer. The call goes to a person, with a staff flag.

## line.no_slots
tab: Menu
name: Say no times are open
under: find_slots
plays: No open time from that day.

## line.no_more_slots
tab: Menu
name: Say no more times are open
under: find_slots
plays: The caller asked for more times from the same search and none are left.

## tool.change_visit
tab: Menu
name: Hand a change to staff
under: change_visit
plays: The model calls it when the caller asks to move or cancel a visit. The call goes to a person, with a flag.

## arg.change_visit.visit
tab: Menu
name: The visit to change
under: change_visit
plays: Filled with the visit's number when the caller heard the visit from Tell a visit.

## line.change_booked
tab: Menu
name: Say the booked visit stays
under: change_visit
plays: A visit was booked on this call: the caller hears it stays booked.

## line.change_visit
tab: Menu
name: Say staff will make the change
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
name: Ask which option
under: choose_option
plays: The number is not one the caller heard. Book the time says it too, when no time was read back.

## line.confirm_slot
tab: Schedule
name: Ask to book the chosen time
under: choose_option
plays: The chosen time is read back for a yes.

## note.question_with_choice
tab: Schedule
name: Note to answer a question asked with a choice
under: choose_option
plays: Sent to the model when the caller's choice came with a question, before the read-back. Never heard.

## tool.book
tab: Schedule
name: Book the time
under: book
plays: The model calls it when the caller says yes to the read-back time.

## line.slot_taken
tab: Schedule
name: Say the time was taken
under: book
plays: Someone else took the time first.

## line.book_not_through
tab: Schedule
name: Ask to try the booking again
under: book
plays: The EHR's answer to a booking was lost and a search found no booking: the caller is asked before the agent tries again (the owner's V2A).

## line.booked
tab: Schedule
name: Say the visit is booked
under: book
plays: The visit is booked.

## tool.end_call
tab: Anytime
name: End the call
under: end_call
plays: The model calls it when the caller says goodbye or has nothing else.

## line.goodbye
tab: Anytime
name: Say goodbye
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
name: Say a person is coming
under: hand_off
plays: Any hand-off but one after the code did not work.

## line.hand_off_code
tab: Anytime
name: Say a person will help with the code
under: hand_off
plays: A hand-off because the code did not work.

## line.hand_off_answer
tab: Anytime
name: Say a person will answer it
under: hand_off
plays: A hand-off on the caller's reply to the offer of a person: a yes, or the question asked again.

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
name: Offer a person when it cannot help
under: offer_staff
plays: The agent cannot do or see what was asked.

## line.offer_staff_private
tab: Anytime
name: Offer a person when the answer is private
under: offer_staff
plays: The record holds it, but it is private.

## tool.medical_question
tab: Anytime
name: Decline medical advice
under: medical_question
plays: The model calls it when the caller asks for medical advice.

## line.medical_question
tab: Anytime
name: Say it cannot give medical advice
under: medical_question
plays: Every time the tool is called.

## tool.place_info
tab: Anytime
name: Tell where a place is
under: place_info
plays: The model calls it when the caller asks where to go, for an address or a phone number, or whether a place is open. Works before verification.

## arg.place_info.place
tab: Anytime
name: The place asked about
under: place_info
plays: Filled with the caller's words for the place; empty for the visit they just heard or booked.

## line.place_address
tab: Anytime
name: Say the address
under: place_info
plays: The place is found and has an address on Config › Hospital › Places (its own, else its facility's).

## line.place_no_address
tab: Anytime
name: Say the address is not on file
under: place_info
plays: The place is found and neither it nor its facility has an address.

## line.place_phone
tab: Anytime
name: Say the phone number
under: place_info
plays: After the address, when the place or its facility has a phone.

## line.place_no_phone
tab: Anytime
name: Say the phone is not on file
under: place_info
plays: After the address, when neither the place nor its facility has a phone.

## line.place_hours
tab: Anytime
name: Say the hours
under: place_info
plays: Last: the place's own hours, else the hospital's (Config › Hospital › Times).

## line.place_info_which
tab: Anytime
name: Ask which place to tell about
under: place_info
plays: The caller asked where to go, named no place, and has heard no visit on this call.

## note.after_place_info
tab: Anytime
name: Note after telling where a place is
under: place_info
plays: Sent to the model with the place's facts, with the hospital's next closed days. Never heard.

## note.after_place_unknown
tab: Anytime
name: Note after a place or care not found
under: place_info
plays: Sent to the model with Say the place is not known, Say the kind of care is not known or Say that care is not at that place (from this tool, Find open times or the visit search). Never heard.

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

## note.summary_pronoun
tab: Anytime
name: Note when a summary guessed a pronoun
under: flag_for_staff
plays: Sent to the model once, when a summary for staff (in a flag or a hand-off) uses he, she, him, her, his or hers. Never heard.

## line.one_moment
tab: Failure
name: Say one moment
under: -
plays: The model's reply ran past its length cap. The agent tries again after it.

## line.say_again
tab: Failure
name: Ask the caller to say it again
under: -
plays: The scheduling system refused a value as unreadable (a date it cannot read). Not for speech the agent did not catch: the model says that in its own words.

## line.not_on_this_call
tab: Failure
name: Say it cannot do that on this call
under: -
plays: The scheduling system says this call may not do that (not verified, or the call closed). The call goes to a person.

## line.not_now
tab: Failure
name: Say it cannot do that now
under: -
plays: The scheduling system refused for now (too many tries). The call goes to a person.

## line.ehr_down
tab: Failure
name: Say the scheduling system is down
under: -
plays: The scheduling system did not answer. The call goes to a person.
