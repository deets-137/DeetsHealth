//! The settings store (CONSOLE.md §7.3): versions of each service's settings in Postgres,
//! insert-only (`migrations/0002_settings.sql`).
//!
//! - **One list of every setting** per service ([`VOICE`]): its name and what it may hold. Every
//!   save is checked against it here, so no value outside a range or of the wrong kind is ever
//!   stored. The agent's `config.py` holds the same ranges for the Config page; a change to one
//!   is a change to both (CONSOLE.md §7.3).
//! - **A version holds every setting.** A save sends the whole set, each setting once; it
//!   becomes a new version, numbered within its day, and the active one.
//! - **Errors name the setting, never its value** (SECURITY.md §4.5). No setting is protected
//!   data, but the rule is the same everywhere.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sqlx::{PgPool, Row};

/// What one setting may hold.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// A number in a range; `null` is the provider's default.
    Number { min: f64, max: f64, nullable: bool },
    /// A whole number in a range.
    Whole { min: i64, max: i64 },
    Bool,
    /// One of a few words; `null` is the provider's default when `nullable`.
    Word { words: &'static [&'static str], nullable: bool },
    /// The voice: `{"name": …, "id": …}`, both short strings.
    Voice,
    /// A table of rows of `columns` short strings, at most `rows` rows and `chars` characters in
    /// all.
    Table { columns: usize, rows: usize, chars: usize },
    /// A section of the agent's words: text that is not empty, at most `PROSE_MAX` characters,
    /// holding these code values (as prompts.py › check reads them). Any other `{name}` must be
    /// a System variable: `prompt_variables` checks that against the System version.
    Prose { placeholders: &'static [&'static str] },
    /// The System variables (SYSTEM-PAGE.md §3.2): a list of `{name, meaning, value}`.
    Variables,
}

/// The longest section of the agent's words. The role prompt, the longest, is about 3,500.
pub const PROSE_MAX: usize = 12_000;

pub struct Setting {
    pub key: &'static str,
    pub kind: Kind,
}

const fn s(key: &'static str, kind: Kind) -> Setting {
    Setting { key, kind }
}

/// The voice service: Cartesia (CARTESIA.md §9). The ranges are Cartesia's.
pub const VOICE: &[Setting] = &[
    s("voice", Kind::Voice),
    s("speed", Kind::Number { min: 0.6, max: 1.5, nullable: true }),
    s("volume", Kind::Number { min: 0.5, max: 2.0, nullable: true }),
    s("emotion", Kind::Word { words: &["neutral", "calm", "content", "sad", "angry"], nullable: true }),
    s("normalization", Kind::Word { words: &["auto", "off"], nullable: false }),
    // Each cell is a short string; 200 rows at most.
    s("pronunciations", Kind::Table { columns: 2, rows: 200, chars: 200 * 2 * SHORT }),
    s("turn_start_threshold", Kind::Number { min: 0.5, max: 0.9, nullable: true }),
    s("turn_eager_end_threshold", Kind::Number { min: 0.3, max: 0.8, nullable: true }),
    s("turn_end_threshold", Kind::Number { min: 0.05, max: 0.5, nullable: true }),
    s("turn_end_timeout_ms", Kind::Number { min: 640.0, max: 11200.0, nullable: true }),
    s("eager_end_of_turn", Kind::Bool),
    s("barge_in", Kind::Bool),
    // Cartesia: up to 100 keyterms, 1,200 characters in all (checked below).
    s("keyterms", Kind::Table { columns: 1, rows: 100, chars: 1200 }),
    s("idle_s", Kind::Whole { min: 10, max: 600 }),
    s("max_s", Kind::Whole { min: 60, max: 3600 }),
    s("plan_credits", Kind::Whole { min: 0, max: 1_000_000_000 }),
    s("offset", Kind::Whole { min: 0, max: 1_000_000_000 }),
    s("cycle_day", Kind::Whole { min: 1, max: 28 }),
];

/// The models the page offers (ANTHROPIC.md §3): the 5.5 family only, pinned by name.
pub const MODELS: &[&str] = &["claude-haiku-5-5", "claude-sonnet-5-5", "claude-opus-5-5"];
pub const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// The language model: Anthropic (ANTHROPIC.md §4; CONSOLE.md §8). The agent's `config.py`
/// holds the same list for the page.
pub const LLM: &[Setting] = &[
    s("model", Kind::Word { words: MODELS, nullable: false }),
    s("effort", Kind::Word { words: EFFORTS, nullable: false }),
    // "on" is adaptive thinking; "off" is `disabled` on Haiku, `between_tools` on Sonnet
    // (checked with the model and effort in `coherent`).
    s("thinking", Kind::Word { words: &["on", "off"], nullable: false }),
    s("max_tokens", Kind::Whole { min: 256, max: 16_000 }),
    s("retry_max_tokens", Kind::Whole { min: 256, max: 32_000 }),
    s("cache_ttl", Kind::Word { words: &["5m", "1h"], nullable: false }),
    s("budget_usd", Kind::Whole { min: 0, max: 1_000_000 }),
    s("offset_usd", Kind::Number { min: 0.0, max: 1_000_000.0, nullable: false }),
    s("cycle_day", Kind::Whole { min: 1, max: 28 }),
];

