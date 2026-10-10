//! Full Disk Access: whether macOS lets Null, and so the agent it starts, read
//! any file without asking.
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

/// Try to open a file that only Full Disk Access unlocks. The attempt is also
/// what puts Null in the Full Disk Access list, switched off.
fn full_disk() -> FullDisk {
    let Some(home) = std::env::var_os("HOME") else { return FullDisk::Unknown };
    let guarded = PathBuf::from(home).join("Library/Application Support/com.apple.TCC/TCC.db");
    read(std::fs::File::open(guarded).map(|_| ()))
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_user_is_sent_to_the_switch_only_when_it_is_off_and_they_were_never_asked() {
        assert!(due(FullDisk::Off, false));
        assert!(!due(FullDisk::Off, true), "asked once, it is their decision");
        assert!(!due(FullDisk::On, false));
        assert!(!due(FullDisk::Unknown, false), "what cannot be told is not asked about");
    }
}
