---
status: project
desk_test: open
sources: [agent/deets_agent/voice.py, agent/deets_agent/config.py, agent/deets_agent/resources.py, agent/deets_agent/flow.py]
updated: 2026-11-16
---
# Cartesia — speech in (Ink) and speech out (Sonic)

> Chosen 2026-10-19 (fork F5 A, VOICE-AGENT.md §7). Wired into the dev voice seat 2026-11-13
> (`voice.py`; CHAT.md §17), not yet run: the account and key come first (§2). Facts from
> Cartesia's docs, read 2026-11-12; model versions change often, so check Cartesia's changelog.
> **§9 lists every lever and how we set it.**

## 1. What we use it for
- **Ink (speech to text, streaming):** the caller's words, with Ink-2's own turn detection (it
  tells us when the caller has finished, not just paused). Turn detection is on its own
  endpoint, `wss://api.cartesia.ai/stt/turns/websocket`; the plain `/stt/websocket` has none
  (§8).
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
- **Pin the model by name** for Ink and Sonic: `ink-2` and `sonic-3.6`, never `sonic-latest` or
  `ink-preview`. The API reference (read 2026-11-12) lists no dated snapshots, so the name is the
  pin. The API reference shows version `2026-08-14`; Pipecat 1.16 sends `2026-03-01` and allows
  no other (§9). Re-run the listening tests
  (VOICE-AGENT.md §9 layer 7) before moving to a new one.
- **Phone audio:** Twilio sends 8 kHz μ-law. Both STT endpoints take `pcm_mulaw` (and
  `pcm_s16le`, `pcm_alaw`, others) at a sample rate we name, so Ink needs no resample. Pipecat's
  turns service always sends `pcm_s16le` at the pipeline's rate (§8). Sonic can send
  `pcm_mulaw` at 8000 Hz, Twilio's format, so we do not resample twice (PIPECAT.md §4).
