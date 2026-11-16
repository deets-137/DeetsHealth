---
status: built
desk_test: open
sources: [ehr/src/texts.rs, ehr/src/verify.rs, ehr/src/service.rs, ehr/src/main.rs, ehr/src/sandbox.rs, ehr/fixtures/sandbox.json, ehr/tests/texts.rs, core/src/text.rs, core/src/shielded.rs, agent/deets_agent/fake_twilio.py, agent/deets_agent/dev_server.py, agent/deets_agent/ehr_process.py, agent/deets_agent/call_record.py, agent/prompts/AGENT-PROMPT.md, agent/tests/test_fake_twilio.py, scripts/dev.mjs]
updated: 2026-11-16
---
# DeetsHealth — The code by text

> The second factor of caller verification (SECURITY.md §4.4): a random code, sent by text to
> the number on file, read back by the caller. Built 2026-11-12, after the owner chose fork A
> (a fake Twilio now, Twilio itself in Phase 5) and asked to "build as close to end product as
> reasonable during dev". The Chat page's side of it (the code button, the sandbox panel) is
> CHAT.md §8. The vendor facts are integrations/TWILIO.md. This doc points to the code; it
> copies no word or value (DOCS-ORG.md §1, rule 8).

## 1. Terms
- **The code:** the six digits the caller reads back. Made and checked in `ehr`, never by the
  model and never by Twilio.
- **The number on file:** the phone number the EHR holds for the matched patient. It may not
  be the number the caller is calling from. Caller ID never ends verification. Only a number
  that takes a text counts: in Epic, the `mobile` phone (EPIC.md §5.2). A patient with none
  still counts in the match, but no code goes out (`Candidate.phone` is an `Option`; the test
  `a_match_with_no_number_for_a_text_sends_no_code`), so the caller is handed off as after a
  failed match (2026-11-13). Which numbers count becomes a hospital setting: FUTURE-SETTINGS.md
  §8.
- **The text service:** Twilio's Programmable Messaging. In dev, **the fake Twilio**: a
  stand-in for the one Twilio call `ehr` makes, inside the agent's dev server.
- **The sender:** a Twilio Messaging Service (`MG…`), which owns the sending number.

## 2. The path, end to end
The same code runs in dev and in production up to the last hop. Only the settings (§4) change.

| # | What happens | Where in code |
|---|---|---|
| 1 | The caller gives a name and a date of birth; the agent reads them back; the caller confirms. | `_identify_fn`, `_confirm_details_fn` (`agent/deets_agent/flow.py`) |
| 2 | `ehr` finds the candidates and keeps exactly one match. No match, or two, sends nothing. The reply is the same either way. | `identify` (`ehr/src/service.rs`), `one_match` (`ehr/src/verify.rs`) |
| 3 | `ehr` makes the code from the OS's random source, each six-digit value equally likely. | `Codes::issue` → `random_code` (`ehr/src/texts.rs`) |
| 4 | `ehr` fills the text's words with the code and sends Twilio's Messages request. | `TwilioCodes::send`, `template` (`ehr/src/texts.rs`); the words: AGENT-PROMPT.md › `text.code` |
| 5 | Twilio (or the fake) answers. A success status is a sent code. Anything else, or no answer in time, is "the text service did not answer". | `TwilioCodes::send`; `down` (`ehr/src/service.rs`); `SEND_LIMIT` |
| 6 | `ehr` keeps the code in the call's session, with the time it was made. | `Pending` (`ehr/src/session.rs`) |
| 7 | The caller reads the code; `ehr` compares it in constant time and checks its age. | `check_code` (`ehr/src/service.rs`), `Shielded::ct_eq` (core), `CODE_LIFETIME` |
| 8 | A resend makes a new code and the old one stops working. | `resend_code` (`ehr/src/service.rs`), `MAX_RESENDS` |

The limits (`MAX_IDENTIFIES`, `MAX_RESENDS`, `MAX_CODE_FAILURES`, `CODE_LIFETIME`,
`SEND_LIMIT`) are listed in CALL-FLOW.md §4.

## 3. The request
`TwilioCodes::send` sends exactly what Twilio's Messages API takes:
- `POST {api}/2010-04-01/Accounts/{AccountSid}/Messages.json`, with the account id checked
  (`is_sid`) so it can never change the path.
- HTTP Basic auth: the account id and its auth token.
- A form: `To` (the number on file, E.164), `MessagingServiceSid` (the sender), `Body` (the
  words with the code).
