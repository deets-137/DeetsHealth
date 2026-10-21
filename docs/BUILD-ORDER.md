---
status: project
desk_test: none
sources: []
updated: 2026-10-20
---
# DeetsHealth — Build order

> The phases that take the phone agent from nothing to a first hospital, each ending in a desk
> test. It rests on three decisions already made: the nucleus is one inbound call that books a
> visit ([DESIGN.md](DESIGN.md) §A); the test layers build in the order 3, 4, 5, 7, 6, 8
> ([architecture/VOICE-AGENT.md](architecture/VOICE-AGENT.md) §9); the Epic plan is E-1 to E-4
> ([integrations/EPIC.md](integrations/EPIC.md) §5). The state of each phase lives here; the
> state of the whole project is [HANDOFF.md](HANDOFF.md).
> Status legend: ✅ done · 🔵 in flight · ⬜ not started · ❓ a fork for the owner

**Terms**
- **Slice:** one path through the product that the owner can desk-test end to end.
- **Text harness:** test layer 3. The real Flow, text in and text out, no audio.
- **Fake scheduler:** the in-memory `Scheduler` that stands in for an EHR's booking in tests.
- **E-1 … E-4:** the Epic steps in EPIC.md §5. E-1 is the owner's registration on fhir.epic.com.

## 1. Why this order
- The patient feels turn-taking, the verify step and the booking first. The Flow and the tools
  can be proven in text for Claude tokens only, so text comes before voice.
- Voice costs Cartesia minutes. It comes once the Flow is right.
- Nothing needs persisting before a call works, so the store comes after voice.
- The PHI wall (the verify gate, the PHI-free log processor) is in from the first phase. It is
  never retrofitted.
- Each phase ends in something the owner can run (LESSONS.md §3.11: kept tooling, not a harness).

## 2. The phases

### Phase 0 — paper 🔵
> **Part:** project · 2026-10-20

No code. Everything the first slice needs to exist first.

| # | Item | Who | State |
|---|---|---|---|
| P0.1 | The cost per call minute, quoted (DESIGN.md §I, VOICE-AGENT.md §8.5) | Claude | ⬜ |
| P0.2 | The repo layout approved (CONVERSION.md §6, a draft): `agent/`, `ehr/`, `console/`, `deploy/`. Each folder is born with the phase that needs it; `ehr/` in Phase 1 | the owner | ✅ 2026-10-21 |
| P0.3 | E-1: the Epic backend app registration (EPIC.md §2) | the owner | ⬜ |
| P0.4 | Dev accounts: Twilio (trial), Cartesia (standard), Anthropic (a separate workspace). Synthetic data only; no BAA needed for that | the owner | ⬜ |
| P0.5 | `npm run check` and the pre-push hook from `../DeetsMusic` | Claude | ⬜ |
| P0.6 | The fork §3 decided: the text slice goes first, Backend Services follows E-1 | the owner | ✅ 2026-10-20 |

### Phase 1 — the text slice 🔵 (started 2026-10-21)
> **Part:** built · 2026-10-21 · `ehr/` and `agent/` built; a scripted call books a visit end to
> end (EHR-SERVICE.md §6a, AGENT.md). Waiting on the owner's desk test. Not built yet: the call
> timeline.

Pipecat Flows with the node map of VOICE-AGENT.md §8.1, in `agent/`. Features C1, C2, C4, S1,
S2 and S5 against the fake scheduler and fake patients. C3's second factor is a fake code. The
text harness (layer 3), the call timeline and the PHI-free log processor are built here.
- **The model (decided 2026-10-20, the owner):** Claude through the Anthropic Messages API
  from the first day, on a dev Console workspace (synthetic data only). Every Claude reply is
  recorded and replayed, keyed by its prompt: a rerun with no change costs nothing and needs
  no network. Not chosen: a local model (llama.cpp) first. It runs Pipecat's OpenAI path, not
  the Anthropic one, and its tool calls and wording do not predict Claude's.
- **The budget (the owner, 2026-10-20): the Max plan's $100 of API credit a month, a hard
  ceiling.** The credit lands in the Deets.Solutions Console organization; the work runs in its
  "DeetsHealth Dev" workspace. No purchased credit and no auto-reload. Every design choice
  counts tokens: the replay cache, prompt caching on the fixed system prompt, a `max_tokens`
  cap on each reply. **Every test starts on the cheapest model (Claude Haiku 5.5) and moves up
  only when a test shows it must** (the owner, 2026-10-20; VOICE-AGENT.md §8.3). This holds for
  the agent and for the test-only roles (the simulated callers of layer 4). The key lives in the git-ignored `.env` (`.env.example`
  shows its shape); `.claude/settings.json` denies Claude any read of it.
