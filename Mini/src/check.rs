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
//! model asked for: `ok` replies, `limit` is used up, `auth` is a bad sign-in,
//! `noaccess` is a model the account may not use, and `slow` says back what it
//! was sent a word at a time, so that a reply is still under way while the
//! next thing is asked (`standin.rs`).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::history;
use crate::standin::{self, StandIn};
use crate::{attach, backups, browser, engine, harness, search, translate};

/// How long one answer from the harness is waited for. It tries a provider that
/// refuses several times before it gives up, which takes a quarter of a minute.
const PATIENCE: Duration = Duration::from_secs(90);

// ── A harness, started as Null starts it ──────────────────────────────────

/// A harness folder of the check's own, with the stand-in's models in it. It
/// is thrown away when the question is over.
struct Folder {
    root: PathBuf,
    /// The port everything the harness sends goes to, when it is kept behind the stand-in.
    way_out: Option<u16>,
    /// Whether the harness keeps its services here too, and not beside the user's.
    own_services: bool,
}

impl Folder {
    fn new(standin: &StandIn) -> Folder {
        static MADE: AtomicU32 = AtomicU32::new(0);
        let root = std::env::temp_dir().join(format!("null-harness-check-{}-{}", std::process::id(), MADE.fetch_add(1, Ordering::Relaxed)));
        let folder = Folder { root, way_out: None, own_services: false };
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

    /// Keep the harness's background services, its relay's among them, in this
    /// folder too, and away from the ones a user's own harness runs.
    fn with_own_services(mut self) -> Folder {
        std::fs::create_dir_all(self.root.join("state/omp")).unwrap();
        self.own_services = true;
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
    fn handed(&self, more: Option<Value>) -> PathBuf {
        let file = self.root.join("harness.yml");
        std::fs::write(&file, harness::own_settings(more).to_string()).unwrap();
        file
    }

    /// One of the harness's own commands, run on this folder and not the user's.
    fn command(&self, binary: &Path) -> Command {
        let mut command = Command::new(binary);
        command.env("PI_CODING_AGENT_DIR", self.settings());
        if self.own_services {
            command.env("XDG_STATE_HOME", self.root.join("state"));
        }
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
    /// The conversation each of those updates came from, in the same order.
    from: Vec<String>,
    /// The answers that came while another was waited for, by the number of their request.
    answers: HashMap<u64, Result<Value, Value>>,
    /// Every question the harness put to Null before a tool acted, oldest first.
    questions: Vec<Value>,
    /// How such a question is answered: with the answer of this kind among those
    /// offered, as the protocol names the kinds. Nothing here refuses to answer at all.
    answering: Option<&'static str>,
}

impl Agent {
    /// Start `binary` on `folder` with the settings Null hands over at start,
    /// and greet it. `more` is what Null adds to those for this run, in the
    /// harness's own form: a backup order, or computer use switched on.
    fn start(binary: &Path, folder: &Folder, more: Option<Value>) -> Agent {
        Agent::start_set_to(binary, folder, more, "yolo")
    }

    /// The same, with the harness set to do this about asking before a tool
    /// acts, as Null passes on what the person set: "yolo" acts freely,
    /// "always-ask" asks every time.
    fn start_set_to(binary: &Path, folder: &Folder, more: Option<Value>, asking: &str) -> Agent {
        Agent::start_with(binary, folder, more, asking, &[])
    }

    /// The same, with flags of Null's own on the start line before `acp`.
    fn start_with(binary: &Path, folder: &Folder, more: Option<Value>, asking: &str, flags: &[&str]) -> Agent {
        let mut child = folder
            .command(binary)
            .args(flags)
            .args(["--approval-mode", asking, "--config"])
            .arg(folder.handed(more))
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
        let mut agent = Agent { child, stdin, said, asked: 0, hello: Value::Null, updates: Vec::new(), from: Vec::new(), answers: HashMap::new(), questions: Vec::new(), answering: None };
        let greeting = json!({ "protocolVersion": 1, "clientCapabilities": {}, "clientInfo": { "name": "null-mini", "title": "Null", "version": env!("CARGO_PKG_VERSION") } });
        agent.hello = agent.request("initialize", greeting).expect("the harness answers a greeting");
        agent
    }

    /// Send one request and wait for its answer: the result, or the error the harness gave.
    fn request(&mut self, method: &str, params: Value) -> Result<Value, Value> {
        let sent = self.send(method, params);
        self.answer(sent, method)
    }

    /// Send one request and do not wait, so that another can be on its way at
    /// the same time. Gives the number its answer is asked for with.
    fn send(&mut self, method: &str, params: Value) -> u64 {
        self.asked += 1;
        writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "id": self.asked, "method": method, "params": params })).expect("the harness is listening");
        self.asked
    }

    /// Wait for the answer to a request sent earlier.
    fn answer(&mut self, sent: u64, method: &str) -> Result<Value, Value> {
        let until = Instant::now() + PATIENCE;
        loop {
            if let Some(answer) = self.answers.remove(&sent) {
                return answer;
            }
            self.hear(until, &format!("answer to {method}"));
        }
    }

    /// Go on listening until `come` says that what was waited for has been heard.
    fn hear_until(&mut self, awaited: &str, come: impl Fn(&Agent) -> bool) {
        let until = Instant::now() + PATIENCE;
        while !come(self) {
            self.hear(until, awaited);
        }
    }

    /// Take in the next thing the harness says: an update is kept with the
    /// conversation it came from, and an answer with the number of its request.
    fn hear(&mut self, until: Instant, awaited: &str) {
        let message = match self.said.recv_timeout(until.saturating_duration_since(Instant::now())) {
            Ok(message) => message,
            Err(mpsc::RecvTimeoutError::Timeout) => panic!("no {awaited} in {} s", PATIENCE.as_secs()),
            Err(mpsc::RecvTimeoutError::Disconnected) => panic!("the harness went away with no {awaited}"),
        };
        self.take(message);
    }

