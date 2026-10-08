//! PROTOTYPE — throwaway. Delete when Context/Plans/NullMini.md item 7 lands.
//!
//! Question (NullMini.md, item 2): can a Null window take typing without
//! taking the app, opened by fn+Space, with the Space keystroke swallowed so
//! it never reaches the app underneath?
//!
//! Enabled only when the app is started with `NULL_MINI_SPIKE=1`. Everything
//! it does is logged to stderr with a `[mini-spike]` prefix, including which
//! app is frontmost before and after the window appears.
//!
//! Two pieces, both macOS-only:
//!   1. A window converted to a non-activating NSPanel (tauri-nspanel, v2).
//!   2. An *active* event tap that drops Space while fn is held. The chord
//!      engine (keytap) taps listen-only and cannot drop a keystroke.
#![allow(deprecated)] // tauri-nspanel v2 re-exports the deprecated cocoa crate

use std::ffi::{c_void, CStr};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use tauri::{
    AppHandle, Emitter, Listener, LogicalSize, PhysicalPosition, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_nspanel::cocoa::appkit::NSWindowCollectionBehavior;
use tauri_nspanel::{ManagerExt, WebviewWindowExt};

const LABEL: &str = "mini-spike";
// Sizing contract with prototype-mini-spike.html. The window is the box plus the
// transparent margin its shadow is drawn in: 16 px each side, 8 px above, 24 px
// below. The page measures its box and asks for `box height + 32`. Change the
// `.box` margin or any height in the page's CSS and these three must follow.
/// The 584 px box plus 16 px each side.
const WIDTH: f64 = 616.0;
/// The idle box (44 px) plus 8 px above and 24 px below.
const HEIGHT: f64 = 76.0;
/// The page asks for more room as a reply comes in, up to this: the tallest the
/// box gets (2 border + 42 row + 344 transcript + 26 notice = 414) plus 32.
const MAX_HEIGHT: f64 = 446.0;

/// Also written to a file so the results can be read without copying terminal output.
const LOG_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/logs/mini-spike.log");

/// The secret the backend's /mini endpoints require. This reads the dev data
/// directory; the real window must get it from wherever the server was started.
const TOKEN_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/mini-token");
const API_BASE: &str = "http://127.0.0.1:17493";

fn append_log(line: &str) {
    use std::io::Write;
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let _ = writeln!(file, "{seconds:.3} {line}");
    }
}

macro_rules! spike_log {
    ($($arg:tt)*) => {{
        let line = format!($($arg)*);
        eprintln!("[mini-spike] {line}");
        append_log(&line);
    }};
}

static APP: OnceLock<AppHandle> = OnceLock::new();
static TAP: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SWALLOW_SPACE_UP: AtomicBool = AtomicBool::new(false);
static FN_DOWN: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
    std::env::var("NULL_MINI_SPIKE").map(|v| v == "1").unwrap_or(false)
}

pub fn start(app: &AppHandle) {
    spike_log!("enabled (NULL_MINI_SPIKE=1)");

    if let Err(e) = app.plugin(tauri_nspanel::init()) {
        spike_log!("could not register tauri-nspanel: {e}");
        return;
    }

    let window = match WebviewWindowBuilder::new(
        app,
        LABEL,
        WebviewUrl::App("prototype-mini-spike.html".into()),
    )
    .title("Null Mini spike")
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
    .build()
    {
        Ok(window) => window,
        Err(e) => {
            spike_log!("could not build the window: {e}");
            return;
        }
    };

    if let Ok(Some(monitor)) = window.current_monitor() {
        let size = monitor.size();
        let position = monitor.position();
        if let Ok(outer) = window.outer_size() {
            let x = position.x + (size.width as i32 - outer.width as i32) / 2;
            let y = position.y + (size.height as f64 * 0.28) as i32;
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
    }

    let panel = match window.to_panel() {
        Ok(panel) => panel,
        Err(e) => {
            spike_log!("to_panel failed: {e}");
            return;
        }
    };
    // Values from tauri-nspanel's own full-screen example.
    const NS_FLOAT_WINDOW_LEVEL: i32 = 4;
    const NS_NONACTIVATING_PANEL: i32 = 1 << 7;
    panel.set_level(NS_FLOAT_WINDOW_LEVEL);
    panel.set_style_mask(NS_NONACTIVATING_PANEL);
    panel.set_collection_behaviour(
        NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
            | NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces,
    );
    spike_log!("window converted to a non-activating panel");

    // Click-away: the panel stops being the key window, so tuck it away.
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(focused) = event {
            spike_log!("panel is key window = {focused}");
            if !*focused {
                hide(&handle, "lost key");
            }
        }
    });

    // Esc in the text box.
    let handle = app.clone();
    app.listen("mini-spike:hide", move |_event| hide(&handle, "esc"));

    // Proof that keystrokes reach the text box: the page reports its length.
    app.listen("mini-spike:typed", move |event| {
        spike_log!("text box received typing: {}", event.payload());
    });

    // The page grows with the reply. Keep the top-left corner where it is.
    let sized = window.clone();
    app.listen("mini-spike:resize", move |event| {
        let Some(height) = serde_json::from_str::<serde_json::Value>(event.payload())
            .ok()
            .and_then(|payload| payload.get("height").and_then(|h| h.as_f64()))
        else {
            return;
        };
        let position = sized.outer_position();
        let _ = sized.set_size(LogicalSize::new(WIDTH, height.clamp(HEIGHT, MAX_HEIGHT)));
        if let Ok(position) = position {
            let _ = sized.set_position(position);
        }
    });

    let _ = APP.set(app.clone());
    if let Err(e) = std::thread::Builder::new()
        .name("mini-spike-key-tap".into())
        .spawn(run_key_tap)
    {
        spike_log!("could not start the key-tap thread: {e}");
    }
}

