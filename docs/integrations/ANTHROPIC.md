---
status: project
desk_test: passed 2026-11-13
sources: [agent/deets_agent/llm.py, agent/deets_agent/config.py, agent/deets_agent/call_record.py]
updated: 2026-11-13
---
# Anthropic — the language model

> In use for test calls since 2026-11-11 (Haiku 5.5; `llm.py`). Config › Anthropic sets its
> levers (§4.2; CONSOLE.md §8). The model API that picks the agent's next step and its words. Model facts from
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

### 4.1 The levers, and how we set them
> **Part:** built · 2026-11-13 — Config › Anthropic sets them for test calls (CONSOLE.md §8).
> Facts from Anthropic's API reference (cached 2026-11-03). The code's values are the Default.

| Lever | What it does | Range | Ours (Default) | Where |
|---|---|---|---|---|
| Model | Which Claude | Haiku 5.5, Sonnet 5.5, Opus 5.5 | `claude-haiku-5-5` | `config.MODEL`; the page |
| Effort (`output_config.effort`) | Tokens and thinking a reply spends | low, medium, high, xhigh, max; the default is medium on Haiku and Opus, high on Sonnet | `low` | `config.EFFORT`; the page |
| Thinking | On: adaptive (no field sent). Off: `disabled` on Haiku, `between_tools` on Sonnet, both at effort high or below; Opus 5.5 cannot | on, off | on | `llm.py › thinking_config`; the page |
| `max_tokens` | The cap on one reply, thinking included | 256–16,000 on the page | 1,200 | `config.MAX_TOKENS`; the page |
| Retry cap | A reply cut off with nothing to say is sent once more with this cap (`llm.py › _CutOffGuard`) | 256–32,000, at least `max_tokens` | 2,400 | `config.RETRY_MAX_TOKENS`; the page |
| Cache life (`ttl`) | The system prompt's cache entry: 5 minutes, or 1 hour at 2× the write price | 5m, 1h | 5m (not sent) | Pipecat's `system_prompt_cache_ttl`; the page |
| Prompt caching | Cache markers on the system prompt and the last messages | on, off | on | `build_llm` |
| `inference_geo` | Where the model runs; `us` costs 1.1× | global, us | `us`, every live request | `llm.py`, added after the replay key |
| Betas | Pipecat asks for `interleaved-thinking`; refusal fallbacks, per-message effort and task budgets are betas | — | all off (outside the BAA) | `llm.py › _clean` |
| `strict` on tools | The tool's input always fits its schema | on, off | off: not set yet (CONSOLE.md §8.4) | `flow.py` |
| `tool_choice` | Forced choice is refused on the 5.5 models | auto | auto | Pipecat |
| Thinking display | Whether thinking text comes back | omitted, summarized | omitted (the default) | — |
| Temperature, top_p, top_k | Refused on the 5.5 models | — | not sent | — |

**Prices** (dollars per 1M tokens; in, out, cache read): Haiku 5.5 $0.10, $0.50, $0.01; Sonnet
5.5 $2, $10, $0.10; Opus 5.5 $4, $20, $0.20. A cache write is 1.25× the input price (5 minutes)
or 2× (1 hour). US-only inference is 1.1× on all of it (`call_record.py › cost_usd`).

### 4.2 How caching works here
- **The API keeps no conversation.** Each turn of a call sends everything again: the system
  prompt, the tool definitions, and every turn so far. Caching lets Anthropic skip the work on
  the part it has already read.
- **What a cache holds:** the model's internal state for a prefix of the request (the key and
  value vectors attention needs for each earlier token). It is found by an exact match on the
  prefix: one changed byte, and everything after it is read again.
- **Two layers in a call:**
  1. **The hospital's prefix** (system prompt, tool definitions, the Flow node's instructions):
     the same for every call at that hospital. Written once, read by every call while it stays
     warm. No patient data in it.
  2. **The call's own history:** each turn extends it, so the next turn reads the earlier turns
     from the cache. This part holds the call's protected data, under the BAA, and expires with
     the cache's lifetime (5 minutes, or 1 hour).
- **Patient data never goes in the prefix.** It arrives through tool results inside the call's
  own messages (SECURITY.md §4.9). This keeps the shared prefix the same for every caller, and it
  keeps one caller's data out of every other call's request.
- **A prefix that is too short is not cached.** Each model has a minimum (512 to 4,096 tokens);
  check `usage.cache_read_input_tokens` on the test calls.
- **Isolation:** caches are kept apart between organizations. One API workspace and key per
  hospital deploy keeps hospitals apart on our side too (to confirm: whether caches are also
  kept apart per workspace).

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
