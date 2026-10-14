// The system browser, for sign-in only.
mod browser;
// The Windows crypto provider and the per-site key store (SECURITY.md §4.6).
mod cng;
// Dev tools: the store inspector (feature `dev` only, never in a release; DEBUGGING.md).
#[cfg(feature = "dev")]
pub mod dev_tools;
// Every failure the user should hear about, as one `app-error` event (ERRORS.md).
mod errors;
// The fetch: runs the core's full fetch off the UI thread; counts to the front end.
mod fetch;
mod keystore;
mod log;
// The sign-in return: a listener on 127.0.0.1 while a sign-in waits (SIGN-IN.md §6).
mod loopback;
mod settings;
// Sign-in: the waiting sign-in, the returning link, the new sealed site (CORE-CRATE.md §6).
mod signin;
// One sealed SQLite file and one key per site; the Sites section's list and sign-out.
mod sites;
mod watchdog;
mod window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // FIRST plugin on purpose: a second process must be turned away BEFORE any setup runs,
    // or it opens the same data dir. The callback brings the running window forward.
    // (Sign-in links no longer arrive this way: they come to loopback.rs, 2026-10-13.)
    let single_instance = tauri_plugin_single_instance::init(|app: &tauri::AppHandle, _argv, _cwd| {
        window::show_main(app);
    });

    tauri::Builder::default()
        .plugin(single_instance)
        .setup(|app| {
            use tauri::Manager;

            let dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&dir).ok();
            // The rolling log first: everything below may need to write.
            log::init(&dir);

            // Errors queue here until the page listens (errors.rs).
            app.manage(errors::AppErrors::default());
            app.manage(settings::Settings::load(dir.clone()));
            app.manage(fetch::Running::default());
            window::restore_pos(app.handle());
            match signin::SignIn::new(sites::SiteFiles::new(&dir)) {
                Ok(s) => {
                    app.manage(s);
                    // Migrate old site files and rebuild stale indexes, off the UI thread.
                    fetch::check_sites(app.handle());
                }
                Err(e) => {
                    log::error(&format!("sign-in: unavailable: {e}"));
                    errors::report(app.handle(), errors::AppError::new("sign-in-unavailable").kind(e));
                }
            }

            // Show the window when the page is ready (`main_ready`), or after 3 s.
            window::reveal_fallback(app.handle());

            // No browser keys in the installed app: Ctrl+F find, Ctrl+P print, Ctrl+R / F5
            // reload, zoom, F7 caret browsing, Alt+Left back. The page still gets the keydown,
            // so the app's own shortcuts keep working. Release only, so F12 stays in dev.
            #[cfg(not(debug_assertions))]
            if let Some(win) = app.get_webview_window(window::MAIN) {
                let _ = win.with_webview(|wv| {
                    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
                    use windows::core::Interface;
                    let off = || -> windows::core::Result<()> {
                        unsafe {
                            let settings = wv.controller().CoreWebView2()?.Settings()?;
                            settings.cast::<ICoreWebView2Settings3>()?.SetAreBrowserAcceleratorKeysEnabled(false)
                        }
                    };
                    if let Err(e) = off() {
                        log::warn(&format!("webview: browser keys stay on: {e}"));
                    }
                });
            }

            // Auto-open devtools in dev so the webview console is visible. DEETS_NO_DEVTOOLS=1
            // holds it shut (`npm run dev:perf`): DevTools renders in the SAME GPU process as
            // the app, so a graphics measurement with it open measures DevTools too. Release
            // builds have no DevTools at all (SECURITY.md §4.3).
            #[cfg(debug_assertions)]
            if std::env::var("DEETS_NO_DEVTOOLS").as_deref() != Ok("1") {
                if let Some(win) = app.get_webview_window(window::MAIN) {
                    win.open_devtools();
                }
            }
            // A freeze cannot report itself: the thread that would write the line is the
            // thread that is stuck. This watches from outside (watchdog.rs).
            watchdog::start(app.handle());
            Ok(())
        })
        .on_window_event(window::on_window_event)
        .invoke_handler(invoke_with_watchdog(tauri::generate_handler![
            window::main_ready,
            signin::sign_in_start,
            signin::sign_in_cancel,
            signin::site_sign_in,
            fetch::site_refresh,
            sites::sites_list,
            sites::site_sign_out,
            errors::app_errors_ready,
            log::diag_flush,
            log::log_event,
        ]))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Wrap the generated handler so every command records its name while it runs
/// (`watchdog.rs`). The wrapper costs one mutex on each side of a call.
///
/// An **async** command returns here almost at once, because `spawn_blocking` moved the
/// body off this thread — so it clears its name and the watchdog sees nothing. A
/// **synchronous** command does not return until its body is done. A name still sitting
/// here after five seconds therefore names a sync command that is blocking the thread that
/// paints (DeetsMusic 0.12.0).
fn invoke_with_watchdog<R: tauri::Runtime, F>(handler: F) -> impl Fn(tauri::ipc::Invoke<R>) -> bool
where
    F: Fn(tauri::ipc::Invoke<R>) -> bool,
{
    move |invoke| {
        let name = invoke.message.command().to_string();
        watchdog::entered(&name);
        let handled = handler(invoke);
        watchdog::left();
        handled
    }
}