    /// Wait for the answer to a request sent earlier, but no later than
    /// `deadline`, however much the harness says meanwhile. None when it did not come.
    fn answer_by(&mut self, sent: u64, deadline: Instant) -> Option<Result<Value, Value>> {
        loop {
            if let Some(answer) = self.answers.remove(&sent) {
                return Some(answer);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            match self.said.recv_timeout(left.min(Duration::from_secs(1))) {
                Ok(message) => self.take(message),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return None,
            }
        }
    }

    /// Keep one thing the harness said, or answer it.
    fn take(&mut self, message: Value) {
        match (message.get("method").and_then(Value::as_str), message.get("id")) {
            (None, Some(answered)) => {
                let answer = match message.get("error") {
                    Some(error) => Err(error.clone()),
                    None => Ok(message["result"].clone()),
                };
                if let Some(sent) = answered.as_u64() {
                    self.answers.insert(sent, answer);
                }
            }
            (Some("session/update"), _) => {
                self.from.push(message["params"]["sessionId"].as_str().unwrap_or_default().to_string());
                self.updates.push(message["params"]["update"].clone());
            }
            // The harness asks before a tool acts. It is answered when the question at hand says how.
            (Some("session/request_permission"), Some(theirs)) if self.answering.is_some() => {
                let asked = message["params"].clone();
                let chosen = asked["options"].as_array().into_iter().flatten().find(|option| option["kind"].as_str() == self.answering).map(|option| option["optionId"].clone());
                let answer = match chosen {
                    Some(id) => json!({ "jsonrpc": "2.0", "id": theirs, "result": { "outcome": { "outcome": "selected", "optionId": id } } }),
                    None => json!({ "jsonrpc": "2.0", "id": theirs, "result": { "outcome": { "outcome": "cancelled" } } }),
                };
                self.questions.push(asked);
                let _ = writeln!(self.stdin, "{answer}");
            }
            // Anything else the harness asks of Null. No question here needs one answered.
            (Some(_), Some(theirs)) => {
                let _ = writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "id": theirs, "error": { "code": -32601, "message": "the check answers no requests" } }));
            }
            _ => {}
        }
    }

    /// The updates of one conversation among those heard from `since` on.
    fn heard(&self, session: &str, since: usize) -> Vec<Value> {
        self.updates[since..].iter().zip(&self.from[since..]).filter(|(_, from)| from.as_str() == session).map(|(update, _)| update.clone()).collect()
    }

    /// The words of one conversation's replies heard from `since` on, in the pieces they came in.
    fn pieces(&self, session: &str, since: usize) -> Vec<String> {
        self.heard(session, since).iter().filter_map(words_in).collect()
    }

    /// Whose reply was heard from `since` on, and whose after it: each
    /// conversation named again every time the word passes to it.
    fn turns(&self, since: usize) -> Vec<String> {
        let mut turns: Vec<String> = self.updates[since..].iter().zip(&self.from[since..]).filter(|(update, _)| words_in(update).is_some()).map(|(_, from)| from.clone()).collect();
        turns.dedup();
        turns
    }

    /// Open a conversation as Null does. Gives its id and its settings.
    fn open(&mut self, folder: &Folder) -> (String, Value) {
        self.open_in(&folder.work())
    }

    /// Open a conversation that works in a folder of the question's choosing.
    fn open_in(&mut self, work: &Path) -> (String, Value) {
        let opened = self.request("session/new", json!({ "cwd": work, "mcpServers": [] })).expect("a conversation opens");
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
        self.hand(session, text, &[])
    }

    /// Send a message with files attached, as Null hands them over.
    fn hand(&mut self, session: &str, text: &str, files: &[PathBuf]) -> Result<(Value, String), Value> {
        let before = self.updates.len();
        let ended = self.request("session/prompt", json!({ "sessionId": session, "prompt": attach::blocks(text, files) }))?;
        Ok((ended, self.pieces(session, before).concat()))
    }

    /// Send a message and do not wait for the reply, so that another
    /// conversation can be answering at the same time. Gives the number that
    /// `ended` is asked with.
    fn begin(&mut self, session: &str, text: &str) -> u64 {
        self.send("session/prompt", json!({ "sessionId": session, "prompt": attach::blocks(text, &[]) }))
    }

    /// Wait for a reply begun earlier to be over. Gives how it ended.
    fn ended(&mut self, begun: u64) -> Result<Value, Value> {
        self.answer(begun, "session/prompt")
    }

    /// Stop a conversation's reply, as Ctrl+C in the box does. The harness answers nothing to it.
    fn stop(&mut self, session: &str) {
        writeln!(self.stdin, "{}", json!({ "jsonrpc": "2.0", "method": "session/cancel", "params": { "sessionId": session } })).expect("the harness is listening");
    }

    /// Every conversation the harness keeps, a page of its list at a time, as Null reads them.
    fn conversations(&mut self) -> Vec<Value> {
        let mut listed = Vec::new();
        let mut next: Option<String> = None;
        loop {
            let asked = next.as_ref().map_or(json!({}), |next| json!({ "cursor": next }));
            let answer = self.request("session/list", asked).unwrap_or_else(|error| panic!("the harness did not list its conversations: {error}"));
            let (kept, more) = history::page(&answer);
            assert!(!kept.is_empty() || more.is_none(), "a page with nothing on it says there is another: {answer}");
            listed.extend(kept);
            next = more;
            if next.is_none() {
                return listed;
            }
        }
    }
}

/// The words an update adds to a reply, when it adds any. The model's thinking is not the reply.
fn words_in(update: &Value) -> Option<String> {
    let event = translate::event_from_update(update, &mut HashMap::new())?;
    (event["type"] == "text_delta" && event["thinking"] == false).then(|| event["text"].as_str().map(str::to_string))?
}

impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A conversation on the program under test, on one of the stand-in's models.
/// Gives the settings as they were once the model was set.
fn conversation_on(model: &str, more: Option<Value>) -> (StandIn, Folder, Agent, String, Value) {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let mut agent = Agent::start(&engine::under_test(), &folder, more);
    let (session, _) = agent.open(&folder);
    let settings = agent.set(&session, "model", json!(model));
    (standin, folder, agent, session, settings)
}

const MESSAGE: &str = "Reply with exactly one word: pong";

/// The model that takes its time, and two messages for it. It says each back
/// a word at a time, so a reply is its message again, and long enough that
/// something else is asked and answered while it is still under way.
const SLOW: &str = "standin-anthropic/slow";
const TREES: &str = "alder birch cedar dogwood elm fir gum hazel ivy juniper";
const BIRDS: &str = "avocet bunting crane dunlin egret finch godwit heron ibis jay";

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

