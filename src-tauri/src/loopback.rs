// The sign-in return: a loopback listener (RFC 8252 §7.3; the owner, 2026-10-13). Epic's
// sign-in page cannot hand the browser to a custom scheme (its CSP `form-action *` blocks it;
// integrations/EPIC.md §3), so the site sends the browser to `http://127.0.0.1:<port>/callback`
// and this file answers it.
//
// - It listens on 127.0.0.1 only, on a port Windows picks, and only while one sign-in waits.
//   A new sign-in, Cancel, or the end of the wait (auth::WAIT_LIMIT_S) closes it.
// - It reads one request line. `/callback?…` goes to `signin::on_link`, which shields the link
//   and matches its `state` (SECURITY.md §4.4). The code is useless without the PKCE verifier,
//   which never leaves the app, so a program that reaches the port first gets nothing.
// - The callback answers `303 /done`, so the code leaves the address bar and the tab's
//   history at once, then `/done` shows a short page. The pages carry fixed words only.
// - The Host header must be this listener's own address: a web page cannot reach the port
//   through a DNS name that points at 127.0.0.1.
// - Nothing from the request is logged: a log line says only what happened.

use crate::log;
use crate::signin::{self, LinkOutcome};
use crate::window;
use deetshealth_core::Shielded;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;

/// The longest request head read. A browser's GET with a code and a state fits well inside.
const HEAD_MAX: usize = 8 * 1024;
/// How long one connection may take to send its request.
const READ_WAIT: Duration = Duration::from_secs(5);
/// After the callback, how long to wait for the browser to follow `303 /done`.
const DONE_WAIT: Duration = Duration::from_secs(10);
/// How often the accept loop looks at the stop flag and the clock.
const TICK: Duration = Duration::from_millis(100);

/// A listener bound and ready; the redirect URI names its port.
pub struct Bound {
    listener: TcpListener,
    pub port: u16,
}

impl Bound {
    /// Bind 127.0.0.1 on a port Windows picks.
    pub fn open() -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        Ok(Self { listener, port })
    }

    /// The redirect URI this listener answers.
    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.port)
    }

    /// Serve on its own thread until the sign-in ends: `stop` is set (a new sign-in, Cancel),
    /// or `wait` runs out.
    pub fn serve(self, app: AppHandle, stop: Arc<AtomicBool>, wait: Duration) {
        let spawned = std::thread::Builder::new().name("sign-in-loopback".into()).spawn(move || run(self, app, stop, wait));
        if spawned.is_err() {
            log::error("sign-in: could not start the return listener's thread");
        }
    }
}

/// Which page `/done` shows.
#[derive(Clone, Copy)]
enum Page {
    SignedIn,
    NotShared,
    Ended,
    NotCurrent,
}

impl Page {
    fn words(self) -> (&'static str, &'static str) {
        match self {
            Page::SignedIn => ("You're signed in", "DeetsHealth has your sign-in. You can close this tab."),
            Page::NotShared => (
                "Sign-in not finished",
                "The health system did not share access. You can close this tab and try again in DeetsHealth.",
            ),
            Page::Ended => ("This sign-in has ended", "Start again in DeetsHealth, under Settings, Sites."),
            Page::NotCurrent => (
                "This link is not for the current sign-in",
                "Finish the newest sign-in, or start again in DeetsHealth.",
            ),
        }
    }
}

fn run(bound: Bound, app: AppHandle, stop: Arc<AtomicBool>, wait: Duration) {
    let host = format!("127.0.0.1:{}", bound.port);
    let started = Instant::now();
    // Once the callback has its answer, only `/done` is left to serve, for DONE_WAIT.
    let mut finished: Option<(Page, Instant)> = None;
    loop {
        match finished {
            Some((_, at)) if at.elapsed() > DONE_WAIT => break,
            None if stop.load(Ordering::Relaxed) || started.elapsed() > wait => break,
            _ => {}
        }
        let stream = match bound.listener.accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(TICK);
                continue;
            }
            Err(e) => {
                log::warn(&format!("sign-in: the return listener stopped: {}", e.kind()));
                break;
            }
        };
        match answer(stream, &host, &app, finished.map(|(p, _)| p)) {
            Served::Done => break,
            Served::Final(page) => finished = Some((page, Instant::now())),
            Served::Other => {}
        }
    }
    log::info("sign-in: the return listener closed");
}

