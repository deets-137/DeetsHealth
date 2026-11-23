---
status: project
desk_test: none
sources: [docs/WORKLOG.md, scripts/docs-check.mjs]
updated: 2026-11-22
---
# DeetsHealth — Pre-live verifications

> The desk tests that were closed on the owner's word, not run, and the open issues found in
> the last runs. **Every row must be run for real before a real caller, a real record or a live
> clinic** (DESIGN.md §0, "set aside, not dropped"). Then its doc's marker becomes
> `passed <date>`, and its row here gets the date and who ran it.

**Terms**
- **Desk test:** the owner runs the work by hand and gives feedback (BUILD-ORDER.md).
- **Closed:** `desk_test: closed <date>` in a doc's front matter, or "desk test closed <date>"
  in a part marker. The owner closed the test on his word; nobody ran it. It is not a pass
  (DOCS-ORG.md §2).
- **Passed:** `desk_test: passed <date>`. Someone ran every step and it held.

**Why (the owner, 2026-11-22):** "close all the hand tests as verified per my word for now,
we'll revisit at a later date". The aim is a portfolio piece (DESIGN.md §0), so the open tests
no longer block the next work. They still block going live.

## 1. Closed on the owner's word, 2026-11-22

Run each one with the steps in its doc. "Ran" stays empty until it is run.

### 1.1 Config › Agent and the agent's words
| Test | Steps | Ran |
|---|---|---|
| Config › Agent, the agent's words in the console | [CONSOLE.md](architecture/CONSOLE.md) §9 | |
| Config › Agent arranged by the call | [AGENT-PAGE.md](architecture/AGENT-PAGE.md) §5a | |
| One layout, Plain \| Code | AGENT-PAGE.md §8 | |
| AI and HC | AGENT-PAGE.md §9 | |
| Core, and a gentle Plain \| Code | AGENT-PAGE.md §10 | |
| Every section's "plays when" ("i") | AGENT-PAGE.md, the section list before §5a | |
| Test \| cost on Config › Agent | [CONSOLE.md](architecture/CONSOLE.md) §9; HANDOFF.md › Open now | |
| Editing the prompt: a save reaches the next call | [PROMPT-EDITOR.md](architecture/PROMPT-EDITOR.md) | |
| Config › System: the variables and the tools | [SYSTEM-PAGE.md](architecture/SYSTEM-PAGE.md) | |

### 1.2 Config › Hospital and booking
| Test | Steps | Ran |
|---|---|---|
| Config › Hospital: the lists, versions, one Epic list per list | [HOSPITAL-CONFIG.md](architecture/HOSPITAL-CONFIG.md) §6, steps 1 to 12 | |
| Places and Import from Epic | HOSPITAL-CONFIG.md §6, steps 13 to 20 | |
| New or established | [BOOKING-RULES.md](architecture/BOOKING-RULES.md) §5.1 | Part, 2026-11-22: fails, §2 |
| The usual place, the PCP, a sooner time | BOOKING-RULES.md §5.2 | Part, 2026-11-22: sooner held |
| The place and the specialty in the search | BOOKING-RULES.md §5.3 | Part, 2026-11-22: steps 1, 4, 5 held |
| Where to go | BOOKING-RULES.md §5.4 | Part, 2026-11-22: §2 |
| The Booking rules | BOOKING-RULES.md §5.5 | |
| The booking keeps the caller's reason (Local) | HANDOFF.md › Open now; [FLAGS.md](architecture/FLAGS.md) §3.3 | |
| Booking and visits in Epic mode, builds 1 and 2 | [EPIC.md](integrations/EPIC.md) §7.10 | |
| What a visit is, and the offer of a person | HANDOFF.md › Open now; FUTURE-SETTINGS.md §9, §10 | |

### 1.3 The Chat page and the console's shell
| Test | Steps | Ran |
|---|---|---|
| The Chat page: the connection, the catalog, the rail | [CHAT.md](architecture/CHAT.md) §6, §8, §9 | |
| The call panel first, in Demo and PHI (narrow) | CHAT.md §15.1 | MOBILE-UI.md: passed 2026-11-21 |
| The grow handle (narrow) | CHAT.md §15.2 | MOBILE-UI.md: passed 2026-11-21 |
| The 2026-11-13 additions | [CONSOLE.md](architecture/CONSOLE.md), "The 2026-11-13 additions" | |
| The compact layout; Chat on a narrow screen | CONSOLE.md §11, §11.4 | MOBILE-UI.md: passed 2026-11-21 |
| Long lists: windowing and the search box | [TABLES.md](architecture/TABLES.md) §6.6 | |
| The Analytics page's shell | [ANALYTICS.md](architecture/ANALYTICS.md) | |

