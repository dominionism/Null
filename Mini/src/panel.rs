//! The box's window: a transparent panel that takes typing without making Null
//! the active app, shows over full-screen apps and on every Space, and goes away
//! on Esc, Control+Space or a click elsewhere.

#![allow(deprecated)] // tauri-nspanel re-exports the deprecated cocoa crate

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
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
use crate::{access, harness, settings, signin};

pub const LABEL: &str = "mini";

// Sizing contract with Page/index.html. The window is the box plus the
// transparent margin its shadow is drawn in: 16 px each side, 8 px above, 24 px
// below. The page measures its box and asks for `box height + 32`. Change the
// `.box` margin or any height in the page's CSS and these must follow. With the
// pet beside the box the window is wider on its left, by `LANE`.
/// The 584 px box plus 16 px each side.
const WIDTH: f64 = 616.0;
/// What the pet adds to the window, on its left: with the pet the box starts
/// 62 px in (4 px, the pet's 48, and 10 between them) where it otherwise starts
/// 16 px in.
const LANE: f64 = 46.0;
/// The idle box (44 px) plus 8 px above and 24 px below.
const HEIGHT: f64 = 76.0;
/// The tallest the box gets (2 border + 42 row + 212 transcript + 26 notice = 282) plus 32.
const MAX_HEIGHT: f64 = 314.0;

// PROTOTYPE (`BackgroundWork.md`, item 15): the pet stays while work
// goes on. It is not promised and is in no release: the owner lives with it
// first. Everything marked PROTOTYPE here, in harness.rs, in main.rs and in the
// page goes again, or is built properly, once that is decided.
//
// A third size in the sizing contract with Page/index.html: the pet alone. The
// window keeps its corner and shrinks to the pet, so the pet does not move:
// 4 px, the pet's 48 and 4 px across; 6 px, the pet's 48 and 6 px down. The
// page draws nothing else for it. The box starts 62 px in, and the window cuts
// it off.
const ALONE_WIDTH: f64 = 56.0;
const ALONE_HEIGHT: f64 = 60.0;
/// The window is the pet alone.
static ALONE: AtomicBool = AtomicBool::new(false);
/// The box has been put away and not shown since. Not so before the first time.
static PUT_AWAY: AtomicBool = AtomicBool::new(false);
/// The height the page last asked for: what the box comes back at.
static BOX_HEIGHT: Mutex<f64> = Mutex::new(HEIGHT);
/// Where the window's corner was when it became the pet alone. Somewhere else
/// by the time the box comes back, and the user has moved the pet.
static STOOD: Mutex<Option<(i32, i32)>> = Mutex::new(None);

/// PROTOTYPE. Whether the box is put away, with the pet alone or with nothing.
/// A reply that ends then has not been seen, in the open conversation either.
pub fn put_away() -> bool {
    PUT_AWAY.load(Ordering::SeqCst)
}

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

/// How much wider the window is for the pet: `LANE` while it shows, nothing when
/// it has been put away.
fn lane(app: &AppHandle) -> f64 {
    if settings::get(app).pet.unwrap_or(true) {
        LANE
    } else {
        0.0
    }
}

// The remembered position is the corner the window has without the pet, which is
// what it was before there was one. So the box stands where it was left whether
// the pet shows or not, and a position saved by an older Null still means the
// same place. These two go between that corner and the window's own.

/// Where the window's corner goes for the box to stand at the remembered `corner`.
fn with_lane(corner: (i32, i32), lane: f64, scale: f64) -> (i32, i32) {
    (corner.0 - (lane * scale).round() as i32, corner.1)
}

/// The corner to remember for a window whose own corner is at `window`.
fn without_lane(window: (i32, i32), lane: f64, scale: f64) -> (i32, i32) {
    (window.0 + (lane * scale).round() as i32, window.1)
}

/// How many physical pixels a logical one is on the screen that holds `position`.
fn scale_at(window: &WebviewWindow, position: (i32, i32)) -> f64 {
    window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .find(|monitor| {
            let (x, y, size) = (monitor.position().x, monitor.position().y, monitor.size());
            position.0 >= x && position.0 < x + size.width as i32 && position.1 >= y && position.1 < y + size.height as i32
        })
        .map(|monitor| monitor.scale_factor())
        .or_else(|| window.scale_factor().ok())
        .unwrap_or(1.0)
}

