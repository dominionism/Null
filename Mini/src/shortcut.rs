//! fn+Space opens and closes the box.
//!
//! An ordinary global shortcut cannot do this: it would fire, but the Space
//! would still be typed into whatever app is in front. So this is an *active*
//! event tap, which may drop a keystroke: it swallows Space while fn is held.
//! macOS lets an app run such a tap only once the user has granted it
//! Accessibility permission.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::log::log;
use crate::panel;

static APP: OnceLock<AppHandle> = OnceLock::new();
static TAP: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SWALLOW_SPACE_UP: AtomicBool = AtomicBool::new(false);
static WAITING_FOR_PERMISSION: AtomicBool = AtomicBool::new(false);

/// True while the app has asked for Accessibility permission and not yet been given it.
pub fn waiting_for_permission() -> bool {
    WAITING_FOR_PERMISSION.load(Ordering::SeqCst)
}

/// Start listening for fn+Space, asking for Accessibility permission first if it is missing.
pub fn start(app: &AppHandle) {
    let _ = APP.set(app.clone());
    if let Err(e) = std::thread::Builder::new().name("mini-key-tap".into()).spawn(run) {
        log!("could not start the key-tap thread: {e}");
    }
}

fn run() {
    let Some(app) = APP.get() else { return };

    if !trusted() {
        log!("Accessibility permission is missing; asking macOS to show its prompt");
        WAITING_FOR_PERMISSION.store(true, Ordering::SeqCst);
        ask_for_permission();
        // With no shortcut yet, the box is the only place to say what is needed.
        panel::show(app);
        let _ = app.emit_to(panel::LABEL, "mini:permission", false);
        while !trusted() {
            std::thread::sleep(Duration::from_secs(2));
        }
        WAITING_FOR_PERMISSION.store(false, Ordering::SeqCst);
        log!("Accessibility permission granted");
        let _ = app.emit_to(panel::LABEL, "mini:permission", true);
    }

    unsafe {
        let mask: u64 = (1 << KEY_DOWN) | (1 << KEY_UP);
        let tap = CGEventTapCreate(SESSION_EVENT_TAP, HEAD_INSERT, TAP_OPTION_DEFAULT, mask, tap_callback, std::ptr::null_mut());
        if tap.is_null() {
            log!("could not create the key tap even with Accessibility permission; quit and reopen Null Mini");
            return;
        }
        TAP.store(tap, Ordering::SeqCst);
        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        log!("key tap running: fn+Space opens the box");
        CFRunLoopRun();
    }
}

fn trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

/// Have macOS show its own "would like to control this computer" prompt, which
/// also adds the app to the Accessibility list so the user only has to switch it on.
fn ask_for_permission() {
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        AXIsProcessTrustedWithOptions(options);
        if !options.is_null() {
            CFRelease(options);
        }
    }
}

extern "C" fn tap_callback(_proxy: *mut c_void, event_type: u32, event: *mut c_void, _user_info: *mut c_void) -> *mut c_void {
    match event_type {
        // The system switches a tap off when it is slow to answer, or on the user's input. Switch it back on.
        TAP_DISABLED_BY_TIMEOUT | TAP_DISABLED_BY_USER_INPUT => {
            let tap = TAP.load(Ordering::SeqCst);
            if !tap.is_null() {
                unsafe { CGEventTapEnable(tap, true) };
            }
            log!("the system disabled the key tap (type {event_type:#x}); re-enabled");
            event
        }
        KEY_DOWN | KEY_UP => {
            let keycode = unsafe { CGEventGetIntegerValueField(event, FIELD_KEYCODE) };
            if keycode != KEYCODE_SPACE {
                return event;
            }
            let flags = unsafe { CGEventGetFlags(event) };
            let fn_only = flags & FLAG_FN != 0 && flags & (FLAG_SHIFT | FLAG_CONTROL | FLAG_OPTION | FLAG_COMMAND) == 0;

            if event_type == KEY_DOWN && fn_only {
                let repeat = unsafe { CGEventGetIntegerValueField(event, FIELD_AUTOREPEAT) } != 0;
                SWALLOW_SPACE_UP.store(true, Ordering::SeqCst);
                if !repeat {
                    if let Some(app) = APP.get() {
                        panel::toggle(app);
                    }
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

type TapCallback = extern "C" fn(proxy: *mut c_void, event_type: u32, event: *mut c_void, user_info: *mut c_void) -> *mut c_void;

/// A CoreFoundation value this file only ever passes by address.
#[repr(C)]
struct Opaque {
    _private: [u8; 0],
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(tap: u32, place: u32, options: u32, events_of_interest: u64, callback: TapCallback, user_info: *mut c_void) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventGetFlags(event: *mut c_void) -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
#[allow(non_upper_case_globals)]
extern "C" {
    fn CFMachPortCreateRunLoopSource(allocator: *const c_void, port: *mut c_void, order: isize) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRun();
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: isize,
        key_callbacks: *const Opaque,
        value_callbacks: *const Opaque,
    ) -> *const c_void;
    fn CFRelease(value: *const c_void);
    static kCFRunLoopCommonModes: *const c_void;
    static kCFBooleanTrue: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: Opaque;
    static kCFTypeDictionaryValueCallBacks: Opaque;
}

#[link(name = "ApplicationServices", kind = "framework")]
#[allow(non_upper_case_globals)]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    static kAXTrustedCheckOptionPrompt: *const c_void;
}

const SESSION_EVENT_TAP: u32 = 1;
const HEAD_INSERT: u32 = 0;
const TAP_OPTION_DEFAULT: u32 = 0; // active: may drop or change events
const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
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