fn hide(app: &AppHandle, why: &'static str) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Ok(panel) = handle.get_webview_panel(LABEL) {
            if panel.is_visible() {
                panel.order_out(None);
                spike_log!("hidden ({why}); frontmost app: {}", frontmost_app());
            }
        }
    });
}

fn toggle() {
    let Some(app) = APP.get() else { return };
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Ok(panel) = handle.get_webview_panel(LABEL) else {
            spike_log!("panel not found");
            return;
        };
        if panel.is_visible() {
            panel.order_out(None);
            spike_log!("fn+Space → hide; frontmost app: {}", frontmost_app());
            return;
        }
        let before = frontmost_app();
        panel.show();
        // Hand the page what it needs to reach the backend. Read each time: the
        // server creates the token on its first start.
        let token = std::fs::read_to_string(TOKEN_PATH)
            .map(|token| token.trim().to_string())
            .unwrap_or_default();
        let _ = handle.emit_to(
            LABEL,
            "mini-spike:shown",
            serde_json::json!({ "token": token, "api": API_BASE }),
        );
        spike_log!("fn+Space → show; frontmost app before: {before}");
        // Activation, if it happens, lands a moment later — report it then.
        let later = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(400));
            let _ = later.run_on_main_thread(move || {
                let after = frontmost_app();
                let verdict = if after == before { "unchanged — good" } else { "CHANGED — the panel activated Null" };
                spike_log!("frontmost app 0.4 s after show: {after} ({verdict})");
            });
        });
    });
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

// ---------------------------------------------------------------------------
// Active event tap: drops Space while fn is held.
// ---------------------------------------------------------------------------

type TapCallback = extern "C" fn(
    proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    user_info: *mut c_void,
) -> *mut c_void;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: TapCallback,
        user_info: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventGetFlags(event: *mut c_void) -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
#[allow(non_upper_case_globals, clashing_extern_declarations)]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: *mut c_void,
        order: isize,
    ) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRun();
    static kCFRunLoopCommonModes: *const c_void;
}

const SESSION_EVENT_TAP: u32 = 1;
const HEAD_INSERT: u32 = 0;
const TAP_OPTION_DEFAULT: u32 = 0; // active: may drop or change events
const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const FLAGS_CHANGED: u32 = 12;
const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
const FIELD_AUTOREPEAT: u32 = 8;
const FIELD_KEYCODE: u32 = 9;
const KEYCODE_SPACE: i64 = 49;
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;
const FLAG_FN: u64 = 0x0080_0000;

fn run_key_tap() {
    unsafe {
        let mask: u64 = (1 << KEY_DOWN) | (1 << KEY_UP) | (1 << FLAGS_CHANGED);
        let tap = CGEventTapCreate(
            SESSION_EVENT_TAP,
            HEAD_INSERT,
            TAP_OPTION_DEFAULT,
            mask,
            tap_callback,
            std::ptr::null_mut(),
        );
        if tap.is_null() {
            spike_log!(
                "could not create the key tap. Grant Accessibility to the app that \
                 launched this (System Settings → Privacy & Security → Accessibility), then restart."
            );
            return;
        }
        TAP.store(tap, Ordering::SeqCst);
        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        spike_log!("key tap running — press fn+Space in any app");
        CFRunLoopRun();
    }
}

extern "C" fn tap_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    _user_info: *mut c_void,
) -> *mut c_void {
    match event_type {
        TAP_DISABLED_BY_TIMEOUT | TAP_DISABLED_BY_USER_INPUT => {
            let tap = TAP.load(Ordering::SeqCst);
            if !tap.is_null() {
                unsafe { CGEventTapEnable(tap, true) };
            }
            spike_log!("the system disabled the key tap (type {event_type:#x}); re-enabled");
            event
        }
        FLAGS_CHANGED => {
            let fn_now = unsafe { CGEventGetFlags(event) } & FLAG_FN != 0;
            if FN_DOWN.swap(fn_now, Ordering::SeqCst) != fn_now {
                spike_log!("fn {}", if fn_now { "down" } else { "up" });
            }
            event
        }
        KEY_DOWN | KEY_UP => {
            let keycode = unsafe { CGEventGetIntegerValueField(event, FIELD_KEYCODE) };
            if keycode != KEYCODE_SPACE {
                return event;
            }
            let flags = unsafe { CGEventGetFlags(event) };
            let fn_only = flags & FLAG_FN != 0
                && flags & (FLAG_SHIFT | FLAG_CONTROL | FLAG_OPTION | FLAG_COMMAND) == 0;

            if event_type == KEY_DOWN && fn_only {
                let repeat = unsafe { CGEventGetIntegerValueField(event, FIELD_AUTOREPEAT) } != 0;
                SWALLOW_SPACE_UP.store(true, Ordering::SeqCst);
                if !repeat {
                    spike_log!("fn+Space pressed — Space swallowed");
                    toggle();
                }
                return std::ptr::null_mut();
            }
            // Drop the matching key-up too, even if fn was released first.
            if event_type == KEY_UP && SWALLOW_SPACE_UP.swap(false, Ordering::SeqCst) {
                return std::ptr::null_mut();
            }
            event
        }
        _ => event,
    }
}
