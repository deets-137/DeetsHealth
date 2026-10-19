// Opens the sign-in page in the user's default browser (SECURITY.md §4.4: never WebView2).
// Rust opens it; no opener plugin is exposed to the front end (§4.3). The URL names the
// site, so it is never logged or kept.

use deetshealth_core::platform::OpenBrowser;
use deetshealth_core::CoreError;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

pub struct SystemBrowser;

impl OpenBrowser for SystemBrowser {
    fn open_url(&self, url: &str) -> Result<(), CoreError> {
        // core::net::open_in_browser refuses anything but https already; this is the host's
        // own check, so no other scheme can ever reach ShellExecute from here.
        if !url.starts_with("https://") {
            return Err(CoreError::Browser);
        }
        let file = HSTRING::from(url);
        // SAFETY: both strings live across the call; no window handle, parameters or folder.
        let r = unsafe { ShellExecuteW(None, &HSTRING::from("open"), &file, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
        // ShellExecute reports success as a value above 32.
        if r.0 as usize > 32 {
            Ok(())
        } else {
            Err(CoreError::Browser)
        }
    }
}
