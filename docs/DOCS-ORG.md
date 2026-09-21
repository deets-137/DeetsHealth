---
status: sop
desk_test: none
sources: []
updated: 2026-09-21
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
   `ops/`, `ideas/` (not built). Make a folder when its first doc arrives.
5. A new doc gets one line in CLAUDE.md.
6. Where a doc has an "as built" section, it is the code; the design above it is history.
7. No real PHI in any doc. Sandbox patient names are fine; they are public fakes.

## 2. Front matter
First thing in the file, above the `#` title:

```yaml
---
status: shipped | built | designed | project | parked | idea | foundation | sop | guide
shipped_in: 0.1.0          # omit unless status is shipped
desk_test: passed 2026-09-21 | open | none
sources: [src-tauri/src/fhir.rs]
updated: 2026-09-21
---
```

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
`npm run docs:check` (links, § pointers, front matter) and `scripts/docs-move.mjs` (move a
doc and re-point every link) come across from DeetsMusic with the scaffold. Once they exist:
run `docs:check` after any doc edit, and never move a doc by hand.