/// Build the window, hidden, and turn it into a non-activating panel.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let lane = lane(app);
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Null")
        .inner_size(WIDTH + lane, HEIGHT)
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

    place(&window, settings::get(app).position, lane);

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
        // PROTOTYPE: the pet alone is not the box. It does not go when it stops
        // being the key window, and a press on it must not leave it one.
        if ALONE.load(Ordering::SeqCst) {
            if let WindowEvent::Focused(true) = event {
                give_back_typing(&handle);
            }
            return;
        }
        if let WindowEvent::Focused(false) = event {
            // While Null is asking for Full Disk Access the box stays up over
            // System Settings, where the user has gone to answer. Likewise during a
            // sign-in, while the user is in their browser.
            if !access::asking() && !signin::under_way(&handle) {
                hide(&handle, "clicked away");
            }
        }
    });

    Ok(())
}

/// Give the window a size and leave its top-left corner where it is.
fn size(window: &WebviewWindow, width: f64, height: f64) {
    let position = window.outer_position();
    let _ = window.set_size(LogicalSize::new(width, height));
    if let Ok(position) = position {
        let _ = window.set_position(position);
    }
}

/// Remember where the user left the box. The pet alone has the same corner.
fn remember(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        if let Ok(position) = window.outer_position() {
            let corner = without_lane((position.x, position.y), lane(app), window.scale_factor().unwrap_or(1.0));
            settings::update(app, |settings| settings.position = Some(corner));
        }
    }
}

/// PROTOTYPE. Whether the pet has something to say with the box away: it is
/// not put away with `/pet`, and a conversation is working, asking or unseen.
fn to_say(app: &AppHandle) -> bool {
    lane(app) > 0.0 && harness::anything_to_say(app)
}

/// PROTOTYPE. The window becomes the pet alone, where the pet stands. On the
/// main thread.
fn stand_alone(app: &AppHandle) {
    let (Ok(panel), Some(window)) = (app.get_webview_panel(LABEL), app.get_webview_window(LABEL)) else { return };
    ALONE.store(true, Ordering::SeqCst);
    PUT_AWAY.store(true, Ordering::SeqCst);
    let _ = app.emit_to(LABEL, "mini:alone", true);
    *STOOD.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) =
        window.outer_position().ok().map(|position| (position.x, position.y));
    size(&window, ALONE_WIDTH, ALONE_HEIGHT);
    out_and_in(&panel);
    log!("the pet stands alone");
}

/// PROTOTYPE. Where a window of `size` goes to be whole on `screen`, as near to
/// `corner` as that allows. All in physical pixels.
fn whole_on(corner: (i32, i32), size: (i32, i32), screen: Screen) -> (i32, i32) {
    let (x, y, width, height) = screen;
    (corner.0.min(x + width as i32 - size.0).max(x), corner.1.min(y + height as i32 - size.1).max(y))
}

/// PROTOTYPE. The pet alone is small, and can be left at an edge of the screen,
/// where the box coming back at the same corner would be off it. When the user
/// has moved the pet, this is the corner at which the box is whole on that
/// screen, at any height it grows to. `None` when the pet stands where the box
/// left it, or the box fits: it comes back at the same corner. Asked while the
/// window is still the pet.
fn brought_onto_screen(window: &WebviewWindow, lane: f64) -> Option<(i32, i32)> {
    let position = window.outer_position().ok()?;
    let here = (position.x, position.y);
    let stood = (*STOOD.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))?;
    if (here.0 - stood.0).abs() <= 2 && (here.1 - stood.1).abs() <= 2 {
        return None;
    }
    let monitor = window.current_monitor().ok()??;
    // Without the menu bar and the Dock.
    let (area, scale) = (monitor.work_area(), monitor.scale_factor());
    let size = (((WIDTH + lane) * scale).round() as i32, (MAX_HEIGHT * scale).round() as i32);
    let corner = whole_on(here, size, (area.position.x, area.position.y, area.size.width, area.size.height));
    (corner != here).then_some(corner)
}

