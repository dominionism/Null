//! The harness check: what Null depends on, asked of a real Oh-my-pi.
//!
//! Null moves to a newer Oh-my-pi only after that version has answered these
//! questions as the one it carries does (`Mini/Scripts/engine --to <version>`).
//! Each question is one test, named for what Null relies on, so a failure says
//! which thing a version changed. They start the real program, so they run only
//! when asked: `cargo test -- --ignored`, on the program Null carries or, with
//! `NULL_MINI_ENGINE=<path>`, on any other.
//!
//! Nothing here touches the user's own Oh-my-pi folder or reaches a real
//! provider. Each question gets a folder of its own, thrown away afterwards,
//! and the provider is a stand-in on this Mac that answers by the name of the
//! model asked for: `ok` replies, `limit` is used up, `auth` is a bad sign-in
//! and `noaccess` is a model the account may not use. Its answers are close
//! copies of what the three kinds of provider the owner uses send, not
//! recordings, so the first real limit is still worth setting beside them.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::{backups, engine, harness, translate};

/// How long one answer from the harness is waited for. It tries a provider that
/// refuses several times before it gives up, which takes a quarter of a minute.
const PATIENCE: Duration = Duration::from_secs(90);

// ── The stand-in provider ─────────────────────────────────────────────────

/// The three ways of talking the owner's providers use: the name in a stand-in
/// address, and what the harness calls that way of talking.
const KINDS: [(&str, &str); 3] = [("anthropic", "anthropic-messages"), ("openai", "openai-completions"), ("codex", "openai-codex-responses")];
const ANSWERS: [&str; 4] = ["ok", "limit", "auth", "noaccess"];

struct StandIn {
    port: u16,
    /// Every model a message was sent to, in order, as `kind/answer`.
    asked: Arc<Mutex<Vec<String>>>,
    /// Every address a harness tried to reach through the stand-in, when it was
    /// set as that harness's way out (`Folder::behind`). Nothing is let through.
    reached: Arc<Mutex<Vec<String>>>,
}

