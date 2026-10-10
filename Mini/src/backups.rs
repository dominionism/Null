//! The backup order: the models to carry on with when the one in use stops answering.
//!
//! Null does not do the switching. The harness does: given a list of fallbacks
//! in its settings, it moves to the next model and sends the message again by
//! itself. This module keeps the order the user chose and puts it in the form
//! of the harness's settings, which `harness.rs` hands over at start in a file
//! of Null's own, beside the user's. It holds model names and nothing else.

use std::path::Path;

use serde_json::{json, Map, Value};
use tauri::AppHandle;

use crate::log::log;
use crate::{harness, settings};

/// The order as the user gave it, without blanks or repeats.
pub fn tidy(order: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for id in order.iter().map(|id| id.trim()).filter(|id| !id.is_empty()) {
        if !kept.iter().any(|known| known == id) {
            kept.push(id.to_string());
        }
    }
    kept
}

/// The harness settings that make `order` its fallbacks for every model. None
/// when there is no order: the harness is then left to its own settings.
///
/// `default` is the list the harness turns to for any model that has no list of
/// its own. It skips the model that just failed, so a model may be in its own
/// list.
///
/// `own` is the lists the user already has in the harness's settings. The
/// harness adds this file to those, and tries a list for one model or one
/// provider before `default`. So that Null's order comes first whatever model
/// is in use, each such list is given again here: the order, then whatever the
/// user's list adds to it.
pub fn overlay(order: &[String], own: &Value) -> Option<Value> {
    let order = tidy(order);
    if order.is_empty() {
        return None;
    }
    let ahead_of = |list: Option<&Value>| -> Vec<String> {
        let mut whole = order.clone();
        for entry in list.and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            if !whole.iter().any(|known| known == entry) {
                whole.push(entry.to_string());
            }
        }
        whole
    };
    let mut lists = Map::new();
    lists.insert("default".into(), json!(ahead_of(own.get("default"))));
    for (key, list) in own.as_object().into_iter().flatten() {
        // A key with a slash names a model or a provider; any other is a role, left as it is.
        if key.contains('/') {
            lists.insert(key.clone(), json!(ahead_of(Some(list))));
        }
    }
    Some(json!({ "retry": { "fallbackChains": lists } }))
}

/// The fallback lists the user has in the harness's own settings, or null.
fn own_lists(binary: &Path) -> Value {
    let output = std::process::Command::new(binary).args(harness::extra_args()).args(["config", "get", "retry.fallbackChains", "--json"]).output();
    output.map(|output| lists_read(&output.stdout)).unwrap_or(Value::Null)
}

/// Those lists, out of what the harness prints when asked for them.
pub fn lists_read(printed: &[u8]) -> Value {
    serde_json::from_slice::<Value>(printed).ok().and_then(|read| read.get("value").cloned()).unwrap_or(Value::Null)
}

/// The order in force: the user's, unless `NULL_MINI_BACKUPS` (model ids with
/// commas between) names one for a scripted check.
fn order(app: &AppHandle) -> Vec<String> {
    match std::env::var("NULL_MINI_BACKUPS") {
        Ok(list) => tidy(&list.split(',').map(str::to_string).collect::<Vec<_>>()),
        Err(_) => tidy(&settings::get(app).backups),
    }
}

/// The harness settings that make it follow the backup order in force, for the
/// harness about to be started. None when there is no order.
pub fn harness_settings(app: &AppHandle, binary: &Path) -> Option<Value> {
    let order = order(app);
    if order.is_empty() {
        return None;
    }
    overlay(&order, &own_lists(binary))
}

/// The backup order, first to last.
#[tauri::command]
pub fn backups(app: AppHandle) -> Vec<String> {
    order(&app)
}

/// Set the backup order. The harness reads it when it starts, so the one that
/// is running is let go; the next message starts another and the conversation
/// is loaded back.
#[tauri::command]
pub fn set_backups(app: AppHandle, ids: Vec<String>) -> Result<Vec<String>, String> {
    if harness::busy(&app) {
        return Err("stop the reply before changing the backups".into());
    }
    let order = tidy(&ids);
    settings::update(&app, |settings| settings.backups = order.clone());
    log!("backup order: {}", if order.is_empty() { "none".to_string() } else { order.join(", then ") });
    harness::restart(&app);
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn the_order_is_kept_and_repeats_and_blanks_are_dropped() {
        assert_eq!(tidy(&ids(&["b/two", " a/one ", "", "b/two"])), ids(&["b/two", "a/one"]));
    }

    #[test]
    fn the_harness_is_given_the_order_as_its_fallbacks_for_every_model() {
        let read = overlay(&ids(&["anthropic/claude-opus-5-5", "opencode-go/deepseek-v4.1-flash"]), &Value::Null).unwrap();
        assert_eq!(read, json!({ "retry": { "fallbackChains": { "default": ["anthropic/claude-opus-5-5", "opencode-go/deepseek-v4.1-flash"] } } }));
    }

    #[test]
    fn the_order_goes_ahead_of_the_lists_the_user_already_has() {
        // The owner's settings on 2026-10-09, with a role and a list for one model added.
        let own = json!({
            "openai-codex/*": ["openai-codex/gpt-5.6-sol"],
            "anthropic/claude-sonnet-5-5": ["anthropic/claude-opus-5-5", "anthropic/*"],
            "default": ["ollama/llama3.2:latest"],
            "smol": ["opencode-go/deepseek-flash"]
        });
        let read = overlay(&ids(&["anthropic/claude-opus-5-5", "opencode-go/deepseek-v4.1-flash"]), &own).unwrap();
        assert_eq!(
            read["retry"]["fallbackChains"],
            json!({
                "default": ["anthropic/claude-opus-5-5", "opencode-go/deepseek-v4.1-flash", "ollama/llama3.2:latest"],
                "openai-codex/*": ["anthropic/claude-opus-5-5", "opencode-go/deepseek-v4.1-flash", "openai-codex/gpt-5.6-sol"],
                "anthropic/claude-sonnet-5-5": ["anthropic/claude-opus-5-5", "opencode-go/deepseek-v4.1-flash", "anthropic/*"]
            })
        );
    }

    #[test]
    fn settings_that_cannot_be_read_change_nothing() {
        let order = ids(&["a/one"]);
        let read = overlay(&order, &json!("nonsense")).unwrap();
        assert_eq!(read["retry"]["fallbackChains"], json!({ "default": ["a/one"] }));
        let read = overlay(&order, &json!({ "b/*": "not a list", "c/two": [1, null, "c/three"] })).unwrap();
        assert_eq!(read["retry"]["fallbackChains"], json!({ "default": ["a/one"], "b/*": ["a/one"], "c/two": ["a/one", "c/three"] }));
    }

    #[test]
    fn with_no_order_the_harness_is_left_to_its_own_settings() {
        assert_eq!(overlay(&[], &Value::Null), None);
        assert_eq!(overlay(&ids(&["", "  "]), &json!({ "b/*": ["c/d"] })), None);
    }
}
