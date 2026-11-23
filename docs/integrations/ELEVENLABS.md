---
status: project
desk_test: open
sources: [vendors/catalog/voice/elevenlabs.json, agent/deets_agent/voice/elevenlabs.py, agent/deets_agent/voice_catalog.py, .env.example]
updated: 2026-11-23
---
# ElevenLabs — speech in (Scribe) and speech out (Flash)

> **Part:** built · 2026-11-23 · desk test open (VOICE-VENDORS.md §11.6). The second voice
> vendor, beside Cartesia (the default). How it is built: VOICE-VENDORS.md §11. Facts from the
> account and Pipecat 1.16, read 2026-11-23.

## 1. What we use
- **Scribe realtime** (`scribe_v2_realtime`) to hear the caller, over a WebSocket, in manual
  commit mode. It gives words only; Pipecat's voice detector and Smart Turn decide the turns.
- **Flash** (`eleven_flash_v2_5`) to speak the agent, over a WebSocket.
- Both through Pipecat's own services (`ElevenLabsRealtimeSTTService`,
  `ElevenLabsTTSService`). No ElevenLabs SDK, and nothing on ElevenLabs' agent platform
  (VOICE-AGENT.md §8: in HIPAA mode it limits the model and the tools).

## 2. The account and the key
- A free account, made 2026-11-23. The plan: 10,000 credits a month, renewing on the 24th. It
  cannot go over its limit (`can_extend_character_limit: false`), so nothing is ever billed.
- The key was made by ElevenLabs' CLI: `npx -y @elevenlabs/cli@latest onboard connect --env-path
  .env`. The owner approved it in his browser. The key sits in `.env` as `ELEVENLABS_API_KEY`.
  The backup copy of `.env` is in Deets' Secrets.
- `npx -y @elevenlabs/cli@latest onboard check --env-path .env` checks the key without printing
  it.
- The CLI also signs itself in (OAuth, in Windows Credential Manager) for its own read commands
  (`user subscription get`, `models list`, `voices search`). That sign-in is the CLI's only; the
  agent reads `.env`.

## 3. The BAA and retention
- A BAA, and Zero Retention Mode, are on the Enterprise plan. The free account has neither:
  **synthetic audio only** (SECURITY.md §5).
- Every request sends `enable_logging=false` (zero retention) while the setting
  `elevenlabs.zero_retention` is on, its default.
- **Tested 2026-11-23: the free plan accepts the flag and ignores it.** A Flash request sent with
  it showed in the account's speech history (VOICE-VENDORS.md §11.7). So the code refuses
  ElevenLabs on any call that is not synthetic, with the switch On or Off. Only an Enterprise
  plan with a signed BAA would change that.

## 4. Cost
| What | Rate | Source |
|---|---|---|
| Flash | 0.5 credits a character | the account's model list, 2026-11-23 |
| Scribe realtime | 5.5 credits a second (330 a minute) | a secondary source; **to confirm** on the usage page |
| A 3-minute test call | about 1,700 credits | the two rates above |

## 5. Configuration (Config › Voice)
The rows and their ranges: VOICE-VENDORS.md §11.5 and the catalog file. Voices: ElevenLabs' own
made voices (`premade`) only. A library voice needs a paid plan through the API, and a cloned
voice is outside zero retention.

## 6. To find out
- Scribe realtime's true rate in credits (§4).
- Smart Turn's added wait on this PC, from the Timeline.
- Whether Pronunciations can reach Flash another way (a pronunciation dictionary), since Flash
  v2.5 reads no phoneme tags.