/// A file attached to a message goes to the harness as a link, and Null reads
/// none of it. So the harness has to tell the model where the file is, and its
/// own tool has to read a file that is outside the conversation's folder, with
/// a name as awkward as names get. The stand-in asks to read what it is
/// handed, as a model does, so what is in the file has to reach it.
#[test]
#[ignore]
fn the_agent_is_told_where_an_attached_file_is_and_can_read_it() {
    let (standin, folder, mut agent, session, _) = conversation_on("standin-anthropic/ok", None);
    let file = folder.root.join("notes \"one\" it's ノート.txt");
    std::fs::write(&file, "the word to find is heliotrope\n").unwrap();
    let path = file.display().to_string();

    let (ended, words) = agent.hand(&session, MESSAGE, std::slice::from_ref(&file)).unwrap_or_else(|error| panic!("a message with a file was answered with an error: {error}"));
    assert_eq!(words.trim(), "pong", "the reply's words: {ended}");

    let heard = standin.heard();
    let told = |request: &Value| request["messages"].as_array().into_iter().flatten().map(standin::words).collect::<String>();
    let first = heard.first().map(told).unwrap_or_default();
    assert!(first.contains(MESSAGE), "the message's own words did not reach the model: {first}");
    assert!(first.contains(&path), "the model was not told where the attached file is: {first}");
    assert!(heard.iter().skip(1).any(|request| told(request).contains("the word to find is heliotrope")), "the harness did not read the file for the model: {} requests, the last {}", heard.len(), heard.last().map(told).unwrap_or_default());

    // Loaded back, the message is its words and then where each file is, which is how the box shows its files again.
    drop(agent);
    let mut again = Agent::start(&engine::under_test(), &folder, None);
    again.load(&folder, &session).expect("the conversation loads back");
    let mut tools = HashMap::new();
    let replayed: Vec<Value> = again.updates.iter().filter_map(|update| translate::event_from_update(update, &mut tools)).filter(|event| event["type"] == "user_message").collect();
    assert_eq!(replayed, [attach::said(MESSAGE, &[file])], "the message as the harness gives it back");
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

/// Deleting a conversation leans on things the harness promises nobody: that
/// it keeps each conversation as one file, found by its id; that it has that
/// file open only for as long as it holds the conversation, and lets go when
/// asked; and that it stops listing a conversation whose file is gone and
/// lists it again when the file is back, which is what "Put Back" in the Trash
/// does. The file goes to the real Trash here and is taken out again.
#[test]
#[ignore]
fn a_conversation_s_file_is_found_by_its_id_and_the_list_follows_the_file() {
    let (_standin, folder, mut agent, session, _) = conversation_on("standin-anthropic/ok", None);
    assert!(translate::closes_conversations(&agent.hello), "the harness does not say that it closes a conversation: {}", agent.hello);
    agent.say(&session, MESSAGE).expect("the message is answered");
    let listed = |agent: &mut Agent| agent.conversations().iter().any(|kept| kept["sessionId"] == session.as_str());
    assert!(listed(&mut agent), "the conversation is in the harness's list");

    let file = history::file_of(&folder.settings().join("sessions"), &session).expect("one file for the conversation, found by its id");
    assert_eq!(history::held_by(&file), vec![agent.child.id()], "the harness has the file open while it holds the conversation");
    assert!(!history::in_a_terminal(agent.child.id()), "a harness another program started, as Null starts its own, is in no terminal");
    agent.request("session/close", json!({ "sessionId": session })).expect("the harness closes the conversation");
    assert_eq!(history::held_by(&file), Vec::<u32>::new(), "nobody has the file open once the harness has closed the conversation");

    let in_the_trash = history::to_trash(&file).expect("macOS moves the file to the Trash");
    assert!(!file.exists() && in_the_trash.is_file(), "the file is in the Trash, at {}", in_the_trash.display());
    assert!(!listed(&mut agent), "a conversation whose file is gone is listed no longer");
    assert!(agent.load(&folder, &session).is_err(), "and does not load");
    assert!(!file.exists(), "the harness does not write the file again");

    std::fs::rename(&in_the_trash, &file).expect("the file is taken out of the Trash again");
    assert!(listed(&mut agent), "put back, the conversation is listed again");
    agent.load(&folder, &session).expect("and loads");
}

/// Searching inside conversations leans on how the harness writes a
/// conversation's file, which it promises nobody: a line for each thing said,
/// with who said it and the words as they were. What the owner wrote and what
/// the agent replied are both found there, by Null's own search.
#[test]
#[ignore]
fn what_is_said_in_a_conversation_is_in_its_file_as_the_search_reads_it() {
    let (_standin, folder, mut agent, session, _) = conversation_on("standin-anthropic/ok", None);
    let (_, replied) = agent.say(&session, "What is the word to remember?\nIt is Marmalade.").expect("the message is answered");
    assert_eq!(replied, "pong", "the stand-in's reply");
    let sessions = folder.settings().join("sessions");
    let found = |typed: &str| search::search(&sessions, &search::words_of(typed), &|| true);

    let by_the_owner = found("remember marmalade");
    assert_eq!(by_the_owner.iter().map(|found| found.id.as_str()).collect::<Vec<_>>(), [session.as_str()], "found by what the owner wrote, whatever the capitals and the order");
    assert_eq!(by_the_owner[0].piece, "What is the word to remember? It is Marmalade.", "with a piece of it, as it was written");
    let by_the_agent = found("pong");
    assert_eq!(by_the_agent.iter().map(|found| found.id.as_str()).collect::<Vec<_>>(), [session.as_str()], "found by what the agent replied");
    assert_eq!(by_the_agent[0].piece, "pong");
    assert_eq!(found("marmalade zebra"), Vec::new(), "every word has to be said");
}

/// `/history` draws the harness's own list of the conversations it keeps, the
/// ones made in a terminal in other folders among them: each with the folder
/// it works in and when it was last used, the most recently used first. And a
/// conversation opens in the box by the harness loading it back in that
/// folder and replaying what was said, so that the next message continues it.
#[test]
#[ignore]
fn earlier_conversations_are_listed_with_their_folders_and_one_loads_back_in_its_own() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let binary = engine::under_test();
    let elsewhere = folder.root.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    const SAID: &str = "Reply with exactly one word: pong\nand nothing after it";

    let (here, there) = {
        let mut agent = Agent::start(&binary, &folder, None);
        assert!(translate::lists_conversations(&agent.hello), "the harness does not say that it lists its conversations: {}", agent.hello);
        let (here, _) = agent.open(&folder);
        agent.set(&here, "model", json!("standin-anthropic/ok"));
        agent.say(&here, MESSAGE).expect("the message is answered");
        // One as a terminal makes them: in a folder that is not Null's.
        let opened = agent.request("session/new", json!({ "cwd": elsewhere, "mcpServers": [] })).expect("a conversation opens in another folder");
        let there = opened["sessionId"].as_str().expect("a conversation has an id").to_string();
        agent.set(&there, "model", json!("standin-anthropic/ok"));
        agent.say(&there, SAID).expect("the message is answered");
        // And one in which nothing is said, which the box leaves out.
        agent.open(&folder);
        (here, there)
    };

    // A harness started afresh, as after Null was.
    let mut agent = Agent::start(&binary, &folder, None);
    let listed = agent.conversations();
    let shown = history::entries(&listed, &Default::default(), &folder.work(), Some(&here));
    assert_eq!(shown.iter().map(|entry| entry.id.as_str()).collect::<Vec<_>>(), [there.as_str(), here.as_str()], "the conversations something was said in, the last used first: {listed:?}");
    assert_eq!((shown[0].folder.as_str(), shown[0].own, shown[0].open), (elsewhere.to_str().unwrap(), false, false), "the one made elsewhere: {listed:?}");
    assert_eq!((shown[1].folder.as_str(), shown[1].own, shown[1].open), (folder.work().to_str().unwrap(), true, true), "the one made in Null's folder: {listed:?}");
    assert!(shown.iter().all(|entry| entry.used.as_deref().is_some_and(|used| used.len() >= 20 && used.contains('T'))), "when each was last used: {listed:?}");

    let its_folder = history::folder_of(&listed, &there).expect("the list names the folder of each");
    let before = agent.updates.len();
    agent.request("session/load", json!({ "sessionId": there, "cwd": its_folder, "mcpServers": [] })).unwrap_or_else(|error| panic!("the conversation did not load back in its own folder: {error}"));
    let replayed = history::replayed(&agent.updates[before..]);
    assert_eq!(replayed.first(), Some(&json!({ "type": "user_message", "text": SAID })), "what was said first is replayed first: {replayed:?}");
    assert!(replayed.contains(&json!({ "type": "text_delta", "text": "pong", "thinking": false })), "the reply is replayed: {replayed:?}");

    let (ended, words) = agent.say(&there, MESSAGE).unwrap_or_else(|error| panic!("the next message was answered with an error: {error}"));
    assert_eq!(words.trim(), "pong", "the next message continues it: {ended}");
    let again = agent.conversations();
    assert_eq!(history::entries(&again, &Default::default(), &folder.work(), None).len(), 2, "continuing a conversation makes no new one: {again:?}");

    // Asked for again while it is loaded, as when the conversation that is open
    // is chosen from the list: all of it is replayed, the new message with it.
    let before = agent.updates.len();
    agent.request("session/load", json!({ "sessionId": there, "cwd": its_folder, "mcpServers": [] })).unwrap_or_else(|error| panic!("the conversation did not load a second time: {error}"));
    let whole = history::replayed(&agent.updates[before..]);
    assert_eq!(whole.iter().filter(|event| event["type"] == "user_message").count(), 2, "both messages are replayed: {whole:?}");

    // One the harness does not have is refused and not made up, which is how
    // the box knows to stay on the conversation it had.
    let missing = agent.request("session/load", json!({ "sessionId": "01a00000-0000-7000-8000-000000000000", "cwd": folder.work(), "mcpServers": [] }));
    assert!(missing.is_err(), "a conversation the harness does not have was loaded: {missing:?}");
}

/// The harness gives its list a page at a time, and the box shows all of it:
/// every page has to say how to ask for the next, and the last that there is none.
#[test]
#[ignore]
fn the_list_of_conversations_is_read_to_its_end_a_page_at_a_time() {
    const MADE: usize = 60;
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let mut agent = Agent::start(&engine::under_test(), &folder, None);
    let made: std::collections::HashSet<String> = (0..MADE).map(|_| agent.open(&folder).0).collect();
    assert_eq!(made.len(), MADE, "every conversation has an id of its own");

    let first = agent.request("session/list", json!({})).expect("the harness lists its conversations");
    let (on_the_first_page, next) = history::page(&first);
    assert!(on_the_first_page.len() < MADE && next.is_some(), "{MADE} conversations no longer come on more than one page, so this can no longer be told: {} on the first, then {next:?}", on_the_first_page.len());

    let listed: std::collections::HashSet<String> = agent.conversations().iter().filter_map(|kept| kept["sessionId"].as_str().map(str::to_string)).collect();
    assert_eq!(listed, made, "the pages together are the conversations made, each once");
}

// ── What work in the background rests on ──────────────────────────────────

/// Work in the background is several conversations answering at once in one
/// harness, each in a folder of its own. So the harness has to take a message
/// for one while another is still answering, and say both replies as they
/// come: a word of one between two words of the other, each reply whole, its
/// own, and ending well.
#[test]
#[ignore]
fn two_conversations_in_two_folders_answer_at_the_same_time() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let elsewhere = folder.root.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let mut agent = Agent::start(&engine::under_test(), &folder, None);
    let (here, there) = (agent.open(&folder).0, agent.open_in(&elsewhere).0);
    for session in [&here, &there] {
        agent.set(session, "model", json!(SLOW));
    }

    let before = agent.updates.len();
    let (first, second) = (agent.begin(&here, TREES), agent.begin(&there, BIRDS));
    for (which, begun, session, sent) in [("the first", first, &here, TREES), ("the second", second, &there, BIRDS)] {
        let ended = agent.ended(begun).unwrap_or_else(|error| panic!("{which} message was answered with an error: {error}"));
        assert_eq!(ended["stopReason"], "end_turn", "{which} reply: {ended}");
        assert!(!translate::reply_failed(&ended), "{which} reply was taken for a refusal: {ended}");
        assert_eq!(agent.pieces(session, before).concat(), sent, "{which} reply is its own message said back, whole");
    }
    let turns = agent.turns(before);
    assert!(turns.len() >= 3, "one reply was over before the other began, so they were not said at the same time: the word passed {} times", turns.len().saturating_sub(1));

    let listed = agent.conversations();
    assert_eq!((history::folder_of(&listed, &here), history::folder_of(&listed, &there)), (Some(folder.work()), Some(elsewhere)), "each in the folder it was opened in: {listed:?}");
}