const fn p(key: &'static str, placeholders: &'static [&'static str]) -> Setting {
    Setting { key, kind: Kind::Prose { placeholders } }
}

/// The agent's words (CONSOLE.md §9): every section of agent/prompts/AGENT-PROMPT.md and the
/// code values it must hold (SYSTEM-PAGE.md §3.1). The agent's prompts.py › PLACEHOLDERS is
/// the same list; a test fails if the two differ (test_prompts.py).
pub const PROMPT: &[Setting] = &[
    p("role", &["today"]),
    p("note.call_start", &["now"]),
    p("note.repeat_dropped", &[]),
    p("task.greet", &[]),
    p("task.verify", &[]),
    p("task.menu", &[]),
    p("task.schedule", &[]),
    p("tool.identify", &[]),
    p("arg.identify.given_name", &[]),
    p("arg.identify.family_name", &[]),
    p("arg.identify.birth_date", &[]),
    p("arg.identify.caller_wants", &[]),
    p("tool.confirm_details", &[]),
    p("tool.check_code", &[]),
    p("arg.check_code.code", &[]),
    p("tool.resend_code", &[]),
    p("tool.find_slots", &[]),
    p("arg.find_slots.reason", &[]),
    p("arg.find_slots.earliest_day", &[]),
    p("arg.find_slots.visit_mode", &[]),
    p("arg.find_slots.latest_day", &[]),
    p("arg.find_slots.time_of_day", &[]),
    p("arg.find_slots.days", &[]),
    p("arg.find_slots.clinician", &[]),
    p("arg.find_slots.more", &[]),
    p("tool.choose_option", &[]),
    p("arg.choose_option.option", &[]),
    p("tool.book", &[]),
    p("tool.change_visit", &[]),
    p("arg.change_visit.visit", &[]),
    p("tool.find_visit", &[]),
    p("arg.find_visit.when", &[]),
    p("arg.find_visit.clinician", &[]),
    p("arg.find_visit.past", &[]),
    p("arg.find_visit.group", &[]),
    p("tool.read_instructions", &[]),
    p("tool.end_call", &[]),
    p("tool.hand_off", &[]),
    p("tool.offer_staff", &[]),
    p("arg.offer_staff.why", &[]),
    p("tool.flag_for_staff", &[]),
    p("arg.flag_for_staff.kind", &[]),
    p("arg.flag_for_staff.summary", &[]),
    p("arg.flag_for_staff.evidence", &[]),
    p("tool.safety_reminder", &[]),
    p("arg.hand_off.kind", &[]),
    p("arg.hand_off.summary", &[]),
    p("arg.hand_off.evidence", &[]),
    p("tool.emergency", &[]),
    p("tool.medical_question", &[]),
    p("line.greeting", &[]),
    p("line.read_back", &["birth", "family", "given"]),
    p("line.ask_details", &[]),
    p("line.birth_not_possible", &[]),
    p("line.name_not_said", &[]),
    p("text.code", &["code"]),
    p("line.code_sent", &[]),
    p("line.code_length", &[]),
    p("line.verified", &[]),
    p("line.verified_go", &[]),
    p("note.after_verified", &[]),
    p("line.locked", &[]),
    p("line.code_wrong", &[]),
    p("line.no_more_codes", &[]),
    p("line.code_resent", &[]),
    p("line.no_slots", &[]),
    p("line.no_more_slots", &[]),
    p("line.slots", &["options"]),
    p("line.slot_option", &["number", "visit"]),
    p("line.which_option", &[]),
    p("line.confirm_slot", &["number", "visit"]),
    p("line.change_booked", &["visit"]),
    p("line.change_visit", &[]),
    p("line.slot_taken", &[]),
    p("line.book_not_through", &[]),
    p("line.booked", &["visit"]),
    p("line.no_visits", &[]),
    p("line.next_visit", &["visit"]),
    p("line.booked_visit_asked", &[]),
    p("line.visit_online", &[]),
    p("line.visit_type", &["visit_type"]),
    p("line.visit_reason", &["reason"]),
    p("line.visit_found", &["visit"]),
    p("line.past_seen", &["visit"]),
    p("line.past_missed", &["visit"]),
    p("line.past_cancelled", &["visit"]),
    p("line.one_more_visit", &[]),
    p("line.more_visits", &["count"]),
    p("line.one_more_visit_within", &["days"]),
    p("line.more_visits_within", &["count", "days"]),
    p("line.no_visit_match", &[]),
    p("line.no_past_visits", &[]),
    p("line.which_visit", &["groups"]),
    p("line.visit_group", &["group"]),
    p("line.another_visit", &[]),
    p("line.choice_or", &[]),
    p("line.offer_instructions", &[]),
    p("line.visit_instructions", &["instructions"]),
    p("line.no_instructions", &[]),
    p("note.after_find_visit", &[]),
    p("note.after_offer_instructions", &[]),
    p("note.after_which_visit", &[]),
    p("line.offer_staff", &[]),
    p("line.offer_staff_private", &[]),
    p("line.goodbye", &[]),
    p("line.hand_off", &[]),
    p("line.hand_off_code", &[]),
    p("note.after_reminder", &[]),
    p("note.after_flag", &[]),
    p("note.summary_pronoun", &[]),
    p("line.emergency_reminder", &[]),
    p("line.emergency", &[]),
    p("line.medical_question", &[]),
    p("line.not_on_this_call", &[]),
    p("line.not_now", &[]),
    p("line.one_moment", &[]),
    p("line.say_again", &[]),
    p("line.ehr_down", &[]),
];

/// The System variables (SYSTEM-PAGE.md §3): one setting, the list.
pub const SYSTEM: &[Setting] = &[s("variables", Kind::Variables)];

/// The System variables that cannot be removed or renamed: the code's own (SYSTEM-PAGE.md §3.1).
pub const BUILT_IN: &[&str] = &["clinic"];
/// The limits of SYSTEM-PAGE.md §3.2. The agent's prompts.py holds the same.
pub const VARIABLE_NAME_MAX: usize = 32;
pub const VARIABLE_MEANING_MAX: usize = 200;
pub const VARIABLE_VALUE_MAX: usize = 300;
pub const VARIABLES_MAX: usize = 100;

/// The System variables the agent's words start from: agent/prompts/VARIABLES.md, compiled in.
/// ehr fills `text.code` from it when a call names no System version.
const VARIABLES_FILE: &str = include_str!("../../agent/prompts/VARIABLES.md");

/// A service's name, its list and its table. The names come from here only, never from a
/// request, so no request text reaches the SQL.
pub fn service(name: &str) -> Option<(&'static str, &'static [Setting], &'static str)> {
    match name {
        "voice" => Some(("voice", VOICE, "settings_voice")),
        "llm" => Some(("llm", LLM, "settings_llm")),
        "prompt" => Some(("prompt", PROMPT, "settings_prompt")),
        "system" => Some(("system", SYSTEM, "settings_system")),
        _ => None,
    }
}

/// Every code value: a `{name}` the code fills, each in its own sections only.
pub fn code_values() -> BTreeSet<&'static str> {
    PROMPT
        .iter()
        .flat_map(|s| match s.kind {
            Kind::Prose { placeholders } => placeholders.to_vec(),
            _ => vec![],
        })
        .collect()
}

