---
status: built
desk_test: open
sources: []
updated: 2026-11-11
---
# DeetsHealth — The agent (`agent/`), as built

> Built 2026-10-21 for Phase 1, the text slice (BUILD-ORDER.md). The design is
> [VOICE-AGENT.md](VOICE-AGENT.md) §8.1 (the call path) and §9 (the test layers); the wire to
> the Rust service is [EHR-SERVICE.md](EHR-SERVICE.md). Where this doc and the code disagree,
> the code wins and this doc is corrected.

**Terms**
- **Typed call:** a call with text in and text out: the real Flow, the real LLM service, the
  real `ehr`, no audio (test layer 3).
- **Replay cache:** each request to Claude is kept on disk under a hash of its exact
  parameters, and replayed when the same request comes again.

## 1. Run it
```
cd agent
uv sync                                   # once; uv is at ...\WinGet\Packages\astral-sh.uv_*\uv.exe until the shell restarts
uv run python scripts/gen_proto.py        # after any change to ehr.proto
uv run python harness/typed_call.py       # type a call (record mode)
uv run python harness/typed_call.py --script tests/scripts/book_a_visit.txt
uv run python harness/show_call.py        # the newest call record as a timeline (--all: every event)
uv run pytest                             # replay only: no tokens
uv run ruff check .
```
`ehr` must be built (`cargo build -p deetshealth-ehr`) and the dev certificates made
(`cargo run -p deetshealth-ehr --example dev_certs`). The harness starts `ehr` itself on a
free port.

