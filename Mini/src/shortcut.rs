//! Control+Space opens and closes the box.
//!
//! It is an ordinary system-wide shortcut, registered with macOS. The system
//! keeps the keystroke for Null, so the Space never reaches the app in front,
//! and registering it needs no permission from the user.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::log::log;
use crate::panel;

/// What the box says when the shortcut could not be registered.
pub const UNAVAILABLE_NOTICE: &str = "Control+Space is not available to Null; another app may be using it";

static UNAVAILABLE: AtomicBool = AtomicBool::new(false);

/// True when Control+Space could not be registered, so the box has to say so.
pub fn unavailable() -> bool {
    UNAVAILABLE.load(Ordering::SeqCst)
}

/// Start listening for Control+Space.
pub fn start(app: &AppHandle) {
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::Space);
    let registered = app.global_shortcut().on_shortcut(shortcut, |app, _shortcut, event| {
        // Holding the keys down does not repeat: one press, one toggle.
        if event.state() == ShortcutState::Pressed {
            panel::toggle(app);
        }
    });
    match registered {
        Ok(()) => log!("Control+Space opens the box"),
        Err(e) => {
            log!("could not register Control+Space: {e}");
            UNAVAILABLE.store(true, Ordering::SeqCst);
            // With no shortcut, the box is the only place to say what is wrong.
            // The page shows the notice once it has loaded (see `page_ready`).
            panel::show(app);
        }
    }
}
