//! Turning what a harness says over the Agent Client Protocol into what the box
//! shows, and the user's harness settings into what the protocol accepts.
//!
//! Everything here works on the protocol's JSON, which is the stable thing; no
//! process is started and nothing is sent.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

/// One model the harness can reach with the sign-ins it holds.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModelChoice {
    /// What the harness accepts, e.g. "opencode-go/deepseek-v4.1-flash".
    pub id: String,
    pub label: String,
    pub provider: Option<String>,
}

/// Translate one `session/update` into an event for the box, or None when the box
/// has no use for it.
///
/// `tools` remembers each tool call, because a progress update names only what changed.
pub fn event_from_update(update: &Value, tools: &mut HashMap<String, Value>) -> Option<Value> {
    match update.get("sessionUpdate")?.as_str()? {
        kind @ ("agent_message_chunk" | "agent_thought_chunk") => {
            let text = update.pointer("/content/text")?.as_str()?;
            if text.is_empty() {
                return None;
            }
            Some(json!({ "type": "text_delta", "text": text, "thinking": kind == "agent_thought_chunk" }))
        }
        // What the user said, replayed when a conversation is loaded again.
        "user_message_chunk" => {
            let text = update.pointer("/content/text")?.as_str()?;
            Some(json!({ "type": "user_message", "text": text }))
        }
        "tool_call" | "tool_call_update" => {
            let call_id = update.get("toolCallId")?.as_str()?.to_string();
            let known = tools.get(&call_id).cloned().unwrap_or(Value::Null);
            let pick = |field: &str, known_field: &str, fallback: Value| -> Value {
                match update.get(field) {
                    Some(value) if !value.is_null() => value.clone(),
                    _ => known.get(known_field).filter(|value| !value.is_null()).cloned().unwrap_or(fallback),
                }
            };
            let event = json!({
                "type": "tool_activity",
                "call_id": call_id,
                "title": pick("title", "title", json!("Tool")),
                "status": pick("status", "status", json!("pending")),
                "kind": pick("kind", "kind", Value::Null),
                "raw_input": pick("rawInput", "raw_input", Value::Null),
                "raw_output": pick("rawOutput", "raw_output", Value::Null),
            });
            tools.insert(call_id, event.clone());
            Some(event)
        }
        _ => None,
    }
}

/// How one reply ended, in the box's words. A failure is an error event, not a stop reason.
pub fn stop_reason(protocol: &str) -> &'static str {
    match protocol {
        "cancelled" => "cancelled",
        "max_tokens" | "max_turn_requests" => "truncated",
        "refusal" => "refused",
        _ => "completed",
    }
}

/// Pull the model list and the current model out of a session's config options.
pub fn models_from_config_options(options: &Value) -> (Vec<ModelChoice>, Option<String>) {
    let Some(option) = options.as_array().and_then(|options| {
        options.iter().find(|option| option.get("id") == Some(&json!("model")) || option.get("category") == Some(&json!("model")))
    }) else {
        return (Vec::new(), None);
    };
    let choice = |entry: &Value, provider: Option<String>| -> Option<ModelChoice> {
        let id = entry.get("value")?.as_str()?.to_string();
        let label = entry.get("name").and_then(Value::as_str).unwrap_or(&id).to_string();
        let provider = provider.or_else(|| id.split_once('/').map(|(provider, _)| provider.to_string()));
        Some(ModelChoice { id, label, provider })
    };
    let mut choices = Vec::new();
    for entry in option.get("options").and_then(Value::as_array).into_iter().flatten() {
        match entry.get("options").and_then(Value::as_array) {
            // A group: its name is the provider.
            Some(grouped) => {
                let provider = entry.get("name").and_then(Value::as_str).map(str::to_string);
                choices.extend(grouped.iter().filter_map(|inner| choice(inner, provider.clone())));
            }
            None => choices.extend(choice(entry, None)),
        }
    }
    let current = option.get("currentValue").and_then(Value::as_str).map(str::to_string);
    (choices, current)
}

