---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# Cartesia — speech in (Ink) and speech out (Sonic)

> Not set up. Chosen 2026-10-19 (fork F5 A, VOICE-AGENT.md §7). Facts from Cartesia's docs and
> reviewers, read 2026-10-19; model versions change often, so check Cartesia's changelog.

## 1. What we use it for
- **Ink (speech to text, streaming):** the caller's words, with Ink-2's own turn detection (it
  tells us when the caller has finished, not just paused).
- **Sonic (text to speech, streaming):** the agent's voice, starting on the first sentence.
- One vendor for both directions: one BAA, one contract.

## 2. Account and BAA
- **Enterprise plan:** a BAA and **zero data retention (ZDR)** for the TTS and STT APIs. ZDR
  still keeps operational metadata (request ids, usage, account data).
- **No voice cloning:** it is outside ZDR. We pick a stock voice.
- Confirm in writing that ZDR covers the exact Ink and Sonic models we use.
- Cartesia sells through the AWS Marketplace too, which can put it on the cloud bill (CLOUD.md).
- Dev: a standard account with synthetic audio only.

## 3. Configuration
- **Pin a dated model version** for Ink and Sonic. Re-run the listening tests (VOICE-AGENT.md §9
  layer 7) before moving to a new one.
- **Phone audio:** Twilio sends 8 kHz μ-law. Resample with care (PIPECAT.md §4); ask Sonic for
  the output format Twilio takes, so we do not resample twice.
- **A pronunciation dictionary** (Sonic 3 and later): each entry maps a word to IPA or a
  "sounds-like" spelling. Fill it from real misreadings only: drug names, provider names, the
  hospital's own place names. One dictionary per hospital, kept lean. Case-insensitive matching
  needs Sonic 3.6 or later; the API refuses two entries that collide.
- **Numbers, dates, codes:** reviewers say Sonic 3.5 and later read phone numbers and codes well.
  Test it on our own templates ("6.1 percent", "the third of March", a verification code read
  digit by digit) before trusting it; write the template so the text is already unambiguous.
- **Language:** Ink-2 is English only (Cartesia changelog, 2026). Spanish needs another Ink model
  or a second engine (ideas/README.md).

## 4. Cost
Not quoted yet: per character for Sonic, per minute for Ink, on the Enterprise plan. Part of the
cost per call minute (DESIGN.md §2 I).

## 5. What users learned
- Test with real phone audio, accents and noise, not a short demo sentence.
- Keep the pronunciation dictionary small: a fix that is right for one region is wrong in another.
- Pin versions; a new Sonic changes how words sound.

## 6. Considered and not chosen: ElevenLabs
Also offers a BAA and a zero-retention mode on its Enterprise plan. Its agent platform, in
HIPAA mode, limits the language model to its allowlist and turns off MCP tools, so we would
use only its speech APIs. Kept as the fallback if a listening test finds Sonic's voice clearly
worse (VOICE-AGENT.md §7, F5 C).

## 7. To find out
- Enterprise pricing for a first, small hospital.
- Which Cartesia STT model handles Spanish on phone audio.
- Whether ZDR covers the pronunciation dictionary itself.

## 8. Sources
- [Cartesia — Zero Data Retention](https://docs.cartesia.ai/enterprise/zero-data-retention)
- [Cartesia — Custom pronunciations](https://docs.cartesia.ai/build-with-cartesia/sonic-3/custom-pronunciations)
- [Cartesia — Changelog 2026](https://docs.cartesia.ai/changelog/2026)
- [Cartesia — Sonic](https://www.cartesia.ai/sonic) · [Ink](https://www.cartesia.ai/ink)
- [Cartesia on AWS Marketplace](https://aws.amazon.com/marketplace/pp/prodview-bcf67mlwzvkce)
- [Cekura — What Sonic does well and how to test it](https://www.cekura.ai/discover/cartesia-text-to-speech)
- [LiveKit — Cartesia TTS](https://docs.livekit.io/agents/models/tts/cartesia/)
- [invideo — Sonic 3 and 3.5: specs and latency](https://invideo.io/blog/cartesia-sonic-ai-voice/)
- [ElevenLabs — Privacy and compliance (DeepWiki)](https://deepwiki.com/elevenlabs/elevenlabs-docs/5.2-privacy-and-compliance)
