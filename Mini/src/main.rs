//! Null: a small floating box that opens on fn+Space and drives the user's
//! own agent harness. It is one app with no server; the page in `Page/` talks to
//! this process through commands and events.
//!
//! Four switches for development, all read from the environment:
//! - `NULL_MINI_EXIT_WHEN_READY`: quit as soon as the page has loaded, which makes
//!   "start, load the page, stop" a check a script can run.
//! - `NULL_MINI_NO_SHORTCUT`: do not listen for fn+Space, and show the box at
//!   start instead. For running beside another copy that owns the shortcut.
//! - `NULL_MINI_SMOKE`: send its value to the harness as one message, print what
//!   comes back, and quit. No window and no shortcut; it checks the harness alone.
//! - `NULL_MINI_SELFTEST`: type its value into the real page, wait for the reply,
//!   log what the page shows, and quit. Use with `NULL_MINI_NO_SHORTCUT`.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "macos"))]
compile_error!("Null is macOS-only for now");

mod harness;
mod log;
mod panel;
mod settings;
mod shortcut;
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
    // The page may have loaded after the app found the permission missing.
    if shortcut::waiting_for_permission() {
        let _ = app.emit_to(panel::LABEL, "mini:permission", false);
    }
}

/// `/quit` in the box. The app has no Dock icon or menu to quit from.
#[tauri::command]
fn quit(app: tauri::AppHandle) {
    log!("quit from the box");
    app.exit(0);
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
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
        ])
        .setup(|app| {
            log!("started, version {}", app.package_info().version);
            // A background app: no Dock icon, no menu bar of its own.
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle();
            settings::init(handle);
            harness::init(handle);
            if let Some(text) = std::env::var_os("NULL_MINI_SMOKE") {
                harness::smoke(handle, text.to_string_lossy().into_owned());
                return Ok(());
            }
            panel::create(handle)?;
            if std::env::var_os("NULL_MINI_NO_SHORTCUT").is_some() {
                log!("fn+Space is off for this run (NULL_MINI_NO_SHORTCUT)");
                panel::show(handle);
            } else {
                shortcut::start(handle);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Null could not start");
}