/// Stopping one conversation is not stopping the harness. The reply that was
/// stopped ends as stopped, and the other, still under way at that moment,
/// goes on to its end and is whole.
#[test]
#[ignore]
fn stopping_one_conversation_leaves_the_other_working() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let mut agent = Agent::start(&engine::under_test(), &folder, None);
    let (stopped, left) = (agent.open(&folder).0, agent.open(&folder).0);
    for session in [&stopped, &left] {
        agent.set(session, "model", json!(SLOW));
    }

    let before = agent.updates.len();
    let (first, second) = (agent.begin(&stopped, TREES), agent.begin(&left, BIRDS));
    agent.hear_until("word from each conversation", |agent| !agent.pieces(&stopped, before).is_empty() && !agent.pieces(&left, before).is_empty());
    let said_by_then = agent.pieces(&left, before).len();
    agent.stop(&stopped);

    let ended = agent.ended(first).unwrap_or_else(|error| panic!("the reply that was stopped ended with an error: {error}"));
    assert_eq!(ended["stopReason"], "cancelled", "the reply that was stopped: {ended}");
    assert_ne!(agent.pieces(&stopped, before).concat(), TREES, "the reply that was stopped was said to its end all the same");
    let ended = agent.ended(second).unwrap_or_else(|error| panic!("the other reply ended with an error: {error}"));
    assert_eq!(ended["stopReason"], "end_turn", "the other reply: {ended}");
    assert_eq!(agent.pieces(&left, before).concat(), BIRDS, "the other reply is whole");
    assert!(agent.pieces(&left, before).len() > said_by_then, "the other reply was over before the first was stopped, so this can no longer be told");
}

/// A second message to a conversation that is still answering is neither
/// refused nor kept for later: the harness stops the first reply and answers
/// the second. So the box has to go on refusing one itself.
#[test]
#[ignore]
fn a_second_message_to_a_conversation_still_answering_stops_the_first_reply() {
    let (_standin, _folder, mut agent, session, _) = conversation_on(SLOW, None);
    let before = agent.updates.len();
    let first = agent.begin(&session, TREES);
    agent.hear_until("word of the first reply", |agent| !agent.pieces(&session, before).is_empty());
    let second = agent.begin(&session, BIRDS);

    let ended = agent.ended(first).unwrap_or_else(|error| panic!("the first reply ended with an error: {error}"));
    assert_eq!(ended["stopReason"], "cancelled", "the first reply: {ended}");
    let ended = agent.ended(second).unwrap_or_else(|error| panic!("the second message was answered with an error, so it is refused now: {error}"));
    assert_eq!(ended["stopReason"], "end_turn", "the second reply: {ended}");
    let said = agent.pieces(&session, before).concat();
    assert!(said.ends_with(BIRDS), "the second message is answered whole: {said}");
    assert!(!said.contains(TREES), "the first reply was said to its end, so the second message waited for it: {said}");
}

/// A conversation is opened from `/history` while another is working: the
/// harness has to load it back and replay what was said in it, and the reply
/// under way has to go on to its end as if nothing had been asked.
#[test]
#[ignore]
fn a_conversation_loads_back_while_another_is_working() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let binary = engine::under_test();
    // One from an earlier run, which this harness does not hold.
    let earlier = {
        let mut agent = Agent::start(&binary, &folder, None);
        let (earlier, _) = agent.open(&folder);
        agent.set(&earlier, "model", json!("standin-anthropic/ok"));
        agent.say(&earlier, MESSAGE).expect("the message is answered");
        earlier
    };

    let mut agent = Agent::start(&binary, &folder, None);
    let (working, _) = agent.open(&folder);
    agent.set(&working, "model", json!(SLOW));
    let before = agent.updates.len();
    let begun = agent.begin(&working, TREES);
    agent.hear_until("word of the reply", |agent| !agent.pieces(&working, before).is_empty());

    agent.load(&folder, &earlier).unwrap_or_else(|error| panic!("the earlier conversation did not load back while another was working: {error}"));
    let said_by_then = agent.pieces(&working, before).len();
    let replayed = history::replayed(&agent.heard(&earlier, before));
    assert_eq!(replayed.first(), Some(&json!({ "type": "user_message", "text": MESSAGE })), "what was said in it is replayed: {replayed:?}");
    assert!(replayed.contains(&json!({ "type": "text_delta", "text": "pong", "thinking": false })), "and what was replied: {replayed:?}");

    let ended = agent.ended(begun).unwrap_or_else(|error| panic!("the reply under way ended with an error: {error}"));
    assert_eq!(ended["stopReason"], "end_turn", "the reply under way: {ended}");
    assert_eq!(agent.pieces(&working, before).concat(), TREES, "the reply under way is whole");
    assert!(agent.pieces(&working, before).len() > said_by_then, "the reply was over before the other had loaded, so this can no longer be told");
}

/// A conversation that has ended and been looked at is closed in the harness,
/// so that Null holds only what is working or waiting to be seen. Closed, it
/// has to stay in the harness's list, load back with what was said in it, and
/// take the next message.
#[test]
#[ignore]
fn a_closed_conversation_loads_back() {
    let (_standin, folder, mut agent, session, _) = conversation_on("standin-anthropic/ok", None);
    assert!(translate::closes_conversations(&agent.hello), "the harness does not say that it closes a conversation: {}", agent.hello);
    agent.say(&session, MESSAGE).expect("the message is answered");
    agent.request("session/close", json!({ "sessionId": session })).unwrap_or_else(|error| panic!("the harness did not close the conversation: {error}"));
    assert!(agent.conversations().iter().any(|kept| kept["sessionId"] == session.as_str()), "a closed conversation is still in the harness's list");

    let before = agent.updates.len();
    agent.load(&folder, &session).unwrap_or_else(|error| panic!("the closed conversation did not load back: {error}"));
    let replayed = history::replayed(&agent.heard(&session, before));
    assert_eq!(replayed.first(), Some(&json!({ "type": "user_message", "text": MESSAGE })), "what was said in it is replayed: {replayed:?}");
    assert!(replayed.contains(&json!({ "type": "text_delta", "text": "pong", "thinking": false })), "and what was replied: {replayed:?}");
    let (ended, words) = agent.say(&session, MESSAGE).unwrap_or_else(|error| panic!("the next message was answered with an error: {error}"));
    assert_eq!(words.trim(), "pong", "the next message continues it: {ended}");
}

/// `/folder` starts a conversation in a folder of the person's choosing, so
/// that the agent has that project's own instructions and files, as in a
/// terminal. The harness has to open it there: tell the model that this is
/// where it works, and list the conversation under that folder, which is all
/// Null reads to know where a conversation works.
#[test]
#[ignore]
fn a_conversation_started_in_a_folder_works_there() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let project = folder.root.join("a project");
    std::fs::create_dir_all(&project).unwrap();
    let mut agent = Agent::start(&engine::under_test(), &folder, None);
    let (session, _) = agent.open_in(&project);
    agent.set(&session, "model", json!("standin-anthropic/ok"));
    agent.say(&session, MESSAGE).expect("the message is answered");

    let heard = standin.heard();
    let told = heard.first().map(|request| request["messages"].as_array().into_iter().flatten().map(standin::words).collect::<String>()).unwrap_or_default();
    assert!(told.contains(project.to_str().unwrap()), "the model was not told that it works in the folder: {told}");
    let listed = agent.conversations();
    assert_eq!(history::folder_of(&listed, &session), Some(project.clone()), "the harness lists it under that folder: {listed:?}");
    assert_eq!(history::folders(&listed, &folder.work()), [project], "and that is a folder `/folder` offers");
}

/// Computer use is the harness's own, and it is off until the harness's
/// settings say otherwise. Null switches it on in the settings it hands over at
/// start, so a harness started that way has to tell the model that there is a
/// desktop to work, by the name the model works it with, and one started
/// without has to say nothing of it. Only what the model is sent is read:
/// nothing here looks at this Mac's screen or presses anything on it.
#[test]
#[ignore]
fn with_computer_use_handed_over_the_model_is_told_of_the_desktop() {
    let told = |more: Option<Value>| {
        let (standin, _folder, mut agent, session, _) = conversation_on("standin-anthropic/ok", more);
        agent.say(&session, MESSAGE).unwrap_or_else(|error| panic!("a message was answered with an error: {error}"));
        standin.heard().first().map(Value::to_string).unwrap_or_default()
    };
    // The model works the desktop through a thing named `computer`, and is told of it by that name.
    let named = "`computer`";
    let without = told(None);
    assert!(without.contains(MESSAGE), "the message did not reach the model, so nothing can be told: {without}");
    assert!(!without.contains(named), "the model is told of computer use though nothing switched it on");
    let with = told(Some(json!({ "computer": { "enabled": true } })));
    assert!(with.contains(named), "with computer use handed over, the model is not told of the desktop");
}

