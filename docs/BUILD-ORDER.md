---
status: project
desk_test: none
sources: []
updated: 2026-11-11
---
# DeetsHealth — Build order

> The path from where we stand to the first hospital. The paper work and the text run are done.
> Three tracks remain: the console, the chart and the store, and the phone and the voice. The
> console starts, and each next step follows from what the console needs. Then going live. The
> state of the whole project is [HANDOFF.md](HANDOFF.md); the record of each sitting is
> [WORKLOG.md](WORKLOG.md).

**Terms**
- **Track:** one of the three bodies of work that must all reach the finish line. Not three
  builds at once: one build is in flight at a time (§4).
- **Desk test:** the owner runs the work by hand and gives feedback.
- **Text run:** the typed call, with no audio and no Epic: text in, text out.
- **Fake scheduler:** the in-memory booking that stands in for an EHR in tests.
- **Record and replay:** each reply from Claude or Epic is recorded once and replayed in tests,
  so a rerun costs nothing and puts no load on Epic.

## 1. Where we stand (2026-11-11)
- **Paper: done.** The repo layout (2026-10-21). The Anthropic dev workspace and the $100 monthly
  ceiling (2026-10-20). The Epic registration: the patient app's registration became the backend
  app, the APIs were checked against Epic's live catalog, SMART v2, and the key set is live on
  Cloudflare ([integrations/EPIC.md](integrations/EPIC.md) §2). Two small items moved on: the
  single check command (§2) and the cost per call minute (phone and voice, §3.3).
- **The text run: closed** (the owner, 2026-11-11). What it built: the call flow (greet, verify,
  menu, book, next visit, hand off, goodbye); the Rust `ehr` service with the verify gate, the
  patient binding and the fake scheduler; gRPC with mTLS between them; the typed harness and the
  dev call record; logs that hold no protected data; the emergency line, the phrase screen, the
  medical-question line, the name guard, the read-back; every prompt word in
  `agent/prompts/AGENT-PROMPT.md`. Three desk tests. 43 agent tests and 32 `ehr` tests. What it
  left is polish (§2), not missing parts. The call timeline it once held is now a console idea
  ([ideas/README.md](ideas/README.md)).

## 2. Next
1. **The single check command** (Claude): `npm run check` and the pre-push hook from
   `../DeetsMusic`. One command runs every test and check.
2. **The console's design, on paper** (the owner, with Claude): what it shows, how staff sign
   in, the look ([DESIGN.md](DESIGN.md) §C, §D). The console track starts here.
3. **Polish, always on.** Every desk test, on any track, feeds fixes back into the call. The
   gaps a caller would notice today are in [architecture/AGENT.md](architecture/AGENT.md) §4: a
   medical question asked with a booking gets the medical line a turn late; the model's own
   words before a fixed line are held by the prompt only; a time said in words is not checked;
   off-topic questions are held by the prompt only.

## 3. The three tracks
Each item says its state in words: **done**, **next**, or **later**.

### 3.1 The console (it leads)
The staff web page: settings, calls, transcripts.
- **Next:** the design on paper (§2).
- **Later:** free the front end from the old Windows host (Tauri), so it runs in a browser
  ([CONVERSION.md](CONVERSION.md) §5.1).
- **Later:** the settings pages, from what the agent already has: what follows the emergency
  line, the medical-question line, the emergency phrase list
  ([architecture/FUTURE-SETTINGS.md](architecture/FUTURE-SETTINGS.md)). The agent reads its
  settings through one interface. A stand-in holds them until the store exists, so the console
  does not wait on the database.
- **Later:** staff sign-in.
- **Later:** the call list, the transcripts and the audit trail. These meet the store (§3.2).
- **An idea, not planned:** a chat view to watch and run calls, with a timeline of each call
  ([ideas/README.md](ideas/README.md)).

**Desk test:** the owner changes a setting in the console, and the next typed call follows it.
Later: he reads the transcript of his own test call, and the audit trail shows that he read it.

### 3.2 The chart and the store
No screens. Epic behind the `ehr` service, and the database.
- **Done:** the Epic registration and the key set (EPIC.md §2).
- **Later:** the Epic sign-in (Backend Services, SMART v2). It replaces `core/src/auth.rs`.
- **Later:** the chart reads through record and replay: visits, medicines and their schedule,
  refill state, lab results with their guards (released results only, never sensitive ones).
  EPIC.md §2.1 lists each API.