/// Turn one entry of the user's `mcp.json` into what `session/new` accepts.
///
/// Returns None for an entry the agent cannot take: an unknown shape, or a
/// transport the agent did not say it supports (`capabilities` is its
/// `mcpCapabilities`).
pub fn mcp_server_from_config(name: &str, entry: &Value, capabilities: &Value) -> Option<Value> {
    let pairs = |field: &str| -> Vec<Value> {
        entry
            .get(field)
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .map(|(key, value)| json!({ "name": key, "value": value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string()) }))
                    .collect()
            })
            .unwrap_or_default()
    };

    if let Some(command) = entry.get("command").and_then(Value::as_str) {
        let args = entry.get("args").and_then(Value::as_array).cloned().unwrap_or_default();
        return Some(json!({ "name": name, "command": command, "args": args, "env": pairs("env") }));
    }

    let url = entry.get("url").and_then(Value::as_str)?;
    let transport = entry.get("type").and_then(Value::as_str).unwrap_or("http");
    if !matches!(transport, "http" | "sse") || capabilities.get(transport) != Some(&json!(true)) {
        return None;
    }
    Some(json!({ "type": transport, "name": name, "url": url, "headers": pairs("headers") }))
}

/// Where harness CLIs install themselves, in the order tried. A login-launched
/// app gets a minimal PATH, so a harness the user runs every day from a terminal
/// is usually not on it.
const INSTALL_DIRS: [&str; 5] = ["~/.omp/bin", "~/.opencode/bin", "~/.local/bin", "/opt/homebrew/bin", "/usr/local/bin"];

/// Absolute path to the CLI `name`, or None when it is not installed.
///
/// `chosen` is a path the user set by hand; it wins when it points at something runnable.
pub fn find_binary(name: &str, chosen: Option<&str>, home: &Path, path_var: &str, install_dirs: &[&str]) -> Option<PathBuf> {
    let expand = |text: &str| -> PathBuf {
        match text.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => PathBuf::from(text),
        }
    };
    let candidates = chosen
        .map(expand)
        .into_iter()
        .chain(path_var.split(':').filter(|dir| !dir.is_empty()).map(|dir| PathBuf::from(dir).join(name)))
        .chain(install_dirs.iter().map(|dir| expand(dir).join(name)));
    candidates.into_iter().find(|candidate| runnable(candidate))
}

/// `find_binary` with this machine's home, PATH and install folders.
pub fn find_installed(name: &str, chosen: Option<&str>) -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let path_var = std::env::var("PATH").unwrap_or_default();
    find_binary(name, chosen, &home, &path_var, &INSTALL_DIRS)
}