/// The System variables, by name: (meaning, value).
pub type Variables = BTreeMap<String, (String, String)>;

/// Why a variable's name does not fit, or None.
fn name_problem(name: &str) -> Option<&'static str> {
    let mut chars = name.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_lowercase()) {
        return Some("starts with a lowercase letter");
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Some("holds only a to z, 0 to 9 and _");
    }
    if name.len() > VARIABLE_NAME_MAX {
        return Some("is too long");
    }
    None
}

fn line_problem(text: &str, max: usize, braces: bool) -> Option<&'static str> {
    if text.chars().count() > max {
        return Some("too long");
    }
    if text.chars().any(char::is_control) {
        return Some("more than one line");
    }
    if !braces && text.contains(['{', '}']) {
        return Some("a { or }");
    }
    None
}

/// The System variables in a checked value, or why they do not fit. The message names a
/// variable, never its value.
pub fn variables(value: &Value) -> Result<Variables, String> {
    let Some(list) = value.as_array() else { return Err("variables: not a list".into()) };
    if list.len() > VARIABLES_MAX {
        return Err(format!("variables: at most {VARIABLES_MAX}"));
    }
    let codes = code_values();
    let mut out = Variables::new();
    for item in list {
        let field = |k: &str| item.get(k).and_then(Value::as_str);
        let (Some(name), Some(meaning), Some(v)) = (field("name"), field("meaning"), field("value")) else {
            return Err("variables: each needs a name, a meaning and a value".into());
        };
        if item.as_object().is_some_and(|o| o.len() != 3) {
            return Err("variables: each holds only a name, a meaning and a value".into());
        }
        if let Some(p) = name_problem(name) {
            return Err(format!("variables: a name {p}"));
        }
        if codes.contains(name) {
            return Err(format!("variables: {{{name}}} is filled by the code"));
        }
        if out.contains_key(name) {
            return Err(format!("variables: {{{name}}} is there twice"));
        }
        if let Some(p) = line_problem(meaning, VARIABLE_MEANING_MAX, true) {
            return Err(format!("variables: {{{name}}}'s meaning: {p}"));
        }
        if v.trim().is_empty() {
            return Err(format!("variables: {{{name}}} has no value"));
        }
        if let Some(p) = line_problem(v, VARIABLE_VALUE_MAX, false) {
            return Err(format!("variables: {{{name}}}'s value: {p}"));
        }
        out.insert(name.to_string(), (meaning.to_string(), v.to_string()));
    }
    for b in BUILT_IN {
        if !out.contains_key(*b) {
            return Err(format!("variables: {{{b}}} is built in and cannot be removed"));
        }
    }
    Ok(out)
}

