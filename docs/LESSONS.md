---
status: sop
desk_test: none
sources: []
updated: 2026-09-21
---
# LESSONS — what the owner tends to want in DeetsHealth

Brought over from `../DeetsMusic/docs/LESSONS.md` on 2026-09-21 and cut to what applies to a
health-record viewer. **The owner decides the forks.** This file does not close a fork. It is
a record of taste and of lessons already paid for, so that the option you build and the
option you bring him are both better. The evidence behind each lesson is in the DeetsMusic
copy; add DeetsHealth evidence here as it happens.

Read `../DeetsMusic/docs/TASTE.md` with it: his favorite works and dislikes. That file is
local only. Never copy it into this repo, a commit or a doc.

**Terms used here**
- **Fork:** a choice between two or more ways to build something.
- **Stop rule:** a kind of change you never make without the owner's word (§2).
- **Desk test:** the owner runs the app and tries the change.
- **Site:** one health system's FHIR server (DATA-ARCHITECTURE.md §1).

## 0. How to use this doc
1. Check §2. If a stop rule applies, stop and ask.
2. Check the feature's own doc and HANDOFF.md › Decided. Never re-open a decision.
3. For anything else he has not chosen, **bring him the fork.** §3 and §4 tell you which
   option to recommend. They do not pick it for you.

## 1. The mission
We are good stewards of our users and of the health systems we read from. A user's record is
the most private data they have; treat it that way in every line of code.

**Priority (the owner, 2026-09-21):** patient experience first, privacy and security second,
everything else after. The rules are SECURITY.md.
- **Altruism.** We do the harder work so the user's life gets easier. If the choice is work
  for us or work for the user (a manual step, a confusing message, a recovery through File
  Explorer) or for a site (extra calls, extra load), the work is ours.
- **Attention to fine detail.** The small things are the product. Nobody asks for them.
  Users feel them anyway.

## 2. Stop rules — ask first
1. **Terms and compliance.** Work freely inside Epic's and each site's API terms. Stop before
   any change that breaks or skirts them: a workaround of a rate limit, an unregistered
   scope, a write back to the record.
2. **Ship and deploy.** Commit, release, publish, a change to the Epic app registration,
   adding a git remote.
3. **Loss of user data.** A destructive migration, a delete of the user's own notes or
   settings, a reset. An additive schema change is NOT a stop rule. (Sign-out deleting the
   record cache is by design: DATA-ARCHITECTURE.md §4.)
4. **Protected data (PHI and PII).** Any step that touches a real record instead of a
   sandbox patient, or that moves record data or any identifier across the wall
   (SECURITY.md §4).

A stop rule is the hard floor, not the whole gate. Outside these, the fork still comes to
him (§0); the difference is that a stop rule is never overtaken by a good reason.

## 3. The lessons
Each one went wrong once in DeetsMusic, or is a preference he has stated more than once.

### 3.1 The thing a control acts on holds that control
Put a control, a fact or a state on the element that owns it. Do not measure it from outside
and layer a copy on top. Before you add an overlay, a cache, a copy column or a watcher, ask
who already knows the answer and attach to it.

### 3.2 Fix the cause, not the symptom
A toast, a retry or a dialog explains a failure; it does not fix it. Start from evidence (the
log, a probe, the source), name the cause, then design. Say plainly when a change only
explains a failure.

### 3.3 Be a good steward of sites
- Keep FHIR calls low: follow `link[rel=next]` once, cache, make no calls for filters, and
  measure the real count after a build.
- A call count or a cache claim names the Rust command and the table it reads. Open both
  before you say it. A doc's call table is a pointer to the code, not a proof.
- When a site's terms block a feature, drop it and keep the record.

### 3.4 Be a good steward of users
- A user never needs File Explorer to recover or to learn what happened.
- One message per failure, in the app's voice. It never blames the user; it says what
  happened and what we do next ("sign in again", never an error loop).
- No jargon anywhere a user reads. No raw FHIR words ("Observation", "MedicationRequest")
  and no bare codes. Settings rows follow the label style: verb-first label, pills of three
  words or fewer, one-sentence hint.
- Privacy is the PHI wall, with no exceptions. Logs carry counts and resource types only.
- The app shows what the record says. It never adds advice, a diagnosis or alarm (DESIGN.md §5).
- Reduced motion means no motion (a snap), not a softer motion.

### 3.5 Keep what the user already knows
- A card keeps its shape when it gains a feature.
- A control lives in exactly one place.
- A control with nothing to act on is disabled, not hidden.

### 3.6 Polish and fine detail are the default
Build the finished version, not the minimum. Motion enters with the primitives; states show
as shape, not color alone. Before you hand off, look for the detail a user would feel but
never report: a flash between states, a shift of a pixel or two, an edge that does not meet a
corner. Fix it in the same pass.

### 3.7 Don't fear complexity; give users power
A deep system or many options are not reasons to build a smaller version. Give users the
control, then make it easy to find and understand. The fault in a bad app is bulk and poor
presentation, not the number of options.

### 3.8 Fewer surfaces, deterministic behavior
Extend an existing tool or menu before adding one. No button that repeats a gesture that
already works.

### 3.9 Fun where it costs nothing
Personality is welcome when it adds no cost, risk or jargon. In a health app, fun never
touches the record's values: a lab result is never a joke.

### 3.10 Trust the ruler before the number
Check the tool first, then measure, then claim.

### 3.11 Hand-rolled and permanent tooling
Prefer small code of our own over a new crate or package when the job is well bounded. Dev
tools stay, documented.

### 3.12 Anything is possible; it can need a new perspective
Never write "impossible" as a verdict. Write what blocks this path, then at least two other
angles: a different layer, data we already have, a different owner of the fact (§3.1), a
different shape of the feature. A new angle must still stay inside the terms (§2).

## 4. Tie-breakers
These decide which option you *recommend* when two lessons pull against each other.

| Question | Answer |
|---|---|
| Nobody chose a default | Recommend the value he would want in daily use, and ask. |
| Polished version costs about twice the plain one | Build the polished one. |
| A choice could be a Settings row | A row when the choice is taste. One behavior with no row when it is correctness. |
| An existing primitive fits most of the need | Extend it. If the need shows a pattern several places will want, bring it up before you build. Read the whole primitive and its callers first. |
| A visual ask is ambiguous | Take the literal reading. |
| Convenience against the PHI wall | The wall wins. |
