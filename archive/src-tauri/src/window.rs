//! The main window: the launch show, a second launch, and where the window sits.
//! DeetsMusic keeps these in tray.rs; DeetsHealth has no tray (HANDOFF.md › Decided, 6B a).
//!
//! The launch show (DeetsMusic's UX-COVERUPS.md §6): the window starts hidden
//! (tauri.conf.json `visible: false`). The page paints behind its launch cover
//! (src/boot-cover.ts), then calls `main_ready`, which shows the window in the page's
//! canvas color. If the page never says so, `reveal_fallback` shows it after 3 s, so a
//! page error cannot leave the app running with no window.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, PhysicalPosition, WindowEvent};

pub const MAIN: &str = "main";

static MAIN_SHOWN: AtomicBool = AtomicBool::new(false);
const READY_FALLBACK: Duration = Duration::from_secs(3);

/// The first show of a launch, in place (setup applied the saved position) and focused.
/// `color` is the page's canvas: the window's own background, so the first shown frame is
/// never WebView2's white.
fn reveal_main(app: &AppHandle, color: Option<[u8; 3]>) {
    if MAIN_SHOWN.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(w) = app.get_webview_window(MAIN) else { return };
    if let Some([r, g, b]) = color {
        w.set_background_color(Some(tauri::window::Color(r, g, b, 255))).ok();
    }
    w.show().ok();
    w.set_focus().ok();
}

/// The page has painted behind its cover (boot-cover.ts): show the window.
#[tauri::command]
pub fn main_ready(app: AppHandle, color: Option<[u8; 3]>) {
    reveal_main(&app, color);
}

/// Setup: show the window after READY_FALLBACK if the page never reported ready.
pub fn reveal_fallback(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(READY_FALLBACK);
        if !MAIN_SHOWN.load(Ordering::SeqCst) {
            crate::log::warn("launch: the page did not report ready; showing the window");
        }
        reveal_main(&app, None);
    });
}

/// A second launch (the pinned taskbar button): bring the running window forward.
pub fn show_main(app: &AppHandle) {
    let Some(w) = app.get_webview_window(MAIN) else { return };
    w.unminimize().ok();
    w.show().ok();
    w.set_focus().ok();
}

/// Put the window back where the last session left it (`settings.json` → `windowPos`).
/// Guarded on the monitor layout: a position from a monitor that is now unplugged would
/// park the window off-screen, so an unmatched position is discarded and Windows places
/// the window as it normally would.
pub fn restore_pos(app: &AppHandle) {
    let Some([x, y]) = app.state::<crate::settings::Settings>().get().window_pos else { return };
    let Some(w) = app.get_webview_window(MAIN) else { return };
    let on_a_monitor = app.available_monitors().is_ok_and(|mons| {
        mons.iter().any(|m| {
            let (p, sz) = (m.position(), m.size());
            x >= p.x && y >= p.y && x < p.x + sz.width as i32 && y < p.y + sz.height as i32
        })
    });
    if on_a_monitor {
        w.set_position(PhysicalPosition::new(x, y)).ok();
    }
}

/// Remember the position when the window closes. A maximized or minimized window reports
/// a position that is not the one to come back to, so those are skipped.
pub fn on_window_event(win: &tauri::Window, ev: &WindowEvent) {
    if win.label() != MAIN {
        return;
    }
    if let WindowEvent::CloseRequested { .. } = ev {
        if win.is_maximized().unwrap_or(false) || win.is_minimized().unwrap_or(false) {
            return;
        }
        let Ok(p) = win.outer_position() else { return };
        let settings = win.app_handle().state::<crate::settings::Settings>();
        if settings.get().window_pos != Some([p.x, p.y]) {
            if let Err(e) = settings.update(|d| d.window_pos = Some([p.x, p.y])) {
                crate::log::warn(&format!("window: could not save the position: {e}"));
            }
        }
    }
}