/// The System variables of agent/prompts/VARIABLES.md: `## name`, then `meaning:` and `value:`
/// lines (prompts.py › load_variables reads it the same way).
pub fn file_variables() -> Result<Variables, String> {
    let mut list: Vec<Value> = vec![];
    for line in VARIABLES_FILE.lines().map(|l| l.trim_end_matches('\r')) {
        if let Some(name) = line.strip_prefix("## ") {
            list.push(serde_json::json!({"name": name.trim(), "meaning": "", "value": ""}));
        } else if let Some(last) = list.last_mut() {
            for k in ["meaning", "value"] {
                if let Some(rest) = line.strip_prefix(&format!("{k}:")) {
                    last[k] = Value::String(rest.trim().to_string());
                }
            }
        }
    }
    variables(&Value::Array(list)).map_err(|e| format!("VARIABLES.md: {e}"))
}

/// Every section of a prompt set holds only its own code values and System variables of
/// `names` (SYSTEM-PAGE.md §3.3). The error names the section and the variable.
pub fn prompt_variables(prompt: &[(&str, Value)], names: &BTreeSet<String>) -> Result<(), String> {
    let codes = code_values();
    for (key, value) in prompt {
        let Some(Setting { kind: Kind::Prose { placeholders: own }, .. }) = PROMPT.iter().find(|s| s.key == *key) else {
            continue;
        };
        let Some(found) = value.as_str().and_then(placeholders) else { continue };
        for name in found {
            if own.contains(&name.as_str()) {
                continue;
            }
            // Only a name that fits is ever echoed back: the rest of the text never is.
            if name_problem(&name).is_some() {
                return Err(format!("{key}: a {{…}} that is not a variable name"));
            }
            if codes.contains(name.as_str()) {
                return Err(format!("{key}: {{{name}}} is filled only in its own section"));
            }
            if !names.contains(&name) {
                return Err(format!("{key}: {{{name}}} is not a System variable"));
            }
        }
    }
    Ok(())
}

/// Fill the `{name}`s in `text` in one pass: a name `lookup` knows becomes its value; any other
/// stays as it is. With `last`, `{{` and `}}` become one brace (the text is final); without,
/// they stay, so a later pass reads them the same. A value is never read again, so a value
/// cannot add a `{name}`.
pub fn fill(text: &str, lookup: &dyn Fn(&str) -> Option<String>, last: bool) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' if chars.peek() == Some(&c) => {
                chars.next();
                out.push(c);
                if !last {
                    out.push(c);
                }
            }
            '{' => {
                let mut field = String::new();
                let mut closed = false;
                for ch in chars.by_ref() {
                    if ch == '}' {
                        closed = true;
                        break;
                    }
                    field.push(ch);
                }
                let name = field.split([':', '!']).next().unwrap_or("");
                match lookup(name).filter(|_| closed) {
                    Some(v) => out.push_str(&v),
                    None => {
                        out.push('{');
                        out.push_str(&field);
                        if closed {
                            out.push('}');
                        }
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// The `{placeholders}` in a text, read as Python's `string.Formatter().parse` reads them:
/// `{{` and `}}` are a plain brace; `{name}`, `{name:spec}` and `{name!r}` are the field `name`.
/// None for a stray brace.
pub fn placeholders(text: &str) -> Option<std::collections::BTreeSet<String>> {
    let mut found = std::collections::BTreeSet::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
            }
            '}' => return None,
            '{' => {
                let mut field = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('{') | None => return None,
                        Some(ch) => field.push(ch),
                    }
                }
                let name = field.split([':', '!']).next().unwrap_or("").to_string();
                found.insert(name);
            }
            _ => {}
        }
    }
    Some(found)
}

/// Why a section's text does not fit, or None. Named by the kind of problem, never the text.
fn prose_problem(placeholders_wanted: &[&str], value: &Value) -> Option<&'static str> {
    let Some(text) = value.as_str() else { return Some("not text") };
    if text.trim().is_empty() {
        return Some("empty");
    }
    if text.chars().count() > PROSE_MAX {
        return Some("too long");
    }
    if text.chars().any(|c| c.is_control() && c != '\n') {
        return Some("a control character");
    }
    let Some(found) = placeholders(text) else { return Some("a stray { or }") };
    if !placeholders_wanted.iter().all(|p| found.contains(*p)) {
        return Some("a code value missing");
    }
    None
}

