//! Null: a small floating box that opens on Control+Space and drives the user's
//! own agent harness. It is one app with no server; the page in `Page/` talks to
//! this process through commands and events.
//!
//! Five switches for development, all read from the environment:
//! - `NULL_MINI_EXIT_WHEN_READY`: quit as soon as the page has loaded, which makes
//!   "start, load the page, stop" a check a script can run.
//! - `NULL_MINI_NO_SHORTCUT`: do not listen for Control+Space, and show the box at
//!   start instead. For running beside another copy that owns the shortcut.
//! - `NULL_MINI_SMOKE`: send its value to the harness as one message, print what
//!   comes back, and quit. No window and no shortcut; it checks the harness alone.
//! - `NULL_MINI_SELFTEST`: type its value into the real page, wait for the reply,
//!   log what the page shows, and quit. Use with `NULL_MINI_NO_SHORTCUT`.
//! - `NULL_MINI_PROFILE`: run the harness under that isolated profile of its own,
//!   so sign-in and first-run behaviour can be tried without the real sign-ins.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "macos"))]
compile_error!("Null is macOS-only for now");

mod access;
mod harness;
mod log;
mod panel;
mod settings;
mod shortcut;
mod signin;
mod translate;

use log::log;
use tauri::Emitter;

/// The page calls this once its script has run.
#[tauri::command]
fn page_ready(app: tauri::AppHandle) {
    log!("page ready");
    if std::env::var_os("NULL_MINI_EXIT_WHEN_READY").is_some() {
        app.exit(0);
    }
    if let Some(text) = std::env::var_os("NULL_MINI_SELFTEST") {
        harness::selftest(&app, text.to_string_lossy().into_owned());
    }
    // The page loads after the app has tried to register the shortcut.
    if shortcut::unavailable() {
        let _ = app.emit_to(panel::LABEL, "mini:notice", shortcut::UNAVAILABLE_NOTICE);
    }
    // Likewise after it has sent the user to switch on Full Disk Access.
    if access::asking() {
        let _ = app.emit_to(panel::LABEL, "mini:notice", access::NOTICE);
    }
}

/// `/quit` or Ctrl+C in the box. The app has no Dock icon or menu to quit from.
///
/// Quitting closes the conversation as well: the next start opens a new one, as
/// a terminal program does. A restart the user did not ask for, at login or after
/// an update, still comes back to the conversation that was open.
#[tauri::command]
fn quit(app: tauri::AppHandle) {
    log!("quit from the box; the conversation is closed");
    settings::update(&app, |settings| settings.session = None);
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            page_ready,
            quit,
            panel::hide_box,
            panel::resize,
            harness::send,
            harness::interrupt,
            harness::respond,
            harness::models,
            harness::set_model,
            harness::new_conversation,
            harness::events_since,
            harness::report,
            signin::signin_providers,
            signin::signin_start,
            signin::signin_answer,
            signin::signin_cancel,
            signin::open_url,
        ])
        .setup(|app| {
            log!("started, version {}", app.package_info().version);
            // A background app: no Dock icon, no menu bar of its own.
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle();
            settings::init(handle);
            harness::init(handle);
            signin::init(handle);
            if let Some(text) = std::env::var_os("NULL_MINI_SMOKE") {
                harness::smoke(handle, text.to_string_lossy().into_owned());
                return Ok(());
            }
            panel::create(handle)?;
            if std::env::var_os("NULL_MINI_NO_SHORTCUT").is_some() {
                log!("Control+Space is off for this run (NULL_MINI_NO_SHORTCUT)");
                panel::show(handle);
            } else {
                shortcut::start(handle);
                // Not on a scripted start-up check, which must leave the user's settings alone.
                if std::env::var_os("NULL_MINI_EXIT_WHEN_READY").is_none() {
                    access::check(handle);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Null could not start");
}