- **Keyterms (Ink-2):** up to 100 words or phrases, 1,200 characters in all, to help Ink hear
  them (drug names, provider names, the hospital's places). They go in the socket's URL, so a
  keyterm is never a word from one patient's chart: hospital-level lists only.
- **A pronunciation dictionary** (Sonic 3 and later): each entry maps a word to IPA or a
  "sounds-like" spelling. Fill it from real misreadings only: drug names, provider names, the
  hospital's own place names. One dictionary per hospital, kept lean. Case-insensitive matching
  needs Sonic 3.6 or later; the API refuses two entries that collide.
- **Numbers, dates, codes:** reviewers say Sonic 3.5 and later read phone numbers and codes well.
  Test it on our own templates ("6.1 percent", "the third of March", a verification code read
  digit by digit) before trusting it; write the template so the text is already unambiguous.
- **Language:** Cartesia's model page (read 2026-11-12) lists `ink-2` (released 2026-09-17) for
  English, French, Hindi, Japanese and Spanish, with the language detected automatically. An
  older Cartesia page and Pipecat 1.16 still say English only, and Pipecat labels every
  transcript English. Test Spanish on phone audio before we trust it (DESIGN.md C8).
- **Voice:** Cartesia's guide says stable, realistic voices do better in agents than studio or
  emotive ones. Its list for agents: Skylar, Gemma, Jacqueline, Emma, Lauren, Daniel, Archie,
  Parker, Jameson, Corey. The guide says nothing about 8 kHz phone audio: listen on a phone.
  **Emma is gone (2026-11-16):** Cartesia's voice list has no voice of that name, and its old id
  answers 404 (`voice_not_found`); it left the console's list (`config.RECOMMENDED_VOICES`). The
  other nine answer. A voice call now checks the active voice before it starts.

## 4. Cost
> **Part:** project · 2026-11-12

Public prices (cartesia.ai/pricing, read 2026-11-12). Enterprise is not quoted yet. Part of the
cost per call minute (DESIGN.md §2 I).

**Credits:**
- **Sonic:** 1 credit per character, about 750–800 credits per minute of speech.
- **Ink:** 3 credits per second of audio, so 180 credits per minute, the same on both STT
  endpoints. **Silence is billed** (docs.cartesia.ai/pricing: "Silence is also included, even if
  no transcript is produced"), so the whole call is billed while the socket streams.
- **Failed requests** cost nothing. The docs do not say whether a cancelled Sonic request
  (barge-in) is billed in full.
- **Out of credits:** with overage off (Free has no overage), a request that would go past the
  allowance fails until the month renews.

| Plan | Price per month | Credits per month | Above the allowance | Concurrency (TTS / STT) | BAA |
|---|---|---|---|---|---|
| Free | $0 | 20K | No overage | 2 / 8 | No |
| Pro | $5 | 100K | $65 per 1M | 3 / 12 | No |
| Startup | $49 | 1.25M | $45 per 1M | 5 / 20 | No |
| Scale | $299 | 8M | $38 per 1M | 15 / 60 | No |
| Enterprise | Custom | Custom | Custom | Custom | Yes (with ZDR, §2) |

**A 3-minute dev call (estimate):**
- Ink: 3 min × 180 = 540 credits.
- Sonic: about 1.5 min of agent speech, about 1,200 credits.
- Total: about 1,750 credits. That is about $0.11 per call, or about $0.04 per call minute, at
  Pro's overage rate.
- Free gives about 11 such calls a month; Pro about 57.

**Dev:** Pro is the recommended plan (not yet decided by the owner). Free stops at its allowance;
Startup is more than dev needs. Synthetic audio only (§2): no plan below Enterprise has a BAA.
The text harness and the replay cache (VOICE-AGENT.md §9) never touch Cartesia, so only voice
tests (layers 5–7) spend credits.

**Twilio, for the same call (TWILIO.md):** nothing with the laptop microphone or the fake Twilio.
With a real number: about $1.15 per month for the number and about $0.0085 per minute for an
inbound call (third-party guides, not Twilio's rate table), plus $0.004 per minute for Media
Streams (Twilio's pricing page).

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
- Whether a Sonic request cancelled by barge-in is billed for all its characters.
- Whether Ink-2 hears Spanish well on phone audio (§3).

## 8. The Pipecat parts (Pipecat 1.16, read 2026-11-12)
- **`CartesiaTurnsSTTService`** (`pipecat.services.cartesia.turns.stt`): the turns endpoint.
  Ink's `turn.start` interrupts the agent; `turn.end` gives the final words. It tells the user
  aggregator to leave turn detection to Cartesia, so no local VAD or smart-turn model runs.
  The four turn thresholds keep Cartesia's defaults unless we set them. Eager end of turn is
  off by default: it starts Claude on a guess that Cartesia may withdraw.
- **`CartesiaTTSService`** (`pipecat.services.cartesia.tts`): Sonic over its WebSocket. Default
  model `sonic-3.6`; it takes the voice id, the output encoding and rate, and a
  `pronunciation_dict_id`.
- **Fixed lines need no change:** the agent already sends them as `TTSSpeakFrame`
  (`flow.py`, `screen.py`), which Sonic speaks. A `TTSSpeakFrame` with `interruptible=False`
  plays to its end even when the caller speaks.
- **Extras:** `pipecat-ai[cartesia]`; `pipecat-ai[webrtc]` for SmallWebRTC (the laptop
  microphone through the browser; on this PC it needs no STUN or TURN server).

## 9. The levers, and how we set them
> **Part:** built · 2026-11-13 — the owner asked for this list "in case we want to tune or
> reconsider decisions". Pipecat 1.16. Where a cell says *default*, we leave Cartesia's or
> Pipecat's own value; to change it, set it in the code named.

**Ink, speech in** (`voice.py › build_stt`, `CartesiaTurnsSTTService`)
| Lever | What it does | Range | Ours | Why |
|---|---|---|---|---|
| Endpoint | Turns endpoint (Ink finds the end of a turn) or plain (we find it, with a local voice detector) | turns, plain | turns | F5; the same price; no local model to tune |
| Model | `ink-2` stable, `ink-preview` newer, `ink-whisper` older and multilingual | three | `ink-2` (`config.STT_MODEL`) | Pinned, stable |
| `turn_start_threshold` | How sure Ink must be that the caller has started, before `turn.start` (which interrupts the agent) | 0.5–0.9 | default 0.8 | Higher: fewer false barge-ins from a cough or noise, slower to stop the agent |
| `turn_end_threshold` | How sure Ink must be that the turn has ended, before `turn.end` | 0.05–0.5 | default 0.3 | Lower: waits for more certainty, fewer cut-off callers, slower replies |
| `turn_end_timeout_ms` | The longest Ink waits after speech stops before it ends the turn anyway | 640–11,200 | default 5,600 | A slow speaker keeps the turn; a long pause still ends it |
| `turn_eager_end_threshold` and eager end of turn | Start Claude on Ink's early guess that the turn ended; drop the reply if the caller goes on | 0.3–0.8; on or off | default 0.6; **off** | A guess could start a tool (a text, a booking), and each guess costs Claude tokens. The 800 ms tuning (BUILD-ORDER.md §3.3) may turn it on |
| Profiles | Not an API parameter: Cartesia's names for sets of the four turn values (start, eager end, end, timeout). **Balanced** (the default): 0.8, 0.6, 0.3, 5,600 ms. **Responsive** (fast back-and-forth): 0.7, 0.6, 0.4, 4,500 ms. **Patient** (callers who pause to think): 0.8, 0.3, 0.1, 8,000 ms (docs.cartesia.ai › STT › Turns, read 2026-11-13) | three | Balanced (`config.TURN_PROFILES`) | Choosing one in Config moves the four sliders (CONSOLE.md §7). Try Patient for older callers when we tune |
| Barge-in (`should_interrupt`) | `turn.start` stops the agent's speech | on, off | on | The caller is never talked over. The 911 line alone plays to its end (`flow.py › emergency_frame`) |
| `keyterm` | Up to 100 words or phrases (1,200 characters) Ink listens for | list | none | Later: hospital-level lists only, never a word from one chart (§3) |
| Language | Ink-2 detects it; Pipecat labels every line English | auto | auto | Spanish untested (§3) |
| Input audio | Encoding and rate sent to Ink | Pipecat fixes `pcm_s16le` | 16 kHz, the pipeline's input rate | Twilio's 8 kHz comes later (§3) |
| `watchdog_min_timeout` | When the microphone stops mid-turn, Pipecat sends silence after this long so Ink can end the turn | seconds | default 0.5 | — |

**Sonic, speech out** (`voice.py › build_tts`, `CartesiaTTSService`)
| Lever | What it does | Range | Ours | Why |
|---|---|---|---|---|
| Model | The voice engine | `sonic-3.6`, `3.5`, `3`, `sonic-latest` | `sonic-3.6` (`config.TTS_MODEL`) | Newest; case-insensitive dictionary; never `latest` |
| Voice | Who speaks | a voice id | Skylar, a draft (`config.DEFAULT_VOICE_ID`; `CARTESIA_VOICE_ID` in `.env` overrides) | From Cartesia's stable agent voices (§3); the owner picks by ear |
| `generation_config.speed` | Speaking rate | 0.6–1.5 | default | Try slower for older callers when we tune |
| `generation_config.volume` | Loudness | 0.5–2.0 | default | — |
| `generation_config.emotion` | `neutral`, `calm`, `angry`, `content`, `sad` | five | default | Calm is a candidate; test it |
| `normalization` | How numbers, dates and phone numbers are read; `off` needs text already spelled out | `auto`, `off`, a locale | default (`auto`) | Our fixed lines are written to read clearly (§3) |
| `pronunciation_dict_id` | A dictionary of words and how to say them | an id | none | Filled from real misreadings only (§3); a hospital setting later (FUTURE-SETTINGS.md) |
| `language`, `accent` | The language and, for a multilingual voice, its accent | codes | English, no accent | — |
| Text aggregation | Pipecat sends Sonic a whole sentence (`SENTENCE`) or each token (`TOKEN`) | two | default `SENTENCE` | `TOKEN` saves about 200–300 ms a sentence (Pipecat's note); try it when we tune |
| `max_buffer_delay_ms` | How long Sonic waits for more text before it starts | 0–5,000 | 0 (Pipecat's for `SENTENCE`) | No wait on a whole sentence |
| Output audio | Encoding and rate | `pcm_s16le`, `pcm_mulaw`, …; 8–48 kHz | `pcm_s16le` at 24 kHz, the pipeline's output rate | For the browser; Twilio will take `pcm_mulaw` at 8 kHz (§3) |
| Word timestamps | Sonic says when each word plays; Pipecat uses them so the model's memory holds only the words the caller heard | on, off | on (Pipecat's) | An interrupted reply is not remembered as said |
| API version | `cartesia_version` | a date | `2026-03-01`, fixed by Pipecat 1.16 | Cartesia's reference shows `2026-08-14`; moves with Pipecat |

**Not on the Config page yet** (a review, 2026-11-13; CONSOLE.md §7.6): text aggregation, text
transforms (Pipecat's find and replace before Sonic: a way to carry the pronunciations with no
Cartesia dictionary), emotions past our five (Sonic 3 takes over 60, best with voices outside
our list), language and accent, number reading by locale.

**Ours, around them** (`config.py`)
| Lever | What it does | Ours | Why |
|---|---|---|---|
| `VOICE_IDLE_S` | A voice test call ends after this long with no speech from the caller | 60 s | The owner, 2026-11-13; FUTURE-SETTINGS.md §12 |
| `VOICE_MAX_S` | A voice test call ends at this length | 6 min | The owner, 2026-11-13 |
| `CARTESIA_PLAN_CREDITS` (`.env`) | The plan's credits a month, for the Resources tab | 20,000 (Free) | Set it when the plan changes |
| `CARTESIA_CYCLE_DAY` (`.env`) | The day of the month the credits renew | 1 | Set it to the day the account was opened |
| `INK_CREDITS_PER_S`, `SONIC_CREDITS_PER_CHAR` | The rates the Resources tab counts with | 3, 1 | Cartesia's pricing page (§4) |

## 10. Sources
- [Cartesia — Pricing](https://cartesia.ai/pricing) · [How usage is charged](https://docs.cartesia.ai/pricing)
- [Cartesia — STT turns WebSocket](https://docs.cartesia.ai/api-reference/stt/turns/websocket) · [STT WebSocket](https://docs.cartesia.ai/api-reference/stt/stt) · [TTS WebSocket](https://docs.cartesia.ai/api-reference/tts/websocket)
- [Cartesia — STT models](https://docs.cartesia.ai/build-with-cartesia/stt-models) · [Choosing a voice](https://docs.cartesia.ai/build-with-cartesia/capability-guides/choosing-a-voice)
- [Pipecat — CartesiaTurnsSTTService source](https://reference-server.pipecat.ai/en/stable/_modules/pipecat/services/cartesia/turns/stt.html) · [SmallWebRTC transport](https://docs.pipecat.ai/server/services/transport/small-webrtc)
- [Twilio — Voice pricing (US)](https://www.twilio.com/voice/pricing/us)
- [Cartesia — Zero Data Retention](https://docs.cartesia.ai/enterprise/zero-data-retention)
- [Cartesia — Custom pronunciations](https://docs.cartesia.ai/build-with-cartesia/sonic-3/custom-pronunciations)
- [Cartesia — Changelog 2026](https://docs.cartesia.ai/changelog/2026)
- [Cartesia — Sonic](https://www.cartesia.ai/sonic) · [Ink](https://www.cartesia.ai/ink)
- [Cartesia on AWS Marketplace](https://aws.amazon.com/marketplace/pp/prodview-bcf67mlwzvkce)
- [Cekura — What Sonic does well and how to test it](https://www.cekura.ai/discover/cartesia-text-to-speech)
- [LiveKit — Cartesia TTS](https://docs.livekit.io/agents/models/tts/cartesia/)
- [invideo — Sonic 3 and 3.5: specs and latency](https://invideo.io/blog/cartesia-sonic-ai-voice/)
- [ElevenLabs — Privacy and compliance (DeepWiki)](https://deepwiki.com/elevenlabs/elevenlabs-docs/5.2-privacy-and-compliance)
