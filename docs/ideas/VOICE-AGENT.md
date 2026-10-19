---
status: idea
desk_test: none
sources: []
updated: 2026-10-19
---
# Idea — a voice agent

> Not built, not designed: forks open (§8). The owner (2026-10-19): "make voice agents". Do not
> tell a user the app does this. Under the recommended scope (V1 A) it changes no decided rule.
> Option V1 B breaks DESIGN.md §5 and SECURITY.md §4.1 and §8.1.

## 1. Terms
- **Voice agent** — the patient speaks a request, and the app does it or answers it.
- **Ask** — one spoken request, from the start of speech to the end.
- **STT** (speech to text) — the step that turns the audio of an ask into words.
- **TTS** (speech synthesis) — the step that reads an answer out loud.
- **Router** — the step that turns the words of an ask into one app action: open a card,
  set a filter, find a value. It never writes the answer.
- **Compass** — DeetsMusic's command bar. It is planned for DeetsHealth and not yet in the
  tree (WORKLOG.md, 2026-09-21).
- **Push to talk** — the microphone is on only while the user holds a key or a button.

## 2. Why the patient gains
- A person with low vision, a tremor or arthritis can reach a value with no small targets.
- An older parent can ask "when was my last flu shot" and not learn the hubs first.
- A person in a clinic room can find a medicine list fast, with both hands busy.
- It costs no call to a FHIR server. Every answer comes from the sealed cache (LESSONS.md §3.3).

## 3. The danger: a new channel for protected data
Voice adds two channels that the PHI wall does not cover today (SECURITY.md §4):
1. **The ask itself is protected data.** "What was my HIV test" is a health fact in the
   user's own voice. The audio and the words get the wrapper type, the same as a record value.
2. **The answer out loud.** A spoken value reaches every person in the room. A screen can be
   turned away; a speaker cannot.

Two common shortcuts send audio off the PC with no sign to the user:
- **The Web Speech API's recognition.** In Chrome and in Edge (so in WebView2), it sends the
  audio to the browser maker's cloud. It is ruled out for STT on every platform.
- **Online TTS voices.** Edge's "Natural" voices run in Microsoft's cloud. The app uses only
  voices where `SpeechSynthesisVoice.localService` is `true`, or the platform's own offline
  voice.

## 4. The shape (recommended)
```
hold key ─► mic ─► on-device STT ─► words (Shielded) ─► router ─► app action
                                                                  │
                     card on screen ◄──────── answer from template ┘
                                         └──► on-device TTS (§5)
```
- **Push to talk only.** No wake word, no microphone that is always on. An always-on mic in a
  health app is less secure and gives the patient little more than a key press.
- **The router only routes.** It picks a card and its filter, as typed Compass would. The
  ideas README already sets this rule for the on-device model: never for dose text or any
  value the user reads.
- **The answer is a template, filled from the normalized model.** "Your last A1C was 6.1
  percent, on 3 March." No model writes a sentence about a value. So the app cannot speak a
  number that the record does not hold.
- **The card opens with the answer.** The spoken answer is never the only copy. The user can
  read it, check the date and see the reference range.
- **No advice.** "Is that high?" gets the site's reference range and the flag, read from the
  record, and nothing more (DESIGN.md §5).
- **Nothing kept.** The audio stays in memory for one ask and is zeroed after STT. The words
  are dropped after the router runs. No history of asks unless the owner asks for one later.
- **Log lines carry no words.** `voice: 1 ask, 2.1 s, routed to labs` — never the ask, never a
  value, never the card's title if it names a test.

## 5. Speaking values out loud
The answer has two levels:
- **Kind only:** "3 lab results. They are on the screen."
- **Values:** "Your last A1C was 6.1 percent, on 3 March."

The fork V2 (§8) decides which one is the default. In both options, a setting changes it, and
the Windows screen-reader state can be a hint.

## 6. The engines (a choice for later; measure first)
| Step | Windows | Web | macOS and iPhone |
|---|---|---|---|
| STT | whisper.cpp in Rust (`whisper-rs`, a 40–150 MB model) **or** the WinRT `SpeechRecognizer` with an offline language pack | whisper.cpp built to WASM, in a worker | `SFSpeechRecognizer` with `requiresOnDeviceRecognition = true` |
| Router | rules on the words first; the Cactus model (ideas README) for unclear asks | the same, in WASM | the same |
| TTS | the OneCore offline voices (WinRT `SpeechSynthesizer`) | `speechSynthesis`, local voices only | `AVSpeechSynthesizer` |

Each one sits behind a core platform trait, as the key store does (DESIGN.md §6). The choice
between whisper.cpp and the WinRT recognizer needs a measure: accuracy on drug and test
names, start time, and installer size. Medical words are where a small STT model fails most.
The router must show what it heard ("Heard: metformin") before it acts on a low-confidence ask.

## 7. What a phone agent would need (option V1 B)
A voice agent that phones a clinic for the patient (book a visit, ask for a refill) needs:
a cloud phone line (Twilio or similar), a cloud speech model, and a cloud language model that
hears protected data. It also acts on the patient's behalf at the site. That breaks three
decided rules: "No cloud copy of the record" and "No writes back to the health system"
(DESIGN.md §5), and "no AI route" (SECURITY.md §8.1). It is in this doc so the owner can say
whether those rules still hold for it (LESSONS.md §5.2, "A rule can be wrong").

## 8. Forks (for the owner, when this comes up)
2 real forks; 5 things follow.

**What follows:**
- **On-device STT and TTS only.** A cloud engine hears protected data (§3); SECURITY.md §8.1.
- **Push to talk, no wake word** (§4).
- **The router routes; templates answer** (§4). The on-device model rule already decides it.
- **The audio and the words are protected data**, in the wrapper type, never logged (§3, §4).
- **When:** after Compass and the first hubs. Voice is a layer on Compass, as the on-device
  model is. The one-feature rule decides it (CLAUDE.md › Working style).

**The real forks:**
- **V1. Scope:** A an in-app voice agent: the patient asks, the app finds and answers
  from the record (recommended; §4) · B a phone agent that calls a clinic for the patient (§7).
  B breaks three decided rules; it stays a fork only if the owner says those rules can change
  for it.
  - Windows: A runs fully on the PC. B needs a cloud account and a phone number.
  - Web: A needs STT in WASM (the browser's own STT sends audio to the cloud). B is the same as
    on Windows.
  - macOS and iPhone: A uses the native on-device engines. B is the same as on Windows.
- **V2. Values out loud by default:** A speak the kind only by default; a setting turns on
  values (recommended: safe in a shared room, one switch for a person who needs it) · B speak
  the values by default; a setting turns them off (best for a blind user on the first day).
  - Every platform: the same setting. On Windows and macOS, a running screen reader can make
    the first-run prompt suggest B.

**Not now:** the STT engine on Windows (§6) waits for a measure.
