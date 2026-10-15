//! What macOS lets Null do, and so the agent it starts: Full Disk Access, and
//! the two permissions computer use needs.
//!
//! Full Disk Access is whether the agent may read any file without macOS asking.
//!
//! macOS holds Null responsible for whatever the harness touches. Without Full
//! Disk Access it asks the user about each protected folder in turn, and an agent
//! that searches the home folder sets off a run of those questions. macOS has no
//! prompt for Full Disk Access itself. An app can only find out whether it has
//! it, and show the user where the switch is.
//!
//! Null shows it once, after the first reply and not at the first start: a
//! person sees Null work before being sent to System Settings. Until then macOS
//! asks folder by folder, which people already know.

use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::log::log;
use crate::{panel, settings};

/// What the box says the one time Null asks.
pub const NOTICE: &str = "To stop macOS asking about each folder, turn on Null under Full Disk Access";

/// The Full Disk Access list in System Settings.
const SETTINGS_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

static ASKING: AtomicBool = AtomicBool::new(false);

/// True from a start that found Full Disk Access off and never asked for, until
/// the first reply of that run.
static WAITING: AtomicBool = AtomicBool::new(false);

/// True from the moment this run sends the user to the switch until they put the box away.
pub fn asking() -> bool {
    ASKING.load(Ordering::SeqCst)
}

/// The user has put the box away. Returns whether Null was asking until now.
pub fn stop_asking() -> bool {
    ASKING.swap(false, Ordering::SeqCst)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FullDisk {
    On,
    Off,
    /// The file that tells could not be tried at all.
    Unknown,
}

fn read(attempt: std::io::Result<()>) -> FullDisk {
    match attempt {
        Ok(()) => FullDisk::On,
        Err(e) if e.kind() == ErrorKind::PermissionDenied => FullDisk::Off,
        Err(_) => FullDisk::Unknown,
    }
}

/// Files that only Full Disk Access unlocks, in the order they are tried: the
/// person's own list of permissions, then folders macOS guards for them. macOS
/// 27 keeps no such list in that place any more, and opens the system's own to
/// every program, so there Safari's, Mail's or Messages' folder tells. Each is
/// only opened, and nothing in it is read.
fn guarded() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return Vec::new() };
    ["Application Support/com.apple.TCC/TCC.db", "Safari", "Containers/com.apple.Safari", "Mail", "Messages"].iter().map(|place| home.join("Library").join(place)).collect()
}

/// How it stands by the first of the attempts that tells anything: one that
/// finds no file goes on to the next.
fn first_told(attempts: impl IntoIterator<Item = std::io::Result<()>>) -> FullDisk {
    attempts.into_iter().map(read).find(|state| *state != FullDisk::Unknown).unwrap_or(FullDisk::Unknown)
}

/// Try to open a file that only Full Disk Access unlocks. The attempt is also
/// what puts Null in the Full Disk Access list, switched off.
fn full_disk() -> FullDisk {
    first_told(guarded().into_iter().map(|file| std::fs::File::open(file).map(|_| ())))
}

/// Whether Null is to send the user to the switch: it is off, and they have
/// never been asked. After the one time it is their decision, and macOS goes
/// back to asking folder by folder.
fn due(state: FullDisk, asked_before: bool) -> bool {
    state == FullDisk::Off && !asked_before
}

/// At start: note how Full Disk Access stands. When it is off and was never
/// asked for, the asking waits for the first reply (`after_reply`).
pub fn at_start(app: &AppHandle) {
    let (state, asked_before) = (full_disk(), settings::get(app).asked_full_disk);
    match state {
        FullDisk::On => log!("Full Disk Access is on"),
        FullDisk::Unknown => log!("could not tell whether Full Disk Access is on"),
        FullDisk::Off if asked_before => log!("Full Disk Access is off; the user has been asked before"),
        FullDisk::Off => log!("Full Disk Access is off; Null asks for it after the first reply"),
    }
    WAITING.store(due(state, asked_before), Ordering::SeqCst);
}

