//! A stand-in provider on this Mac, so that Null can be tried without a real
//! sign-in, a real limit or a real bill.
//!
//! It answers by the name of the model asked for: `ok` replies, `limit` is used
//! up, `auth` is a bad sign-in and `noaccess` is a model the account may not
//! use, each in the three ways of talking the owner's providers use. Its answers
//! are close copies of what those providers send, not recordings, so the first
//! real limit is still worth setting beside them.
//!
//! Handed a file with a message (`attach.rs`), `ok` in Anthropic's way of
//! talking does what a model does: it asks the harness to read the file, and
//! replies once it has been given what is in it.
//!
//! Two things use it. The harness check (`check.rs`) asks a real Oh-my-pi its
//! questions against it. And a scripted run of the app itself offers its models
//! to the harness with `NULL_MINI_STANDIN`, on a folder of the run's own
//! (`NULL_MINI_FOLDER`), so that nothing of the user's is read or written.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::log::log;
use crate::settings;

/// The three ways of talking the owner's providers use: the name in a stand-in
/// address, and what the harness calls that way of talking.
const KINDS: [(&str, &str); 3] = [("anthropic", "anthropic-messages"), ("openai", "openai-completions"), ("codex", "openai-codex-responses")];
const ANSWERS: [&str; 4] = ["ok", "limit", "auth", "noaccess"];

/// The first line of the file the stand-in writes, by which it knows its own file again.
const MARK: &str = "# Offered by Null's stand-in provider (NULL_MINI_STANDIN). Not a real provider.\n";

// The harness check reads what was asked and reached. A run of the app only needs the port.
#[cfg_attr(not(test), allow(dead_code))]
pub struct StandIn {
    pub port: u16,
    /// Every model a message was sent to, in order, as `kind/answer`.
    asked: Arc<Mutex<Vec<String>>>,
    /// Every address a harness tried to reach through the stand-in, when it was
    /// set as that harness's way out (`Folder::behind`). Nothing is let through.
    reached: Arc<Mutex<Vec<String>>>,
    /// Everything a model was sent, in order: the body of each message to one.
    heard: Arc<Mutex<Vec<Value>>>,
}

impl StandIn {
    pub fn start() -> StandIn {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port on this Mac");
        let port = listener.local_addr().expect("the port it was given").port();
        let (asked, reached, heard) = (Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Vec::new())));
        let record = (asked.clone(), reached.clone(), heard.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let record = record.clone();
                std::thread::spawn(move || answer(stream, &record.0, &record.1, &record.2));
            }
        });
        StandIn { port, asked, reached, heard }
    }

    /// The file that offers the stand-in's models to a harness, as `standin-<kind>/<answer>`.
    pub fn models_yml(&self) -> String {
        let mut text = format!("{MARK}providers:\n");
        for (kind, api) in KINDS {
            let path = if kind == "openai" { "openai/v1" } else { kind };
            text.push_str(&format!("  standin-{kind}:\n    baseUrl: http://127.0.0.1:{}/{path}\n    api: {api}\n    apiKey: standin-not-a-key\n    models:\n", self.port));
            for answer in ANSWERS {
                text.push_str(&format!("      - {{ id: {answer}, name: Stand-in {answer}, contextWindow: 128000, maxTokens: 8192 }}\n"));
            }
        }
        text
    }

    #[cfg(test)]
    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }

    #[cfg(test)]
    pub fn reached(&self) -> Vec<String> {
        self.reached.lock().unwrap().clone()
    }

    #[cfg(test)]
    pub fn heard(&self) -> Vec<Value> {
        self.heard.lock().unwrap().clone()
    }
}

