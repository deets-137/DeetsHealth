//! The rolling app log — `<app_data>/deetshealth.log` (DeetsMusic's docs/ops/LOGGING.md).
//!
//! Hand-rolled, no crate. One line per event, three levels, always on, bounded:
//! rotate at 512 KB, keep one generation (`deetshealth.1.log`), so the file can
//! never grow past about 1 MB. Every line also goes to stdout (visible under the
//! dev runner).
//!
//! **The PHI wall (SECURITY.md §4.5).** A line carries counts, resource types, durations
//! and error kinds. Never a value from a record, a name, an id, a URL with a query string,
//! or a real site's name. The wrapper type makes a protected value impossible to format
//! here; the rules below are the second layer, and they live INSIDE `write`, not at the
//! call sites, so one careless `format!` cannot defeat them:
//! - **Redaction**: anything shaped like a JWT (`eyJ…`), anything after `Bearer `, every
//!   value passed to `register_secret`, and every URL's query string (`?code=…&state=…`
//!   is how a sign-in returns) are replaced. A path under the user's folder loses the
//!   Windows account name.
//! - **Panics**: the hook writes where the panic happened and never what it said. A panic
//!   message can quote the value that caused it.
//! - **Never panic**: a log line must never take the app down. Every I/O result is dropped.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const FILE: &str = "deetshealth.log";
const PREV: &str = "deetshealth.1.log";
const ROTATE_AT: u64 = 512 * 1024;

static PATH: OnceLock<PathBuf> = OnceLock::new();

/// The open log file, kept open across lines. The length is counted here so the rotation
/// check needs no stat. Each line is still one unbuffered `write`, so a crash loses nothing.
struct Sink {
    file: std::fs::File,
    /// Our count of the file's bytes; re-read from disk every RESYNC_EVERY lines.
    len: u64,
    /// Lines written since the last disk check.
    since_check: u32,
    /// After a failed rotation (a reader holds the file without share-delete), no retry
    /// before this — or every line would pay the rename attempt.
    retry_rotate_at: Option<Instant>,
}
static SINK: Mutex<Option<Sink>> = Mutex::new(None);
/// Another process can share the file (a second dev run). Its writes are not in our count,
/// and its rotation renames the file from under our handle. So every RESYNC_EVERY lines the
/// path is stat'ed: a shorter file than we counted (or none) means someone rotated — reopen;
/// a longer one means someone else wrote — adopt its length.
const RESYNC_EVERY: u32 = 32;
const ROTATE_RETRY: Duration = Duration::from_secs(30);

/// Append `text` (which must end in its newline) to the log file, rotating first when the
/// file has passed ROTATE_AT. Every error is dropped. A poisoned lock is taken over, never
/// left: the panic hook logs through here.
fn append(text: &str) {
    let Some(path) = PATH.get() else { return };
    let mut sink = SINK.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(s) = sink.as_mut() {
        if s.since_check >= RESYNC_EVERY {
            s.since_check = 0;
            match std::fs::metadata(path) {
                Ok(m) if m.len() >= s.len => s.len = m.len(),
                _ => *sink = None, // rotated or deleted by another process: reopen the path
            }
        }
    }
    let due = sink.as_ref().map_or(false, |s| {
        s.len > ROTATE_AT && s.retry_rotate_at.map_or(true, |t| Instant::now() >= t)
    });
    if due {
        // Two files rather than a truncate: the run-up to a fault is the part worth reading.
        // Close ours first: the handle would follow the renamed file.
        let old = sink.take();
        if std::fs::rename(path, path.with_file_name(PREV)).is_err() {
            *sink = old.map(|mut s| {
                s.retry_rotate_at = Some(Instant::now() + ROTATE_RETRY);
                s
            });
        }
    }
    if sink.is_none() {
        let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(path) else { return };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        *sink = Some(Sink { file, len, since_check: 0, retry_rotate_at: None });
    }
    if let Some(s) = sink.as_mut() {
        if s.file.write_all(text.as_bytes()).is_ok() {
            s.len += text.len() as u64;
            s.since_check += 1;
        } else {
            *sink = None; // reopen on the next line (the file was deleted or locked)
        }
    }
}