enum Served {
    /// The callback was answered; `/done` may follow.
    Final(Page),
    /// `/done` was shown: the listener can close.
    Done,
    /// Anything else (an icon request, a request that did not match).
    Other,
}

fn answer(mut stream: TcpStream, host: &str, app: &AppHandle, finished: Option<Page>) -> Served {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(READ_WAIT));
    let Some(head) = read_head(&mut stream) else { return Served::Other };
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or("").split(' ');
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let host_ok = lines.any(|l| l.split_once(':').is_some_and(|(k, v)| k.eq_ignore_ascii_case("host") && v.trim() == host));
    if !host_ok {
        reply(&mut stream, "421 Misdirected Request", None, None);
        return Served::Other;
    }
    if method != "GET" {
        reply(&mut stream, "405 Method Not Allowed", None, None);
        return Served::Other;
    }
    let path = target.split('?').next().unwrap_or("");
    match (path, finished) {
        ("/callback", None) => {
            // Shielded at once: the target holds the code.
            let link = Shielded::new(format!("http://{host}{target}"));
            let page = match signin::on_link(app, link) {
                LinkOutcome::Accepted => {
                    window::show_main(app);
                    Page::SignedIn
                }
                LinkOutcome::Refused => Page::NotShared,
                LinkOutcome::NotWaiting => Page::Ended,
                LinkOutcome::NotOurs => {
                    reply(&mut stream, "400 Bad Request", None, Some(Page::NotCurrent));
                    return Served::Other;
                }
            };
            reply(&mut stream, "303 See Other", Some("/done"), None);
            Served::Final(page)
        }
        ("/done", Some(page)) => {
            reply(&mut stream, "200 OK", None, Some(page));
            Served::Done
        }
        _ => {
            reply(&mut stream, "404 Not Found", None, None);
            Served::Other
        }
    }
}

/// The request head up to the blank line, as text, or `None` when it is too long, too slow,
/// or not text.
fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 || buf.len() + n > HEAD_MAX {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    String::from_utf8(buf).ok()
}

/// One response, then the connection closes. Fixed words only.
fn reply(stream: &mut TcpStream, status: &str, location: Option<&str>, page: Option<Page>) {
    let body = page.map(html).unwrap_or_default();
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\n\
         Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; form-action 'none'; frame-ancestors 'none'\r\n\
         Connection: close\r\n",
        body.len()
    );
    if let Some(to) = location {
        head.push_str(&format!("Location: {to}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

/// The page the browser tab shows. System colors and the system font: it follows the
/// browser's light or dark look, with no color of our own.
fn html(page: Page) -> String {
    let (title, line) = page.words();
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>DeetsHealth</title><style>\
         :root{{color-scheme:light dark}}\
         body{{margin:0;min-height:100vh;display:grid;place-items:center;background:Canvas;color:CanvasText;\
         font:16px/1.5 system-ui,sans-serif}}\
         main{{max-width:28rem;padding:2rem;text-align:center}}\
         h1{{font-size:1.4rem;margin:0 0 .5rem}}p{{margin:0;opacity:.8}}\
         </style></head><body><main><h1>{title}</h1><p>{line}</p></main></body></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_loopback_only_on_a_free_port() {
        let b = Bound::open().unwrap();
        let addr = b.listener.local_addr().unwrap();
        assert!(addr.ip().is_loopback());
        assert_ne!(b.port, 0);
        assert_eq!(b.redirect_uri(), format!("http://127.0.0.1:{}/callback", b.port));
    }

    #[test]
    fn pages_carry_fixed_words_only() {
        for p in [Page::SignedIn, Page::NotShared, Page::Ended, Page::NotCurrent] {
            let h = html(p);
            assert!(h.contains(p.words().0) && !h.contains("<script"));
        }
    }
}