- **Later:** booking on Epic's sandbox (`$find` and `$book`), if the sandbox serves it. Moving
  and cancelling hand off to staff: Epic has no API for them (EPIC.md §2.2).
- **Later:** simulated callers become the test suite.
- **Later:** the pass redesign; `core/src/store/` and `payload.rs` move to `archive/`
  (CONVERSION.md §5.1).
- **Later:** the store. Postgres, sealed field by field under the hospital's own key. It holds
  the settings (in place of the stand-in), the call log, transcripts and the audit trail
  ([architecture/VOICE-AGENT.md](architecture/VOICE-AGENT.md) §8.7).

**Desk test:** the typed call answers "when is my next visit?" and "when do I take my
medicine?" from Epic's sandbox, through recorded replies.

### 3.3 The phone and the voice
- **Later:** dev accounts with Twilio and Cartesia, synthetic data only (the owner).
- **Later:** a fake phone line, Cartesia speech in and out, interruptions (barge-in), filler
  words while a slow tool runs, the laptop microphone.
- **Later:** measure the speed against the 800 ms target and pick the model from the numbers
  (VOICE-AGENT.md §8.3). Quote the cost per call minute. Collect the timing of each step; the
  console's call timeline idea would show it.
- **Later:** the owner's audio cues: connect, goodbye, a hold sound for a slow tool.
- **Later:** a real Twilio number and a tunnel. It meets the chart (§3.2) for booking on Epic.

**Desk test:** the owner speaks and books a visit. Then he phones the number from his own phone
and books one.

### 3.4 Going live
The deploy interface and A2, one deploy per hospital in our cloud (VOICE-AGENT.md §7.2). The
production key set on AWS (EPIC.md §2.3). Signed BAAs with every vendor (VOICE-AGENT.md §8.2).
The paperwork in [architecture/SECURITY.md](architecture/SECURITY.md) §6. The first hospital.

## 4. Decisions that shape the order
- **The console starts; each next step follows as we go** (the owner, 2026-11-11). He leads the
  console first, so that its uses, features and settings grow one at a time. Work on the other
  two tracks comes when the console needs it, or while the console waits on the owner's design
  or desk test. **One load-bearing build is in flight at a time** (CLAUDE.md › Working style).
- **The text run first** (the owner, 2026-10-20): the patient feels the call's shape first, and
  text proves the flow for Claude tokens only. Chosen over the Epic sign-in first, which had to
  wait for the registration.
- **Claude through a replay cache, cheapest model first** (the owner, 2026-10-20): Haiku first,
  a hard $100-a-month ceiling, no purchased credit. A local model was not chosen: its tool calls
  and wording do not predict Claude's.
- **The Rust `ehr` service from the first call** (the owner, 2026-10-21): it binds every tool to
  the verified patient, so the binding is never retrofitted (SECURITY.md §4.9).
- **gRPC with mTLS between the agent and `ehr`** (the owner, 2026-10-21): deadlines and cancels
  reach Rust, and the protected-data tag lives in the contract
  ([architecture/EHR-SERVICE.md](architecture/EHR-SERVICE.md)).
- **The verify gate is tested with no model:** a scripted stub drives the flow and checks which
  tools each step offers.
- **The protected-data wall is in from the start** (the verify gate, the logs). It is never
  retrofitted.

## 5. Where the old phase names went
Older docs and the work log name phases. They map to this order:

| Old name | Now |
|---|---|
| Phase 0, paper | §1, done |
| Phase 1, the text slice | §1, the text run, closed 2026-11-11 |
| Phase 2, the EHR | §3.2, the chart and the store |
| Phase 3, voice | §3.3, the phone and the voice |
| Phase 4, the store and the console | §3.2 (the store) and §3.1 (the console) |
| Phase 5, the real network | §3.3 (the real number) and §3.2 (booking on Epic) |
| Phase 6, ship | §3.4, going live |

## 6. Choices that wait for their work
- The console's layout, staff sign-in and its looks (§3.1; DESIGN.md §C, §D).
- The agent's words, name and voice (§3.3 at the latest; DESIGN.md §B).
- Transcript retention, 30 days as a draft (§3.2; VOICE-AGENT.md §8.7).
- The Spanish path (after §3.3; VOICE-AGENT.md §8.6).
- Outbound calls, medication check-ins among them (after inbound; ideas/README.md).