/// Point the log at the app data dir, install the panic hook, and write the startup line.
/// Call once, first thing in `setup()`.
pub fn init(dir: &Path) {
    let _ = PATH.set(dir.join(FILE));

    // A panic on a worker thread is otherwise silent in a release build. The default hook is
    // NOT chained: it prints the panic message to stderr, and the message can carry a value
    // (SECURITY.md §4.5). The place is enough to find the line.
    std::panic::set_hook(Box::new(|info| {
        let at = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_default();
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let line = format!("panic in thread `{thread}` at {at} (the message is withheld: it can carry record data)");
        error(&line);
        let _ = writeln!(std::io::stderr(), "{line}");
    }));

    let which = if dir.file_name().and_then(|n| n.to_str()) == Some("com.deetshealth.dev") { "dev" } else { "release" };
    info(&format!(
        // The UTC offset: line times are local.
        "start: DeetsHealth {} · Windows {} · data dir {which} · UTC{}",
        env!("CARGO_PKG_VERSION"),
        windows_build(),
        chrono::Local::now().format("%:z")
    ));
}

pub fn info(msg: &str) {
    write("INFO ", msg);
}
pub fn warn(msg: &str) {
    write("WARN ", msg);
}
pub fn error(msg: &str) {
    write("ERROR", msg);
}

/// The front-end half (`src/diag.ts`): its ring buffer, appended as one block — a header
/// line through the normal path, then the body indented and scrubbed, file only. The ring
/// holds tags, counts and kinds by rule (diag.ts header). Called on an uncaught error, on
/// `beforeunload`, and on the five-minute timer.
#[tauri::command]
pub fn diag_flush(text: String) {
    let lines: Vec<&str> = text.lines().collect();
    let Some((head, body)) = lines.split_first() else { return };
    info(&format!("diag: {head}"));
    let mut block = String::new();
    for l in body {
        block.push_str("    ");
        block.push_str(&scrub(l));
        block.push('\n');
    }
    append(&block);
}

/// Front-end events arrive without bound (a broken page can call in a loop), so the file
/// takes at most FE_BURST of them per FE_WINDOW; the overflow is counted and reported.
const FE_BURST: u32 = 60;
const FE_WINDOW: Duration = Duration::from_secs(60);
/// (window start, lines written in it, lines dropped in it)
static FE_RATE: Mutex<(Option<Instant>, u32, u32)> = Mutex::new((None, 0, 0));

/// One front-end warning or error, written AS IT HAPPENS (`diag.warn` / `diag.error`).
/// Line: `WARN  fe: <tag> <json>`. Scrubbed by `write` like every other line.
#[tauri::command]
pub fn log_event(level: String, tag: String, data: Option<String>) {
    let dropped = {
        let Ok(mut rate) = FE_RATE.lock() else { return };
        let mut reported = 0;
        if rate.0.map_or(true, |start| start.elapsed() > FE_WINDOW) {
            reported = rate.2;
            *rate = (Some(Instant::now()), 0, 0);
        }
        if rate.1 >= FE_BURST {
            rate.2 += 1;
            return;
        }
        rate.1 += 1;
        reported
    };
    if dropped > 0 {
        warn(&format!("fe: {dropped} front-end event(s) dropped by the rate limit"));
    }
    let tag: String = tag
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-' | '.'))
        .take(48)
        .collect();
    let data: String = data.unwrap_or_default().chars().take(600).collect();
    let msg = if data.is_empty() { format!("fe: {tag}") } else { format!("fe: {tag} {data}") };
    match level.as_str() {
        "error" => error(&msg),
        "warn" => warn(&msg),
        _ => info(&msg),
    }
}

fn write(level: &str, msg: &str) {
    let msg = scrub(msg);
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("{ts}  {level} {msg}");
    // Not println!: it panics if stdout is a pipe whose reader went away (the dev runner
    // killed while the app lives on).
    let _ = writeln!(std::io::stdout(), "{line}");
    append(&format!("{line}\n"));
}

/// Credentials that no pattern can catch, redacted by exact value.
static SECRETS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// `C:\Users\<name>`: `scrub` writes it as `%USERPROFILE%`. None if unset or too short to be safe.
static PROFILE: OnceLock<Option<String>> = OnceLock::new();
fn profile_dir() -> Option<String> {
    std::env::var("USERPROFILE").ok().map(|p| p.trim_end_matches('\\').to_string()).filter(|p| p.len() >= 8)
}

/// Redact `secret` from every later log line. Call it wherever a credential that is not
/// JWT-shaped enters the process (a refresh token, a client key).
#[allow(dead_code)] // the first caller is sign-in, after the shell
pub fn register_secret(secret: &str) {
    let secret = secret.trim();
    if secret.len() < 16 {
        return; // a short value would redact ordinary words
    }
    if let Ok(mut v) = SECRETS.lock() {
        if !v.iter().any(|s| s == secret) {
            v.push(secret.to_string());
        }
    }
}