impl StandIn {
    fn start() -> StandIn {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port on this Mac");
        let port = listener.local_addr().expect("the port it was given").port();
        let (asked, reached) = (Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Vec::new())));
        let record = (asked.clone(), reached.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let record = record.clone();
                std::thread::spawn(move || answer(stream, &record.0, &record.1));
            }
        });
        StandIn { port, asked, reached }
    }

    /// The file that offers the stand-in's models to a harness, as `standin-<kind>/<answer>`.
    fn models_yml(&self) -> String {
        let mut text = String::from("providers:\n");
        for (kind, api) in KINDS {
            let path = if kind == "openai" { "openai/v1" } else { kind };
            text.push_str(&format!("  standin-{kind}:\n    baseUrl: http://127.0.0.1:{}/{path}\n    api: {api}\n    apiKey: standin-not-a-key\n    models:\n", self.port));
            for answer in ANSWERS {
                text.push_str(&format!("      - {{ id: {answer}, name: Stand-in {answer}, contextWindow: 128000, maxTokens: 8192 }}\n"));
            }
        }
        text
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }

    fn reached(&self) -> Vec<String> {
        self.reached.lock().unwrap().clone()
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

fn answer(mut stream: TcpStream, asked: &Mutex<Vec<String>>, reached: &Mutex<Vec<String>>) {
    let Some((method, path, body)) = read_request(&mut stream) else { return };
    // Asked to pass a request on to somewhere else: write down where, and refuse.
    if method == "CONNECT" || path.starts_with("http") {
        reached.lock().unwrap().push(path);
        let _ = stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\nconnection: close\r\ncontent-length: 0\r\n\r\n");
        return;
    }
    let kind = path.split('/').nth(1).unwrap_or_default();
    let model = serde_json::from_slice::<Value>(&body).ok().and_then(|body| Some(body.get("model")?.as_str()?.to_string())).unwrap_or_default();
    let which = model.rsplit('/').next().unwrap_or_default();
    let (status, headers, body) = match (method.as_str(), which) {
        ("POST", "ok" | "limit" | "auth" | "noaccess") => {
            asked.lock().unwrap().push(format!("{kind}/{which}"));
            reply(kind, which, &model)
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
fn reply(kind: &str, which: &str, model: &str) -> (u16, Vec<(&'static str, String)>, String) {
    let soon = |seconds: u64| (SystemTime::now().duration_since(UNIX_EPOCH).map(|now| now.as_secs()).unwrap_or(0) + seconds).to_string();
    let anthropic = |kind_of_error: &str, message: &str| json!({ "type": "error", "error": { "type": kind_of_error, "message": message }, "request_id": "req_standin" }).to_string();
    let openai = |message: String, kind_of_error: &str, code: &str| json!({ "error": { "message": message, "type": kind_of_error, "param": null, "code": code } }).to_string();
    let events = |frames: &[(&str, Value)]| frames.iter().map(|(event, data)| format!("event: {event}\ndata: {data}\n\n")).collect::<String>();
    match (kind, which) {
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

// ── A harness, started as Null starts it ──────────────────────────────────

/// A harness folder of the check's own, with the stand-in's models in it. It
/// is thrown away when the question is over.
struct Folder {
    root: PathBuf,
    /// The port everything the harness sends goes to, when it is kept behind the stand-in.
    way_out: Option<u16>,
}

impl Folder {
    fn new(standin: &StandIn) -> Folder {
        static MADE: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!("null-harness-check-{}-{}", std::process::id(), MADE.fetch_add(1, Ordering::Relaxed)));
        let folder = Folder { root, way_out: None };
        std::fs::create_dir_all(folder.settings()).unwrap();
        std::fs::create_dir_all(folder.work()).unwrap();
        std::fs::write(folder.settings().join("models.yml"), standin.models_yml()).unwrap();
        folder
    }

    /// Send everything a harness on this folder tries to reach to the stand-in,
    /// which writes the address down and lets nothing through.
    fn behind(mut self, standin: &StandIn) -> Folder {
        self.way_out = Some(standin.port);
        self
    }

    /// What the harness keeps: sign-ins, settings, conversations.
    fn settings(&self) -> PathBuf {
        self.root.join("agent")
    }

    /// Where a conversation works.
    fn work(&self) -> PathBuf {
        self.root.join("work")
    }

    /// The settings Null hands the harness at start, in a file as Null writes it.
    fn handed(&self, backups: Option<Value>) -> PathBuf {
        let file = self.root.join("harness.yml");
        std::fs::write(&file, harness::own_settings(backups).to_string()).unwrap();
        file
    }

    /// One of the harness's own commands, run on this folder and not the user's.
    fn command(&self, binary: &Path) -> Command {
        let mut command = Command::new(binary);
        command.env("PI_CODING_AGENT_DIR", self.settings());
        if let Some(port) = self.way_out {
            let stand_in = format!("http://127.0.0.1:{port}");
            command.envs([("HTTPS_PROXY", &stand_in), ("HTTP_PROXY", &stand_in), ("https_proxy", &stand_in), ("http_proxy", &stand_in)]);
            command.envs([("NO_PROXY", ""), ("no_proxy", "")]);
        }
        command
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct Agent {
    child: Child,
    stdin: ChildStdin,
    said: Receiver<Value>,
    asked: u64,
    /// What the harness answered when greeted.
    hello: Value,
    /// Every update of a conversation heard so far, oldest first.
    updates: Vec<Value>,
}

impl Agent {
    /// Start `binary` on `folder` with the settings Null hands over at start,
    /// and greet it. `backups` is the backup order in the harness's own form.
    fn start(binary: &Path, folder: &Folder, backups: Option<Value>) -> Agent {
        let mut child = folder
            .command(binary)
            .args(["--approval-mode", "yolo", "--config"])
            .arg(folder.handed(backups))
            .arg("acp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("{} did not start: {e}", binary.display()));
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (heard, said) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(message) = serde_json::from_str::<Value>(&line) {
                    if heard.send(message).is_err() {
                        break;
                    }
                }
            }
        });
        let mut agent = Agent { child, stdin, said, asked: 0, hello: Value::Null, updates: Vec::new() };
        let greeting = json!({ "protocolVersion": 1, "clientCapabilities": {}, "clientInfo": { "name": "null-mini", "title": "Null", "version": env!("CARGO_PKG_VERSION") } });
        agent.hello = agent.request("initialize", greeting).expect("the harness answers a greeting");
        agent
    }

    /// Send one request and wait for its answer: the result, or the error the harness gave.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, Value> {
        self.asked += 1;
        let id = self.asked;
        writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })).expect("the harness is listening");
        let until = Instant::now() + PATIENCE;
        loop {
            let message = match self.said.recv_timeout(until.saturating_duration_since(Instant::now())) {
                Ok(message) => message,
                Err(mpsc::RecvTimeoutError::Timeout) => panic!("no answer to {method} in {} s", PATIENCE.as_secs()),
                Err(mpsc::RecvTimeoutError::Disconnected) => panic!("the harness went away before it answered {method}"),
            };
            match (message.get("method").and_then(Value::as_str), message.get("id")) {
                (None, Some(answered)) if *answered == json!(id) => {
                    return match message.get("error") {
                        Some(error) => Err(error.clone()),
                        None => Ok(message["result"].clone()),
                    };
                }
                (Some("session/update"), _) => self.updates.push(message["params"]["update"].clone()),
                // Something the harness asks of Null. No question here needs one answered.
                (Some(_), Some(theirs)) => {
                    let _ = writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "id": theirs, "error": { "code": -32601, "message": "the check answers no requests" } }));
                }
                _ => {}
            }
        }
    }

    /// Open a conversation as Null does. Gives its id and its settings.
    fn open(&mut self, folder: &Folder) -> (String, Value) {
        let opened = self.request("session/new", json!({ "cwd": folder.work(), "mcpServers": [] })).expect("a conversation opens");
        (opened["sessionId"].as_str().expect("a conversation has an id").to_string(), opened["configOptions"].clone())
    }

    /// Load a conversation back, as Null does after the harness was started again.
    fn load(&mut self, folder: &Folder, session: &str) -> Result<Value, Value> {
        self.request("session/load", json!({ "sessionId": session, "cwd": folder.work(), "mcpServers": [] }))
    }

    /// Set one setting of the conversation. Gives the settings as they are afterwards.
    fn set(&mut self, session: &str, setting: &str, value: Value) -> Value {
        let answered = self.request("session/set_config_option", json!({ "sessionId": session, "configId": setting, "value": value }));
        answered.unwrap_or_else(|error| panic!("{setting} could not be set to {value}: {error}"))["configOptions"].clone()
    }

    /// Send a message. Gives how the reply ended and the words of it, or the
    /// error when the harness answered the message with one.
    fn say(&mut self, session: &str, text: &str) -> Result<(Value, String), Value> {
        let before = self.updates.len();
        let ended = self.request("session/prompt", json!({ "sessionId": session, "prompt": [{ "type": "text", "text": text }] }))?;
        let mut tools = HashMap::new();
        let words = self.updates[before..]
            .iter()
            .filter_map(|update| translate::event_from_update(update, &mut tools))
            .filter(|event| event["type"] == "text_delta" && event["thinking"] == false)
            .filter_map(|event| event["text"].as_str().map(str::to_string))
            .collect();
        Ok((ended, words))
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A conversation on the program under test, on one of the stand-in's models.
/// Gives the settings as they were once the model was set.
fn conversation_on(model: &str, backups: Option<Value>) -> (StandIn, Folder, Agent, String, Value) {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let mut agent = Agent::start(&engine::under_test(), &folder, backups);
    let (session, _) = agent.open(&folder);
    let settings = agent.set(&session, "model", json!(model));
    (standin, folder, agent, session, settings)
}

const MESSAGE: &str = "Reply with exactly one word: pong";

fn order(models: &[&str]) -> Vec<String> {
    models.iter().map(|model| model.to_string()).collect()
}

fn checksum(program: &Path) -> String {
    let output = Command::new("/usr/bin/shasum").args(["-a", "256"]).arg(program).output().expect("shasum runs");
    String::from_utf8_lossy(&output.stdout).split_whitespace().next().expect("a checksum").to_string()
}

// ── The questions ─────────────────────────────────────────────────────────

/// Null speaks version 1 of the protocol, loads a conversation back after the
/// harness was started again, lists models from the conversation's model
/// setting, and asks which model is in use through another setting.
#[test]
#[ignore]
fn a_conversation_opens_on_protocol_version_1_with_a_model_setting() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let mut agent = Agent::start(&engine::under_test(), &folder, None);
    assert_eq!(agent.hello["protocolVersion"], 1, "the harness answered the greeting with {}", agent.hello);
    assert_eq!(agent.hello.pointer("/agentCapabilities/loadSession"), Some(&json!(true)), "the harness does not load a conversation back");

    let (_, settings) = agent.open(&folder);
    let (models, current) = translate::models_from_config_options(&settings);
    assert!(models.iter().any(|model| model.id == "standin-anthropic/ok"), "the model setting does not offer the folder's own models: {models:?}");
    assert!(current.is_some(), "the model setting does not say which model is in use");
    assert!(translate::other_option(&settings).is_some(), "there is no other setting to ask which model is in use with: {settings}");
}