/// While computer use is on, Null tells the model its rules for working the
/// Mac, in a file of its own named on the start line as
/// `--append-system-prompt`. So the harness has to read that file and pass its
/// words to the model in a conversation opened over the protocol, and pass
/// nothing when none was named.
#[test]
#[ignore]
fn rules_handed_over_in_a_file_at_start_reach_the_model() {
    const RULE: &str = "Box check rule: open a tab of your own, never the one in view.";
    let told = |rules: Option<&str>| {
        let standin = StandIn::start();
        let folder = Folder::new(&standin);
        let file = folder.root.join("rules.md");
        let flags: Vec<String> = match rules {
            Some(rules) => {
                std::fs::write(&file, rules).expect("the rules are written");
                vec!["--append-system-prompt".into(), file.display().to_string()]
            }
            None => Vec::new(),
        };
        let flags: Vec<&str> = flags.iter().map(String::as_str).collect();
        let mut agent = Agent::start_with(&engine::under_test(), &folder, None, "yolo", &flags);
        let (session, _) = agent.open_in(&folder.work());
        agent.set(&session, "model", json!("standin-anthropic/ok"));
        agent.say(&session, MESSAGE).unwrap_or_else(|error| panic!("a message was answered with an error: {error}"));
        standin.heard().first().map(Value::to_string).unwrap_or_default()
    };
    let without = told(None);
    assert!(without.contains(MESSAGE), "the message did not reach the model, so nothing can be told: {without}");
    assert!(!without.contains(RULE), "the model is told the rule though none was handed over");
    let with = told(Some(RULE));
    assert!(with.contains(RULE), "rules handed over in a file at start do not reach the model: {with}");
    assert!(!with.contains("rules.md"), "the harness passed the file's name, not its words: {with}");
}

/// Null makes its own copy of the browser link's extension from the engine it
/// carries, in one folder that Chrome loads and keeps loading. The harness's
/// relay refuses an extension of a protocol other than its own ("out of
/// date"). So the extension this engine writes has to speak the protocol this
/// engine's relay expects. The check writes it, starts the relay on a port of
/// its own, says hello as the extension does, with the number the extension
/// states, and reads what the relay makes of it. No Chrome is needed.
#[test]
#[ignore]
fn the_extension_the_engine_writes_speaks_the_relay_s_protocol() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let binary = engine::under_test();
    let written = folder.root.join("extension");
    let installed = folder.command(&binary).args(["browser-relay", "install", "--dir"]).arg(&written).output().expect("the harness runs");
    assert!(installed.status.success(), "the harness did not write its extension: {}", String::from_utf8_lossy(&installed.stderr));
    let script = std::fs::read_to_string(written.join("background.js")).expect("the extension has its script");
    let stated: String = script.split("discardedTabsProtocol:").nth(1).expect("the extension states its protocol").trim_start().chars().take_while(char::is_ascii_digit).collect();
    assert!(!stated.is_empty(), "the extension states no number for its protocol");

    let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|free| free.local_addr()).expect("a free port").port();
    // With nothing to read the relay stops at once, so it is given an input
    // that stays open, and it is stopped however this ends.
    let relay = Stopped(folder.command(&binary).args(["browser-relay", "-p", &port.to_string()]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("the relay starts"));
    let until = Instant::now() + PATIENCE;
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < until, "the relay never listened on port {port}");
        std::thread::sleep(Duration::from_millis(100));
    }
    let hello = json!({ "t": "hello", "instanceId": "harness-check", "userAgent": "Chrome/155.0", "browserVersion": "Chrome/155.0", "discardedTabsProtocol": stated.parse::<u64>().unwrap(), "tabs": [], "attachedTabIds": [] });
    let _extension = as_the_extension(port, &hello);
    let mut said = Value::Null;
    while said.get("ompExtensionDiscardedTabsProtocol").is_none() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
        said = browser::relay_says(port);
    }
    drop(relay);
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err(), "the relay went on listening once it was stopped");
    assert_eq!(said["ompExtensionDiscardedTabsProtocol"], json!(stated), "the relay did not hear the extension's protocol: {said}");
    assert_eq!(said["ompRelayDiscardedTabsProtocol"], said["ompExtensionDiscardedTabsProtocol"], "the extension this engine writes is out of date for its own relay: {said}");
}

/// A program the check started, stopped when it is let go, whether the check
/// passed or not.
struct Stopped(Child);

impl Drop for Stopped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Null offers the link under its own name: it makes its own copy of the
/// extension the engine in use writes, and changes what Chrome shows of it.
/// Each change looks for words that stand once in the engine's extension, so
/// an engine whose extension has moved them would give a link half under the
/// harness's name; Null then makes none. The extension this engine writes has
/// to be one Null knows how to name, and a copy made again, as for a newer
/// engine, has to stand in the same folder Chrome loads. Made again from the
/// same engine, it has to come out the same, or the box would ask the person
/// to reload Chrome's copy after every `/harness` and `/update`.
#[test]
#[ignore]
fn the_extension_null_makes_from_the_engine_is_under_null_s_name() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let binary = engine::under_test();
    let link = browser::folder(&folder.root);
    let read = |file: &str| std::fs::read_to_string(link.join(file)).unwrap_or_default();
    for time in ["first", "again"] {
        if time == "first" {
            browser::make(&binary, &link).unwrap_or_else(|why| panic!("Null could not make its copy, the first time: {why}"));
        } else {
            let changed = browser::make_again(&binary, &link).unwrap_or_else(|why| panic!("Null could not make its copy again: {why}"));
            assert!(!changed, "made again from the same engine, the copy came out different, so the box would ask for a reload for nothing");
        }
        let manifest: Value = serde_json::from_str(&read("manifest.json")).expect("the copy has a manifest");
        assert_eq!((&manifest["name"], &manifest["action"]["default_title"]), (&json!("Null"), &json!("Null")), "{manifest}");
        for (_, icon) in manifest["icons"].as_object().expect("the copy names its icons") {
            assert!(link.join(icon.as_str().unwrap()).is_file(), "the icon {icon} is in the copy");
        }
        assert!(read("background.js").contains("const title = \"Null\";"), "the agent's tabs are gathered under Null's name");
        assert!(read("background.js").contains("connected ? \"on\" : \"\""), "the toolbar says nothing, not \"off\", while the agent is not linked");
        assert!(read("options.html").contains("<h1>Null</h1>"), "its settings page is headed with Null's name");
        assert!(link.join("LICENSE").is_file() && link.join("THIRD-PARTY-NOTICES.txt").is_file(), "the harness's licence and notices stay with it");
        assert!(!link.with_extension("new").exists(), "nothing is left beside the folder");
    }
}

/// Setting Chrome up waits for the extension to reach the link, so that the
/// box can say "Chrome is linked" before the agent ever needs it. With no
/// relay running, Null starts one of the harness's for the wait, on the port
/// the extension looks for, and stops it afterwards. So that relay has to
/// start when asked to, say when an extension of this engine has reached it,
/// and stop when Null stops it. No Chrome is needed: the check says hello as
/// the extension does.
#[test]
#[ignore]
fn setting_chrome_up_waits_for_the_extension_and_stops_the_relay_it_started() {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    let binary = engine::under_test();
    let link = browser::folder(&folder.root);
    browser::make(&binary, &link).expect("Null makes its copy of the extension");
    let script = std::fs::read_to_string(link.join("background.js")).expect("the copy has its script");
    let stated: u64 = script.split("discardedTabsProtocol:").nth(1).expect("the extension states its protocol").trim_start().chars().take_while(char::is_ascii_digit).collect::<String>().parse().expect("a number");
    let relay = |port: u16| {
        let mut command = folder.command(&binary);
        command.args(["browser-relay", "--port", &port.to_string()]);
        command
    };
    let free = || std::net::TcpListener::bind("127.0.0.1:0").and_then(|free| free.local_addr()).expect("a free port").port();

    // Nothing reaches it: the wait ends, said as not seen, and the relay is gone.
    let port = free();
    assert!(!browser::wait_for_extension(relay(port), port, Duration::from_secs(3)), "no extension reached the relay, yet one was seen");
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err(), "the relay Null started went on listening");

    // The extension comes while Null waits.
    let port = free();
    let hello = json!({ "t": "hello", "instanceId": "harness-check", "userAgent": "Chrome/155.0", "browserVersion": "Chrome/155.0", "discardedTabsProtocol": stated, "tabs": [], "attachedTabIds": [] });
    let chrome = std::thread::spawn(move || {
        let until = Instant::now() + PATIENCE;
        while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
        }
        let extension = as_the_extension(port, &hello);
        std::thread::sleep(Duration::from_secs(3));
        drop(extension);
    });
    assert!(browser::wait_for_extension(relay(port), port, PATIENCE), "the extension reached the relay and was not seen");
    chrome.join().expect("the stand-in for Chrome");
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err(), "the relay Null started went on listening once the wait was over");
}