- **The Rust `ehr` service is born here (decided 2026-10-21, the owner).** From the first
  typed call it holds the verify state, binds every tool call to the verified patient and runs
  the fake scheduler (SECURITY.md §4.9). The Python agent only asks it; Pipecat Flows decides
  which tools each node offers. Phase 2 swaps the fake scheduler for Epic behind the same
  interface. Not chosen: Python fakes first, which would move the patient binding from Python
  to Rust in Phase 2, the retrofit §1 rules out.
- **The wire is gRPC with mTLS (decided 2026-10-21, the owner):** deadlines and cancels reach
  Rust, and the PHI tag lives in the contract. Not chosen: HTTP with JSON. The design is
  [architecture/EHR-SERVICE.md](architecture/EHR-SERVICE.md).
- **The verify gate is tested with no model:** a scripted stub drives the Flow and asserts
  which tools each node offers. The chart tools must not exist before the check passes.

**Desk test:** the owner types a call and books a visit.

### Phase 2 — the EHR ⬜
E-2: Backend Services sign-in replaces `core/src/auth.rs`. E-3: the reads R1 and R2 through
record and replay, behind the `ehr` service that Phase 1 built.
The pass redesign lands and `core/src/store/` and `payload.rs` move to `archive/`
(CONVERSION.md §5.1). Simulated callers (layer 4) become the test suite.
**Desk test:** the text harness answers "when is my next visit?" from the Epic sandbox's
recorded replies.

### Phase 3 — voice ⬜
The fake Twilio (layer 5). Cartesia Ink in and Sonic out. C6 barge-in, C7 fillers. Layer 7, the
owner's laptop microphone. Measure the 800 ms target and pick the model from the numbers
(VOICE-AGENT.md §8.3).
**Desk test:** the owner speaks and books a visit.

### Phase 4 — the store and the console ⬜
Postgres, sealing, the KMS key (X3). The call log, transcripts, the audit trail (X6, X7). Then
the console K1 to K5, once the owner has designed it (DESIGN.md §C, §D).
**Desk test:** the owner reads the transcript of his own test call in the console; the audit
trail shows the view.

### Phase 5 — the real network ⬜
A Twilio trial number and a tunnel (layer 8). E-4: booking on the Epic sandbox, if it serves
`$find` and `$book`. R4 results with guards. R3 refills, once Epic confirms what a backend app
may do.
**Desk test:** the owner phones the number from his own phone and books a visit.

### Phase 6 — ship ⬜
The deploy interface and A2 (X4). Signed BAAs with every vendor (VOICE-AGENT.md §8.2). The
SECURITY.md §6 paperwork. The first hospital.

## 3. The fork: what goes first while E-1 waits ✅
**Decided 2026-10-20: the text slice first** (the owner). The typed, Epic-free call of Phase 1
is built now; Backend Services sign-in (Phase 2) starts when E-1 lands. The two options, as
they were put:

CONVERSION.md step 5 records "the first feature: SMART Backend Services against the Epic
sandbox." That step is blocked until E-1 lands and the sandbox accepts the new client (about
25 minutes to an hour, EPIC.md §3). Claude wrote the step on 2026-10-19, before the test layers
were designed.
- **A. The text slice first (chosen).** Build the call's shape now. Backend Services starts the
  moment E-1 lands, as a second small feature. The two share no primitive until the `ehr`
  service's contract exists in Phase 2, so the one-in-flight rule holds in spirit.
- **B. Backend Services first, as recorded (not chosen).** Paper work only until E-1 lands. Cost: idle build
  time, and the call's shape, which the patient feels first, waits.

## 4. Choices that wait for their phase
- The console's layout, staff sign-in and which looks it keeps (Phase 4; DESIGN.md §C, §D).
- The agent's words, name and voice (Phase 3 at the latest; DESIGN.md §B).
- Transcript retention, 30 days as a draft (Phase 4; VOICE-AGENT.md §8.7).
- The Spanish path (after Phase 3; VOICE-AGENT.md §8.6).