/// Read one request: its method, its path and its body.
fn read_request(stream: &mut TcpStream) -> Option<(String, String, Vec<u8>)> {
    stream.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    let mut reader = BufReader::new(stream);
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    let mut words = first.split_whitespace();
    let (method, path) = (words.next()?.to_string(), words.next()?.to_string());
    let (mut length, mut in_pieces) = (0, false);
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let Some((name, value)) = header.trim_end().split_once(':') else { break };
        match name.to_ascii_lowercase().as_str() {
            "content-length" => length = value.trim().parse().ok()?,
            "transfer-encoding" => in_pieces = value.to_ascii_lowercase().contains("chunked"),
            _ => {}
        }
    }
    let mut body = vec![0; length];
    if !in_pieces {
        reader.read_exact(&mut body).ok()?;
        return Some((method, path, body));
    }
    loop {
        let mut size = String::new();
        reader.read_line(&mut size).ok()?;
        let size = usize::from_str_radix(size.trim().split(';').next()?, 16).ok()?;
        if size == 0 {
            return Some((method, path, body));
        }
        let mut piece = vec![0; size + 2]; // the piece, and the line end after it
        reader.read_exact(&mut piece).ok()?;
        body.extend_from_slice(&piece[..size]);
    }
}

fn answer(mut stream: TcpStream, asked: &Mutex<Vec<String>>, reached: &Mutex<Vec<String>>, heard: &Mutex<Vec<Value>>) {
    let Some((method, path, body)) = read_request(&mut stream) else { return };
    // Asked to pass a request on to somewhere else: write down where, and refuse.
    if method == "CONNECT" || path.starts_with("http") {
        reached.lock().unwrap().push(path);
        let _ = stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\nconnection: close\r\ncontent-length: 0\r\n\r\n");
        return;
    }
    let kind = path.split('/').nth(1).unwrap_or_default();
    let said = serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null);
    let model = said.get("model").and_then(Value::as_str).unwrap_or_default().to_string();
    let which = model.rsplit('/').next().unwrap_or_default();
    let (status, headers, body) = match (method.as_str(), which) {
        ("POST", "ok" | "limit" | "auth" | "noaccess") => {
            asked.lock().unwrap().push(format!("{kind}/{which}"));
            let answered = reply(kind, which, &model, &said);
            heard.lock().unwrap().push(said);
            answered
        }
        _ => (404, Vec::new(), json!({ "error": { "message": "stand-in: nothing here", "type": "not_found" } }).to_string()),
    };
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        403 => "Forbidden",
        429 => "Too Many Requests",
        _ => "Not Found",
    };
    let kind_of_body = if status == 200 { "text/event-stream" } else { "application/json" };
    let mut head = format!("HTTP/1.1 {status} {reason}\r\nconnection: close\r\ncontent-type: {kind_of_body}\r\ncontent-length: {}\r\n", body.len());
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    let _ = stream.write_all(format!("{head}\r\n{body}").as_bytes());
}

