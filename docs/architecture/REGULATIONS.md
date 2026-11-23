---
status: project
desk_test: closed 2026-11-22
sources: [agent/prompts/AGENT-PROMPT.md, agent/deets_agent/flow.py, ehr/src/service.rs, ehr/src/verify.rs]
updated: 2026-11-12
---
# Regulations — what the phone agent is built toward

The laws, federal guidance, state laws and standards that shape the phone agent, and what each
one asks of us. SECURITY.md holds our own rules; this doc holds the outside rules behind them.
Started 2026-11-12 from a review of a test call (WORKLOG.md, 2026-11-12).

**This is a map, not legal advice.** Each item marked *confirm* needs counsel, or a check of
its current status, before the first hospital.

**Marks:** ✅ covered (built, or a decided rule in SECURITY.md) · ◐ partly · ✗ not yet.

## 1. Terms
- **Covered entity (CE):** the hospital. **Business associate (BA):** us. HIPAA binds both.
- **Verification:** the check that a caller is the patient, or may act for the patient, before
  the agent gives or changes anything in the chart.
- **Factor:** one proof in a verification: something the caller knows, has, or is.
- **AAL:** an authenticator assurance level in NIST SP 800-63B (AAL1 lowest, AAL3 highest).
- **Disclosure:** words the caller must hear: that the agent is automated, that the call is
  transcribed or recorded, how to reach a person.

## 2. Verifying a patient: what the rules say
> **Part:** project · 2026-11-12 · forks §10

**No law names a method.** The rules ask for *reasonable* verification, set by the hospital's
own written policy, without barriers that keep a patient from their own care or records.

| Source | What it asks | Us |
|---|---|---|
| HIPAA Privacy Rule, 45 CFR 164.514(h) | Verify the identity **and the authority** of anyone who asks for PHI, when the CE does not know them. The CE may rely on statements or documents if that is reasonable. | ✅ name + date of birth + a code to the number on file (SECURITY.md §4.4) |
| 45 CFR 164.502(g) | A **personal representative** (a parent of a minor, a guardian, a person with a health-care power of attorney) is treated as the patient. | ✗ the agent only serves the patient. A proxy caller goes to a person today. |
| 45 CFR 164.510(b) | The CE may share relevant PHI with family who help with care, if the patient agrees or does not object. | ✗ out of scope for v1; goes to a person. |
| 45 CFR 164.522(b) | A patient may ask for **confidential communications**: another number or address. | ✗ the code must go to that number, not the main one (§10, R3). |
| 45 CFR 164.524 and OCR's right-of-access guidance | Verification must not be an unreasonable burden (for example, OCR says a CE may not require a visit in person). | ◐ every failure path must end at a person, never at a dead end (§2.1). |
| Information blocking, 45 CFR 171.202–203 | Refusing an unverified caller is allowed when it follows a written policy that is applied the same way to everyone. | ◐ the policy is the hospital's; the agent applies one rule to all callers. *confirm* |
| NIST SP 800-63B-4 (2025) | Guidance, binding only on federal agencies, but OCR and auditors read it as the bar. A **text code is a "restricted" authenticator**: offer another option and tell users the risk. A **date of birth is not an authenticator** at all. Limit failed tries **per account**, not per session. | ◐ about AAL1. The call back is the other option. The limits are per call only (§10, R2). |
| The Joint Commission, NPSG.01.01.01 | Two patient identifiers (name and date of birth) before care. This is about treatment, not phone access, but it is why name + date of birth is the industry habit. | ✅ |

**Where we stand:** name + date of birth + a code to the number on file is **above** what a
front desk does by phone today (name and date of birth only). It is **not** multi-factor in
NIST's terms, because a date of birth is not a secret. Never call it MFA to a hospital; call
it "a code to the number on file, after name and date of birth".

### 2.1 When verification fails
> **Part:** built · 2026-11-12

