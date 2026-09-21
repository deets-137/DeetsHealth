mod log;
mod settings;
mod watchdog;
mod window;

/// The sign-in return scheme (DATA-ARCHITECTURE.md §2). A debug build owns its own, so a
/// dev run never takes over the installed app's links.
#[cfg(debug_assertions)]
const LINK_SCHEME: &str = "deetshealth-dev";
#[cfg(not(debug_assertions))]
const LINK_SCHEME: &str = "deetshealth";

/// A `deetshealth://…` link among the arguments of a launch, if there is one.
fn link_in_args<'a>(mut args: impl Iterator<Item = &'a String>) -> Option<&'a String> {
    let prefix = format!("{LINK_SCHEME}://");
    args.find(|a| a.starts_with(&prefix))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // FIRST plugin on purpose: a second process must be turned away BEFORE any setup runs,
    // or it opens the same data dir. The callback brings the running window forward.
    // The same second process is how a sign-in link will reach the app: the browser starts
    // the exe with the link as its argument, and it lands here as argv. No sign-in exists
    // yet, so a link is dropped and logged as ignored, with no values (SECURITY.md §4.4).
    let single_instance = tauri_plugin_single_instance::init(|app: &tauri::AppHandle, argv, _cwd| {
        if link_in_args(argv.iter()).is_some() {
            log::warn("sign-in: a link arrived, but no sign-in was waiting; ignored");
        }
        window::show_main(app);
    });

    tauri::Builder::default()
        .plugin(single_instance)
        // Registers the scheme (the installer for a release, `register()` below for a debug
        // build). Link delivery itself is the single-instance callback above.
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            use tauri::Manager;

            let dir = app.path().app_data_dir().expect("app data dir");
            std::fs::create_dir_all(&dir).ok();
            // The rolling log first: everything below may need to write.
            log::init(&dir);

            app.manage(settings::Settings::load(dir.clone()));
            window::restore_pos(app.handle());

            // The deep-link scheme. A debug build owns `deetshealth-dev://` and points it at
            // this exe on every launch — one HKCU key, never the installed app's scheme.
            // From a Claude desktop session this write lands in the MSIX private hive, not the
            // real registry (CLAUDE.md › How to verify).
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                #[cfg(debug_assertions)]
                let registered = app.deep_link().register(LINK_SCHEME);
                #[cfg(not(debug_assertions))]
                let registered = app.deep_link().register_all();
                if let Err(e) = registered {
                    log::warn(&format!("sign-in: could not register {LINK_SCHEME}://: {e}"));
                }
            }
            let args: Vec<String> = std::env::args().collect();
            if link_in_args(args.iter()).is_some() {
                log::warn("sign-in: a link started the app, but no sign-in was waiting; ignored");
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