/// Replace every registered secret, the user's folder, anything shaped like a JWT (`eyJ` +
/// base64url with dots), anything following `Bearer ` up to the next whitespace, and the
/// query string of every URL.
pub(crate) fn scrub(msg: &str) -> String {
    let mut owned = msg.to_string();
    if let Ok(v) = SECRETS.lock() {
        for s in v.iter() {
            if owned.contains(s.as_str()) {
                owned = owned.replace(s.as_str(), "[redacted]");
            }
        }
    }
    // A path under the user's folder names the Windows account: keep the rest of the path.
    if let Some(home) = PROFILE.get_or_init(profile_dir) {
        for form in [home.clone(), home.replace('\\', "/")] {
            if owned.contains(form.as_str()) {
                owned = owned.replace(form.as_str(), "%USERPROFILE%");
            }
        }
    }
    let owned = scrub_queries(&owned);
    let mut out = String::with_capacity(owned.len());
    let mut rest = owned.as_str();
    loop {
        let jwt = rest.find("eyJ");
        let bearer = rest.find("Bearer ");
        let (at, skip_prefix) = match (jwt, bearer) {
            (None, None) => break,
            (Some(j), None) => (j, 0),
            (None, Some(b)) => (b, "Bearer ".len()),
            (Some(j), Some(b)) => {
                if j <= b {
                    (j, 0)
                } else {
                    (b, "Bearer ".len())
                }
            }
        };
        out.push_str(&rest[..at + skip_prefix]);
        let tail = &rest[at + skip_prefix..];
        let end = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '=')))
            .unwrap_or(tail.len());
        out.push_str("[redacted]");
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// Cut the query string (and any fragment) off every `scheme://…` in `msg`: a sign-in
/// return carries `code` and `state`, and a FHIR search carries the patient id.
fn scrub_queries(msg: &str) -> String {
    let mut out = String::with_capacity(msg.len());
    let mut rest = msg;
    while let Some(at) = rest.find("://") {
        let (head, tail) = rest.split_at(at);
        out.push_str(head);
        let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
        let url = &tail[..end];
        match url.find(['?', '#']) {
            Some(q) => {
                out.push_str(&url[..q]);
                out.push_str("?[redacted]");
            }
            None => out.push_str(url),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// "11 24H2 build 26200.6584" — from the registry via `reg.exe`. Cheap, once at startup;
/// empty on any failure.
fn windows_build() -> String {
    const KEY: &str = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let value = |name: &str| -> String {
        reg(&["query", KEY, "/v", name])
            .ok()
            .and_then(|s| s.lines().find(|l| l.trim_start().starts_with(name)).map(|l| l.split_whitespace().last().unwrap_or("").to_string()))
            .unwrap_or_default()
    };
    let build = value("CurrentBuild");
    let ubr = value("UBR");
    let display = value("DisplayVersion");
    let ubr = u64::from_str_radix(ubr.trim_start_matches("0x"), 16).map(|n| n.to_string()).unwrap_or(ubr);
    let major = if build.parse::<u32>().map(|b| b >= 22000).unwrap_or(false) { "11" } else { "10" };
    format!("{major} {display} build {build}.{ubr}")
}

/// `reg.exe` with no console window. It waits for the process, so it never runs on the UI
/// thread from a command (CLAUDE.md › Conventions); `setup` calls it once.
fn reg(args: &[&str]) -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg").args(args).creation_flags(CREATE_NO_WINDOW).output().map_err(|e| format!("reg.exe: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::scrub;

    #[test]
    fn scrubs_jwt_and_bearer() {
        assert_eq!(scrub("token eyJabc.def-ghi_jk done"), "token [redacted] done");
        assert_eq!(scrub("Authorization: Bearer abc.def; next"), "Authorization: Bearer [redacted]; next");
        assert_eq!(scrub("plain line 401 /metadata"), "plain line 401 /metadata");
    }

    #[test]
    fn scrubs_registered_secret() {
        super::register_secret("NotAJwtShapedRefreshToken0123456789");
        assert_eq!(scrub("rt=NotAJwtShapedRefreshToken0123456789 ok"), "rt=[redacted] ok");
        super::register_secret("short"); // ignored: would redact ordinary words
        assert_eq!(scrub("a short word"), "a short word");
    }

    #[test]
    fn scrubs_url_queries() {
        assert_eq!(
            scrub("link deetshealth://callback?code=abc&state=xyz ignored"),
            "link deetshealth://callback?[redacted] ignored"
        );
        assert_eq!(scrub("GET https://example.org/fhir/Observation?patient=123 200"), "GET https://example.org/fhir/Observation?[redacted] 200");
        assert_eq!(scrub("GET https://example.org/metadata 200"), "GET https://example.org/metadata 200");
    }
}
