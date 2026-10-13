//! The browser link: the harness's way into the person's own Chrome, offered
//! by Null under its own name.
//!
//! The link is the harness's own: an extension the person loads in Chrome,
//! and a relay on this Mac that the harness starts when its browser needs it.
//! Null has the engine in use write that extension into a folder of Null's,
//! `BrowserLink`, and changes what Chrome shows of it: the name, the toolbar's
//! words and icon, the heading of its settings page, and the title of the tab
//! group the agent's tabs are gathered in. Nothing else of it is changed, and
//! the harness's licence and notices stay beside it. Chrome loads it from that
//! folder and goes on loading it from there, so the folder never moves: the
//! extension of a newer engine is written into the same place.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use crate::{harness, settings};

/// The name Chrome shows for the extension, wherever it shows one.
pub const NAME: &str = "Null";

/// What Chrome says the extension is for.
const DESCRIPTION: &str = "Lets the agent in Null work your own Chrome tabs.";

/// Null's own icon, at the two sizes Chrome asks for.
const ICONS: [(&str, &str, &[u8]); 2] = [("32", "null-32.png", include_bytes!("../Icons/32x32.png")), ("128", "null-128.png", include_bytes!("../Icons/128x128.png"))];

/// Each change to the extension's own files: the file, what stands there, and
/// what goes in its place. The tab group's title is the relay's to give, and
/// the extension is made to keep its own instead.
const CHANGES: [(&str, &str, &str); 3] = [
    ("background.js", "async function groupTabs(tabIds, title, color) {", "async function groupTabs(tabIds, _title, color) {\n  const title = \"Null\";"),
    ("options.html", "<title>OMP Browser Relay settings</title>", "<title>Null settings</title>"),
    ("options.html", "<h1>OMP Browser Relay</h1>", "<h1>Null</h1>"),
];

/// The folder Null keeps its copy of the extension in, which Chrome loads.
pub fn folder(dir: &Path) -> PathBuf {
    dir.join(settings::folder_name("BrowserLink"))
}

/// Make Null's copy of the extension in `folder`, from the one `binary` writes.
/// It is written beside the folder and put in its place only once it is whole
/// and renamed, so that Chrome never finds half an extension there. An
/// extension that is not the one Null knows how to rename is not put there
/// at all: the link is never offered half under the harness's name.
pub fn make(binary: &Path, folder: &Path) -> Result<(), String> {
    let fresh = folder.with_extension("new");
    let _ = std::fs::remove_dir_all(&fresh);
    let made = write(binary, &fresh).and_then(|()| rename_in(&fresh));
    if let Err(why) = made {
        let _ = std::fs::remove_dir_all(&fresh);
        return Err(why);
    }
    let _ = std::fs::remove_dir_all(folder);
    std::fs::rename(&fresh, folder).map_err(|e| format!("could not put Null's copy of the extension in place: {e}"))
}

/// Have the harness write its extension into `folder`.
fn write(binary: &Path, folder: &Path) -> Result<(), String> {
    let written = Command::new(binary)
        .args(harness::extra_args())
        .args(["browser-relay", "install", "--dir"])
        .arg(folder)
        .output()
        .map_err(|e| format!("{} could not be started: {e}", harness::HARNESS_NAME))?;
    if !written.status.success() || !folder.join("manifest.json").is_file() {
        return Err(format!("{} would not write its browser extension", harness::HARNESS_NAME));
    }
    Ok(())
}

/// Give the extension in `folder` Null's name, words and icon.
fn rename_in(folder: &Path) -> Result<(), String> {
    let read = |file: &str| std::fs::read_to_string(folder.join(file)).map_err(|_| unknown(file));
    let put = |file: &str, text: &[u8]| std::fs::write(folder.join(file), text).map_err(|e| format!("could not write {file}: {e}"));
    put("manifest.json", renamed_manifest(&read("manifest.json")?)?.as_bytes())?;
    for (file, from, to) in CHANGES {
        put(file, changed(&read(file)?, from, to, file)?.as_bytes())?;
    }
    for (_, file, bytes) in ICONS {
        put(file, bytes)?;
    }
    Ok(())
}

