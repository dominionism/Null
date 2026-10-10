//! Null: a small floating box that opens on Control+Space and drives the user's
//! own agent harness. It is one app with no server; the page in `Page/` talks to
//! this process through commands and events.
//!
//! Nine switches for development, all read from the environment:
//! - `NULL_MINI_EXIT_WHEN_READY`: quit as soon as the page has loaded, which makes
//!   "start, load the page, stop" a check a script can run.
//! - `NULL_MINI_NO_SHORTCUT`: do not listen for Control+Space, and show the box at
//!   start instead. For running beside another copy that owns the shortcut.
//! - `NULL_MINI_SMOKE`: send its value to the harness as one message, print what
//!   comes back, and quit. No window and no shortcut; it checks the harness alone.
//! - `NULL_MINI_SELFTEST`: type its value into the real page, a line at a time,
//!   waiting each time until the page has stopped working, log what the page
//!   shows, and quit. A message, a command, a choice from a list and an answer to
//!   a sign-in can follow one another. Use with `NULL_MINI_NO_SHORTCUT`.
//! - `NULL_MINI_PROFILE`: run the harness under that isolated profile of its own,
//!   so sign-in and first-run behaviour can be tried without the real sign-ins.
//! - `NULL_MINI_UPDATES`: where `/update` reads which version is checked, in
//!   place of the repository: a made-up file, as a `file://` address.
//! - `NULL_MINI_RELEASE`: where `/update` learns which Null is the newest, in
//!   place of GitHub: a made-up address whose last part is a tag, `null-v0.2.0`.
//! - `NULL_MINI_FOLDER`: run on that folder, made if it is not there, in place of
//!   the user's: the harness's folder, Null's own files and the log are all under
//!   it. A new one is a Mac with nothing signed in, and removing it removes the run.
//! - `NULL_MINI_STANDIN`: offer a stand-in provider's models in that folder, so a
//!   message is answered, or refused, with no real sign-in. Needs
//!   `NULL_MINI_FOLDER`. The models are `standin-anthropic/ok` and the like.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "macos"))]
compile_error!("Null is macOS-only for now");

mod access;
mod backups;
#[cfg(test)]
mod check;
mod engine;
mod harness;
mod history;
mod log;
mod markdown;
mod panel;
mod providers;
mod settings;
mod shortcut;
mod signin;
mod standin;
mod translate;
mod welcome;

use log::log;
use tauri::Emitter;

/// The page calls this once its script has run.
#[tauri::command]
fn page_ready(app: tauri::AppHandle) {
    log!("page ready");
    welcome::page_ready();
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
/// A reply's Markdown as the parts the box draws.
#[tauri::command(async)]
fn layout(text: String) -> serde_json::Value {
    markdown::tree(&text)
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    log!("quit from the box; the conversation is closed");
    settings::update(&app, |settings| settings.session = None);
    app.exit(0);
}

fn main() {
    // A run on a folder of its own is set up before anything is started.
    if let Err(reason) = settings::enter_own_folder().and_then(|()| standin::for_this_run()) {
        log!("not started: {reason}");
        std::process::exit(2);
    }
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
            history::history,
            history::open_conversation,
            layout,
            providers::providers,
            backups::backups,
            backups::set_backups,
            engine::harnesses,
            engine::set_harness,
            engine::update_harness,
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
            engine::tidy(handle);
            harness::init(handle);
            signin::init(handle);
            providers::init(handle);
            if let Some(text) = std::env::var_os("NULL_MINI_SMOKE") {
                harness::smoke(handle, text.to_string_lossy().into_owned());
                return Ok(());
            }
            panel::create(handle)?;
            let no_shortcut = std::env::var_os("NULL_MINI_NO_SHORTCUT").is_some();
            if no_shortcut {
                log!("Control+Space is off for this run (NULL_MINI_NO_SHORTCUT)");
                panel::show(handle);
            } else {
                shortcut::start(handle);
            }
            // The first opening and the Full Disk Access question belong to a person's
            // run. A scripted one leaves the user's settings alone, unless it has a
            // folder of its own, where both can be tried.
            let startup_check = std::env::var_os("NULL_MINI_EXIT_WHEN_READY").is_some();
            if !startup_check && (!no_shortcut || settings::own_folder().is_some()) {
                welcome::check(handle);
                access::at_start(handle);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Null could not start");
}
