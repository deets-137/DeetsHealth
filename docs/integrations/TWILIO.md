---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# Twilio — the phone line

> Not set up. What we use Twilio for, its BAA, how to configure it, what it costs, and what its
> users learned the hard way. Facts from Twilio's pages and practitioners' write-ups, read
> 2026-10-19; confirm each in Twilio's console and contract. The choice of Media Streams over
> ConversationRelay is VOICE-AGENT.md §8.6.

## 1. What we use it for
- **Phone numbers** for each hospital's agent line (or the hospital forwards its own line).
- **Programmable Voice** to take the call, and **Media Streams** (a two-way WebSocket of 8 kHz
  μ-law audio) to carry the audio to our Pipecat service and back.
- **Programmable Messaging (SMS)** for the verification code, and an **outbound call** for the
  call back to the number on file (SECURITY.md §4.4).
- **Transfer** (`<Dial>`) to a person at the hospital.
- Not used: ConversationRelay (no Cartesia), Twilio's recording and transcription (we keep our
  own, sealed, only when a rule says so).

## 2. Account and BAA
- Twilio signs a BAA on its **Security or Enterprise Edition**. Only products on its
  HIPAA-eligible list are covered: Programmable Voice, Media Streams, Programmable Messaging,
  Elastic SIP Trunking and ConversationRelay are listed. A product not on the list falls outside
  the BAA, so check the current list before using any new Twilio feature with a real call.
- **One subaccount per hospital deploy**, so numbers, logs and keys never mix.
- Dev: a trial or a dev subaccount with synthetic calls only (PROTOCOLS.md §8).

## 3. Configuration
- **Region and edge:** keep the media edge and our call service close. Twilio's own guidance:
  the distance that matters is from Twilio's media ingress to our service, not to the caller.
  US1 (Virginia) is the default; host the call service near it (CLOUD.md).
- **Validate the `X-Twilio-Signature`** on every webhook before acting; an unsigned request can
  flood our quota or start calls.
- **Barge-in:** when the caller interrupts, send Media Streams' `clear` message so Twilio drops
  the audio it still holds. Without it, old audio plays for 2–3 seconds after the caller
  speaks.
- **`mark` messages** tell us what the caller actually heard. Pipecat's serializer does not use
  them today (PIPECAT.md §4), so the transcript can say more than the caller heard. We add them.
- **Log hygiene:** Twilio's own request logs hold phone numbers. Keep them in the hospital's
  subaccount, set the shortest retention Twilio allows, and never copy them into our logs.
- **Caller ID** is not proof of identity (SECURITY.md §4.4): it starts the patient search, it
  never ends verification.

## 4. Cost (US, 2026, public pages)
| Item | Price |
|---|---|
| Inbound voice minute | about $0.0085 |
| Media Streams minute | $0.0044 |
| **Total for one inbound minute on Media Streams** | **about $0.013** |
| ConversationRelay (not used) | about $0.07 a minute on top |
| A phone number | about $1 a month |
| SMS, outbound call back | per message and per minute; quote with the first hospital |

## 5. What users learned
- Latency adds up across about ten network hops in a cascaded agent; Twilio's own example totals
  about 1.1 s mouth to ear. Measure each step separately (DEBUGGING.md §2).
- Flush on barge-in (§3), or the agent talks over the caller.
- Reuse connections between turns; never reconnect per turn.
- Wait for a final transcript before a consequential action (booking, cancel); start lookups
  from partial transcripts if useful.
- Alert on spend, call volume, failure rate, latency and transfer rate.

## 6. To find out
- The edition and price of the BAA tier for a small first customer.
- Whether a hospital forwards its main number to ours, or moves a number into our subaccount.
- Twilio's minimum log retention for a HIPAA account.

## 7. Sources
- [Twilio — Core latency in AI voice agents](https://www.twilio.com/en-us/blog/developers/best-practices/guide-core-latency-ai-voice-agents)
- [Twilio — HIPAA eligible products (PDF)](https://twilio-cms-prod.s3.amazonaws.com/documents/HIPAA-eligible-products-and-services.pdf)
- [Twilio — ConversationRelay is HIPAA eligible](https://www.twilio.com/en-us/changelog/conversationrelay-is-now-hipaa-eligible)
- [Twilio — Voice pricing](https://www.twilio.com/en-us/voice/pricing/gb)
- [Zian AI — Twilio vs Telnyx costs](https://zian.ai/twilio-vs-telnyx-ai-voice-agents/)
- [Husnain Bukhari — Self-hosting a voice agent: the latency budget](https://www.husnainbukhari.com/blog/self-host-ai-voice-agent)
- [DEV — A sub-500 ms voice agent on Twilio](https://dev.to/devtrivedi/how-we-built-a-sub-500ms-real-time-ai-voice-agent-with-typescript-twilio-websockets-3hlg)
- [Strac — Is Twilio HIPAA compliant](https://www.strac.io/blog/is-twilio-hipaa-compliant)