/// The rules between settings of one checked set, after `check`. The llm service: thinking is
/// off only where the model allows it (Haiku and Sonnet, at effort high or below; Opus never),
/// and the retry cap is at least the first cap. The error names a setting, never a value.
pub fn coherent(service: &str, values: &[(&'static str, Value)]) -> Result<(), String> {
    if service != "llm" {
        return Ok(());
    }
    let get = |k: &str| values.iter().find(|(key, _)| *key == k).map(|(_, v)| v);
    let word = |k: &str| get(k).and_then(Value::as_str).unwrap_or("");
    if word("thinking") == "off" {
        if word("model") == "claude-opus-5-5" {
            return Err("thinking: Opus 5.5 cannot turn thinking off".to_string());
        }
        if !matches!(word("effort"), "low" | "medium" | "high") {
            return Err("thinking: off needs effort high or below".to_string());
        }
    }
    let n = |k: &str| get(k).and_then(Value::as_i64).unwrap_or(0);
    if n("retry_max_tokens") < n("max_tokens") {
        return Err("retry_max_tokens: below max_tokens".to_string());
    }
    Ok(())
}

/// The longest short string a setting holds (a voice name or id).
const SHORT: usize = 64;

fn short(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty() && s.chars().count() <= SHORT)
}

/// Whether `value` fits `kind`.
pub fn fits(kind: Kind, value: &Value) -> bool {
    match kind {
        Kind::Number { min, max, nullable } => match value {
            Value::Null => nullable,
            v => v.as_f64().is_some_and(|x| x.is_finite() && x >= min - 1e-9 && x <= max + 1e-9),
        },
        Kind::Whole { min, max } => value.as_i64().is_some_and(|x| (min..=max).contains(&x)),
        Kind::Bool => value.is_boolean(),
        Kind::Word { words, nullable } => match value {
            Value::Null => nullable,
            v => v.as_str().is_some_and(|w| words.contains(&w)),
        },
        Kind::Voice => {
            value.as_object().is_some_and(|o| o.len() == 2 && o.get("name").is_some_and(short) && o.get("id").is_some_and(short))
        }
        Kind::Table { columns, rows, chars } => {
            let Some(list) = value.as_array() else { return false };
            if list.len() > rows {
                return false;
            }
            let mut total = 0usize;
            for row in list {
                let Some(cells) = row.as_array() else { return false };
                if cells.len() != columns || !cells.iter().all(short) {
                    return false;
                }
                total += cells.iter().map(|c| c.as_str().unwrap_or("").chars().count()).sum::<usize>();
            }
            total <= chars
        }
        Kind::Prose { placeholders } => prose_problem(placeholders, value).is_none(),
        Kind::Variables => variables(value).is_ok(),
    }
}

/// Check a whole set: every setting of the service, once each, each value fitting its kind.
/// The error names the first setting that does not fit.
pub fn check(list: &[Setting], values: &[(String, String)]) -> Result<Vec<(&'static str, Value)>, String> {
    let mut out = Vec::with_capacity(list.len());
    for (key, _) in values {
        if !list.iter().any(|s| s.key == key) {
            return Err(format!("{}: not a setting of this service", if key.len() <= SHORT && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { key.as_str() } else { "a setting" }));
        }
    }
    for setting in list {
        let mut found = values.iter().filter(|(k, _)| k == setting.key);
        let Some((_, raw)) = found.next() else {
            return Err(format!("{}: missing", setting.key));
        };
        if found.next().is_some() {
            return Err(format!("{}: given twice", setting.key));
        }
        let value: Value = serde_json::from_str(raw).map_err(|_| format!("{}: not JSON", setting.key))?;
        if let Kind::Prose { placeholders } = setting.kind {
            // The page shows this, so it says what to fix: the kind of problem, never the text.
            if let Some(problem) = prose_problem(placeholders, &value) {
                let want = placeholders.iter().map(|p| format!("{{{p}}}")).collect::<Vec<_>>().join(" ");
                return Err(match problem {
                    "a code value missing" => format!("{}: needs {want}", setting.key),
                    p => format!("{}: {p}", setting.key),
                });
            }
        } else if let Kind::Variables = setting.kind {
            variables(&value)?;
        } else if !fits(setting.kind, &value) {
            return Err(format!("{}: out of range or the wrong kind", setting.key));
        }
        out.push((setting.key, value));
    }
    Ok(out)
}

/// One saved version.
#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    pub id: i64,
    pub day: String,
    pub seq: i32,
    pub created_at: String,
    pub created_by: String,
    pub active: bool,
}

fn db(step: &str, e: sqlx::Error) -> String {
    let code = e.as_database_error().and_then(|d| d.code().map(|c| c.to_string())).unwrap_or_default();
    format!("settings: {step} failed {code}").trim_end().to_string()
}