/// A reply is told from a provider's refusal by its token count, so a working
/// reply has to carry one.
#[test]
#[ignore]
fn a_working_reply_counts_its_tokens() {
    for kind in ["anthropic", "openai"] {
        let (_standin, _folder, mut agent, session, _) = conversation_on(&format!("standin-{kind}/ok"), None);
        let (ended, words) = agent.say(&session, MESSAGE).unwrap_or_else(|error| panic!("{kind}: the message was answered with an error: {error}"));
        assert_eq!(words.trim(), "pong", "{kind}: the reply's words");
        assert_eq!(ended["stopReason"], "end_turn", "{kind}: {ended}");
        assert!(!translate::reply_failed(&ended), "{kind}: a working reply was taken for a refusal: {ended}");
    }
}

/// A used-up limit, a bad sign-in and a refused model never come back as an
/// error. Each is a reply like any other, with the provider's words as its
/// text, and all that sets it apart is that it counts no tokens.
fn refusals_are_replies_that_count_no_tokens(kind: &str) {
    for which in ["limit", "auth", "noaccess"] {
        let (_standin, _folder, mut agent, session, _) = conversation_on(&format!("standin-{kind}/{which}"), None);
        let (ended, words) = agent.say(&session, MESSAGE).unwrap_or_else(|error| panic!("{kind}/{which}: the message was answered with an error, not a reply: {error}"));
        assert!(!words.trim().is_empty(), "{kind}/{which}: the provider's words did not come back: {ended}");
        assert!(translate::reply_failed(&ended), "{kind}/{which}: a refusal was taken for a working reply: {ended}");
    }
}

