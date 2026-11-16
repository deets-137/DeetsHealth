---
status: built
desk_test: open
sources: [agent/prompts/AGENT-PROMPT.md, agent/deets_agent/prompts.py, agent/deets_agent/flow.py, agent/deets_agent/dev_server.py, agent/deets_agent/llm.py, ehr/src/texts.rs, ehr/src/service.rs, ehr/migrations/0005_settings_prompt.sql, console/src/pages/agent.ts]
updated: 2026-11-16
---
# DeetsHealth — Edit the agent's prompt in the console

> **Built 2026-11-13 as Config › Agent** (CONSOLE.md §9 is the as-built record and the desk
> test). This doc keeps the design of 2026-11-12: how a save reaches new calls, and what was
> decided. §3 describes the code before the build; §4 to §6 are now built as written, with the
> differences in CONSOLE.md §9.3. §7 and §8 are still open.

## 1. Terms
- **The prompt file** — `agent/prompts/AGENT-PROMPT.md`: every word the model reads and every
  fixed line the caller hears, one `## name` section each.
- **A version** — one saved copy of every section, with a number. Exactly one version is
  **current**.
- **A save** — the console's action that checks the words and makes a new current version.
- **The kept scripts** — the typed calls in `agent/tests/scripts/`. Each one is also a replay
  test in `agent/tests/test_typed_call.py` (AGENT.md §1).
- **Record and test** — record the kept scripts against a version (live requests to Claude),
  then run their tests and show the result.

## 2. What the owner asked (2026-11-12)
"Let's work on wiring in the agent prompt to the console for direct editing. First question,
are we able to set it up such that pressing save (theoretical button choice, will decide more
when we design but imagine a trigger) can have all new agents from the trigger moment
onwards?"

**The answer: yes.** Every call that starts after a save uses the saved words. A call already
in progress keeps the words it started with. This works the same in dev, in A2 (our cloud) and
in B (the hospital's cloud account).

## 3. How the prompt loaded before the build (2026-11-12)
> **Part:** archived · 2026-11-13 — replaced by CONSOLE.md §9.3. Kept as the reason for §4.
- `prompts.py` reads the prompt file once, when the module loads (`SECTIONS = load()`), and
  checks it (`check()`). The dev server is one long process, so an edit reaches no call until
  `npm run dev` starts again.
- `flow.py` builds each call's nodes and tools when the call starts, and calls `text()` then.
  Only the module-level dict is fixed at start-up.
- Two copies are taken earlier than a call:
  - `flow.py` › `EMERGENCY_LINE`, read once at import.
  - `ehr/src/texts.rs` compiles the prompt file into the `ehr` binary (`include_str!`) for
    `text.code`. An edit to `text.code` needs a rebuild of `ehr` today.
- The clinic's name is a constant in `flow.py` (`CLINIC`), not in the prompt file. (Since
  2026-11-14 it is the System variable `{clinic}`: SYSTEM-PAGE.md.)
- The replay cache keys each request by a hash of its exact parameters (`llm.py` ›
  `request_key`). Any edit to the words is a new key: every kept script misses until it is
  recorded again.
- The `calls` table has no column for which prompt a call used.

## 4. How a save would reach new calls
- **The save.** The console sends the edited sections. The same `check()` runs: a missing
  section, an unknown section or a wrong `{placeholder}` refuses the save with the same
  message. A refused save never reaches a call.
- **The version.** A save writes a new numbered version and makes it current.
- **A call's copy.** At its start, each call takes the current version (its number and its
  sections) and uses that copy until it ends. A save during a call does not change it.
- **Dev.** The dev server reads the current version at each call start, not at start-up.
- **A2 and B.** The store (Postgres) holds the versions. The agent never opens the database
  (VOICE-AGENT.md §8.7), so at call start it asks `ehr` for the current version: one request
  inside the deploy. Every worker picks up a save on its next call, with no restart.
- **Each EHR.** No effect: the words are the same whatever EHR a call reads.

## 5. What was decided
- **Record and test is an explicit button** (the owner, 2026-11-12). A save does not record
  the kept scripts. After a save, the console shows that the recordings are out of date. The
  owner presses the button when he chooses, for example after a group of edits. The button
  states the cost before it runs: about a cent per script (the prompt file's own note; not
  measured), and about a minute for all of them.
- **The analyst's path is the same button, later** (the owner, 2026-11-12). An analyst at a
  hospital who changes the words can test how the change affects the agent through a similar
  button or path. What that needs is §8.