/// Null's copy of the link's extension in a Chrome that really loads it from
/// its folder: it reaches the relay this engine starts, the agent works a tab
/// through it, and Chrome gathers that tab in a group under Null's name. Only
/// Chrome for Testing loads an extension from the command line, so the check
/// needs one: `NULL_MINI_CHROME` names its program, and without it this
/// question is passed over, saying so. It runs headless, with a profile of its
/// own, and nothing is shown. Null's copy is pointed at the check's own relay
/// on a free port, the one word changed from what `/browser` makes, so that it
/// never reaches a relay a person's own Chrome may be linked to.
#[test]
#[ignore]
fn chrome_loads_null_s_copy_and_the_agent_works_a_tab_through_it_under_null_s_name() {
    let Some(chrome) = std::env::var_os("NULL_MINI_CHROME").map(PathBuf::from) else {
        eprintln!("passed over: NULL_MINI_CHROME names no Chrome for Testing to load Null's copy in");
        return;
    };
    let standin = StandIn::start();
    let folder = Folder::new(&standin).with_own_services();
    let binary = engine::under_test();
    let link = browser::folder(&folder.root);
    browser::make(&binary, &link).expect("Null makes its copy of the extension");
    let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|free| free.local_addr()).expect("a free port").port();
    let script = std::fs::read_to_string(link.join("background.js")).unwrap();
    assert_eq!(script.matches("var DEFAULT_PORT = 9224;").count(), 1, "the extension no longer says which port it looks for its relay on");
    std::fs::write(link.join("background.js"), script.replace("var DEFAULT_PORT = 9224;", &format!("var DEFAULT_PORT = {port};"))).unwrap();

    let _relay = Stopped(folder.command(&binary).args(["browser-relay", "--port", &port.to_string()]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("the relay starts"));
    let profile = folder.root.join("chrome");
    let page = "data:text/html,<title>null-link-check</title><p>read through the link</p>";
    let flags = [format!("--user-data-dir={}", profile.display()), format!("--load-extension={}", link.display()), "--headless=new".into(), "--no-first-run".into(), "--no-default-browser-check".into(), "--remote-debugging-port=0".into(), page.into()];
    let _chrome = Stopped(Command::new(&chrome).args(&flags).stdout(Stdio::null()).stderr(Stdio::null()).spawn().expect("Chrome for Testing starts"));
    let until = Instant::now() + PATIENCE;
    while browser::relay_says(port).get("ompExtensionDiscardedTabsProtocol").is_none() {
        assert!(Instant::now() < until, "Chrome did not load Null's copy from its folder, or it never reached the relay");
        std::thread::sleep(Duration::from_millis(250));
    }

    let mut agent = Agent::start(&binary, &folder, Some(json!({ "browser": { "relayUrl": format!("http://127.0.0.1:{port}") } })));
    let (session, _) = agent.open(&folder);
    agent.set(&session, "model", json!("standin-anthropic/ok"));
    let code = "const tab = await browser.open({ name: \"check\", timeout: 20000, app: { relay: true, target: \"null-link-check\" } }); return await tab.evaluate(() => document.body.innerText)";
    let begun = agent.begin(&session, &format!("{MESSAGE}\n{}{code}", standin::ASK_FOR_CODE));
    if agent.answer_by(begun, Instant::now() + PATIENCE).is_none() {
        let said: Vec<String> = agent.updates.iter().rev().take(6).map(Value::to_string).collect();
        panic!("the harness did not finish working the tab through the link in {} s. Last said: {said:#?}", PATIENCE.as_secs());
    }
    let sent = last_sent(&standin);
    assert!(sent.contains("read through the link"), "the agent did not read the tab through the link: {sent}");

    let active = std::fs::read_to_string(profile.join("DevToolsActivePort")).expect("Chrome says where its DevTools are");
    let devtools = active.lines().next().unwrap_or_default().trim().to_string();
    let targets: Value = serde_json::from_str(&http_get(&format!("127.0.0.1:{devtools}"), "/json/list")).expect("Chrome lists what it runs");
    // Chrome runs extensions of its own as well, so Null's worker is known by the name in its manifest.
    let workers = targets.as_array().into_iter().flatten().filter(|t| t["type"] == "service_worker").filter_map(|t| t["webSocketDebuggerUrl"].as_str());
    let worker = workers.into_iter().find(|address| devtools_evaluate(address, "chrome.runtime.getManifest().name")["result"]["value"] == "Null").expect("Null's copy has a worker running");
    let answer = devtools_evaluate(worker, "chrome.tabGroups.query({}).then((groups) => groups.map((group) => group.title).join(','))");
    assert_eq!(answer["result"]["value"], json!("Null"), "the agent's tab is not gathered under Null's name: {answer}");
}

/// One plain request to a server on this Mac, and the body of its answer.
/// Chrome keeps the connection open whatever it is asked, so the body is read
/// to the length it says, not to the end.
fn http_get(host: &str, path: &str) -> String {
    let mut socket = BufReader::new(std::net::TcpStream::connect(host).expect("the server takes the connection"));
    socket.get_ref().set_read_timeout(Some(PATIENCE)).unwrap();
    write!(socket.get_mut(), "GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").unwrap();
    let mut length = 0;
    loop {
        let mut line = String::new();
        socket.read_line(&mut line).expect("the server answers");
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0u8; length];
    std::io::Read::read_exact(&mut socket, &mut body).expect("the server sends what it said it would");
    String::from_utf8_lossy(&body).into_owned()
}

/// Have a page or worker of a Chrome work out `expression` over its DevTools
/// socket, and give back Chrome's answer: what it came to, or why it could not.
fn devtools_evaluate(address: &str, expression: &str) -> Value {
    use std::io::Read;
    let (host, path) = address.strip_prefix("ws://").and_then(|rest| rest.split_once('/')).expect("a DevTools address");
    let mut socket = std::net::TcpStream::connect(host).expect("Chrome takes the connection");
    socket.set_read_timeout(Some(PATIENCE)).unwrap();
    write!(socket, "GET /{path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n").unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        socket.read_exact(&mut byte).expect("Chrome answers");
        head.push(byte[0]);
    }
    assert!(head.starts_with(b"HTTP/1.1 101"), "Chrome would not open its DevTools socket: {}", String::from_utf8_lossy(&head));
    let asked = json!({ "id": 1, "method": "Runtime.evaluate", "params": { "expression": expression, "awaitPromise": true, "returnByValue": true } });
    send_frame(&mut socket, &asked);
    loop {
        // A server's frames come unmasked: a byte of kind, a length, and the words.
        let mut top = [0u8; 2];
        socket.read_exact(&mut top).expect("Chrome says something");
        let length = match top[1] & 0x7f {
            126 => {
                let mut more = [0u8; 2];
                socket.read_exact(&mut more).unwrap();
                u16::from_be_bytes(more) as usize
            }
            127 => {
                let mut more = [0u8; 8];
                socket.read_exact(&mut more).unwrap();
                u64::from_be_bytes(more) as usize
            }
            short => short as usize,
        };
        let mut words = vec![0u8; length];
        socket.read_exact(&mut words).unwrap();
        let said: Value = serde_json::from_slice(&words).unwrap_or(Value::Null);
        if top[0] & 0x0f == 1 && said["id"] == 1 {
            return said["result"].clone();
        }
    }
}

