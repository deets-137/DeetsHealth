---
status: built
desk_test: open
sources: []
updated: 2026-10-21
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
uv run pytest                             # replay only: no tokens
uv run ruff check .
```
`ehr` must be built (`cargo build -p deetshealth-ehr`) and the dev certificates made
(`cargo run -p deetshealth-ehr --example dev_certs`). The harness starts `ehr` itself on a
free port.

**Fake callers** (`ehr/src/verify.rs`): Jordan Ellis, 1984-03-12, has a visit on November 10.
Priya Raman, 1971-11-30, has none. Casey Morgan and Casey Morgen share 1990-06-05, so neither
verifies. The text code is always `246810`.

## 2. The parts
| File | What it does |
|---|---|
| `deets_agent/flow.py` | The nodes: greet, verify, menu, schedule, goodbye, hand off. Each node's tools. The words that carry a value (`say`). |
| `deets_agent/call.py` | One typed call: the pipeline, the turn collector, `start()`, `say()`, `hang_up()`. |
| `deets_agent/ehr_client.py` | The gRPC client: mTLS, a deadline on each call, the edge into `Shielded` types, error kinds. |
| `deets_agent/llm.py` | Pipecat's Anthropic service behind our client: GA only, and the replay cache. |
| `deets_agent/redact.py` | The PHI-free log processor; `redacted_text` for a wire message. |
| `deets_agent/shielded.py` | The Python wrapper for protected data. |
| `deets_agent/ehr_process.py` | Starts `ehr` on a free port and reads its address. |
| `harness/typed_call.py` | The typed call for the owner's desk test. |
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
- **A fixed "today"** (Monday, October 26, 2026) in the prompt, so a script's requests are the
  same on every run and the cache holds.
- **The model never sees a slot token or a handle.** It sees numbered options and picks a
  number; `flow.py` maps the number to the token. The handle and the offered slots live on the
  call's `CallFlow` object as `Shielded`, never in Flows' shared state.
- **Library log text is withheld whole.** Pipecat, the Anthropic SDK and grpc log prompts and
  tool arguments at some level, so a line from any library keeps its level and place and loses
  its text. Our own lines carry kinds and counts only. An exception keeps its type, never its
  message.
- **`NextVisit`** was added to the contract for S5 (EHR-SERVICE.md §6a).
- **One line on emergencies** in the role prompt: "tell them to hang up and call 911 now, then
  hand off." C5 (emergency transfer) is not in Phase 1; this line is a stopgap the owner may
  remove.
- **The turn ends** when a response ends, no tool call is still running, and no new response
  starts within 0.4 seconds.

## 4. Known gaps
- **The model does not always read `say` as written.** In the first typed call, after
  `identify` it said "the text message we just sent" and dropped "if those details match our
  records". The no-echo rule held (the words do not depend on a match), but the template rule
  of SECURITY.md §4.9 is held by the prompt only. Speaking the template directly, without the
  model, is a choice for Phase 3, when the owner decides the agent's words.
- **No call timeline yet** (OpenTelemetry across the agent and `ehr`, VOICE-AGENT.md §9). It is
  the next Phase 1 item and the main debugging tool for a call.
- **Simulated callers** (layer 4) are not built; they come in Phase 2.
- `grpcio` is the wheel on dev; the FIPS build against OpenSSL is a deploy-image step
  (EHR-SERVICE.md §6).