/// A reply has ended well, so the user has seen Null work. The first time, if
/// Full Disk Access is still off, send them to the switch, once.
pub fn after_reply(app: &AppHandle) {
    if !WAITING.swap(false, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    // Off the harness's own thread: this opens another app.
    std::thread::spawn(move || {
        // It may have been switched on since the start.
        if !due(full_disk(), settings::get(&app).asked_full_disk) {
            return;
        }
        settings::update(&app, |settings| settings.asked_full_disk = true);
        ASKING.store(true, Ordering::SeqCst);
        if settings::own_folder().is_some() {
            // A scripted run on a folder of its own sends nobody anywhere.
            log!("Full Disk Access is off; a run on a folder of its own does not open System Settings");
        } else {
            log!("Full Disk Access is off; opening its list in System Settings");
            if let Err(e) = std::process::Command::new("/usr/bin/open").arg(SETTINGS_URL).status() {
                log!("could not open System Settings: {e}");
            }
        }
        // The box says why, and stays up over System Settings until the user puts it away.
        panel::show(&app);
        let _ = app.emit_to(panel::LABEL, "mini:notice", NOTICE);
    });
}

// ── What computer use needs ───────────────────────────────────────────────
//
// The harness's computer use reads what a window holds and presses in it, which
// macOS lets a program do only with Accessibility, and takes pictures of the
// screen, which needs Screen Recording. macOS charges both to Null, which
// starts the harness. Unlike Full Disk Access, macOS has a question of its own
// for each, and asking it is also what puts Null in the list. Null asks only
// when the person switches computer use on, or asks for one in the `/computer`
// list: never at a start, and never by surprise.

/// One of macOS's permissions that computer use needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    /// To read what a window holds, and to press and type in it.
    Accessibility,
    /// To take a picture of the screen, for what a window's controls do not say.
    Screen,
}

/// How one of them stands, for the box to say.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Need {
    pub permission: Permission,
    /// The name of its list in System Settings.
    pub name: &'static str,
    /// Whether macOS has given it to Null.
    pub given: bool,
}

/// macOS's own functions for the two. The first of each pair only answers; the
/// second shows macOS's dialog.
#[allow(non_snake_case, non_upper_case_globals)]
mod macos {
    use std::ffi::c_void;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        pub fn AXIsProcessTrusted() -> u8;
        pub fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
        pub static kAXTrustedCheckOptionPrompt: *const c_void;
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        pub fn CGPreflightScreenCaptureAccess() -> bool;
        pub fn CGRequestScreenCaptureAccess() -> bool;
    }
}

impl Permission {
    pub const BOTH: [Permission; 2] = [Permission::Accessibility, Permission::Screen];

    pub fn name(self) -> &'static str {
        match self {
            Permission::Accessibility => "Accessibility",
            Permission::Screen => "Screen Recording",
        }
    }

    /// Its list in System Settings.
    fn list(self) -> &'static str {
        match self {
            Permission::Accessibility => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            Permission::Screen => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
        }
    }

    /// Whether macOS has given it to Null. Finding out shows nothing.
    pub fn given(self) -> bool {
        // SAFETY: both take nothing and only answer.
        unsafe {
            match self {
                Permission::Accessibility => macos::AXIsProcessTrusted() != 0,
                Permission::Screen => macos::CGPreflightScreenCaptureAccess(),
            }
        }
    }
}

/// How the two stand now.
pub fn needs() -> Vec<Need> {
    Permission::BOTH.iter().map(|&permission| Need { permission, name: permission.name(), given: permission.given() }).collect()
}

/// The first of them macOS has not given, in the order they are asked for.
pub fn first_missing(needs: &[Need]) -> Option<Permission> {
    needs.iter().find(|need| !need.given).map(|need| need.permission)
}

