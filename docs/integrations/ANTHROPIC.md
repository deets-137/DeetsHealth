---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# Anthropic — the language model

> Not set up. The model API that picks the agent's next step and its words. Model facts from
> Anthropic's API reference (cached 2026-10-06); voice-latency figures from practitioners'
> write-ups. Which model we use is a measure, not a fork yet (VOICE-AGENT.md §8.3).

## 1. What we use it for
- **The Messages API with our own tools** (find slots, book, read visits, read medicines, hand
  off), streaming, inside each Pipecat Flows node (PIPECAT.md).
- It routes and words. It never names a patient, never checks a verification answer, and never
  writes a value from the chart into speech by itself (SECURITY.md §4.9).

## 2. Account and BAA
- **"HIPAA readiness"**: the BAA is signed in the Claude Console. The Messages API is eligible.
- **Beta features are reported as outside the BAA.** Keep the PHI path on generally available
  features only; check the eligibility table before turning on any beta.
- ZDR is not needed for PHI under HIPAA readiness. Claude Fable 5.1 needs 30-day retention and
  is not in our plan.
- **US-only inference:** `inference_geo: "us"` on each request.
- Dev: a separate workspace, synthetic calls only.

## 3. The models (cached 2026-10-06)
| Model | ID | Input / output per 1M tokens | Note for voice |
|---|---|---|---|
| Claude Opus 5.5 | `claude-opus-5-5` | $4 / $20 | Thinking cannot be turned off; use effort `low`. The first candidate. |
| Claude Sonnet 5.5 | `claude-sonnet-5-5` | $2 / $10 | `thinking: {type: "between_tools"}` turns thinking off (effort `high` or below). |
| Claude Haiku 5.5 | `claude-haiku-5-5` | $0.10 / $0.50 | The cheapest; check its tool accuracy. |

**The measure** (VOICE-AGENT.md §8.3): time to first token and tool accuracy for each model on
the same recorded test calls. The owner picks from the numbers.

## 4. Configuration
- **Prompt caching:** the system prompt and tool definitions come first and stay byte for byte
  the same across turns; the call's turns come after. Put the timestamp, the call id and the
  hospital's settings after the cache breakpoint, or every turn misses the cache.
- **Pre-warm the cache** before the caller's first turn (a request with `max_tokens: 0`) so the
  first answer does not pay the cold-cache cost.
- **The 1-hour cache** for a hospital's system prompt, since calls arrive minutes apart.
- **Forced tool choice is refused** on the 5.5 models: use `tool_choice: auto`, `strict: true` on
  each tool, and the Flow node's instructions.
- **Check `stop_reason`** before reading the content; a `refusal` hands the call to a person.
  Server-side refusal fallbacks are a beta: check the BAA before using them.
- **Streaming** with fine-grained tool input streaming, so a short tool call does not wait.
- **One Flow node, one small tool set** (PIPECAT.md): fewer tools, faster and more accurate.

## 5. Cost
Per call: tokens per turn × turns, with most of the prompt read from the cache. To be measured on
the test calls and quoted with the cost per minute (DESIGN.md §2 I).

## 6. What users learned
- Practitioners budget about 200–400 ms for the first token of a cached mid-size model inside a
  sub-second turn. Not an official figure; measure it.
- Caching cuts first-token time a lot on long prompts (Anthropic's own test: 11.5 s to 2.4 s on a
  100,000-token prompt); expect less on a short voice prompt.
- Say a short filler while a tool runs ("One moment, I am checking the schedule").
- Stream the first sentence to TTS before the model finishes.

## 7. To find out
- The eligibility table entry for each feature we use.
- Rate limits for our tier at peak call volume.

## 8. Sources
- [Anthropic — API and data retention](https://platform.claude.com/docs/en/manage-claude/api-and-data-retention)
- [Anthropic — Prompt caching docs](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)
- [Anthropic — Prompt caching launch](https://www.anthropic.com/news/prompt-caching)
- [IntuitionLabs — Is Claude HIPAA compliant](https://intuitionlabs.ai/articles/is-claude-hipaa-compliant)
- [aiinfox — Voice agents under one second](https://aiinfox.com/blog/voice-agents-sub-second-latency)
- [Claudexia — Voice agents with Claude in 2026](https://claudexia.tech/blog/claude-voice-realtime-stack-2026)
- [SigNoz — Reducing Claude API latency](https://signoz.io/guides/claude-api-latency/)
