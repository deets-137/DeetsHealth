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
Finds open times for a visit, for the verified caller only, from the day the caller asks. Each
time gets an option number that stays the same for the whole call.
record: reads
lines: line.no_slots, line.slots, line.slot_option

## choose_option
Takes the option number the caller chose and reads that time back for a yes. Nothing is booked
yet.
record: none
lines: line.which_option, line.confirm_slot

## book
Books the time the caller said yes to. A retry never books twice. If someone else took the time
first, the caller hears so and can choose again.
record: writes
lines: line.which_option, line.slot_taken, line.booked

## change_visit
The agent cannot move or cancel a visit. Tells the caller that staff will do it, flags the
call for staff with the caller's own words, and hands the caller to a person.
record: none
lines: line.change_booked, line.change_visit

## next_visit
Reads the verified caller's next visit: its time, then what it is (online, its type, its
reason), as the record says. A private visit's type and reason are never spoken.
record: reads
lines: line.no_visits, line.next_visit, line.visit_online, line.visit_type, line.visit_reason

## hand_off
Ends the call with a hand-off to a person, with the model's reason and summary for staff.
After a code that did not work, the caller hears an apology.
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
words as evidence. The code keeps only the words the caller said.
record: none
lines: none