/// Ask macOS for one. Its own dialog names Null and leads to the switch. The
/// list in System Settings is opened as well, since macOS shows its dialog for
/// Screen Recording once only, and afterwards answers no without a word.
pub fn ask(app: &AppHandle, which: Permission) {
    if settings::own_folder().is_some() {
        // A scripted run on a folder of its own sends nobody anywhere.
        log!("{} is off; a run on a folder of its own asks macOS for nothing", which.name());
        return;
    }
    log!("{} is off; asking macOS for it, and opening its list in System Settings", which.name());
    // macOS's dialogs belong to the main thread.
    let _ = app.run_on_main_thread(move || {
        use objc::runtime::{Object, YES};
        use objc::{class, msg_send, sel, sel_impl};
        // SAFETY: the dialog is asked for with the one option it takes, in a dictionary that
        // lives until the call returns; the other function takes nothing.
        unsafe {
            match which {
                Permission::Accessibility => {
                    let yes: *mut Object = msg_send![class!(NSNumber), numberWithBool: YES];
                    let options: *mut Object = msg_send![class!(NSDictionary), dictionaryWithObject: yes forKey: macos::kAXTrustedCheckOptionPrompt as *mut Object];
                    macos::AXIsProcessTrustedWithOptions(options as *const std::ffi::c_void);
                }
                Permission::Screen => {
                    macos::CGRequestScreenCaptureAccess();
                }
            }
        }
    });
    // Off the caller's thread: this opens another app.
    std::thread::spawn(move || {
        if let Err(e) = std::process::Command::new("/usr/bin/open").arg(which.list()).status() {
            log!("could not open System Settings: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_permission_not_given_is_the_one_asked_for() {
        let stands = |accessibility: bool, screen: bool| vec![Need { permission: Permission::Accessibility, name: "Accessibility", given: accessibility }, Need { permission: Permission::Screen, name: "Screen Recording", given: screen }];
        assert_eq!(first_missing(&stands(false, false)), Some(Permission::Accessibility), "reading and pressing comes before pictures");
        assert_eq!(first_missing(&stands(true, false)), Some(Permission::Screen));
        assert_eq!(first_missing(&stands(false, true)), Some(Permission::Accessibility));
        assert_eq!(first_missing(&stands(true, true)), None, "with both given, macOS is asked for nothing");
    }

    #[test]
    fn a_permission_is_named_to_the_page_in_one_small_word() {
        assert_eq!(serde_json::to_value(Permission::Accessibility).unwrap(), "accessibility");
        assert_eq!(serde_json::to_value(Permission::Screen).unwrap(), "screen");
        assert_eq!(serde_json::from_value::<Permission>(serde_json::json!("screen")).unwrap(), Permission::Screen);
    }

    #[test]
    fn a_refused_file_means_off_and_an_opened_one_means_on() {
        assert_eq!(read(Ok(())), FullDisk::On);
        assert_eq!(read(Err(ErrorKind::PermissionDenied.into())), FullDisk::Off);
    }

    #[test]
    fn a_file_that_is_not_there_tells_nothing() {
        assert_eq!(read(Err(ErrorKind::NotFound.into())), FullDisk::Unknown);
    }

    #[test]
    fn a_missing_file_hands_over_to_the_next_and_the_first_that_tells_decides() {
        let missing = || Err(ErrorKind::NotFound.into());
        let refused = || Err(ErrorKind::PermissionDenied.into());
        // As on macOS 27: the person's own list is gone, and a guarded folder tells.
        assert_eq!(first_told([missing(), Ok(())]), FullDisk::On);
        assert_eq!(first_told([missing(), refused()]), FullDisk::Off);
        // As before it: the person's own list tells, and the folders are not tried.
        assert_eq!(first_told([refused(), Ok(())]), FullDisk::Off);
        assert_eq!(first_told([Ok(()), refused()]), FullDisk::On);
        assert_eq!(first_told([missing(), missing()]), FullDisk::Unknown);
        assert_eq!(first_told(Vec::<std::io::Result<()>>::new()), FullDisk::Unknown);
    }

    #[test]
    fn the_person_s_own_list_is_tried_first_and_a_file_every_program_opens_never() {
        let files = guarded();
        assert!(files.first().is_some_and(|file| file.ends_with("Library/Application Support/com.apple.TCC/TCC.db")), "{files:?}");
        assert!(files.iter().any(|file| file.ends_with("Library/Safari")), "{files:?}");
        // On macOS 27 the system's list opens without Full Disk Access, so it would always say on.
        assert!(!files.contains(&PathBuf::from("/Library/Application Support/com.apple.TCC/TCC.db")));
    }

    #[test]
    fn the_user_is_sent_to_the_switch_only_when_it_is_off_and_they_were_never_asked() {
        assert!(due(FullDisk::Off, false));
        assert!(!due(FullDisk::Off, true), "asked once, it is their decision");
        assert!(!due(FullDisk::On, false));
        assert!(!due(FullDisk::Unknown, false), "what cannot be told is not asked about");
    }
}