### 1.4 Voice and texts
| Test | Steps | Ran |
|---|---|---|
| Voice: the seat bar, the voice seat, the Resources tab | [CHAT.md](architecture/CHAT.md) §17 | The owner is running it, 2026-11-22 |
| Config › Cartesia and the settings store | [CONSOLE.md](architecture/CONSOLE.md) §7, §7.3 | |
| The voice list, checked live | CONSOLE.md §7.8 | |
| Cartesia: Ink and Sonic | [CARTESIA.md](integrations/CARTESIA.md) | |
| The code by text | [TEXTS.md](architecture/TEXTS.md) | |

### 1.5 The agent, the guardrails and the flags
| Test | Steps | Ran |
|---|---|---|
| The agent as built | [AGENT.md](architecture/AGENT.md) | |
| The default call flow | [CALL-FLOW.md](architecture/CALL-FLOW.md) | |
| The call guardrails; the code step's words | [REGULATIONS.md](architecture/REGULATIONS.md) §9 | |
| Staff flags | [FLAGS.md](architecture/FLAGS.md) §3.1 to §3.4, §6 | |

### 1.6 The store, the dev API and the logs
| Test | Steps | Ran |
|---|---|---|
| The store in `ehr` | [DATABASE.md](architecture/DATABASE.md) §4a | |
| Synthetic and real rows apart; the kept scripts' runs; annotations | DATABASE.md §10 (for §7 to §9) | |
| The dev API and the annotation gate | [DEV-API.md](architecture/DEV-API.md) §8 | |
| The request log, the EHR ledger, the repeat guard | HANDOFF.md › Open now; EPIC.md §6 | |

### 1.7 The patient app's code (legacy)
These test code that moves to the archive with its work (DOCS-ORG.md §2, `legacy`). Run them
only if that code stays.

| Test | Steps | Ran |
|---|---|---|
| The core crate, stage 7 | [CORE-CRATE.md](architecture/CORE-CRATE.md), "Stage 7 desk test" | |
| Sign-in | [SIGN-IN.md](architecture/SIGN-IN.md) | |
| The fetch | [FETCH.md](architecture/FETCH.md) | |
| App errors and their toasts | [ERRORS.md](architecture/ERRORS.md) | |

## 2. Open issues from the last runs

Found in Claude's typed Test calls on Local, 2026-11-22 (WORKLOG.md, that date). Not fixed.

| Issue | Where it shows | Next step |
|---|---|---|
| A new patient's booking reads back as "an office visit", not "a new patient visit" | BOOKING-RULES.md §5.1 step 4, as Priya Raman | Find the cause: the active hospital version's Patients column, or the fake's slot type |
| The Places imported from Epic replace the Local sandbox's places: Main Street Clinic has no address or phone; Jordan Ellis has no usual place | BOOKING-RULES.md §5.2 step 4, §5.4 step 4 | A fork for the owner: Local's places beside Epic's, or Local reads its own list |
| "A heart doctor" is not a kind of care the lists know | BOOKING-RULES.md §5.3 step 2 | Check the Specialties list's words after the import |
| "Cardiology procedures" without "EMC" names only "Cardiology" | `Shielded::names_terms` | A caller's words that hold more of one name should win |
| The options go on counting across searches ("Option 2" first) | Any second search in a call | Polish |
| An hours question gets "I don't have the address … I don't have a phone number" first | BOOKING-RULES.md §5.4 step 2 | Polish: say only what was asked |
| `pip-audit` has never run | [DEPENDENCIES.md](architecture/DEPENDENCIES.md) §6 | Run it in the owner's shell |

Fixed the same day: a place's full name could not be picked ("EMC Cardiology Procedures"),
`core/src/shielded.rs`; its desk test is in WORKLOG.md, 2026-11-22.