/// PROTOTYPE. Off the screen and straight back: going out gives the typing
/// back to the app in front, and coming in this way does not take it. Without
/// the system's fade, so that the pet does not blink.
fn out_and_in(panel: &tauri_nspanel::raw_nspanel::RawNSPanel) {
    const NONE: isize = 2; // NSWindowAnimationBehaviorNone
    const DEFAULT: isize = 0; // NSWindowAnimationBehaviorDefault
    unsafe {
        let _: () = msg_send![panel, setAnimationBehavior: NONE];
        panel.order_out(None);
        panel.order_front_regardless();
        let _: () = msg_send![panel, setAnimationBehavior: DEFAULT];
    }
}

/// PROTOTYPE. The pet stands alone while the box is away and there is
/// something to say, and goes when nothing is left. Asked at every change in
/// how the conversations stand, and once the page has loaded: work that was
/// cut off is something to say from the start.
pub fn settle(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Ok(panel) = handle.get_webview_panel(LABEL) else { return };
        let (alone, to_say) = (ALONE.load(Ordering::SeqCst), to_say(&handle));
        if alone && !to_say {
            remember(&handle);
            ALONE.store(false, Ordering::SeqCst);
            panel.order_out(None);
            let _ = handle.emit_to(LABEL, "mini:alone", false);
            log!("the pet has gone: nothing is left to say");
        } else if !alone && !panel.is_visible() && to_say {
            stand_alone(&handle);
        }
    });
}

/// PROTOTYPE. A press on the pet alone makes its window the one that takes
/// typing. When the press was a move, and not a press to bring the box back,
/// the typing goes back to the app in front once the button is let go.
fn give_back_typing(app: &AppHandle) {
    let handle = app.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(60));
            let down: usize = unsafe { msg_send![class!(NSEvent), pressedMouseButtons] };
            if down == 0 {
                break;
            }
        }
        // A moment for a press to have brought the box back.
        std::thread::sleep(Duration::from_millis(300));
        let _ = handle.clone().run_on_main_thread(move || {
            if !ALONE.load(Ordering::SeqCst) {
                return;
            }
            if let Ok(panel) = handle.get_webview_panel(LABEL) {
                out_and_in(&panel);
                log!("the pet was pressed and the box not opened; the typing is back with the app in front");
            }
        });
    });
}

/// Put the box where it was left, or centred a little above the middle of the screen.
/// The pet's `lane` is to the left of that.
fn place(window: &WebviewWindow, saved: Option<(i32, i32)>, lane: f64) {
    let screens: Vec<Screen> = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|monitor| (monitor.position().x, monitor.position().y, monitor.size().width, monitor.size().height))
        .collect();
    if let Some(position) = saved.filter(|position| reachable(*position, &screens)) {
        let corner = with_lane(position, lane, scale_at(window, position));
        let _ = window.set_position(PhysicalPosition::new(corner.0, corner.1));
        return;
    }
    if let (Ok(Some(monitor)), Ok(outer)) = (window.current_monitor(), window.outer_size()) {
        // The box is what is centred, not the box and the pet together.
        let lane = (lane * monitor.scale_factor()).round() as i32;
        let x = monitor.position().x + (monitor.size().width as i32 - (outer.width as i32 - lane)) / 2 - lane;
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
        // PROTOTYPE: the pet alone is on the screen as well, and the box is not up then.
        let alone = ALONE.swap(false, Ordering::SeqCst);
        if panel.is_visible() && !alone {
            return;
        }
        PUT_AWAY.store(false, Ordering::SeqCst);
        if alone {
            // The window grows back from the pet to the box, at the same corner.
            if let Some(window) = handle.get_webview_window(LABEL) {
                let height = *BOX_HEIGHT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                let onto = brought_onto_screen(&window, lane(&handle));
                size(&window, WIDTH + lane(&handle), height);
                if let Some(corner) = onto {
                    let _ = window.set_position(PhysicalPosition::new(corner.0, corner.1));
                    log!("the pet had been moved to where the box does not fit; the box is brought onto the screen");
                }
            }
        }
        let before = frontmost_app();
        panel.show();
        let _ = handle.emit_to(LABEL, "mini:shown", serde_json::json!({ "alone": alone }));
        log!("shown{}; frontmost app: {before}", if alone { ", back from the pet alone" } else { "" });
        // PROTOTYPE: what ended in the open conversation while the box was away has been seen now.
        harness::seen(&handle);
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
        // PROTOTYPE: the pet alone is not the box. It goes by `settle`.
        if !panel.is_visible() || ALONE.load(Ordering::SeqCst) {
            return;
        }
        PUT_AWAY.store(true, Ordering::SeqCst);
        remember(&handle);
        // PROTOTYPE: with something to say, the pet stays where it stands.
        if to_say(&handle) {
            log!("the box is put away ({why}); the pet stays");
            stand_alone(&handle);
        } else {
            panel.order_out(None);
            log!("hidden ({why})");
        }
        // Putting the box away ends the asking, and takes its words with it.
        if access::stop_asking() {
            let _ = handle.emit_to(LABEL, "mini:notice", ());
        }
    });
}

