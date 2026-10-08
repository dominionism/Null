//! The box's window: a transparent panel that takes typing without making Null
//! the active app, shows over full-screen apps and on every Space, and goes away
//! on Esc, fn+Space or a click elsewhere.

#![allow(deprecated)] // tauri-nspanel re-exports the deprecated cocoa crate

use std::ffi::CStr;
use std::os::raw::c_char;
use std::time::Duration;

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
use tauri_nspanel::{ManagerExt, WebviewWindowExt};

use crate::log::log;
use crate::settings;

pub const LABEL: &str = "mini";

// Sizing contract with Page/index.html. The window is the box plus the
// transparent margin its shadow is drawn in: 16 px each side, 8 px above, 24 px
// below. The page measures its box and asks for `box height + 32`. Change the
// `.box` margin or any height in the page's CSS and these three must follow.
/// The 584 px box plus 16 px each side.
const WIDTH: f64 = 616.0;
/// The idle box (44 px) plus 8 px above and 24 px below.
const HEIGHT: f64 = 76.0;
/// The tallest the box gets (2 border + 42 row + 344 transcript + 26 notice = 414) plus 32.
const MAX_HEIGHT: f64 = 446.0;

/// A screen's top-left corner and size, in physical pixels.
type Screen = (i32, i32, u32, u32);

/// Whether a window whose top-left corner is at `position` would still be reachable:
/// enough of its top edge is on some screen to see it and grab it.
fn reachable(position: (i32, i32), screens: &[Screen]) -> bool {
    const GRAB: i32 = 80;
    screens.iter().any(|&(x, y, width, height)| {
        position.0 + GRAB > x
            && position.0 + GRAB < x + width as i32
            && position.1 >= y
            && position.1 + GRAB < y + height as i32
    })
}

/// Build the window, hidden, and turn it into a non-activating panel.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Null")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        // A press acts at once even when the panel is not the key window. Without
        // this macOS spends that first press on making it key, so a grab to move
        // the box (or a click on a button in it) would do nothing the first time.
        .accept_first_mouse(true)
        .visible(false)
        .build()?;

    place(&window, settings::get(app).position);

    match window.to_panel() {
        Ok(panel) => {
            // Values from tauri-nspanel's own full-screen example.
            const NS_FLOAT_WINDOW_LEVEL: i32 = 4;
            const NS_NONACTIVATING_PANEL: i32 = 1 << 7;
            panel.set_level(NS_FLOAT_WINDOW_LEVEL);
            panel.set_style_mask(NS_NONACTIVATING_PANEL);
            panel.set_collection_behaviour(
                NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces,
            );
            log!("panel ready");
        }
        // Still usable as an ordinary window, but it will take activation.
        Err(e) => log!("could not turn the window into a panel: {e}"),
    }

    // Click-away: the panel stops being the key window, so tuck it away.
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            hide(&handle, "clicked away");
        }
    });

    Ok(())
}

/// Put the window where it was left, or centred a little above the middle of the screen.
fn place(window: &WebviewWindow, saved: Option<(i32, i32)>) {
    let screens: Vec<Screen> = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|monitor| (monitor.position().x, monitor.position().y, monitor.size().width, monitor.size().height))
        .collect();
    if let Some(position) = saved.filter(|position| reachable(*position, &screens)) {
        let _ = window.set_position(PhysicalPosition::new(position.0, position.1));
        return;
    }
    if let (Ok(Some(monitor)), Ok(outer)) = (window.current_monitor(), window.outer_size()) {
        let x = monitor.position().x + (monitor.size().width as i32 - outer.width as i32) / 2;
        let y = monitor.position().y + (monitor.size().height as f64 * 0.28) as i32;
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

pub fn show(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Ok(panel) = handle.get_webview_panel(LABEL) else {
            log!("panel not found");
            return;
        };
        if panel.is_visible() {
            return;
        }
        let before = frontmost_app();
        panel.show();
        let _ = handle.emit_to(LABEL, "mini:shown", ());
        log!("shown; frontmost app: {before}");
        // The box must never make Null the active app. Activation, if it
        // happens, lands a moment later, so look again then.
        let later = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            let _ = later.run_on_main_thread(move || {
                let after = frontmost_app();
                if after != before {
                    log!("the frontmost app changed from {before} to {after} after showing the box");
                }
            });
        });
    });
}

pub fn hide(app: &AppHandle, why: &'static str) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Ok(panel) = handle.get_webview_panel(LABEL) else { return };
        if !panel.is_visible() {
            return;
        }
        // Remember where the user left the box.
        if let Some(position) = handle.get_webview_window(LABEL).and_then(|window| window.outer_position().ok()) {
            settings::update(&handle, |settings| settings.position = Some((position.x, position.y)));
        }
        panel.order_out(None);
        log!("hidden ({why})");
    });
}

pub fn toggle(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let visible = handle.get_webview_panel(LABEL).map(|panel| panel.is_visible()).unwrap_or(false);
        if visible {
            hide(&handle, "fn+Space");
        } else {
            show(&handle);
        }
    });
}

/// The page asks to be hidden (Esc).
#[tauri::command]
pub fn hide_box(app: AppHandle) {
    hide(&app, "esc");
}

/// The page grows with the reply. Keep the top-left corner where it is.
#[tauri::command]
pub fn resize(window: WebviewWindow, height: f64) {
    let position = window.outer_position();
    let _ = window.set_size(LogicalSize::new(WIDTH, height.clamp(HEIGHT, MAX_HEIGHT)));
    if let Ok(position) = position {
        let _ = window.set_position(position);
    }
}

fn frontmost_app() -> String {
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: *mut Object = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return "?".into();
        }
        let name: *mut Object = msg_send![app, localizedName];
        if name.is_null() {
            return "?".into();
        }
        let utf8: *const c_char = msg_send![name, UTF8String];
        if utf8.is_null() {
            return "?".into();
        }
        CStr::from_ptr(utf8).to_string_lossy().into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAPTOP: Screen = (0, 0, 2560, 1600);

    #[test]
    fn a_position_on_the_screen_is_kept() {
        assert!(reachable((700, 400), &[LAPTOP]));
    }

    #[test]
    fn a_position_on_a_second_screen_to_the_left_is_kept() {
        assert!(reachable((-1500, 300), &[LAPTOP, (-1920, 0, 1920, 1080)]));
    }

    #[test]
    fn a_position_left_on_a_screen_that_is_gone_is_not_kept() {
        assert!(!reachable((-1500, 300), &[LAPTOP]));
        assert!(!reachable((3000, 300), &[LAPTOP]));
    }

    #[test]
    fn a_position_above_or_below_every_screen_is_not_kept() {
        assert!(!reachable((700, -40), &[LAPTOP]));
        assert!(!reachable((700, 1590), &[LAPTOP]));
    }
}