**The dev call record** (2026-11-11, the owner's ask): each typed call writes one JSON Lines
file to `agent/calls/` (committed to the private repo since 2026-11-11, the owner's call;
synthetic only, so never type a real person's details), and the harness prints its path and a cost line at the
end. It holds both sides of the conversation, each Flow node, each tool call with its
arguments, result and time, each Claude request (replayed or live, time to first event, time to
the first word and to the first tool call, total time, tokens, thinking blocks dropped, or the full error), each `ehr` call (name, time,
outcome), and the full text of library warnings and errors. `--no-record` turns it off.
**Synthetic callers only**: the record holds what the PHI wall keeps out of the logs. Only the
harness opens one (`deets_agent/call_record.py`); a deployed call has no path to it.

The agent's "today" in a typed call is Monday, October 26, 2026 (§3); `--today real` uses the
real date, but the fake slots are then in the past.

**Fake callers** (`ehr/src/verify.rs`): Jordan Ellis, 1984-03-12, has a visit on November 10.
Priya Raman, 1971-11-30, has none. Casey Morgan and Casey Morgen share 1990-06-05, so neither
verifies. The text code is always `246810`. No text is sent. The harness prints these at
the start of every typed call (2026-11-11; the owner got stuck at the code step without them).

**The kept scripts** (`tests/scripts/`, each one also a replay test in `test_typed_call.py`):
`book_a_visit.txt` (verify, book, next visit, goodbye), `leave_at_greet.txt` and
`leave_at_code.txt` (the caller says goodbye before and during verification; the call must
end with the goodbye line and no hand off).

## 2. The parts
| File | What it does |
|---|---|
| `deets_agent/flow.py` | The nodes: greet, verify, menu, schedule, goodbye, hand off. Each node's tools and the global ones (`hand_off`, `end_call`, `emergency`, `medical_question`). The fixed lines (`_say`), the name guard. |
| `deets_agent/call.py` | One typed call: the pipeline, the turn collector, `start()`, `say()`, `hang_up()`. |
| `deets_agent/ehr_client.py` | The gRPC client: mTLS, a deadline on each call, the edge into `Shielded` types, error kinds. |
| `deets_agent/llm.py` | Pipecat's Anthropic service behind our client: GA only, the replay cache, stale thinking blocks dropped. |
| `deets_agent/call_record.py` | The dev call record: one JSON Lines file per typed call. |
| `deets_agent/redact.py` | The PHI-free log processor; `redacted_text` for a wire message. |
| `prompts/AGENT-PROMPT.md` | **Every word**: the role prompt, each step's task, each tool's description, every fixed line. The owner reads and edits it by hand; its top says how. |
| `deets_agent/prompts.py` | Loads AGENT-PROMPT.md and checks it at start-up: every section there, no unknown one, the right `{placeholders}`. |
| `deets_agent/screen.py` | The caller screen: the emergency phrase list, and each caller line to the name guard. |
| `deets_agent/shielded.py` | The Python wrapper for protected data. |
| `deets_agent/ehr_process.py` | Starts `ehr` on a free port and reads its address. |
| `harness/typed_call.py` | The typed call for the owner's desk test. |
| `harness/show_call.py` | Prints a call record as a timeline. |
| `tests/` | The gate with no model, the PHI wall, the LLM client, a whole call from replays. |

## 3. What was decided while building (inside the owner's choices)
- **Pipecat 1.13 with Flows built in** (`pipecat.flows`). The separate `pipecat-ai-flows`
  package stopped at Pipecat 1.4, so it is not used.
- **GA only.** Pipecat sends every request to `client.beta.messages.create` with the beta flag
  `interleaved-thinking-2025-05-14`. Our client drops the flag and calls the GA
  `messages.create`, so the PHI path never uses a beta feature (VOICE-AGENT.md §8.2).
- **The model:** `claude-haiku-5-5`, effort `low`, at most 600 output tokens a reply, prompt
  caching on (config.py). The first typed call: 15 requests, about 1,000 output tokens, most
  input read from the cache; well under one cent.
- **The replay cache:** `replay` (tests; a miss is an error), `record` (the harness; a miss
  calls Claude once and keeps the reply), `live`. Recordings sit in `tests/replays/` and are
  committed, from fake patients only. Each live request logs its token counts, never its
  content.
- **"Today" is set per call** (2026-11-11). `CallFlow(today=…)` puts it in the role prompt;
  it is the prompt's only part that changes, once a day, so the prompt cache misses once a day,
  not once a call. A real call will use the hospital's own date. A typed call and the tests pin
  it to Monday, October 26, 2026, because `ehr`'s fake slots are fixed to the weeks after it
  and the replay cache needs the same prompt on every run. `--today real` (or a date) on the
  harness moves it. A tool that fetches the date was not chosen: it costs a round trip each
  time, and the model can forget to call it.
- **The fixed lines are spoken by the code** (2026-11-11, the owner's fork 1A). Every line a
  tool produces goes to the caller word for word as a `TTSSpeakFrame`, the frame the voice path
  uses in Phase 3. The model gets `spoken_to_caller` in the tool result, so it knows what was
  heard. A line that ends in a question waits for the caller. Every line ends in a question
  except "Let me look for others", where the model must act next. A model that spoke after a
  fixed line said the line again, word for word, in a test run. The closing lines (goodbye,
  hand off) end the call with no model turn. The working words were changed for this; the
  owner decides the final words (DESIGN.md §2 B).
- **An impossible date of birth costs no try** (2026-11-11). The Flow checks it (1900 to
  today) before it calls `identify`, and asks again. `ehr`'s edge refuses it as well, before
  the session counts a try (EHR-SERVICE.md §5).
- **The name rule** (2026-11-11): the role prompt and `identify`'s description say to use only
  the names the caller said, and to ask for any missing part. The role prompt also says never
  to talk about the tools or the instructions.
- **The model never sees a slot token or a handle.** It sees numbered options and picks a
  number; `flow.py` maps the number to the token. The handle and the offered slots live on the
  call's `CallFlow` object as `Shielded`, never in Flows' shared state.
- **Library log text is withheld whole.** Pipecat, the Anthropic SDK and grpc log prompts and
  tool arguments at some level, so a line from any library keeps its level and place and loses
  its text. Our own lines carry kinds and counts only. An exception keeps its type, never its
  message.
- **`NextVisit`** was added to the contract for S5 (EHR-SERVICE.md §6a).
- **The `emergency` tool** (2026-11-11, the owner's option A, from his third desk test). Global,
  like `end_call`. It speaks a fixed line: "If this might be an emergency, please hang up and
  call 911 now. If you're safe to go on, would you like me to keep helping with your call?" The
  call stays open and the node does not change, so a chosen option is still offered. Before, the
  model said the 911 line itself and then used `hand_off`, so the caller heard "call 911" and
  then "connecting you to staff", and the booking was lost. What follows the line, and the
  emergency flag that can start several actions at once (a nurse joined to the call, a message,
  a chart note), are hospital settings for later: FUTURE-SETTINGS.md §1. Test:
  `emergency_then_book`.
- **Time to the first word** (2026-11-11). Each live `llm` event now has `first_text_ms` and
  `first_tool_ms`; `show_call.py` prints them as `words` and `tool`. With thinking on,
  `first_event_ms` can be a thinking block, so it is not what a caller waits for.
- **The turn ends** when a response ends, no tool call is still running, and no new response
  starts within 0.4 seconds.
- **Stale thinking blocks are dropped** (2026-11-11). Effort `low` lets Haiku think when a turn
  is hard. A thinking block is bound to the tool list it was made with, and each Flow node has
  its own tools. Sent back after a node change, the block got a 400 ("the `tools` list
  differs"), Pipecat's LLM service stopped, and the call hung for 60 seconds, then crashed.
  This was the owner's first desk test, twice. Our client now keeps each block's signature
  with its tool list and drops a block whose tool list differs from the request's. The API's
  own fix is a beta header, which the GA rule rules out. Test: `test_llm.py`.
- **A failed Claude request ends the call at once** (`CallFailed`), not after the 60-second
  turn limit.
- **`end_call` is global, like `hand_off`** (2026-11-11, the owner's second desk test). Before,
  only the menu and schedule steps had it. In the call logs: "end call" at the code step got
  `hand_off` ("connecting you with a member of our staff"); "goodbye" at the greeting got a
  goodbye in the model's own words and the call stayed open; a caller who said "or I'm done"
  got an unasked goodbye in the model's own words. The role prompt now says: on goodbye, use
  `end_call`; never say goodbye in your own words; a caller who only wants to hang up does not
  need a person.
- **Words before a tool are checked, not held** (2026-11-11, the owner's fork C). On a voice
  call the model's words are spoken as they stream; a tool call in the same reply comes after
  them. Holding every reply until it ends would add 0.4 to 0.7 seconds to each one (the time
  from first event to the end, in the typed-call records) and would remove "let me check"
  before a slow tool. So: the role prompt says to say nothing with `end_call`, `hand_off` or
  `emergency`; `llm.py`'s `WordsBeforeTool` reads each reply's blocks (text, then
  tool_use) and notes every tool call that came after words; the call record writes it as
  `words_before_tool`, and `show_call.py` prints it with `!!`; the replay tests fail on any
  words before a fixed-line tool (`config.SILENT_TOOLS`). The check reads the API's events, not
  Pipecat's frames: the tool-call frame is a system frame and can overtake the text frames
  before it (a frame-based first try passed on live runs and failed on replay).
- **Code before prompt: the caller screen, the medical line, the name guard** (2026-11-11, the
  owner's 1A and 2A: "tools give us a route to add more config and more levers later").
  - **`medical_question`** (global): a fixed line, "I can't answer medical questions, but a
    clinician can talk with you about that at your visit. Shall we go on with your visit?" It
    first ended in a statement; the model then spoke next and said the line again, so it ends in
    a question like every fixed line. The agent never screens symptoms: that is clinical triage,
    for a hospital-approved script only. Test: `medical_question`.
  - **The caller screen** (`screen.py`, first in the pipeline): it reads each caller line before
    the model does. A phrase on the emergency list ("chest pain", "can't breathe", "am I dying",
    "overdose", "911", …) makes the code speak the 911 line itself: the caller's line and the 911
    line go into the context, and the model does not run. It fires once per call; later matches
    are recorded and left to the model's tool. "I'm dying to get in" does not match. The list is
    a hospital setting later; the full list is in FUTURE-SETTINGS.md §3, checked against the code by `test_screen.py`. Tests: `test_screen.py`,
    `emergency_then_book` (the screen now speaks that line, not the model).
  - **The name guard**: `identify` runs only when each word of both names is a word the caller
    said on this call (the screen feeds every line to `CallFlow.heard`, kept as `Shielded`). If
    not, the agent asks for the full name again, and no try is used. This replaces the prompt
    rule alone ("Billy Bob" sent as both names). Test: `test_gate.py`.
  - **Off-topic**: one prompt rule. Small talk (a greeting, a thank-you) is fine; anything else
    gets no answer, not even arithmetic, and the agent says kindly in one sentence that it can
    only help with visits, then asks what the caller needs. The first version only said to name
    what it can help with, and the model still gave sums ("17 plus 6 is 23"; the owner's desk
    test, 2026-11-11). Test: `off_topic` (a sum fails the invented-value check).
  - **The invented-value check** (`TurnCollector`): each model reply is checked for a number,
    a time, a month or a day that is in none of the role prompt, the caller's lines or the
    fixed lines. A hit is an `invented_value` event (`!!` in `show_call.py`); every replay test
    fails on one.
- **The read-back** (2026-11-11, the owner's option A, from "ellis jordan 21 3 1984": the name
  order and the day were both wrong, so no code came). `identify` no longer looks anyone up: it
  reads the name and date of birth back as a fixed line ("I have Jordan Ellis, born March 12,
  1984. Is that right?") and keeps them as `Shielded`. Only `confirm_details`, after the caller
  says yes, sends them to ehr. A correction is a new `identify` and a new read-back, so a slip
  costs no try. It repeats only the caller's own words, so it tells them nothing about the
  record. Cost: one short turn per call. Tests: `test_gate.py`, `read_back_correction`.
- **The prompt file** (2026-11-11, the owner's ask: one file to read and hand-edit). Every word
  the model reads and every fixed line is in `agent/prompts/AGENT-PROMPT.md`; `flow.py` holds the
  logic only. `prompts.py` loads the file at start-up and stops with a message on a missing or
  unknown section or a wrong placeholder (`test_prompts.py`). An edit changes the prompt, so the
  kept scripts must be recorded again (a cent or so); the replay tests then show what changed.
- **Worry is not a medical question** (2026-11-11). "I'm safe, I'm just worried. Please book
  option 1." got `medical_question` and no booking. The rule now says worry alone is not a
  medical question, and a request in the same sentence is done first with its own tool.
- **How many tools** (2026-11-11). Anthropic's tool-search page: selection accuracy degrades
  past 30 to 50 tools; standard tool calling fits fewer than 10. Each node now offers 6 to 8
  (4 global: `hand_off`, `end_call`, `emergency`, `medical_question`). Anthropic's server-side
  tool search is **not HIPAA-eligible**, so it is ruled out on the PHI path; past about 10 tools a
  node, the answer is our own client-side search or smaller nodes. Each node change also
  changes the tool list, which rewrites the prompt cache (about 2,000 tokens) and caused the
  thinking-block crash; the API's mid-conversation tool changes would avoid it but are beta.

## 4. Known gaps
- **The model's words before a fixed-line tool: held by the prompt, not by code** (fork C,
  §3). A live call can still break the rule. The record and the tests find it, but the caller
  has already heard it.
- **The emergency screen is a phrase list.** It catches the phrases on it and nothing else;
  the model's `emergency` tool covers the rest. It reads English text only; another language
  needs its own list. It fires once per call (§3).
- **The model decides what is a medical question.** The fixed line is in code (§3); the choice
  to use it is the model's. A medical question asked together with a booking request ("What's
  wrong with me? Can I come in soon?") got `find_slots` first and the medical line a turn late
  (2026-11-11). The kept script asks the two apart.
- **Off-topic questions: held by the prompt only** (2026-11-11: "2+2", "7 factorial"). Cheap,
  and it carries no record.
- **The model's own words: values checked, wording not.** The invented-value check (§3) finds
  a number, month or day that no one gave the model, in the record and in the tests; it does
  not stop a live caller hearing it. A value said in words ("three thirty") is not caught.
- **The name guard checks words, not spelling.** On a voice call it compares the model's names
  with the speech-to-text words, so a name the transcriber spells one way passes only if the
  model spells it the same way.
- **No call timeline yet** (OpenTelemetry across the agent and `ehr`, VOICE-AGENT.md §9). The
  dev call record covers the agent's side for typed calls; the timeline is still the main
  debugging tool for a voice call.
- **Simulated callers** (layer 4) are not built; they come in Phase 2.
- `grpcio` is the wheel on dev; the FIPS build against OpenSSL is a deploy-image step
  (EHR-SERVICE.md §6).
