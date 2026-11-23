---
status: sop
desk_test: none
sources: [scripts/docs-check.mjs]
updated: 2026-11-12
---
# DeetsHealth — Docs organization

The rules are DeetsMusic's (`../DeetsMusic/docs/DOCS-ORG.md`), in short. Where this file
and that one differ, this file wins for DeetsHealth.

## 1. The rules
1. Every doc under `docs/` starts with front matter (§2).
2. A part of a doc with its own state carries a part marker (§3).
3. **A fact that is still true belongs in HANDOFF.md; a record of a day belongs in
   WORKLOG.md.** A desk test is a record of a day. A gotcha is still true. CLAUDE.md holds
   no state.
4. Area folders: `architecture/` (the engine), `cards/`, `features/`, `integrations/`,
   `ops/`, `ideas/` (not built), `archive/` (a record, §5). Make a folder when its first doc
   arrives.
5. A new doc gets one line in CLAUDE.md.
6. Where a doc has an "as built" section, it is the code; the design above it is history.
7. No real PHI in any doc. Sandbox patient names are fine; they are public fakes.
8. **A doc points to the code; it never copies a word or a value the code holds** (the owner,
   2026-11-12). The agent's words, its limits and its lists will become settings a console
   page changes, so the code (and AGENT-PROMPT.md) is the one copy. Name the file and the
   symbol, or the AGENT-PROMPT.md section: "`line.greeting`", "`MAX_CODE_FAILURES` in
   `ehr/src/service.rs`". The map of them all is `architecture/CALL-FLOW.md`. WORKLOG.md and
   the decision records in AGENT.md §3 may quote the words of their day.

## 2. Front matter
First thing in the file, above the `#` title:

```yaml
---
status: shipped | built | designed | project | parked | idea | foundation | sop | guide
shipped_in: 0.1.0          # omit unless status is shipped
desk_test: passed 2026-09-21 | closed 2026-11-22 | open | none
sources: [src-tauri/src/fhir.rs]
updated: 2026-09-21
legacy: patient-app       # optional: see below
---
```

**`desk_test: closed <date>`** (the owner, 2026-11-22) means the owner closed the test on his
word, and nobody ran it. It is not a pass. Each closed test has a row in
[PRE-LIVE-VERIFICATIONS.md](PRE-LIVE-VERIFICATIONS.md), and every row must be run before going
live. A part marker says the same: "desk test closed <date>".

**`legacy: patient-app`** (2026-11-12) marks a doc that describes the patient app's code while
that code is still in the tree (`core/`). Its `status` stays the state of that code. The doc
says so in one line under its title, and it moves to `docs/archive/` with its code (§5). The
checker (DEV-API.md §7, check 6) lists a legacy doc whose sources are all archived: that doc is
ready to move.

| value | meaning |
|---|---|
| `shipped` | in a published installer, and in someone's hands |
| `built` | in the tree, not yet in a published installer |
| `designed` | every fork closed, no code |
| `project` | started, not complete: forks still open, or a build begun and not finished |
| `parked` | designed and deliberately stopped; kept as a record |
| `idea` | not designed. Everything in `ideas/` |
| `foundation` | the engine the features run on |
| `sop` | a standard operating procedure: how the work is done |
| `guide` | written for the user |
| `archived` | no longer needed; kept as a record. Everything in `archive/` (§5) |

## 3. Part markers
When one doc holds parts in different states, front matter gives the state of the part that
works today. Every other part carries one line directly under its heading:

```
## 5. The PHI wall
> **Part:** designed · 2026-09-21
```

The shape is `> **Part:** <status> · <date>`, with the values above. An optional third field
names the open forks: `> **Part:** project · 2026-09-21 · forks §9.6`. A heading with no
marker has the doc's own status.

## 4. Tooling
`npm run docs:check` (links, mentions, § pointers, front matter, the ideas and archive rules,
and the drift checks below) and `scripts/docs-move.mjs` (move a doc and re-point every link).
Run `docs:check` after any doc edit, and never move a doc by hand.

**The drift checks** (2026-11-12) compare a doc with the code it maps. A fact fails the run; a
suspicion is listed and never fails it.
- **Check 12** (fact): the route ledger, DEV-API.md §2, equals the routes in `dev_server.py`.
- **Check 13** (fact): the RPC ledger, EHR-SERVICE.md §3, equals the `rpc` lines in
  `ehr.proto`, and each RPC has its two ends in the code.
- **Check 14** (fact): the console and the agent agree on the annotation fields and the watch
  messages, and every client that DEV-API.md names exists.
- **Check 6** (suspicion): a `built` or `foundation` doc whose `sources` changed in git after
  its `updated` date, and a `legacy` doc whose sources are all archived. Read the doc against
  the change; then set `updated`, or fix the doc.
A new ledger (a doc table that must equal a list in the code) gets its own numbered check in
`scripts/docs-check.mjs` and a line here.

## 5. The archive (DeetsMusic's DOCS-ORG.md §5.3, 2026-10-19)
A doc that no one needs any more moves to `docs/archive/` with
`node scripts/docs-move.mjs NAME=archive`, and its status becomes `archived`. Nothing is
deleted. Before the move, every item in it that is still open goes to its doc or to
HANDOFF.md › Open now, so the archive holds no open work. The checker treats an archived doc
as a record: its `sources` and the files it names may be gone. Every doc in `docs/archive/`
says `archived`, and only they do (check 11).

**Code that is no longer built** goes to `archive/` at the repo root, at its old path
(`src-tauri/x.rs` → `archive/src-tauri/x.rs`), with `git mv`. The workspace and the checks leave
it out (`archive/README.md`). This is DeetsHealth's own rule: DeetsMusic has no archived code.

A doc whose name a new doc takes (the patient app's DESIGN.md and HANDOFF.md, 2026-10-19) is
archived under a prefixed name, `archive/PATIENT-APP-DESIGN.md`, by `git mv`, and its links
are re-pointed by hand.
