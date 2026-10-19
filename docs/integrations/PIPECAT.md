---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# Pipecat — the call loop

> Not set up. An open-source Python framework (from Daily), not a vendor we pay: we run it in
> each hospital's deploy. Chosen 2026-10-19 with Pipecat Flows, open to heavy tuning (VOICE-AGENT.md
> §8.6). Bug reports below are from GitHub issues and write-ups and may already be fixed: read
> each issue before relying on it.

## 1. What we use it for
- The pipeline: Twilio in → Cartesia Ink → context → Claude → Cartesia Sonic → Twilio out.
- **Pipecat Flows:** the call as a state machine; each node has its own instructions and tools,
  so a chart tool does not exist before verification (VOICE-AGENT.md §8.1).
- Interruption handling, filler speech, per-step metrics.

## 2. Account and BAA
None: we self-host, so no data goes to Daily. **Pipecat Cloud is not used** (another vendor, a
BAA, and it would break the deploy plan). The Python process holds a call's text in memory for
one call (SECURITY.md §4.1).

## 3. Configuration
- **Pin the version**, and read the changelog and open issues before each upgrade.
- **Our own processors** (the tuning order is VOICE-AGENT.md §8.6): the PHI redactor in front of
  every log and metric, the verify gate, a latency recorder per step.
- **Tools call the Rust service only**, bound to the verified patient (SECURITY.md §4.9).
- **Host near Twilio's media region** (TWILIO.md §3).

## 4. What users learned (and what to test first)
- **Barge-in over the phone is harder than in a demo.** An interruption must reach the Twilio
  serializer and send Twilio's `clear` message; if it does not, every barge-in becomes two voices
  at once (issue #366).
- **Interruptions during a tool call** have had several bug classes (queue rebuilds, deadlocks,
  dropped frames, races); in one report the agent went silent until the call ended. Test
  "caller interrupts while the agent books" on the fake Twilio (VOICE-AGENT.md §9 layer 5).
- **8 kHz pacing:** one report (issue #5592, version 1.8.0) says about two of three writes went
  unpaced when resampling to 8 kHz, so audio played too fast. Another reports choppy audio on
  Twilio while the server recording was clear (issue #2551).
- **Turn detection at 8 kHz:** Pipecat's Twilio guide says 8 kHz input, while Smart Turn needs
  16 kHz. We use Ink-2's turn detection instead (CARTESIA.md §1).
- **No `mark` events** in the Twilio serializer, so Pipecat cannot know what the caller heard; the
  transcript may run ahead of the audio. We add marks (TWILIO.md §3).
- **Stalls:** reports of a 1-second aggregation delay from one parameter (issue #1319) and a
  pipeline that stops receiving audio with no error (issue #721). Watch the call timeline.

## 5. To find out
- The current version's state of each issue above.
- Whether Flows can hold our node map with a tool set per node exactly as drawn.

## 6. Sources
- [GitHub — Interruptions not working with Twilio (#366)](https://github.com/pipecat-ai/pipecat/issues/366)
- [GitHub — Voice breakage in a Twilio call (#2551)](https://github.com/pipecat-ai/pipecat/issues/2551)
- [Pipecat Flows overview](https://docs.pipecat.ai/api-reference/pipecat-flows/overview)
- [Pipecat in production: issues and architecture](https://luonghongthuan.com/en/blog/pipecat-voice-agent-production-scalable-guide/)
- [Theory Delta — Pipecat voice framework](https://www.theorydelta.com/findings/pipecat-voice-framework/)
- [Evalgent — Pipecat and Twilio](https://www.evalgent.com/blog/pipecat-twilio-telnyx-phone-calls)
- [Evalgent — Testing a Pipecat agent](https://www.evalgent.com/blog/pipecat-voice-agent-testing-guide)
- [Vadim — Where to put the phone line](https://vadim.blog/twilio-livekit-pipecat-phone-line/)
- [Medium — What it takes to build a Pipecat agent](https://medium.com/@pankaj_pandey/pipecat-what-it-really-takes-to-build-a-real-time-voice-ai-agent-c39d6d31b162)