- **A confused caller goes to a person at once** (the owner, 2026-11-12: "If we know there is
  confusion, transfer to a person since we know we can't help"). After a failed code, if the
  caller asks why, says the code is right, or still has no text after a new code, the agent
  uses `hand_off` (AGENT-PROMPT.md `task.verify`), and the caller hears an apology
  (`line.hand_off_code`). How soon this happens is a hospital setting (FUTURE-SETTINGS.md §7).
- **The failed-code line offers a person** (`line.code_wrong`): read it again, a new code, or a
  member of staff.
- **Repeated failed codes lock the chart path** for the call and hand off (the limit is
  `MAX_CODE_FAILURES`, CALL-FLOW.md §4).
- **Nothing says why.** A name and date of birth that match no record, or match two
  near-identical records (twins, duplicates), send no code. The caller only hears that the
  code did not match, or does not get it. Every line about a code says it went out only if the
  details match our records, so no line is false for these callers. An instant transfer for
  them is a hospital setting, off by default (FUTURE-SETTINGS.md §7). The person they reach can then help. The kept script
  `tests/scripts/code_confusion.txt` covers the twin case (the fakes Casey Morgan and Casey
  Morgen share a date of birth).

## 3. HIPAA
| Rule | What it asks | Us |
|---|---|---|
| Security Rule, 164.308 | Risk analysis, a security officer, workforce training, incident procedures, a contingency plan, BA contracts | ◐ planned before the first hospital (SECURITY.md §6) |
| 164.310 | Physical safeguards: facility, workstation and device controls | ◐ the cloud provider's, under its BAA (CLOUD.md) |
| 164.312(a) | Access control: unique user ids, emergency access, automatic logoff, encryption | ✅ designed (SECURITY.md §4.6, §4.10) |
| 164.312(b) | Audit controls | ✅ designed: each transcript view, play and export is audited (SECURITY.md §4.10) |
| 164.312(c) | Integrity: protect PHI from change or loss, and be able to show it | ✗ nothing yet shows that a transcript or an audit row was not changed (§10, R6) |
| 164.312(d) | Person or entity authentication | ◐ staff: the hospital's SSO. Callers: §2 |
| 164.312(e) | Transmission security | ✅ TLS everywhere (SECURITY.md §4.1) |
| Privacy Rule, 164.502(b) | Minimum necessary | ✅ each tool returns only its fields (SECURITY.md §4.9) |
| Breach Notification, 164.400–414 | A BA tells the CE without unreasonable delay, at most 60 days | ✅ designed (SECURITY.md §6) |
| HITECH, Pub. L. 116-321 (2021) | "Recognized security practices" in place for 12 months (NIST CSF, HHS 405(d) HICP) must lower OCR's penalties | ✗ pick one and start the record now (§10, R7) |
| Security Rule update, proposed January 2025 | Would make MFA, encryption, an asset inventory, network maps and a 72-hour restore mandatory | ◐ we meet most of it by design. *confirm* whether it is final |

## 4. Other federal rules
| Rule | What it asks | Us |
|---|---|---|
| ACA §1557, 45 CFR 92.210 (in force May 2025) | Find and reduce the risk of discrimination in AI tools used in patient care | ◐ the agent books and routes; it does not decide care. Speech-to-text fails more for accents and speech impairments: test for it. *confirm* scope |
| §1557, 45 CFR 92.201 | Meaningful access for callers with limited English | ✗ one greeting per language; a person or an interpreter line otherwise |
| §1557, 45 CFR 92.202; ADA; Section 504 | Effective communication for callers with disabilities (relay calls, speech impairments, no mobile phone) | ◐ the call back serves a landline; a person is always one step away. Keypad entry for the code helps (§10, R4) |
| 42 CFR Part 2 (compliance date February 16, 2026) | Records of substance-use treatment have stricter consent and use rules | ✗ chart tools must never read them; the EHR adapter filters them |
| FCC, TCPA (47 USC 227) and the FCC's 2024 ruling on AI voices | An AI voice is an "artificial" voice. An **outbound** call with one needs consent or a healthcare exemption | ◐ inbound calls are fine. The **call back** factor is outbound: *confirm* the healthcare exemption (47 CFR 64.1200(a)(9)). A one-time code text the caller asked for is generally allowed |
| FDA, software as a medical device; the 2022 CDS guidance | Advice, diagnosis or triage can make software a device | ✅ out by design: `medical_question` and `emergency` only route (SECURITY.md §6) |
| NIST AI RMF 1.0 and the GenAI profile, NIST AI 600-1 | Manage AI risks: prompt injection, made-up output, data leaks | ✅ fixed lines carry every value; tools are bound to the verified patient (SECURITY.md §4.9) |
| NIST SP 800-66r2 (2024) | The HIPAA Security Rule mapped to controls | the outline for our risk analysis |
| HHS Cybersecurity Performance Goals (2024) | Essential and enhanced goals for health organizations | hospitals ask about them |

## 5. State laws
The strictest state sets the design, because a hospital's patients call from anywhere.

| Law | What it asks | Us |
|---|---|---|
| California Invasion of Privacy Act, Penal Code §632 (and other all-party-consent states) | Every party agrees before a call is recorded. Recent suits treat a vendor's live **transcription** as recording | ✅ the greeting says the call is transcribed (§6) |
| California AB 3030 (Health & Safety Code §1339.75, 2025) | Generative AI that sends **clinical** information to a patient must say it is AI, and say how to reach a person. For audio: at the start and at the end. Scheduling and billing are excluded | ✅ at the start (§6). ✗ at the end, needed when chart answers arrive. *confirm* |
| California SB 1001 (Business & Professions Code §17940); Utah's AI disclosure law | A bot must say it is a bot when asked, or up front in some cases | ✅ the greeting says it |
| Colorado AI Act, SB 24-205 (delayed to 2026) | Duties for "high-risk" AI in consequential decisions, including health-care services | *confirm* whether booking counts, and the current date |
| Texas HB 300 | Stricter than HIPAA: training within 90 days of hire, rules on electronic disclosure | ◐ training is planned (SECURITY.md §6) |
| Washington My Health My Data Act | Consumer health data outside HIPAA | likely out (we are a BA). *confirm* |
| Minors' confidential care (varies by state) | Some care a minor consents to stays hidden from parents | ✗ ties to personal representatives (§2) |

## 6. Disclosures on the call
> **Part:** built · 2026-11-12

The greeting is a fixed line (`line.greeting`), spoken by the code word for word before the
model runs, so its words never vary (flow.py `greet`). It says:
1. **An automated assistant, not a person** (AB 3030, SB 1001, Utah).
2. **The call is transcribed** (CIPA and the other all-party states). When a recording rule is
   on, it must also say the call is recorded (VOICE-AGENT.md §8.7).
3. **How to reach a person:** ask for a member of staff at any time (AB 3030).

A hospital will set its own words above this floor (FUTURE-SETTINGS.md §4). The words today are
working words; the owner edits them in AGENT-PROMPT.md.

## 7. ISO and audit frameworks
Hospitals ask for these in procurement. None is a law.

| Standard | What it covers | When |
|---|---|---|
| SOC 2 Type II | The common US ask: security, availability, confidentiality | business choice, at the first sales talk (SECURITY.md §6) |
| HITRUST CSF | Health care's own framework; maps to HIPAA, NIST and ISO | often asked by large systems |
| ISO/IEC 27001:2022 and 27002:2022 | An information security management system and its controls | when a buyer asks |
| ISO 27799:2016 | 27002 applied to health information | with 27001 |
| ISO/IEC 27701:2025 | Privacy management; now a standalone standard | with 27001 |
| ISO/IEC 27018:2019 | PII in the public cloud, for a processor | fits our role |
| ISO/IEC 42001:2023 and 23894:2023 | An AI management system; AI risk management | the AI-specific ask; growing |
| ISO 14971, IEC 62304, IEC 82304-1 | Medical device risk and software life cycle | only if we become a device. Not the plan |

The Joint Commission and the Coalition for Health AI published guidance on the responsible use
of AI in health care in 2025. Hospitals will ask how we meet it. *confirm* its current form.

## 8. Test mode and the logs
> **Part:** designed · 2026-11-12

- **Test mode keeps full logs** (the owner, 2026-11-12): the call record in `agent/calls/`
  holds the caller's words and the tools' arguments, so a test call can be debugged. Every
  value in it is synthetic.
- **Before the first real call, the record splits:** a call log with no values, and a
  transcript sealed under the hospital's key (SECURITY.md §4.5). The split is a gate on the
  first real call (HANDOFF.md › Open now).

## 9. As built
- 2026-11-12: the greeting with its disclosures (§6); a confused caller goes to a person after
  a failed code (§2.1); the kept script `code_confusion.txt`. Desk test: closed 2026-11-22.

## 10. Open
Each is a later piece of work, in the order recommended. None is built.
- **R1. Twins and duplicate records.** Today they reach a person only through confusion or the
  lock. A better path is open: the service knows no code went out, but it must not say so.
- **R2. Limits across calls.** Failed codes, resends and identify tries per patient and per
  calling number, across calls (NIST 800-63B). Stops text-message flooding of a patient's phone
  and SMS-pumping charges.
- **R3. The confidential-communications number** (164.522(b)): the code and the call back go to
  the number the patient chose. Needs a FHIR read of the patient's preferences.
- **R4. The code by keypad** (DTMF): the code then never reaches speech-to-text, the model or a
  log. Keep speech as the fallback.
- **R5. Personal representatives** (164.502(g)): a parent or a caregiver calling for a patient.
  Today (2026-11-12) such a caller goes to a person at once, with a staff flag
  (`calling_for_someone_else`, FLAGS.md), and is never asked for their own details.
- **R6. Integrity** (164.312(c)): show that a transcript or an audit row was not changed.
- **R7. Recognized security practices** (HITECH 2021): choose NIST CSF 2.0 or 405(d) HICP, and
  start the 12-month record.
- **R8. AB 3030 at the end of the call,** once chart answers arrive.
- **R9. Language access** (§1557): a greeting per language and a path to an interpreter.