## 6. What follows (one option was left each time)
1. **A save applies from the next call; a call in progress keeps its words.** The owner's own
   words chose this.
2. **The store holds the versions; the file stays.** In dev, each save also writes
   `AGENT-PROMPT.md`, so git, the tests and the owner's edits by hand keep working. A hand edit
   to the file becomes a new version when the dev server starts. The only option that serves
   dev, A2 and B and keeps the file.
3. **The save runs `check()` first.** The same rules as start-up today.
4. **`ehr` reads `text.code` from the call's version**, not from its binary. `EMERGENCY_LINE`
   moves to the call's copy too.
5. **Each call records its version number**, in the call record and in a column of the
   `calls` table, so Analytics can compare outcomes by version.
6. **Every save is audited** (who, when, which sections), **and can be rolled back**: rolling
   back makes an old version current again.
7. **For hospitals: our base version plus each hospital's own sections.** A hospital that
   held a whole copy would miss our safety fixes; a copy of only the sections it changed gets
   every other fix.

## 7. Not now (each waits for its own work)
> **Part:** idea · 2026-11-13 — the editor's design is now built (CONSOLE.md §9); the rest waits.
- Who may edit which sections (ideas/ACCESS-ROLES.md, the owner's design).
- Whether a deploy needs the kept scripts to pass before a version becomes current.
- The editor's design: its place in the nav, one box for the whole file or one per section,
  the save control.
- How the greeting's three required items are held (FUTURE-SETTINGS.md §4).
- Whether values held in code join the editor: the emergency phrases (`screen.py`), the
  sensitive-visit list (`visit_about.rs`), the counts and limits (FUTURE-SETTINGS.md). The
  owner's words named the prompt only. The clinic's name joined on 2026-11-14, as the System
  variable `{clinic}` on Config › System (SYSTEM-PAGE.md).

## 8. The analyst's test path (open)
> **Part:** idea · 2026-11-13
An analyst changes some words and wants to know what the change does before it reaches a
caller. What it needs:
- **A draft version.** The analyst's edits are tested as a draft, never as the current
  version, so a test never changes a live call.
- **The run.** The same Record and test, against the draft: each kept script as a typed call
  with synthetic callers and sandbox data only.
- **The result in the console.** Each script passed or failed, and what changed: the replies
  under the draft beside the replies under the current version.
- **The cost.** Stated before the run and counted against the hospital's budget.
- **Open:** whether an analyst may add scripts of their own; whether the test checks are
  readable to an analyst (today they are Python in `test_typed_call.py`); who may make a tested
  draft current (§7).

## 9. A release that changes the words, at a hospital
> **Part:** built · 2026-11-14 — found while adding three sections for the booking work (the
> owner asked whether building it this way causes issues in customer environments). The rules
> here apply from the first hospital deploy; in dev nothing below shows.

A hospital's store holds its own versions of the words. A release can add a section, or change
one; the hospital's active version was saved before the release. How each case reaches a call:
- **A section the release adds:** it takes the shipped words, and every other section keeps the
  hospital's words (`active_words` in `dev_server.py`; the stderr line counts the sections it
  filled). Before 2026-11-14 a missing section failed the check, and the whole call fell back to
  the shipped files, so a hospital's edits were lost without a word.
- **A section the release removes:** dropped from the call's words. Avoid it: an older version
  shown on Config › Agent still lists it.
- **A code value the release adds to an existing section** (a new `{name}`): the hospital's
  version fails the check and the call falls back to the shipped files. **From the first
  hospital deploy on, a release does not add a code value to an existing section;** it adds a
  new section, or brings a migration for the saved versions. Until then (active development,
  the owner, 2026-11-14: "I don't think it quite matters") sections and code values change
  freely: dev saves the file as a new version at start-up, so nothing is ever missing.
- **Changed words in a section the hospital never edited:** the hospital keeps the words of its
  saved version until someone saves again. Open: whether a release should offer "take the new
  shipped words" per section on Config › Agent, and how the hospital sees what changed.
- **In dev** none of this shows: at start-up, a file that differs from the active version is
  saved as a new version (CONSOLE.md §9), so the active version always has every section.
- **The settings pages need no change by hand:** Config › Agent lists the sections from
  `PLACEHOLDERS` and their names and "plays when" from `agent/prompts/GUIDE.md`; Config › System
  › Tools reads `agent/prompts/TOOLS.md`. A new section needs its entry in AGENT-PROMPT.md,
  `PLACEHOLDERS` (`prompts.py`), `PROMPT` (`ehr/src/settings.rs`) and GUIDE.md; tests hold the
  four together.
