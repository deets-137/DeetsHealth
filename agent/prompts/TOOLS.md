# The tools, for an analyst

What each tool of the agent does, in plain words, for Config › System › Tools (SYSTEM-PAGE.md
§3.6). The model never reads this file: the model reads each tool's own words in
AGENT-PROMPT.md (`tool.<name>`, `arg.<name>.<argument>`). The steps that offer each tool come
from the code (`flow.py` › `tool_steps`), not from here.

Each tool: a `## name` heading, a paragraph that says what it does, then:
- `record:` what it does with the patient's record: `reads`, `writes` or `none`.
- `lines:` the fixed lines it can speak, in the order it may speak them.

A tool that reads or writes the record can also speak the four trouble lines when ehr does not
answer as it should: `line.not_on_this_call`, `line.not_now`, `line.say_again`,
`line.ehr_down`. A test checks that every tool has an entry and that every line named here
exists.

## identify
Takes the name and the date of birth that the caller said, and reads them back for a yes.
Nothing goes to the record yet. The code refuses a name the caller did not say, and a date of
birth that cannot be true; each refusal asks again and costs no try.
record: none
lines: line.birth_not_possible, line.name_not_said, line.read_back

## confirm_details
After the caller says yes to the read-back, sends the details to the record. If exactly one
patient matches, a code goes by text to the number on file. The caller hears the same words
whether or not a patient matched.
record: reads
lines: line.ask_details, line.code_sent

## check_code
Checks the six-digit code the caller reads back. A right code verifies the caller and opens the
chart and booking. Too many wrong codes end verification and hand the caller to a person.
record: reads
lines: line.code_length, line.verified, line.locked, line.code_wrong

## resend_code
Sends the code again, to the same number. After the last one, hands the caller to a person.
record: reads
lines: line.code_resent, line.no_more_codes

## find_slots
Finds open times for a visit, for the verified caller only, from the day the caller asks, in
person, online or either, as the caller said. Each time gets an option number that stays the
same for the whole call, and an online time says "online". The reason goes with the search in
the caller's own words: the code keeps the model's words only if the caller said them, or else
sends the caller's latest line. ehr keeps it for the booking. The caller may also name a last
day, a time of day from the hospital's list, days of the week and a clinician. A few times are
offered at a time (the hospital's setting); "more" offers the next ones of the same search. An
online time says the hospital's time zone.
record: reads
lines: line.no_slots, line.no_more_slots, line.slots, line.slot_option

## choose_option
Takes the option number the caller chose and reads that time back for a yes. Nothing is booked
yet.
record: none
lines: line.which_option, line.confirm_slot

## book
Books the time the caller said yes to, with the caller's reason as the visit's note for the
scheduler (Epic's `appointmentNote`). A retry never books twice. If someone else took the time
first, the caller hears so and can choose again. If the EHR's answer is lost, ehr searches for
the booking; when none is found, the caller is asked before it tries again (never twice without
a yes).
record: writes
lines: line.which_option, line.slot_taken, line.book_not_through, line.booked

## change_visit
The agent cannot move or cancel a visit. Tells the caller that staff will do it, flags the
call for staff with the caller's own words and the visit's number at the hospital (its CSN),
and hands the caller to a person.
record: none
lines: line.change_booked, line.change_visit

## find_visit
Reads one of the verified caller's visits: the next one, or the one they name by a day, a
month, a clinician, a kind of visit or the past. Says its time, then what it is (online, its
type, its reason), as the record says, and for a past visit whether the caller was seen. When
too many visits match, asks which kind of visit the caller means (the hospital's visit groups).
Offers the visit's instructions, or reads them at once when the caller named the visit. A
private visit's type, reason and instructions are never spoken. If it is the visit booked on
this call, its time is not said again.
record: reads
lines: line.no_visits, line.no_visit_match, line.no_past_visits, line.which_visit, line.visit_group, line.another_visit, line.choice_or, line.next_visit, line.visit_found, line.past_seen, line.past_missed, line.past_cancelled, line.booked_visit_asked, line.visit_online, line.visit_type, line.visit_reason, line.one_more_visit, line.more_visits, line.visit_instructions, line.offer_instructions

## read_instructions
Reads the instructions of the visit the caller just heard: how to prepare for it. Asks the
record nothing new.
record: none
lines: line.visit_instructions, line.no_instructions

## hand_off
Ends the call with a hand-off to a person, with the model's reason and summary for staff.
After a code that did not work, the caller hears an apology. A summary that uses a pronoun for
the caller goes back to the model once, to name the caller instead.
record: none
lines: line.hand_off, line.hand_off_code

## offer_staff
When the agent cannot do or see what the caller asks, it says so and offers a person. The
caller chooses: a yes is `hand_off`; a no, the call goes on.
record: none
lines: line.offer_staff, line.offer_staff_private

## end_call
Says goodbye and ends the call, at any step.
record: none
lines: line.goodbye

## emergency
Speaks the 911 line in full. No caller can cut it off. Flags the call for staff. The call
stays open.
record: none
lines: line.emergency

## safety_reminder
A short reminder after the caller has heard the 911 line. If the caller has not heard the full
line yet, the full line is spoken in its place.
record: none
lines: line.emergency, line.emergency_reminder

## medical_question
Tells the caller that the agent cannot answer medical questions, and what to do.
record: none
lines: line.medical_question

## flag_for_staff
Marks the call for staff, silent to the caller, with the model's summary and the caller's own
words as evidence. The code keeps only the words the caller said. A summary that uses a pronoun
for the caller goes back to the model once, to name the caller instead. A reason that does not
read as a health need is booked as said and flagged as `visit_reason_review`.
record: none
lines: none