pub fn toggle(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        // PROTOTYPE: from the pet alone, Control+Space brings the box back.
        let visible = handle.get_webview_panel(LABEL).map(|panel| panel.is_visible()).unwrap_or(false) && !ALONE.load(Ordering::SeqCst);
        if visible {
            hide(&handle, "Control+Space");
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

/// PROTOTYPE. A press on the pet alone brings the box back.
#[tauri::command]
pub fn show_box(app: AppHandle) {
    show(&app);
}

/// The page grows with the reply. Keep the top-left corner where it is.
#[tauri::command]
pub fn resize(app: AppHandle, window: WebviewWindow, height: f64) {
    let height = height.clamp(HEIGHT, MAX_HEIGHT);
    // PROTOTYPE: kept for when the box comes back. The pet alone keeps its size.
    *BOX_HEIGHT.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = height;
    if ALONE.load(Ordering::SeqCst) {
        return;
    }
    size(&window, WIDTH + lane(&app), height);
}

/// Whether the pet shows. The page asks when it loads.
#[tauri::command]
pub fn pet(app: AppHandle) -> bool {
    settings::get(&app).pet.unwrap_or(true)
}

/// `/pet` in the box: show the pet or put it away. The window grows or shrinks on
/// its left, so the box stays where it is.
#[tauri::command]
pub fn set_pet(app: AppHandle, window: WebviewWindow, on: bool) {
    let before = lane(&app);
    settings::update(&app, |settings| settings.pet = Some(on));
    let after = lane(&app);
    log!("the pet is {}", if on { "shown" } else { "put away" });
    if after == before {
        return;
    }
    let scale = window.scale_factor().unwrap_or(1.0);
    if let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) {
        let corner = with_lane(without_lane((position.x, position.y), before, scale), after, scale);
        let height = (size.height as f64 / scale).clamp(HEIGHT, MAX_HEIGHT);
        let _ = window.set_size(LogicalSize::new(WIDTH + after, height));
        let _ = window.set_position(PhysicalPosition::new(corner.0, corner.1));
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
    fn the_box_stands_where_it_was_left_with_the_pet_or_without() {
        // A position an older Null saved, on a screen with two pixels to the point.
        let saved = (958, 322);
        let window = with_lane(saved, LANE, 2.0);
        assert_eq!(window, (866, 322), "the window starts further left, by the pet's lane");
        assert_eq!(without_lane(window, LANE, 2.0), saved, "and the same place is remembered");
        assert_eq!(with_lane(saved, 0.0, 2.0), saved, "with the pet put away the window is as it always was");
    }

    // PROTOTYPE (item 15): the box coming back from a pet that was moved.
    #[test]
    fn a_box_that_fits_comes_back_where_the_pet_is() {
        assert_eq!(whole_on((700, 400), (1324, 628), LAPTOP), (700, 400));
    }

    #[test]
    fn a_box_back_from_a_pet_at_the_edge_is_whole_on_the_screen() {
        let size = (1324, 628); // 662 by 314, two pixels to the point
        assert_eq!(whole_on((2448, 1480), size, LAPTOP), (1236, 972), "from the bottom right corner");
        assert_eq!(whole_on((-20, -10), size, LAPTOP), (0, 0), "from past the top left corner");
        let left: Screen = (-1920, 0, 1920, 1080);
        assert_eq!(whole_on((-100, 1000), size, left), (-1324, 452), "on a second screen to the left");
    }

    #[test]
    fn a_position_above_or_below_every_screen_is_not_kept() {
        assert!(!reachable((700, -40), &[LAPTOP]));
        assert!(!reachable((700, 1590), &[LAPTOP]));
    }
}