const VERSION_COLUMNS: &str = "v.id, to_char(v.day, 'YYYY-MM-DD') AS day, v.seq, \
    to_char(v.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, v.created_by, \
    (v.id = (SELECT a.version_id FROM setting_activations a WHERE a.service = v.service ORDER BY a.id DESC LIMIT 1)) AS active";

fn version(row: &sqlx::postgres::PgRow) -> Version {
    Version {
        id: row.get("id"),
        day: row.get("day"),
        seq: row.get("seq"),
        created_at: row.get("created_at"),
        created_by: row.get("created_by"),
        active: row.get::<Option<bool>, _>("active").unwrap_or(false),
    }
}

/// Every version of a service, newest first, the Default version (number 0) last.
pub async fn versions(pool: &PgPool, service: &str) -> Result<Vec<Version>, String> {
    let rows = sqlx::query(&format!(
        "SELECT {VERSION_COLUMNS} FROM setting_versions v WHERE v.service = $1 ORDER BY (v.seq = 0), v.id DESC"
    ))
        .bind(service)
        .fetch_all(pool)
        .await
        .map_err(|e| db("list", e))?;
    Ok(rows.iter().map(version).collect())
}

/// One version and its values: the active one when `id` is 0. None when there is none.
pub async fn get(pool: &PgPool, service: &str, table: &str, id: i64) -> Result<Option<(Version, Vec<(String, String)>)>, String> {
    let row = if id == 0 {
        sqlx::query(&format!(
            "SELECT {VERSION_COLUMNS} FROM setting_versions v WHERE v.service = $1 AND v.id = \
             (SELECT a.version_id FROM setting_activations a WHERE a.service = $1 ORDER BY a.id DESC LIMIT 1)"
        ))
        .bind(service)
        .fetch_optional(pool)
        .await
    } else {
        sqlx::query(&format!("SELECT {VERSION_COLUMNS} FROM setting_versions v WHERE v.service = $1 AND v.id = $2"))
            .bind(service)
            .bind(id)
            .fetch_optional(pool)
            .await
    }
    .map_err(|e| db("get", e))?;
    let Some(row) = row else { return Ok(None) };
    let v = version(&row);
    let values = sqlx::query(&format!("SELECT key, value::text AS value FROM {table} WHERE version_id = $1 ORDER BY key"))
        .bind(v.id)
        .fetch_all(pool)
        .await
        .map_err(|e| db("values", e))?
        .iter()
        .map(|r| (r.get::<String, _>("key"), r.get::<String, _>("value")))
        .collect();
    Ok(Some((v, values)))
}

/// Why a save did not happen.
#[derive(Debug, PartialEq)]
pub enum SaveError {
    /// A seed, and the service has its Default version already.
    DefaultExists,
    Db(String),
}

