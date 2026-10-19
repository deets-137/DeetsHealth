---
status: project
desk_test: none
sources: []
updated: 2026-10-19
---
# DeetsHealth — Dev and helper protocols (a draft)

> **Tentative** (the owner, 2026-10-19: "document a protocols.md on dev / helper protocols
> tentatively"). How the owner, the people who help him, and their AI agents work in this repo.
> Every rule here is a draft for the owner to edit; §10 lists what is still his to decide. The
> model is DeetsMusic's way of working (CLAUDE.md, `../DeetsMusic/CLAUDE.md`).

## 1. Terms
- **The owner** — the person who decides every fork and owns the repo.
- **A helper** — a person who writes code or docs here at the owner's ask.
- **An agent** — an AI coding tool a helper or the owner runs (Claude Code, Codex, others).
- **A fork** — a choice with more than one real option (CLAUDE.md › Working style).
- **Protected data** — PHI and PII (SECURITY.md §2).
- **Synthetic data** — sandbox patients and simulated callers. Public fakes.

## 2. The rules that never bend
1. **No real protected data, anywhere, until the owner says so** in writing, after every BAA is
   signed (SECURITY.md §4.8). Not in a branch, an issue, a PR, a screenshot, a chat, an agent's
   prompt, a test fixture or a demo.
2. **No secret in the repo.** Keys and client IDs live in the secret manager or a git-ignored
   file (`epic-client.json`). A secret that reaches git is rotated at once, then removed.
3. **The owner decides every fork.** A helper brings the options and a recommendation; a helper
   never ships a choice the owner has not seen.
4. **No force-push. No history rewrite on a shared branch. No new remote.**
5. **The PHI wall** (SECURITY.md §4): a change that touches the wrapper type, sealing, the
   verification step, the tool binding or the logs needs the owner's review before it merges.

## 3. Joining and leaving
**Joining:**
1. The owner adds the helper to the private GitHub repo, with the least access the work needs.
2. The helper reads CLAUDE.md, HANDOFF.md, DESIGN.md, SECURITY.md and this doc.
3. Vendor access is through the project's shared accounts, never a personal one, and only to
   dev projects that hold synthetic data.
4. A helper who may ever touch real protected data first signs the agreements and takes the
   training that HIPAA asks of workforce (SECURITY.md §6). Until then: synthetic data only.

**Leaving:** the owner removes repo and vendor access the same day, and rotates any secret the
helper could read.

## 4. Picking up work
- **One load-bearing feature in flight** (CLAUDE.md). A helper takes work that does not touch
  the schema, the PHI wall or a shared primitive at the same time as another.
- **Work that suits a helper now** (synthetic data, apart from the core, VOICE-AGENT.md §9):
  the fake Twilio client, the text call harness, simulated-caller personas, a Pipecat
  prototype on the laptop microphone, the record-and-replay recorder.
- **Where open work lives:** HANDOFF.md › Open now (as DeetsMusic: no separate backlog doc). A
  helper writes their name next to the item they take.

## 5. Branches, commits and pull requests
- **One branch per piece of work**, from `main`: `<helper>/<short-name>`.
- **Commits:** a short subject that says what changed, a body that says why. An agent's commit
  carries its co-author trailer (CLAUDE.md › Repo).
- **Before a push, the checks pass:** `npx tsc --noEmit`, `npx vite build`,
  `cargo check --workspace`, `cargo test -p deetshealth-core --features sqlite`,
  `npm run docs:check`. (DeetsMusic's `npm run check` and its pre-push hook will run them in one
  step: HANDOFF.md › Open now.)
- **A pull request into `main`** for every change. The owner reviews and merges. The PR says
  what changed, the desk test, and what the helper decided inside a fork the owner chose.
- **The review checklist** (the reviewer reads the diff for each):
  - a log line, trace, metric, toast, hint or error that could carry protected data;
  - a value that leaves `Shielded<T>` other than through a pass;
  - a tool that takes a patient argument (SECURITY.md §4.9);
  - a secret, a real record, or a real phone number;
  - a fork decided without the owner;
  - a doc that the change made untrue (§6).

## 6. Docs
As DeetsMusic (DOCS-ORG.md):
- A change and the doc that describes it land in **the same commit**. A doc's "as built"
  section is the code.
- A new doc gets front matter and one line in CLAUDE.md. Move a doc only with
  `scripts/docs-move.mjs`.
- **WORKLOG.md:** one dated `##` per sitting, with the helper's name. **HANDOFF.md:** what is
  still true, corrected in place.
- Plain words (ASD-STE100): short sentences, one idea per sentence.

## 7. Agents
- **The rules for people bind agents too.** CLAUDE.md is the agent's brief; a helper whose
  agent reads another file gets an `AGENTS.md` at the root that points to CLAUDE.md, as
  DeetsMusic has.
- **An agent never sees protected data.** Synthetic data only; no real transcript, log or
  record pasted into an agent's chat.
- **An agent brings forks; it does not decide them.** The helper takes the fork to the owner.
- **No agent account holds production credentials.**
- Agents follow CLAUDE.md's working rules (design on paper first, the fork test, one feature in
  flight, the checks before hand-off).

## 8. Vendors and accounts
- The project's own accounts at Anthropic, Cartesia, Twilio, the cloud and Epic, owned by the
  owner's company, with a separate dev project in each.
- **Dev projects get synthetic data only.** Production projects are made only when the BAAs
  are signed (SECURITY.md §5).
- A helper gets a key scoped to the dev project, never the account's owner key.
- Each spend that can grow (Claude tokens, Cartesia and Twilio minutes) has a budget alert.

## 9. When something goes wrong
- **A suspected leak** (protected data or a secret where it should not be): stop, tell the owner
  at once, and do not delete the evidence. The owner decides the next step; a real breach
  starts the hospital's notice clock (SECURITY.md §6).
- **A broken `main`:** the person who broke it fixes it or reverts their own commit, with a
  new commit, never a force-push.
- **A desk test that fails:** the feature goes back to its builder; nothing new starts on top of
  it.

## 10. Still the owner's to decide
- Branch names and whether helpers merge their own PRs after review.
- Whether open work moves from HANDOFF.md to GitHub issues as the team grows.
- The agreements a helper signs (an NDA, a contractor agreement, IP assignment): a lawyer's
  work, not this doc's.
- When the project's company, accounts and vendor contracts exist (§8).
- Whether to add `AGENTS.md` now (§7).
