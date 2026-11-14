# The System variables

Every variable that any section of AGENT-PROMPT.md may hold as `{name}`, with its meaning and
its value (SYSTEM-PAGE.md). Config › System edits them; a save there writes this file. A hand
edit here becomes a new System version when the dev server starts.

Each variable: a `## name` heading, then one `meaning:` line and one `value:` line. A value is
the clinic's own text, one line, at most 300 characters, with no `{` or `}`. Never put
protected data here.

`{clinic}` is built in: its value can change; it cannot be removed. The code values
(`{today}`, `{given}`, `{visit}` and the others) are not here: the code fills them, each in
its own section.

## clinic
meaning: The clinic's name, as a caller hears it.
value: Main Street Clinic