/// Save a checked set as a new version, in one transaction. A plain save is the day's next
/// number and becomes the active one. A `seed` is the Default version (number 0, one per
/// service), active only when nothing is active yet. The day's number comes under a lock, so two
/// saves at once never share one.
pub async fn save(
    pool: &PgPool,
    service: &str,
    table: &str,
    values: &[(&str, Value)],
    by: &str,
    seed: bool,
) -> Result<Version, SaveError> {
    let mut tx = pool.begin().await.map_err(|e| SaveError::Db(db("begin", e)))?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('deets.settings'))")
        .execute(&mut *tx)
        .await
        .map_err(|e| SaveError::Db(db("lock", e)))?;
    let has_default: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM setting_versions WHERE service = $1 AND seq = 0)")
        .bind(service)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| SaveError::Db(db("default", e)))?;
    if seed && has_default {
        return Err(SaveError::DefaultExists);
    }
    let has_active: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM setting_activations WHERE service = $1)")
        .bind(service)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| SaveError::Db(db("active", e)))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO setting_versions (service, day, seq, created_by) \
         SELECT $1, CURRENT_DATE, CASE WHEN $3 THEN 0 ELSE COALESCE(MAX(seq), 0) + 1 END, $2 \
         FROM setting_versions WHERE service = $1 AND day = CURRENT_DATE \
         RETURNING id",
    )
    .bind(service)
    .bind(by)
    .bind(seed)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| SaveError::Db(db("version", e)))?;
    let activate = !seed || !has_active;
    for (key, value) in values {
        sqlx::query(&format!("INSERT INTO {table} (version_id, key, value) VALUES ($1, $2, $3::jsonb)"))
            .bind(id)
            .bind(key)
            .bind(value.to_string())
            .execute(&mut *tx)
            .await
            .map_err(|e| SaveError::Db(db("value", e)))?;
    }
    if activate {
        sqlx::query("INSERT INTO setting_activations (service, version_id, by_who) VALUES ($1, $2, $3)")
            .bind(service)
            .bind(id)
            .bind(by)
            .execute(&mut *tx)
            .await
            .map_err(|e| SaveError::Db(db("activate", e)))?;
    }
    tx.commit().await.map_err(|e| SaveError::Db(db("commit", e)))?;
    let (v, _) = get(pool, service, table, id)
        .await
        .map_err(SaveError::Db)?
        .ok_or_else(|| SaveError::Db("settings: the new version is missing".to_string()))?;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn full() -> Vec<(String, String)> {
        let v = |k: &str, x: Value| (k.to_string(), x.to_string());
        vec![
            v("voice", json!({"name": "Skylar", "id": "db6b0ed5-d5d3-463d-ae85-518a07d3c2b4"})),
            v("speed", json!(null)),
            v("volume", json!(1.2)),
            v("emotion", json!("calm")),
            v("normalization", json!("auto")),
            v("pronunciations", json!([["Lisinopril", "lie-SIN-oh-pril"]])),
            v("turn_start_threshold", json!(0.7)),
            v("turn_eager_end_threshold", json!(null)),
            v("turn_end_threshold", json!(0.4)),
            v("turn_end_timeout_ms", json!(4500)),
            v("eager_end_of_turn", json!(false)),
            v("barge_in", json!(true)),
            v("keyterms", json!([["Main Street Clinic"]])),
            v("idle_s", json!(60)),
            v("max_s", json!(360)),
            v("plan_credits", json!(20000)),
            v("offset", json!(270)),
            v("cycle_day", json!(12)),
        ]
    }

    #[test]
    fn a_whole_set_passes() {
        assert_eq!(check(VOICE, &full()).unwrap().len(), VOICE.len());
    }

    #[test]
    fn each_refusal_names_the_setting() {
        let mut set = full();
        set.pop();
        assert_eq!(check(VOICE, &set).unwrap_err(), "cycle_day: missing");

        let mut set = full();
        set.push(("speed".into(), "1.0".into()));
        assert_eq!(check(VOICE, &set).unwrap_err(), "speed: given twice");

        let mut set = full();
        set.push(("colour".into(), "1".into()));
        assert_eq!(check(VOICE, &set).unwrap_err(), "colour: not a setting of this service");

        let mut set = full();
        set[1].1 = "1.6".into(); // speed above 1.5
        assert_eq!(check(VOICE, &set).unwrap_err(), "speed: out of range or the wrong kind");

        let mut set = full();
        set[17].1 = "29".into(); // cycle_day above 28
        assert!(check(VOICE, &set).unwrap_err().starts_with("cycle_day:"));
    }

    fn llm(model: &str, effort: &str, thinking: &str, max: i64, retry: i64) -> Vec<(String, String)> {
        let v = |k: &str, x: Value| (k.to_string(), x.to_string());
        vec![
            v("model", json!(model)),
            v("effort", json!(effort)),
            v("thinking", json!(thinking)),
            v("max_tokens", json!(max)),
            v("retry_max_tokens", json!(retry)),
            v("cache_ttl", json!("5m")),
            v("budget_usd", json!(100)),
            v("offset_usd", json!(12.5)),
            v("cycle_day", json!(1)),
        ]
    }

    #[test]
    fn the_llm_rules() {
        let ok = |set: Vec<(String, String)>| coherent("llm", &check(LLM, &set).unwrap());
        assert!(ok(llm("claude-haiku-5-5", "low", "on", 1200, 2400)).is_ok());
        assert!(ok(llm("claude-haiku-5-5", "high", "off", 1200, 2400)).is_ok());
        assert!(ok(llm("claude-sonnet-5-5", "medium", "off", 1200, 1200)).is_ok());
        assert_eq!(ok(llm("claude-opus-5-5", "low", "off", 1200, 2400)).unwrap_err(), "thinking: Opus 5.5 cannot turn thinking off");
        assert_eq!(ok(llm("claude-sonnet-5-5", "xhigh", "off", 1200, 2400)).unwrap_err(), "thinking: off needs effort high or below");
        assert_eq!(ok(llm("claude-haiku-5-5", "low", "on", 2400, 1200)).unwrap_err(), "retry_max_tokens: below max_tokens");
        assert!(check(LLM, &llm("claude-haiku-4-5", "low", "on", 1200, 2400)).unwrap_err().starts_with("model:"));
    }

    #[test]
    fn the_prompt_sections_are_checked() {
        let set: Vec<(String, String)> = PROMPT
            .iter()
            .map(|s| {
                let Kind::Prose { placeholders } = s.kind else { unreachable!() };
                let words = placeholders.iter().map(|p| format!("{{{p}}}")).collect::<Vec<_>>().join(" ");
                (s.key.to_string(), json!(format!("Words {words}.")).to_string())
            })
            .collect();
        assert_eq!(check(PROMPT, &set).unwrap().len(), 118);

        let with = |key: &str, text: &str| {
            let mut s = set.clone();
            s.iter_mut().find(|(k, _)| k == key).unwrap().1 = json!(text).to_string();
            check(PROMPT, &s)
        };
        assert_eq!(with("line.booked", "You're booked.").unwrap_err(), "line.booked: needs {visit}");
        assert_eq!(with("line.goodbye", "Bye { now.").unwrap_err(), "line.goodbye: a stray { or }");
        assert_eq!(with("line.goodbye", "  ").unwrap_err(), "line.goodbye: empty");
        assert!(with("line.goodbye", "Use {{braces}} freely.").is_ok());
        assert!(with("line.booked", "Booked: {visit:>3}.").is_ok());
        // Other names pass `check`; `prompt_variables` holds them to the System variables.
        assert!(with("line.goodbye", "Bye from {clinic}.").is_ok());
    }

    fn prompt_with(key: &str, text: &str) -> Vec<(&'static str, Value)> {
        PROMPT
            .iter()
            .map(|s| {
                let Kind::Prose { placeholders } = s.kind else { unreachable!() };
                let words = placeholders.iter().map(|p| format!("{{{p}}}")).collect::<Vec<_>>().join(" ");
                (s.key, json!(if s.key == key { text.to_string() } else { format!("Words {words}.") }))
            })
            .collect()
    }

    #[test]
    fn a_section_holds_its_own_code_values_and_system_variables() {
        let names: BTreeSet<String> = ["clinic", "pharmacy_hours"].map(String::from).into();
        assert!(prompt_variables(&prompt_with("line.goodbye", "Bye from {clinic}, open {pharmacy_hours}."), &names).is_ok());
        assert_eq!(
            prompt_variables(&prompt_with("line.goodbye", "Bye {parking}."), &names).unwrap_err(),
            "line.goodbye: {parking} is not a System variable"
        );
        assert_eq!(
            prompt_variables(&prompt_with("line.goodbye", "Bye {given}."), &names).unwrap_err(),
            "line.goodbye: {given} is filled only in its own section"
        );
        assert!(prompt_variables(&prompt_with("line.booked", "{clinic}: booked {visit}."), &names).is_ok());
    }

    #[test]
    fn the_system_variables_are_checked() {
        let one = |name: &str, meaning: &str, value: &str| json!({"name": name, "meaning": meaning, "value": value});
        let clinic = one("clinic", "The clinic's name.", "Main Street Clinic");
        let ok = |list: Vec<Value>| variables(&Value::Array(list));
        assert_eq!(ok(vec![clinic.clone(), one("pharmacy_hours", "When it opens.", "9 to 5")]).unwrap().len(), 2);
        assert_eq!(ok(vec![]).unwrap_err(), "variables: {clinic} is built in and cannot be removed");
        assert_eq!(ok(vec![clinic.clone(), one("Bad", "", "x")]).unwrap_err(), "variables: a name starts with a lowercase letter");
        assert_eq!(ok(vec![clinic.clone(), one("given", "", "x")]).unwrap_err(), "variables: {given} is filled by the code");
        assert_eq!(ok(vec![clinic.clone(), clinic.clone()]).unwrap_err(), "variables: {clinic} is there twice");
        assert_eq!(ok(vec![clinic.clone(), one("x", "", "{code}")]).unwrap_err(), "variables: {x}'s value: a { or }");
        assert_eq!(ok(vec![clinic.clone(), one("x", "", " ")]).unwrap_err(), "variables: {x} has no value");
        assert_eq!(ok(vec![clinic.clone(), one("x", "", "a\nb")]).unwrap_err(), "variables: {x}'s value: more than one line");
        assert!(file_variables().unwrap().contains_key("clinic"));
    }

    #[test]
    fn fill_is_one_pass() {
        let vars = |n: &str| match n {
            "clinic" => Some("Main {clinic} Clinic".to_string()),
            _ => None,
        };
        // In one pass a value is never read again (across passes, `variables` refuses a brace in
        // a value); an unknown name and the escapes stay for the last pass.
        assert_eq!(fill("{clinic}: {code} {{x}}", &vars, false), "Main {clinic} Clinic: {code} {{x}}");
        assert_eq!(fill("{{x}} {code}", &|n| (n == "code").then(|| "123".to_string()), true), "{x} 123");
    }

    #[test]
    fn the_kinds() {
        assert!(fits(Kind::Number { min: 0.5, max: 0.9, nullable: true }, &json!(null)));
        assert!(!fits(Kind::Word { words: &["auto", "off"], nullable: false }, &json!(null)));
        assert!(!fits(Kind::Voice, &json!({"name": "Skylar"})));
        assert!(!fits(Kind::Table { columns: 2, rows: 1, chars: 64 }, &json!([["a", "b"], ["c", "d"]])));
        assert!(!fits(Kind::Table { columns: 1, rows: 100, chars: 1200 }, &json!([["   "]])));
        let long: Vec<Value> = (0..30).map(|_| json!(["x".repeat(50)])).collect();
        assert!(!fits(Kind::Table { columns: 1, rows: 100, chars: 1200 }, &Value::Array(long)), "1,500 characters");
    }
}