fn runnable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_text_and_thinking_text_are_told_apart() {
        let mut tools = HashMap::new();
        let answer = json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "hi" } });
        let thought = json!({ "sessionUpdate": "agent_thought_chunk", "content": { "type": "text", "text": "hmm" } });

        assert_eq!(event_from_update(&answer, &mut tools), Some(json!({ "type": "text_delta", "text": "hi", "thinking": false })));
        assert_eq!(event_from_update(&thought, &mut tools), Some(json!({ "type": "text_delta", "text": "hmm", "thinking": true })));
    }

    #[test]
    fn a_progress_update_keeps_what_the_tool_call_started_with() {
        let mut tools = HashMap::new();
        let started = json!({ "sessionUpdate": "tool_call", "toolCallId": "c1", "title": "$ ls", "kind": "execute", "status": "pending", "rawInput": { "command": "ls" } });
        let finished = json!({ "sessionUpdate": "tool_call_update", "toolCallId": "c1", "status": "completed", "rawOutput": "a.txt" });

        event_from_update(&started, &mut tools);
        let event = event_from_update(&finished, &mut tools).unwrap();

        assert_eq!(event["title"], "$ ls");
        assert_eq!(event["kind"], "execute");
        assert_eq!(event["status"], "completed");
        assert_eq!(event["raw_input"], json!({ "command": "ls" }));
        assert_eq!(event["raw_output"], "a.txt");
    }

    #[test]
    fn a_replayed_user_message_becomes_a_prompt_line() {
        let update = json!({ "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "hello" } });
        assert_eq!(event_from_update(&update, &mut HashMap::new()), Some(json!({ "type": "user_message", "text": "hello" })));
    }

    #[test]
    fn an_update_the_box_has_no_use_for_is_dropped() {
        let usage = json!({ "sessionUpdate": "usage_update", "used": 10, "size": 100 });
        assert_eq!(event_from_update(&usage, &mut HashMap::new()), None);
    }

    #[test]
    fn stop_reasons_are_put_in_the_boxes_words() {
        assert_eq!(stop_reason("end_turn"), "completed");
        assert_eq!(stop_reason("cancelled"), "cancelled");
        assert_eq!(stop_reason("max_tokens"), "truncated");
        assert_eq!(stop_reason("refusal"), "refused");
    }

    fn model_options() -> Value {
        json!([
            { "id": "thinking", "currentValue": "auto", "options": [] },
            { "id": "model", "currentValue": "go/flash", "options": [
                { "group": "go", "name": "opencode-go", "options": [{ "value": "go/flash", "name": "Flash" }] },
                { "group": "codex", "name": "openai-codex", "options": [{ "value": "codex/luna", "name": "Luna" }] },
            ] },
        ])
    }

    #[test]
    fn models_are_listed_with_their_provider_and_the_current_one() {
        let (choices, current) = models_from_config_options(&model_options());

        assert_eq!(
            choices,
            vec![
                ModelChoice { id: "go/flash".into(), label: "Flash".into(), provider: Some("opencode-go".into()) },
                ModelChoice { id: "codex/luna".into(), label: "Luna".into(), provider: Some("openai-codex".into()) },
            ]
        );
        assert_eq!(current.as_deref(), Some("go/flash"));
    }

    #[test]
    fn ungrouped_models_take_their_provider_from_the_id() {
        let options = json!([{ "id": "model", "currentValue": "a/b", "options": [{ "value": "a/b", "name": "B" }, { "value": "plain", "name": "Plain" }] }]);
        let (choices, _) = models_from_config_options(&options);

        assert_eq!(choices[0].provider.as_deref(), Some("a"));
        assert_eq!(choices[1].provider, None);
    }

    #[test]
    fn a_session_without_a_model_option_lists_nothing() {
        assert_eq!(models_from_config_options(&json!([])), (Vec::new(), None));
    }

    #[test]
    fn an_http_mcp_server_carries_its_headers() {
        let entry = json!({ "type": "http", "url": "http://127.0.0.1:17493/mcp", "headers": { "X-Id": "omp" } });
        let server = mcp_server_from_config("voicebox", &entry, &json!({ "http": true, "sse": false })).unwrap();

        assert_eq!(server, json!({ "type": "http", "name": "voicebox", "url": "http://127.0.0.1:17493/mcp", "headers": [{ "name": "X-Id", "value": "omp" }] }));
    }

    #[test]
    fn a_transport_the_agent_does_not_support_is_left_out() {
        let capabilities = json!({ "http": true, "sse": false });
        assert_eq!(mcp_server_from_config("old", &json!({ "type": "sse", "url": "http://x/sse" }), &capabilities), None);
        assert_eq!(mcp_server_from_config("odd", &json!({ "type": "carrier-pigeon" }), &capabilities), None);
    }

    #[test]
    fn a_command_mcp_server_carries_its_arguments_and_environment() {
        let entry = json!({ "command": "npx", "args": ["-y", "fs"], "env": { "ROOT": "/tmp" } });
        let server = mcp_server_from_config("files", &entry, &Value::Null).unwrap();

        assert_eq!(server, json!({ "name": "files", "command": "npx", "args": ["-y", "fs"], "env": [{ "name": "ROOT", "value": "/tmp" }] }));
    }

    fn make_cli(dir: &Path, mode: u32) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join("omp");
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("null-mini-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_cli_is_found_on_path_then_where_it_installs_itself() {
        let root = scratch("find");
        let on_path = make_cli(&root.join("bin"), 0o755);
        let installed = make_cli(&root.join("home/.omp/bin"), 0o755);

        let path_var = root.join("bin").display().to_string();
        assert_eq!(find_binary("omp", None, &root.join("home"), &path_var, &["~/.omp/bin"]), Some(on_path));
        assert_eq!(find_binary("omp", None, &root.join("home"), "", &["~/.omp/bin"]), Some(installed));
        assert_eq!(find_binary("omp", None, &root.join("home"), "", &["~/.nowhere"]), None);
    }

    #[test]
    fn a_path_the_user_set_wins_and_a_broken_one_is_ignored() {
        let root = scratch("chosen");
        let on_path = make_cli(&root.join("bin"), 0o755);
        let chosen = make_cli(&root.join("custom"), 0o755);
        let path_var = root.join("bin").display().to_string();

        assert_eq!(find_binary("omp", Some(&chosen.display().to_string()), &root, &path_var, &[]), Some(chosen));
        assert_eq!(find_binary("omp", Some("/nowhere/omp"), &root, &path_var, &[]), Some(on_path));
    }

    #[test]
    fn a_file_that_cannot_be_run_is_skipped() {
        let root = scratch("mode");
        make_cli(&root.join("bin"), 0o644);

        assert_eq!(find_binary("omp", None, &root, &root.join("bin").display().to_string(), &[]), None);
    }
}