#[test]
#[ignore]
fn refusals_in_anthropic_s_way_of_talking_are_replies_that_count_no_tokens() {
    refusals_are_replies_that_count_no_tokens("anthropic");
}

#[test]
#[ignore]
fn refusals_in_openai_s_way_of_talking_are_replies_that_count_no_tokens() {
    refusals_are_replies_that_count_no_tokens("openai");
}

#[test]
#[ignore]
fn refusals_in_chatgpt_s_way_of_talking_are_replies_that_count_no_tokens() {
    refusals_are_replies_that_count_no_tokens("codex");
}

/// Null hands over a backup order and leaves the switching to the harness: it
/// has to move to the backup, send the same message again, and not go back to
/// the model that failed for the next message.
#[test]
#[ignore]
fn a_backup_takes_over_and_the_failed_model_is_not_tried_again() {
    let backups = backups::overlay(&order(&["standin-openai/ok"]), &Value::Null);
    let (standin, _folder, mut agent, session, _) = conversation_on("standin-anthropic/limit", backups);

    let (ended, words) = agent.say(&session, MESSAGE).expect("the message is answered");
    assert_eq!(words.trim(), "pong", "the backup's reply");
    assert!(!translate::reply_failed(&ended), "the backup's reply was taken for a refusal: {ended}");
    let first = standin.asked();
    let failed = first.iter().position(|model| model == "anthropic/limit").unwrap_or_else(|| panic!("the model in use was never asked: {first:?}"));
    assert!(first[failed..].iter().any(|model| model == "openai/ok"), "the message was not sent again to the backup: {first:?}");

    agent.say(&session, MESSAGE).expect("the next message is answered");
    let second = standin.asked()[first.len()..].to_vec();
    assert!(second.iter().any(|model| model == "openai/ok") && !second.iter().any(|model| model == "anthropic/limit"), "the next message went to {second:?}");
}

/// The harness does not always say that it moved to a backup, so Null asks
/// after every reply: it sets another setting to the value it already has, and
/// the answer has to name the model now in use.
#[test]
#[ignore]
fn the_model_in_use_is_answered_when_asked() {
    let backups = backups::overlay(&order(&["standin-openai/ok"]), &Value::Null);
    let (_standin, _folder, mut agent, session, settings) = conversation_on("standin-anthropic/limit", backups);
    let (setting, value) = translate::other_option(&settings).expect("another setting to ask with");
    agent.say(&session, MESSAGE).expect("the message is answered");

    let (_, current) = translate::models_from_config_options(&agent.set(&session, &setting, value));
    assert_eq!(current.as_deref(), Some("standin-openai/ok"), "asked through {setting}");
}