/// The extension's manifest with Null's name, words and icon in place of the
/// harness's. Everything else in it, what it asks Chrome for above all, is as
/// the harness wrote it.
fn renamed_manifest(text: &str) -> Result<String, String> {
    let mut manifest: Value = serde_json::from_str(text).map_err(|_| unknown("manifest.json"))?;
    let icons: Value = ICONS.iter().map(|(size, file, _)| (size.to_string(), json!(file))).collect::<serde_json::Map<_, _>>().into();
    let fields = manifest.as_object_mut().ok_or_else(|| unknown("manifest.json"))?;
    if !fields.contains_key("name") {
        return Err(unknown("manifest.json"));
    }
    fields.insert("name".into(), json!(NAME));
    fields.insert("description".into(), json!(DESCRIPTION));
    fields.insert("icons".into(), icons.clone());
    let action = fields.get_mut("action").and_then(Value::as_object_mut).ok_or_else(|| unknown("manifest.json"))?;
    action.insert("default_title".into(), json!(NAME));
    action.insert("default_icon".into(), icons);
    serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())
}

/// `text` with `from` replaced by `to`, where `from` stands exactly once.
fn changed(text: &str, from: &str, to: &str, file: &str) -> Result<String, String> {
    match text.matches(from).count() {
        1 => Ok(text.replacen(from, to, 1)),
        _ => Err(unknown(file)),
    }
}

/// What is said when the harness's extension is not the one Null knows.
fn unknown(file: &str) -> String {
    format!("this {}'s browser extension is not the one Null knows how to name ({file})", harness::HARNESS_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As Oh-my-pi 18.8.7 writes it.
    const MANIFEST: &str = r#"{
	"manifest_version": 3,
	"name": "OMP Browser Relay",
	"version": "0.1.0",
	"description": "Lets the omp coding agent drive your existing tabs through a local CDP relay.",
	"permissions": ["debugger", "tabs", "tabGroups", "storage", "alarms"],
	"background": { "service_worker": "background.js" },
	"options_page": "options.html",
	"action": { "default_title": "OMP Browser Relay (click for settings)" }
}"#;

    #[test]
    fn chrome_shows_null_s_name_words_and_icon_and_nothing_else_changes() {
        let renamed: Value = serde_json::from_str(&renamed_manifest(MANIFEST).unwrap()).unwrap();
        let before: Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(renamed["name"], "Null");
        assert_eq!(renamed["action"]["default_title"], "Null");
        assert_eq!(renamed["description"], DESCRIPTION);
        assert_eq!(renamed["icons"], json!({ "32": "null-32.png", "128": "null-128.png" }));
        assert_eq!(renamed["action"]["default_icon"], renamed["icons"]);
        for kept in ["manifest_version", "version", "permissions", "background", "options_page"] {
            assert_eq!(renamed[kept], before[kept], "{kept} is as the harness wrote it");
        }
    }

    #[test]
    fn a_manifest_null_does_not_know_is_not_renamed() {
        assert!(renamed_manifest("not json").is_err());
        assert!(renamed_manifest(r#"{ "name": "x" }"#).is_err(), "no toolbar button");
        assert!(renamed_manifest(r#"{ "action": {} }"#).is_err(), "no name");
    }

    #[test]
    fn a_change_is_made_only_where_its_words_stand_once() {
        assert_eq!(changed("a <h1>X</h1> b", "<h1>X</h1>", "<h1>Null</h1>", "f").unwrap(), "a <h1>Null</h1> b");
        assert!(changed("a b", "<h1>X</h1>", "<h1>Null</h1>", "f").is_err(), "not there");
        assert!(changed("<h1>X</h1><h1>X</h1>", "<h1>X</h1>", "<h1>Null</h1>", "f").is_err(), "there twice");
    }
}
