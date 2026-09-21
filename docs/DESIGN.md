---
status: project
desk_test: none
sources: []
updated: 2026-09-21
---
# DeetsHealth — Design

## 1. Intent
Give a patient a better way to read and use their own health record than a portal page.
The record comes from their health system through the SMART on FHIR patient API that US
law requires (21st Century Cures Act). The app reads it; it does not change it.

## 2. Who it is for
One patient, on their own PC, looking at their own record. Proxy access (a parent, a
caregiver) is a later idea, not the first build.

## 3. Same as DeetsMusic, and what differs
| Area | DeetsMusic | DeetsHealth |
|---|---|---|
| Shell | Tauri v2 + WebView2 | same |
| Front end | vanilla TS, Vite | same |
| Back end | Rust, normalizes Apple JSON | Rust, normalizes FHIR JSON |
| Tokens | palette → theme → skin | same |
| Surfaces | Mini, Midi, Max | **Midi and Max only** |
| Cards | collection-card engine, slots | same engine; health cards |
| Docs | front matter, HANDOFF/WORKLOG, area folders | same |
| Sign-in | MusicKit token via DeetsSupport | SMART on FHIR, PKCE, public client |
| Servers | one Apple API | one FHIR base URL per health system |
| Agent / MCP / SQL export | yes | **no, until the PHI wall is designed** |
| Telemetry, reports, diag | rich | PHI-free by rule (DATA-ARCHITECTURE.md §5) |
| Playback, queue, AirPlay, rooms | yes | not applicable |

## 4. First cards (ideas, not decided)
- **Labs** — results over time, with the reference range.
- **Medications** — active list, with the prescriber and dates.
- **Allergies** — substance, reaction, severity.
- **Timeline** — encounters in date order.

## 5. Non-goals
- No diagnosis or medical advice. The app shows what the record says.
- No writes back to the health system.
- No cloud copy of the record.