- **TLS:** rustls on the same AWS-LC provider as the gRPC wire (`transport::provider`; its FIPS
  module in the deploy build), Mozilla's roots (`webpki-roots`), no redirects, a time limit
  (`SEND_LIMIT`). No second crypto library enters the build: checked with `cargo tree -i
  ring`, 2026-11-12.
- **Plain http only to this machine:** `TwilioConfig::new` refuses `http` unless the host is
  127.0.0.1, localhost or ::1 (the fake).
- The reply's body is never read; only its status. A refusal carries no text from Twilio into
  a log or an error.
- Not Twilio Verify: with Verify, Twilio makes and checks the code, so the second factor would
  sit with a second vendor. Here `ehr` keeps it.

## 4. Settings
`ehr` reads them from its environment at start, before it binds its port, so a bad setting
stops it before its first line (`TwilioConfig::from_env`, `ehr/src/main.rs`). Never a flag: a
flag shows in the process list.

| Setting | Meaning |
|---|---|
| `DEETS_TWILIO_ACCOUNT_SID` | `AC` and 32 hex digits. **Unset: no texts**; `ehr` uses `FakeCodes` (§6). |
| `DEETS_TWILIO_AUTH_TOKEN` | The account's auth token. Held in a wiped-on-drop string. |
| `DEETS_TWILIO_MESSAGING_SERVICE` | `MG` and 32 hex digits: the sender. |
| `DEETS_TWILIO_API` | The API's base. Default `DEFAULT_API` (`ehr/src/texts.rs`), Twilio itself. |

In production the auth token comes from the deploy's secret store into `ehr`'s environment
(CLOUD.md). One Twilio subaccount per hospital (TWILIO.md §2).

## 5. The protected values: `TextPass`
The number and the code are protected data (SECURITY.md §2), held in `Shielded<String>`. The
request must carry them in plain form, so core has a fifth pass, `TextPass`
(`core/src/text.rs`; the list is in `core/src/shielded.rs`).
- Only `ehr/src/texts.rs` may name it. A test fails if any other file in `ehr/src` does
  (`only_this_file_names_the_text_pass`), as `WirePass` is held to `edge.rs`.
- The body is built in a wiped-on-drop string. reqwest's own copy of the form is not wiped; it
  lives until the request is sent.
- Twilio keeps each message's body in its logs, and the body holds the code. Before a real
  text: message body redaction or the shortest retention on the hospital's subaccount
  (TWILIO.md §6).

## 6. The two code senders
`ehr` is generic over the `Codes` trait (`ehr/src/verify.rs`); `main.rs` picks one at start.
- **`TwilioCodes`** (`ehr/src/texts.rs`): a random code, a real request. Used when the
  settings (§4) are set: by `npm run dev`'s Test calls, and from Phase 5 in production.
- **`FakeCodes`** (`ehr/src/verify.rs`): always `FAKE_CODE`, sends nothing. Used when the
  settings are unset: the typed harness (AGENT.md §1), the agent's tests and `ehr`'s service
  tests. A fixed code keeps the replay cache valid, because the code is in the caller's
  messages.

## 7. The fake Twilio (dev only)
`agent/deets_agent/fake_twilio.py`, served by the agent's dev server
(`agent/deets_agent/dev_server.py`).
- **Its account is new at each launch:** `FakeTwilio` makes an account id, an auth token and a
  sender. They mean nothing to Twilio. `ehr_env()` hands them to `ehr` through
  `running_ehr(env=…)` (`agent/deets_agent/ehr_process.py`), with `DEETS_TWILIO_API` pointing
  at the dev server's own port.
- **Who may call it:** the dev server's wall lets the Messages path skip the console's launch
  token and checks instead: the request comes from this machine, carries no `Origin` (so no
  browser), and carries this launch's Basic auth, compared in constant time
  (`FakeTwilio.authorized`). A wrong one gets Twilio's own 401 shape.
- **What it checks, as Twilio does** (`FakeTwilio.take`): the sender is this launch's; `To` is
  E.164; the body is not empty. It also refuses a body longer than one plain text segment
  (`MAX_BODY`), which Twilio would split and bill twice. A refusal gets a Twilio-style error
  code and a 400.
- **What it does with a text:** answers 201 with a message id, and writes a `text` event (the
  number and the body) into the live test call's record (`agent/deets_agent/call_record.py`).
  The watch stream carries it to the Chat page like any other event, and Demo plays it back.