/// Null reads the backup lists the user already has and puts its own order
/// ahead of each, so that a list for one provider does not cost a round of
/// tries before Null's first backup is reached.
#[test]
#[ignore]
fn null_s_backups_go_ahead_of_a_list_the_user_already_has() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let binary = engine::under_test();
    std::fs::write(folder.settings().join("config.yml"), "retry:\n  fallbackChains:\n    \"standin-anthropic/*\": [\"standin-anthropic/auth\"]\n").unwrap();

    let printed = folder.command(&binary).args(["config", "get", "retry.fallbackChains", "--json"]).output().expect("the harness runs");
    let own = backups::lists_read(&printed.stdout);
    assert_eq!(own, json!({ "standin-anthropic/*": ["standin-anthropic/auth"] }), "the user's own lists, as the harness printed them: {}", String::from_utf8_lossy(&printed.stdout));

    let mut agent = Agent::start(&binary, &folder, backups::overlay(&order(&["standin-openai/ok"]), &own));
    let (session, _) = agent.open(&folder);
    agent.set(&session, "model", json!("standin-anthropic/limit"));
    let (_, words) = agent.say(&session, MESSAGE).expect("the message is answered");
    assert_eq!(words.trim(), "pong", "the backup's reply");
    assert!(!standin.asked().iter().any(|model| model == "anthropic/auth"), "the user's own list was tried before Null's backup: {:?}", standin.asked());
}

/// Null carries one version, so it tells the harness not to look for a newer
/// one. The harness's own `config get` does not show a setting handed over for
/// one run, so the effect cannot be read back. What can be asked is that the
/// setting still exists under that name as a yes or no, since a setting that
/// was renamed would be handed over and quietly ignored.
#[test]
#[ignore]
fn the_setting_that_stops_the_update_check_is_still_a_yes_or_no() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let printed = folder.command(&engine::under_test()).args(["config", "get", "startup.checkUpdate", "--json"]).output().expect("the harness runs");
    let read = serde_json::from_slice::<Value>(&printed.stdout).unwrap_or(Value::Null);
    assert!(read["value"].is_boolean(), "startup.checkUpdate, as the harness printed it: {}", String::from_utf8_lossy(&printed.stdout));
}

/// And that the harness, started as Null starts it, does not go looking: it is
/// kept behind the stand-in, which writes down every address it tries to reach.
/// It asks several places for their model lists at start, which shows that the
/// writing down works; none of them may be where its own releases are.
#[test]
#[ignore]
fn started_as_null_starts_it_the_harness_does_not_look_for_a_newer_version() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin).behind(&standin);
    let mut agent = Agent::start(&engine::under_test(), &folder, None);
    agent.open(&folder);

    let outside = |reached: &[String]| reached.iter().any(|address| !address.contains("127.0.0.1") && !address.contains("localhost"));
    let asked = Instant::now();
    while !outside(&standin.reached()) {
        assert!(asked.elapsed() < Duration::from_secs(30), "the harness reached for nothing outside this Mac, so this can no longer be told: {:?}", standin.reached());
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_secs(5)); // whatever else it does at start
    let reached = standin.reached();
    let looked: Vec<&String> = reached.iter().filter(|address| address.contains("github") || address.contains("npmjs")).collect();
    assert!(looked.is_empty(), "the harness went looking for its own releases: {looked:?}");
}

/// The program Null carries has to stay the one it chose: a harness that
/// replaced itself with a newer version would undo the choice.
#[test]
#[ignore]
fn the_program_is_the_same_after_answering() {
    let binary = engine::under_test();
    let before = checksum(&binary);
    {
        let (_standin, _folder, mut agent, session, _) = conversation_on("standin-anthropic/ok", None);
        agent.say(&session, MESSAGE).expect("the message is answered");
    }
    assert_eq!(checksum(&binary), before, "{} changed while it ran", binary.display());
}

/// Null's harness and the user's own share one folder, and a new Null brings a
/// new version to a folder the old one used. So the version Null carries and
/// the one under test take turns on one conversation.
#[test]
#[ignore]
fn two_versions_take_turns_on_one_folder() {
    let (carried, other) = (engine::carried().expect("Mini/Scripts/engine has fetched Oh-my-pi"), engine::under_test());
    if carried == other {
        eprintln!("only the version Null carries is here; set NULL_MINI_ENGINE to another to ask this");
        return;
    }
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let session = {
        let mut agent = Agent::start(&carried, &folder, None);
        let (session, _) = agent.open(&folder);
        agent.set(&session, "model", json!("standin-anthropic/ok"));
        agent.say(&session, MESSAGE).expect("the carried version answers");
        session
    };
    for (whose, binary) in [("the version under test", &other), ("the version Null carries", &carried)] {
        let mut agent = Agent::start(binary, &folder, None);
        agent.load(&folder, &session).unwrap_or_else(|error| panic!("{whose} could not load the other's conversation: {error}"));
        let (ended, words) = agent.say(&session, MESSAGE).unwrap_or_else(|error| panic!("{whose} answered with an error: {error}"));
        assert_eq!(words.trim(), "pong", "{whose}: {ended}");
    }
}