/// What a provider of this kind sends back for this answer: the status, the
/// headers that say when to come back, and the body.
fn reply(kind: &str, which: &str, model: &str, said: &Value) -> (u16, Vec<(&'static str, String)>, String) {
    let soon = |seconds: u64| (SystemTime::now().duration_since(UNIX_EPOCH).map(|now| now.as_secs()).unwrap_or(0) + seconds).to_string();
    let anthropic = |kind_of_error: &str, message: &str| json!({ "type": "error", "error": { "type": kind_of_error, "message": message }, "request_id": "req_standin" }).to_string();
    let openai = |message: String, kind_of_error: &str, code: &str| json!({ "error": { "message": message, "type": kind_of_error, "param": null, "code": code } }).to_string();
    let events = |frames: &[(&str, Value)]| frames.iter().map(|(event, data)| format!("event: {event}\ndata: {data}\n\n")).collect::<String>();
    match (kind, which) {
        // Handed a file it has not read yet, a model asks to read it.
        ("anthropic", "ok") if reading(said).is_some() => {
            let (tool, input) = reading(said).unwrap_or_default();
            (
                200,
                Vec::new(),
                events(&[
                    ("message_start", json!({ "type": "message_start", "message": { "id": "msg_standin", "type": "message", "role": "assistant", "model": model, "content": [], "stop_reason": null, "stop_sequence": null, "usage": { "input_tokens": 10, "output_tokens": 1 } } })),
                    ("content_block_start", json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "tool_use", "id": "toolu_standin", "name": tool, "input": {} } })),
                    ("content_block_delta", json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "input_json_delta", "partial_json": input.to_string() } })),
                    ("content_block_stop", json!({ "type": "content_block_stop", "index": 0 })),
                    ("message_delta", json!({ "type": "message_delta", "delta": { "stop_reason": "tool_use", "stop_sequence": null }, "usage": { "output_tokens": 1 } })),
                    ("message_stop", json!({ "type": "message_stop" })),
                ]),
            )
        }
        ("anthropic", "ok") => (
            200,
            Vec::new(),
            events(&[
                ("message_start", json!({ "type": "message_start", "message": { "id": "msg_standin", "type": "message", "role": "assistant", "model": model, "content": [], "stop_reason": null, "stop_sequence": null, "usage": { "input_tokens": 10, "output_tokens": 1 } } })),
                ("content_block_start", json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } })),
                ("content_block_delta", json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "pong" } })),
                ("content_block_stop", json!({ "type": "content_block_stop", "index": 0 })),
                ("message_delta", json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn", "stop_sequence": null }, "usage": { "output_tokens": 1 } })),
                ("message_stop", json!({ "type": "message_stop" })),
            ]),
        ),
        ("anthropic", "limit") => (
            429,
            vec![
                ("retry-after", "5400".into()),
                ("anthropic-ratelimit-unified-status", "rejected".into()),
                ("anthropic-ratelimit-unified-reset", soon(5400)),
                ("anthropic-ratelimit-unified-5h-reset", soon(5400)),
                ("anthropic-ratelimit-unified-7d-reset", soon(4 * 86400)),
                ("anthropic-ratelimit-unified-representative-claim", "five_hour".into()),
            ],
            anthropic("rate_limit_error", "This request would exceed your account's rate limit. Please try again later."),
        ),
        ("anthropic", "auth") => (401, Vec::new(), anthropic("authentication_error", "invalid x-api-key")),
        ("anthropic", _) => (403, Vec::new(), anthropic("permission_error", "Your account does not have access to this model.")),
        ("openai", "ok") => {
            let piece = |delta: Value, finish: Value| json!({ "id": "chatcmpl-standin", "object": "chat.completion.chunk", "created": 0, "model": model, "choices": [{ "index": 0, "delta": delta, "finish_reason": finish }] });
            let mut last = piece(json!({}), json!("stop"));
            last["usage"] = json!({ "prompt_tokens": 10, "completion_tokens": 1, "total_tokens": 11 });
            (200, Vec::new(), format!("data: {}\n\ndata: {last}\n\ndata: [DONE]\n\n", piece(json!({ "role": "assistant", "content": "pong" }), Value::Null)))
        }
        ("openai", "limit") => (429, Vec::new(), openai("You exceeded your current quota, please check your plan and billing details.".into(), "insufficient_quota", "insufficient_quota")),
        ("openai", "auth") => (401, Vec::new(), openai("Incorrect API key provided.".into(), "invalid_request_error", "invalid_api_key")),
        ("openai", _) => (403, Vec::new(), openai(format!("You do not have access to model {model}."), "invalid_request_error", "model_not_found")),
        ("codex", "limit") => (
            429,
            vec![
                ("x-codex-primary-used-percent", "100".into()),
                ("x-codex-primary-window-minutes", "300".into()),
                ("x-codex-primary-reset-at", soon(5400)),
                ("x-codex-secondary-used-percent", "43".into()),
                ("x-codex-secondary-window-minutes", "10080".into()),
                ("x-codex-secondary-reset-at", soon(4 * 86400)),
            ],
            json!({ "error": { "type": "usage_limit_reached", "message": "The usage limit has been reached", "plan_type": "plus", "resets_at": soon(5400).parse::<u64>().unwrap_or(0) } }).to_string(),
        ),
        ("codex", "auth") => (
            401,
            Vec::new(),
            json!({ "error": { "message": "Your authentication token has been invalidated. Please try signing in again.", "type": "invalid_request_error", "code": "token_invalidated" } }).to_string(),
        ),
        // The ChatGPT sign-in has no stand-in that replies: only its refusals differ from the others.
        _ => (403, Vec::new(), json!({ "detail": format!("The '{model}' model is not available on your plan.") }).to_string()),
    }
}