- **Safe only because every caller is synthetic.** A real call has no dev call record, and
  Twilio itself copies nothing back here.

## 8. The sandbox list
`ehr/fixtures/sandbox.json` holds the synthetic patients (each with a number in 555-0100 to
555-0199, which no real phone uses) and their visits. `ehr/src/sandbox.rs` builds it into the
binary; the fake patient search (`FakePatients`) and the fake scheduler's existing visits
(`FakeScheduler::visits`) read it; `GET /agent/sandbox` sends it to the Chat page. One
file, so the page always shows what the fakes use. Two tests guard it: every visit has its
patient, and every number is in the unused range.

## 9. The words of the text
AGENT-PROMPT.md › `text.code`, with every other word the caller reads or hears, so the owner
edits them in one place. `{code}` is its one code value; since 2026-11-14 it may also hold any
System variable, `{clinic}` for one (SYSTEM-PAGE.md §3.4).
- `ehr` builds the file in at compile time (`PROMPT_FILE`, `template`). Lines next to each
  other join with a space and a blank line starts a paragraph, as in `prompts.py` (a line that
  starts with "- " is not a list item here: a text has no lists). It fills the System variables
  of `agent/prompts/VARIABLES.md` (compiled in too), and refuses to start if `{code}` is
  missing or doubled, or another placeholder is left.
- A call with a saved version: `OpenCall` names the prompt version and the System version;
  `ehr` fills that version's variables into `text.code` when the call opens, and `{code}` when
  the text goes out (`settings.rs › fill`, one pass each; a variable's value holds no brace).
- A value counts toward the segment: a long clinic name can push the text past 160
  characters. The test below checks the file's own values only.
- The agent checks the same section at its start (`PLACEHOLDERS` in
  `agent/deets_agent/prompts.py`), so a bad edit fails on both sides.
- A test holds the filled text to one segment: plain ASCII and at most 160 characters
  (`the_words_fit_one_text`). A character outside GSM-7 would send it as UCS-2, 70 characters
  to a segment.
- An edit needs an `ehr` rebuild (`npm run dev` does it, §10).

## 10. Running it
- `npm run dev` (`scripts/dev.mjs`) builds `ehr` first, so the dev server never starts an old
  binary that would ignore the settings and send no text. On Windows it ends the whole process
  tree on Ctrl+C: a kill reaches only `uv`, and an `ehr` left behind locks the binary for the
  next build. If the build says the file is in use, end `deetshealth-ehr.exe` in Task Manager.
- The Chat page shows each text as the code button (CHAT.md §8).

## 11. Tests (kept)
- `ehr/tests/texts.rs`: the real request against a local listener: the path, the Basic auth,
  the form type, `To`, the sender, the code in the body; a 400 is "did not answer"; codes vary.
- `ehr/src/texts.rs` (unit): six digits that vary; one segment; the settings' checks (plain
  http away from this machine, an id that bends the path, a bad sender); only this file names
  `TextPass`.
- `ehr/src/sandbox.rs` (unit): the fixture parses; the numbers are in the unused range.
- `agent/tests/test_fake_twilio.py`: only this machine, this account and no browser get in;
  a good text is taken and handed on; what Twilio refuses, the fake refuses; `ehr`'s settings
  point at the dev server.

## 12. Phase 5: real texts
1. Twilio's BAA tier for the hospital's subaccount (TWILIO.md §2), with Programmable Messaging
   on its HIPAA-eligible list.
2. **Sender registration, started early:** toll-free verification, or A2P 10DLC brand and
   campaign registration. Either can take days to weeks (TWILIO.md §6).
3. The clinic's name in `text.code`: carriers want the sender's brand in each text. It needs
   the name from the hospital's settings, which `ehr` does not read yet (FUTURE-SETTINGS.md).
4. Message body redaction or the shortest log retention (§5).
5. Set the four settings (§4) on the deploy's `ehr`. No code change.
6. The desk test against a real phone: a sandbox patient's number on file set to the tester's
   own phone (synthetic record, real phone).

## 13. Known gaps
- The fake always accepts. A switch to make it refuse, to hear the agent's line for a send
  that failed, is not built.
- No delivery status: Twilio can call back when a text is delivered or fails. Not used yet;
  the caller's "I got no text" leads to a resend or a hand-off instead (CALL-FLOW.md).
- A visit booked during a test call stays in the fake scheduler's memory only; the sandbox
  list does not show it.

## 14. Desk test
CHAT.md §8 › Desk test: it walks this path from the Chat page.