/// One text message on a socket, masked as a client's has to be.
fn send_frame(socket: &mut std::net::TcpStream, said: &Value) {
    let text = said.to_string().into_bytes();
    let mask = [7u8, 11, 13, 17];
    let mut frame = vec![0x81];
    match text.len() {
        length if length < 126 => frame.push(0x80 | length as u8),
        length => {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(length as u16).to_be_bytes());
        }
    }
    frame.extend_from_slice(&mask);
    frame.extend(text.iter().enumerate().map(|(at, byte)| byte ^ mask[at % 4]));
    socket.write_all(&frame).unwrap();
}

/// Connect to a relay as its extension does, with no origin, and say one
/// thing. The connection is given back: the relay forgets an extension that
/// has gone.
fn as_the_extension(port: u16, said: &Value) -> std::net::TcpStream {
    use std::io::Read;
    let mut socket = std::net::TcpStream::connect(("127.0.0.1", port)).expect("the relay takes the connection");
    // The key is the protocol's own example: sixteen bytes, as a key has to be.
    write!(socket, "GET /ext HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n").unwrap();
    let mut answer = [0u8; 512];
    let read = socket.read(&mut answer).expect("the relay answers");
    assert!(answer[..read].starts_with(b"HTTP/1.1 101"), "the relay would not take the extension: {}", String::from_utf8_lossy(&answer[..read]));
    send_frame(&mut socket, said);
    socket
}

/// What the model was last sent in the stand-in's hearing: for a tool it asked
/// for, what the harness gave back.
fn last_sent(standin: &StandIn) -> String {
    standin.heard().last().and_then(|request| request["messages"].as_array().and_then(|messages| messages.last()).map(standin::words)).unwrap_or_default()
}

/// A person can set the harness to ask before a tool acts, and Null passes that
/// on. The box then shows the harness's question with the answers it offers,
/// and sends back the one chosen. So the question has to come through the
/// protocol, a "no" has to stop the tool, a "yes" has to let it run, and one of
/// the answers offered has to hold for the rest of the conversation, so that a
/// person is not asked about every step.
#[test]
#[ignore]
fn set_to_ask_the_harness_asks_before_the_shell_acts_and_keeps_to_the_answer() {
    let asked_and = |kind: &'static str| {
        let standin = StandIn::start();
        let folder = Folder::new(&standin);
        let mut agent = Agent::start_set_to(&engine::under_test(), &folder, None, "always-ask");
        agent.answering = Some(kind);
        let (session, _) = agent.open(&folder);
        agent.set(&session, "model", json!("standin-anthropic/ok"));
        let made = folder.work().join("made by the shell");
        agent.say(&session, &format!("{MESSAGE}\n{}touch 'made by the shell'", standin::ASK_THE_SHELL)).unwrap_or_else(|error| panic!("a message was answered with an error: {error}"));
        (agent.questions.clone(), made.exists(), last_sent(&standin))
    };

    let (questions, made, sent) = asked_and("reject_once");
    assert_eq!(questions.len(), 1, "the harness put {} questions to Null before the shell acted", questions.len());
    let kinds: Vec<&str> = questions[0]["options"].as_array().into_iter().flatten().filter_map(|option| option["kind"].as_str()).collect();
    assert!(kinds.contains(&"allow_once") && kinds.contains(&"reject_once"), "the answers offered: {kinds:?}");
    assert!(kinds.contains(&"allow_always"), "no answer offered holds for the rest of the conversation: {kinds:?}");
    assert!(questions[0]["toolCall"]["title"].as_str().is_some_and(|title| title.contains("touch")), "the question does not say what is about to be done: {}", questions[0]["toolCall"]);
    assert!(!made, "answered no, the shell acted all the same");
    assert!(sent.contains("rejected"), "the model was not told that the person said no: {sent}");

    let (questions, made, _) = asked_and("allow_once");
    assert_eq!(questions.len(), 1, "the harness put {} questions to Null before the shell acted", questions.len());
    assert!(made, "answered yes, the shell did not act");
}

/// Computer use is worked through the tool that runs code. Set to ask, this
/// harness does not put a question about that tool through the protocol: it
/// refuses the call outright. So computer use works in the box only when the
/// harness acts freely, and the box says so. Should a newer harness ask, this
/// question fails, which is the day computer use can ask first in the box.
/// Whatever it does, a call nobody said yes to must not run.
#[test]
#[ignore]
fn set_to_ask_the_harness_refuses_a_code_run_and_never_runs_it_unasked() {
    for (more, why) in [(None, "set to ask before every tool"), (Some(json!({ "tools": { "approval": { "computer": "prompt" } } })), "set to ask before computer use alone")] {
        let standin = StandIn::start();
        let folder = Folder::new(&standin);
        let handed = json!({ "computer": { "enabled": true } });
        let (handed, asking) = match more {
            Some(Value::Object(policy)) => (Value::Object(handed.as_object().cloned().unwrap_or_default().into_iter().chain(policy).collect()), "yolo"),
            _ => (handed, "always-ask"),
        };
        let mut agent = Agent::start_set_to(&engine::under_test(), &folder, Some(handed), asking);
        // Were a question to come after all, it is answered no: nothing here may work this Mac.
        agent.answering = Some("reject_once");
        let (session, _) = agent.open(&folder);
        agent.set(&session, "model", json!("standin-anthropic/ok"));
        agent.say(&session, &format!("{MESSAGE}\n{}return await computer.capabilities()", standin::ASK_FOR_CODE)).unwrap_or_else(|error| panic!("a message was answered with an error: {error}"));
        let sent = last_sent(&standin);
        assert!(!sent.contains("axPermission"), "{why}, the harness ran computer use with nobody asked: {sent}");
        assert!(agent.questions.is_empty(), "{why}, the harness now asks about a code run through the protocol, so computer use can ask first in the box: {:?}", agent.questions);
        assert!(sent.contains("denied") || sent.contains("requires approval"), "{why}, the model was not told that the call was refused: {sent}");
    }
}

/// And acting freely, which is how computer use works in the box: a call that
/// only asks how the desktop stands is answered, with no question to Null. It
/// asks which of macOS's permissions this Mac has given. No window is read, no
/// picture is taken and nothing is pressed.
#[test]
#[ignore]
fn acting_freely_with_computer_use_handed_over_the_harness_says_how_the_desktop_stands() {
    let (standin, _folder, mut agent, session, _) = conversation_on("standin-anthropic/ok", Some(json!({ "computer": { "enabled": true } })));
    agent.answering = Some("reject_once");
    agent.say(&session, &format!("{MESSAGE}\n{}return await computer.capabilities()", standin::ASK_FOR_CODE)).unwrap_or_else(|error| panic!("a message was answered with an error: {error}"));
    let sent = last_sent(&standin);
    assert!(agent.questions.is_empty(), "acting freely, the harness asked all the same: {:?}", agent.questions);
    for word in ["axPermission", "capturePermission", "inputPermission"] {
        assert!(sent.contains(word), "the harness did not say how {word} stands: {sent}");
    }
}

/// To learn which model a conversation is on, Null sets one of its other
/// settings to the value that setting already has, and reads what comes back.
/// The harness has commands of its own that change such a setting, and the box
/// carries them: `/effort low` sets how hard the model thinks. So the harness
/// has to say so when one of them changes a setting, or Null would ask with
/// the old value and put it back. And asked with the value as it is now, the
/// setting has to stay as the person set it.
#[test]
#[ignore]
fn the_harness_says_so_when_its_own_command_changes_a_setting() {
    let (_standin, _folder, mut agent, session, settings) = conversation_that_thinks();
    let asked_with = |settings: &Value| translate::other_option(settings).filter(|(id, _)| id == "thinking").map(|(_, value)| value);
    let began = asked_with(&settings).expect("the setting Null asks with is how hard the model thinks");
    assert_ne!(began, json!("low"), "the level the conversation began on, which the command is to change");

    let before = agent.updates.len();
    agent.say(&session, "/effort low").expect("the harness answers its own command");
    let said: Vec<Value> = agent.heard(&session, before).into_iter().filter(|update| update["sessionUpdate"] == "config_option_update").collect();
    let now = said.last().and_then(|update| asked_with(&update["configOptions"]));
    assert_eq!(now, Some(json!("low")), "the harness did not say that its command changed the setting: {said:?}");

    // Asked with it as it is now, as Null asks after every reply, it stays.
    let after = agent.set(&session, "thinking", json!("low"));
    assert_eq!(asked_with(&after), Some(json!("low")), "the setting did not stay as the command left it");
}