/// Every word of one message of a request, whatever it is made of: plain
/// text, parts, or what a tool gave back.
pub fn words(message: &Value) -> String {
    fn gather(part: &Value, words: &mut String) {
        match part {
            Value::String(text) => {
                words.push_str(text);
                words.push('\n');
            }
            Value::Array(parts) => parts.iter().for_each(|part| gather(part, words)),
            Value::Object(part) => ["text", "content"].iter().filter_map(|field| part.get(*field)).for_each(|inner| gather(inner, words)),
            _ => {}
        }
    }
    let mut words = String::new();
    gather(&message["content"], &mut words);
    words
}

/// The tool a model handed a file would call to read it, and with what: the
/// harness's own tool for reading, given the path the message names. None when
/// no file was handed over, when the tool has already answered, or when the
/// harness offers no such tool.
fn reading(said: &Value) -> Option<(String, Value)> {
    let last = said.get("messages")?.as_array()?.last()?;
    if last["content"].as_array().is_some_and(|parts| parts.iter().any(|part| part["type"] == "tool_result")) {
        return None;
    }
    let told = words(last);
    let file = crate::attach::split(told.trim_end()).1.into_iter().next()?;
    let tool = said.get("tools")?.as_array()?.iter().find(|tool| tool["name"].as_str().is_some_and(|name| name.trim_start_matches('_') == "read"))?;
    // Whatever else the tool insists on is a word about why.
    let mut input = serde_json::Map::new();
    input.insert("path".into(), json!(file));
    for wanted in tool.pointer("/input_schema/required").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
        input.entry(wanted).or_insert(json!("read the attached file"));
    }
    Some((tool["name"].as_str()?.to_string(), Value::Object(input)))
}

/// `NULL_MINI_STANDIN`: start the stand-in and offer its models to the harness of
/// this run. The run has to be on a folder of its own (`NULL_MINI_FOLDER`), so
/// that the stand-in's models are never written among the user's own.
pub fn for_this_run() -> Result<(), String> {
    if std::env::var_os("NULL_MINI_STANDIN").is_none() {
        return Ok(());
    }
    let Some(folder) = settings::harness_folder() else {
        return Err("NULL_MINI_STANDIN needs NULL_MINI_FOLDER: the stand-in's models go in a folder of the run's own, never among the user's".into());
    };
    let file = folder.join("models.yml");
    // A file the stand-in did not write is somebody's own models, and is left alone.
    if std::fs::read_to_string(&file).is_ok_and(|there| !there.starts_with(MARK)) {
        return Err(format!("{} is not the stand-in's own file, so NULL_MINI_STANDIN left it alone", file.display()));
    }
    let standin = StandIn::start();
    std::fs::write(&file, standin.models_yml()).map_err(|e| format!("could not write {}: {e}", file.display()))?;
    log!("the stand-in provider answers on port {}; its models are offered in {}", standin.port, file.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_it_offers_is_marked_as_its_own_and_names_every_model() {
        let standin = StandIn::start();
        let text = standin.models_yml();
        assert!(text.starts_with(MARK), "{text}");
        for (kind, _) in KINDS {
            assert!(text.contains(&format!("  standin-{kind}:\n    baseUrl: http://127.0.0.1:{}/", standin.port)), "{kind}: {text}");
        }
        assert_eq!(text.matches("      - { id: ").count(), KINDS.len() * ANSWERS.len(), "{text}");
    }
}
