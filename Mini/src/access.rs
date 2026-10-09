//! Full Disk Access: whether macOS lets Null, and so the agent it starts, read
//! any file without asking.
//!
//! macOS holds Null responsible for whatever the harness touches. Without Full
//! Disk Access it asks the user about each protected folder in turn, and an agent
//! that searches the home folder sets off a run of those questions. macOS has no
//! prompt for Full Disk Access itself. An app can only find out whether it has
//! it, and show the user where the switch is.

use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;

use crate::log::log;
use crate::{panel, settings};

/// What the box says the one time Null asks.
pub const NOTICE: &str = "To stop macOS asking about each folder, turn on Null under Full Disk Access";

/// The Full Disk Access list in System Settings.
const SETTINGS_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

static ASKING: AtomicBool = AtomicBool::new(false);

/// True from the moment this run sends the user to the switch until they put the box away.
pub fn asking() -> bool {
    ASKING.load(Ordering::SeqCst)
}

/// The user has put the box away. Returns whether Null was asking until now.
pub fn stop_asking() -> bool {
    ASKING.swap(false, Ordering::SeqCst)
}

#[derive(Debug, PartialEq)]
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

/// At start: if Full Disk Access is off, send the user to the switch, once.
/// After that it is their decision, and macOS goes back to asking folder by folder.
pub fn check(app: &AppHandle) {
    match full_disk() {
        FullDisk::On => log!("Full Disk Access is on"),
        FullDisk::Unknown => log!("could not tell whether Full Disk Access is on"),
        FullDisk::Off => {
            if settings::get(app).asked_full_disk {
                log!("Full Disk Access is off; the user has been asked before");
                return;
            }
            log!("Full Disk Access is off; opening its list in System Settings");
            settings::update(app, |settings| settings.asked_full_disk = true);
            ASKING.store(true, Ordering::SeqCst);
            if let Err(e) = std::process::Command::new("/usr/bin/open").arg(SETTINGS_URL).status() {
                log!("could not open System Settings: {e}");
            }
            // The box says why, and stays up over System Settings until the user puts
            // it away. The page shows the notice once it has loaded (see `page_ready`).
            panel::show(app);
        }
    }
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
}