/// A conversation on a stand-in model that says it can think, so that the
/// harness lets its level be set. Gives its settings as they are on that model.
fn conversation_that_thinks() -> (StandIn, Folder, Agent, String, Value) {
    let standin = StandIn::start();
    let folder = Folder::new(&standin);
    std::fs::write(folder.settings().join("models.yml"), standin.models_that(true)).unwrap();
    let mut agent = Agent::start(&engine::under_test(), &folder, None);
    let (session, _) = agent.open(&folder);
    let settings = agent.set(&session, "model", json!("standin-anthropic/ok"));
    (standin, folder, agent, session, settings)
}

/// The harness starts every conversation thinking at its own level. Null
/// remembers the level the person last set with `/effort`, and sets a new
/// conversation to it as it opens. So the harness has to name the levels a
/// conversation can be set to, a level set that way has to hold through a
/// reply and through a move to another model, the command's own change has to
/// come with the model unchanged, which is how Null tells it from a backup
/// taking over, and a level the harness does not know has to be refused.
#[test]
#[ignore]
fn a_thinking_level_set_as_a_conversation_opens_holds() {
    let (_standin, _folder, mut agent, session, settings) = conversation_that_thinks();
    let (levels, began) = translate::thinking_from_config_options(&settings);
    assert!(levels.iter().any(|level| level == "low"), "the levels the conversation can be set to: {levels:?}");
    assert_ne!(began.as_deref(), Some("low"), "the level the conversation began on, which Null is to change");

    let level = |settings: &Value| translate::thinking_from_config_options(settings).1;
    assert_eq!(level(&agent.set(&session, "thinking", json!("low"))).as_deref(), Some("low"));
    let before = agent.updates.len();
    agent.say(&session, MESSAGE).expect("the message is answered");
    let said: Vec<Value> = agent.heard(&session, before).into_iter().filter(|update| update["sessionUpdate"] == "config_option_update").collect();
    assert!(said.iter().all(|update| level(&update["configOptions"]).as_deref() == Some("low")), "a reply changed the level: {said:?}");
    assert_eq!(level(&agent.set(&session, "thinking", json!("low"))).as_deref(), Some("low"), "the level did not hold through a reply");

    // On another model, and back: the level goes with the conversation.
    assert_eq!(level(&agent.set(&session, "model", json!("standin-openai/ok"))).as_deref(), Some("low"), "the level did not hold through a move to another model");
    assert_eq!(level(&agent.set(&session, "model", json!("standin-anthropic/ok"))).as_deref(), Some("low"));

    // The person's own change says which model it was made on: the same one.
    let before = agent.updates.len();
    agent.say(&session, "/effort high").expect("the harness answers its own command");
    let said: Vec<Value> = agent.heard(&session, before).into_iter().filter(|update| update["sessionUpdate"] == "config_option_update").collect();
    let last = said.last().unwrap_or_else(|| panic!("the harness did not say that its command changed the level"));
    assert_eq!(level(&last["configOptions"]).as_deref(), Some("high"));
    assert_eq!(translate::models_from_config_options(&last["configOptions"]).1.as_deref(), Some("standin-anthropic/ok"), "the command's change did not name the model it was made on");

    let odd = agent.request("session/set_config_option", json!({ "sessionId": session, "configId": "thinking", "value": "no such level" }));
    assert!(odd.is_err(), "a level the harness does not know was taken: {odd:?}");
}

/// The person can name a second model that looks at an answer again
/// (`/verify`). Null then moves the conversation to that model once a reply
/// has ended, asks it in Null's own words, and puts the conversation back. So
/// a conversation has to take another model between two replies and keep what
/// was said in it, the second model has to be sent what went before, its
/// reply has to end as any reply does, and the conversation has to answer on
/// its own model again afterwards.
#[test]
#[ignore]
fn a_conversation_is_moved_to_a_second_model_for_one_reply_and_back() {
    let (standin, _folder, mut agent, session, _) = conversation_on("standin-anthropic/ok", None);
    agent.say(&session, &format!("{MESSAGE}\n{}true", standin::ASK_THE_SHELL)).expect("the message is answered");
    assert_eq!(standin.asked(), ["anthropic/ok", "anthropic/ok"], "a step, and then the answer");

    let (_, on) = translate::models_from_config_options(&agent.set(&session, "model", json!("standin-openai/ok")));
    assert_eq!(on.as_deref(), Some("standin-openai/ok"));
    let (ended, words) = agent.say(&session, crate::verify::ASK).expect("the second model answers");
    assert_eq!(standin.asked().last().map(String::as_str), Some("openai/ok"), "the second model was not the one asked");
    assert_eq!((ended["stopReason"].as_str(), words.as_str()), (Some("end_turn"), "pong"), "its reply did not end as a reply does");
    assert!(!translate::reply_failed(&ended), "its reply counted no tokens: {ended}");
    let sent = standin.heard().last().map(|request| request["messages"].to_string()).unwrap_or_default();
    assert!(sent.contains(MESSAGE), "the second model was not sent what went before: {sent}");
    assert!(sent.contains("the one word: stands"), "nor what Null asks of it: {sent}");

    let (_, on) = translate::models_from_config_options(&agent.set(&session, "model", json!("standin-anthropic/ok")));
    assert_eq!(on.as_deref(), Some("standin-anthropic/ok"));
    agent.say(&session, MESSAGE).expect("the next message is answered");
    assert_eq!(standin.asked().last().map(String::as_str), Some("anthropic/ok"), "put back, the conversation did not answer on its own model");
}

/// Why Null sets no model on a conversation while a backup is answering
/// (`harness.rs`, `on_a_backup`). Left alone, such a conversation goes
/// straight to the backup, as the question about the backup taking over has
/// it. Set back on the model that refused, as putting it back after `/verify`
/// would, the harness takes that model and tries it again before the backup
/// answers: the wait of a refusal, paid before every reply.
#[test]
#[ignore]
fn set_back_on_a_model_that_refused_the_harness_tries_it_again_before_the_backup() {
    let backups = backups::overlay(&order(&["standin-anthropic/ok"]), &Value::Null);
    let (standin, _folder, mut agent, session, _) = conversation_on("standin-codex/limit", backups);
    agent.say(&session, MESSAGE).expect("the backup answers");
    assert_eq!(standin.asked().last().map(String::as_str), Some("anthropic/ok"), "the backup did not take over: {:?}", standin.asked());

    let (_, on) = translate::models_from_config_options(&agent.set(&session, "model", json!("standin-codex/limit")));
    assert_eq!(on.as_deref(), Some("standin-codex/limit"), "the harness did not take the model back");
    let before = standin.asked().len();
    let (ended, words) = agent.say(&session, MESSAGE).expect("the next message is answered");
    let asked = standin.asked()[before..].to_vec();
    assert_eq!(words.trim(), "pong", "the backup's reply");
    assert!(!translate::reply_failed(&ended), "the backup's reply was taken for a refusal: {ended}");
    assert!(asked.first().map(String::as_str) == Some("codex/limit") && asked.last().map(String::as_str) == Some("anthropic/ok"), "set back on the model that refused, the message went to {asked:?}");
}

/// Null counts the steps of a reply and the steps among them that failed
/// (`routing.rs`), and has only the harness's word for either. A step begins
/// as one `tool_call`, and how it went comes in a `tool_call_update` for that
/// same step: `failed` when it failed, `completed` when it did not.
#[test]
#[ignore]
fn a_step_is_said_once_and_one_that_fails_is_said_to_have_failed() {
    let (_standin, _folder, mut agent, session, _) = conversation_on("standin-anthropic/ok", None);
    for (command, went) in [("false", "failed"), ("true", "completed")] {
        let before = agent.updates.len();
        agent.say(&session, &format!("{MESSAGE}\n{}{command}", standin::ASK_THE_SHELL)).expect("the message is answered");
        let heard = agent.heard(&session, before);
        let begun: Vec<&Value> = heard.iter().filter(|update| update["sessionUpdate"] == "tool_call").collect();
        assert_eq!(begun.len(), 1, "`{command}` is one step, said once: {heard:?}");
        let id = &begun[0]["toolCallId"];
        assert!(id.is_string(), "a step has an id: {}", begun[0]);
        let last = heard.iter().filter(|update| update["sessionUpdate"] == "tool_call_update" && &update["toolCallId"] == id).filter_map(|update| update["status"].as_str()).last();
        assert_eq!(last, Some(went), "how `{command}` went: {heard:?}");
    }
}
