//! The connection to the user's agent harness, over the Agent Client Protocol.
//!
//! The harness is the agent CLI behind the box (Oh-my-pi today): the copy Null
//! carries, or the user's own when they chose it (`engine.rs`). This module
//! starts it, holds on it the conversations this run knows, and turns what it
//! reports of each into the events the box shows. The harness keeps its own
//! sign-in, tools and transcript; nothing here touches a provider token.
//!
//! Each conversation has a record of its own, under a number Null gives it,
//! since the harness's id for one comes only when the first message opens it.
//! One of them is open: the one the box shows. Every event names the
//! conversation it belongs to, and the page draws the open one's.
//!
//! Two things the protocol does not carry over from the terminal, learned from
//! OMP: the approval mode the user configured, and the user's MCP servers. Both
//! are passed explicitly so the agent behind the box is the one the user gets in
//! a terminal.
//!
//! One thread owns the connection and takes its orders from a channel. The
//! harness process is started on the first order and started again, with the
//! conversation loaded back, if it has gone away.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use agent_client_protocol::schema::v1::{
    CancelNotification, InitializeRequest, LoadSessionRequest, NewSessionRequest, PromptRequest,
    RequestPermissionRequest, RequestPermissionResponse, SessionNotification, SetSessionConfigOptionRequest,
};
use agent_client_protocol::schema::v1::{CloseSessionRequest, ListSessionsRequest};
use agent_client_protocol::{AcpAgent, Agent, Client, ConnectionTo, Responder};
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, StreamExt};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::engine::{self, Origin};
use crate::log::log;
use crate::translate::{self, ModelChoice};
use crate::history;
use crate::{access, attach, backups, panel, providers, settings};

/// The harness this app drives. One for now; the name is also its CLI.
pub const HARNESS: &str = "omp";
pub const HARNESS_NAME: &str = "Oh-my-pi";

/// Events kept for a page that reloads or reopens. A long reply is a few hundred.
const MAX_EVENTS: usize = 5000;

/// How many pages of the harness's list of conversations are read, at most.
/// Oh-my-pi gives fifty to a page. A list that never ended would never be shown.
const MAX_PAGES: usize = 200;

#[derive(Clone, Debug, Serialize)]
pub struct ModelList {
    pub current: Option<String>,
    pub models: Vec<ModelChoice>,
}

enum Command {
    Send { which: u64, text: String, files: Vec<PathBuf> },
    Interrupt { which: u64 },
    /// Let the harness process go, so that the next order starts a fresh one.
    Restart,
    Models { reply: oneshot::Sender<Result<ModelList, String>> },
    SetModel { which: u64, id: String, reply: oneshot::Sender<Result<(), String>> },
    /// Ask the harness which model a conversation is on now. It can move to a
    /// backup by itself in the middle of a reply, and does not always say so.
    CheckModel { which: u64 },
    /// List the conversations the harness keeps, for `/history`: each as the
    /// harness gives it. No list when this harness cannot give one.
    Conversations { reply: oneshot::Sender<Result<Option<Vec<Value>>, String>> },
    /// Open one of them in the box, in place of the conversation that is open.
    OpenEarlier { id: String, reply: oneshot::Sender<Result<(), String>> },
    /// Have the harness let go of a conversation, if it holds it, so that the
    /// conversation's file is nobody's. Answers when that is done.
    Close { id: String, reply: oneshot::Sender<()> },
}

/// What to do with the updates a harness replays while a conversation is loaded back.
#[derive(Clone, Copy, Default, PartialEq)]
enum Replay {
    /// Not loading: every update is live.
    #[default]
    Live,
    /// The box already shows this conversation; drop the replay.
    Drop,
    /// The box is empty (the app was restarted); show the conversation again.
    Show,
}

/// One conversation this run knows: how it stands, and what was said in it
/// since Null came to hold it. The harness keeps the conversation itself.
#[derive(Default)]
struct Conversation {
    /// The harness's id for it. None until the harness has opened it, which
    /// the first message does.
    session: Option<String>,
    /// Whether the harness process that is running holds it in memory.
    loaded: bool,
    events: VecDeque<(u64, Value)>,
    busy: bool,
    model: Option<String>,
    tools: HashMap<String, Value>,
    pending: HashMap<String, Responder<RequestPermissionResponse>>,
    replay: Replay,
    /// True while Null itself is opening the conversation or setting its model. A
    /// change of model then is Null's doing, not the harness moving to a backup.
    settling: bool,
    /// A setting of the conversation other than its model, and its value. Setting
    /// it to that same value changes nothing and answers with the model in use.
    ask: Option<(String, Value)>,
}

struct Shared {
    /// The conversations this run knows, each under the number Null gave it.
    held: HashMap<u64, Conversation>,
    /// The number of the one the box shows. It is always among them.
    open: u64,
    /// The last number given to a conversation.
    numbered: u64,
    /// The last number given to an event. One count for every conversation, so
    /// that a page asking for what it missed asks with one number, whichever
    /// conversation it shows.
    seq: u64,
    models: Vec<ModelChoice>,
    approvals: u64,
    /// The version of the harness whose abilities were last noted. Another
    /// version is found out about afresh.
    noted: Option<String>,
    /// Whether this harness has been seen to count a reply's tokens. Until it
    /// has, a reply without a count is not taken for a provider's refusal.
    counts_tokens: bool,
    /// A conversation being opened from the list, by the harness's id for it,
    /// and what the harness has replayed of it so far. Nothing of it is shown
    /// until it has loaded.
    opening: Option<(String, Vec<Value>)>,
}

impl Shared {
    /// A box with one conversation in it: the one that was open last, to be
    /// loaded back, or a new one that the first message opens.
    fn new(session: Option<String>, model: Option<String>) -> Shared {
        let mut shared = Shared { held: HashMap::new(), open: 0, numbered: 0, seq: 0, models: Vec::new(), approvals: 0, noted: None, counts_tokens: false, opening: None };
        shared.open = shared.hold(Conversation { session, model, ..Default::default() });
        shared
    }

    /// Come to hold a conversation. Gives the number it is known by from now on.
    fn hold(&mut self, conversation: Conversation) -> u64 {
        self.numbered += 1;
        self.held.insert(self.numbered, conversation);
        self.numbered
    }

    /// One conversation's record, if this run still holds it.
    fn of(&mut self, which: u64) -> Option<&mut Conversation> {
        self.held.get_mut(&which)
    }

    /// The record of the conversation the box shows.
    fn open_one(&mut self) -> &mut Conversation {
        self.held.entry(self.open).or_default()
    }

    /// The number of the conversation the harness knows by this id, when this run holds it.
    fn named(&self, session: &str) -> Option<u64> {
        self.held.iter().find(|(_, held)| held.session.as_deref() == Some(session)).map(|(which, _)| *which)
    }

    /// Keep an event with its conversation. Gives the event's number, or None
    /// for a conversation this run no longer holds.
    fn record(&mut self, which: u64, event: Value) -> Option<u64> {
        let conversation = self.held.get_mut(&which)?;
        self.seq += 1;
        conversation.events.push_back((self.seq, event));
        while conversation.events.len() > MAX_EVENTS {
            conversation.events.pop_front();
        }
        Some(self.seq)
    }

    /// The box turns to another conversation, and the one it showed is held
    /// no longer. Gives the number of the one it shows now.
    fn turn_to(&mut self, conversation: Conversation) -> u64 {
        let was = self.open;
        self.open = self.hold(conversation);
        self.held.remove(&was);
        self.open
    }

    /// Start over: the box turns to a new conversation, which the next message
    /// opens on the model the last one was on.
    fn start_over(&mut self) -> u64 {
        let model = self.open_one().model.clone();
        self.turn_to(Conversation { model, ..Default::default() })
    }

    /// Take an approval that waits for its answer, with the number of the conversation that asked.
    fn waiting(&mut self, request_id: &str) -> Option<(u64, Responder<RequestPermissionResponse>)> {
        self.held.iter_mut().find_map(|(which, held)| held.pending.remove(request_id).map(|responder| (*which, responder)))
    }

    /// The harness process has gone, and with it whatever it held in memory.
    fn let_go(&mut self) {
        for conversation in self.held.values_mut() {
            conversation.loaded = false;
            conversation.tools.clear();
        }
    }

    /// What the open conversation has said after event `after`, and how it stands.
    fn since(&mut self, after: u64) -> Snapshot {
        let (conversation, last_seq) = (self.open, self.seq);
        let open = self.open_one();
        let events = open.events.iter().filter(|(seq, _)| *seq > after).map(|(seq, event)| json!({ "seq": seq, "conversation": conversation, "event": event })).collect();
        Snapshot { conversation, busy: open.busy, model: open.model.clone(), last_seq, events }
    }
}

/// The app's handle on the harness thread.
pub struct Harness {
    shared: Arc<Mutex<Shared>>,
    commands: mpsc::UnboundedSender<Command>,
}

fn lock(shared: &Arc<Mutex<Shared>>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Do something with one conversation's record, if this run still holds it.
fn with<T>(shared: &Arc<Mutex<Shared>>, which: u64, see: impl FnOnce(&mut Conversation) -> T) -> Option<T> {
    lock(shared).of(which).map(see)
}

/// Keep an event with its conversation and tell the page, which draws it when
/// that is the conversation it shows.
fn publish(app: &AppHandle, shared: &Arc<Mutex<Shared>>, which: u64, event: Value) {
    let Some(seq) = lock(shared).record(which, event.clone()) else { return };
    let _ = app.emit_to(panel::LABEL, "mini:event", json!({ "seq": seq, "conversation": which, "event": event }));
}

fn typed<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("could not build a request: {e}"))
}

fn as_json<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// A protocol error as the box shows it: the agent's own words when it gave any.
fn error_event(error: &agent_client_protocol::Error) -> Value {
    let raw = as_json(error);
    let message = raw.get("message").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| error.to_string());
    let details = match raw.get("data") {
        Some(Value::Object(data)) => data.get("details").and_then(Value::as_str).map(str::to_string),
        Some(Value::String(text)) => Some(text.clone()),
        _ => None,
    };
    json!({ "type": "error", "message": message, "code": raw.get("code").cloned().unwrap_or(Value::Null), "details": details })
}

fn describe(error: &agent_client_protocol::Error) -> String {
    let event = error_event(error);
    match event["details"].as_str() {
        Some(details) => details.to_string(),
        None => event["message"].as_str().unwrap_or("the harness reported an error").to_string(),
    }
}

/// End a conversation's reply in flight, if there is one, with an error the box can show.
fn fail_reply(app: &AppHandle, shared: &Arc<Mutex<Shared>>, which: u64, event: Value) {
    if !with(shared, which, |held| held.busy).unwrap_or(false) {
        return;
    }
    publish(app, shared, which, json!({ "type": "status_change", "status": "blocked", "reason": event["message"] }));
    publish(app, shared, which, event);
    finish(shared, which);
}

/// The reply is over: free the conversation and answer any approval still open as cancelled.
fn finish(shared: &Arc<Mutex<Shared>>, which: u64) {
    let pending: Vec<_> = with(shared, which, |held| {
        held.busy = false;
        held.pending.drain().map(|(_, responder)| responder).collect()
    })
    .unwrap_or_default();
    for responder in pending {
        if let Ok(response) = typed::<RequestPermissionResponse>(json!({ "outcome": { "outcome": "cancelled" } })) {
            let _ = responder.respond(response);
        }
    }
}

// ── Start-up ──────────────────────────────────────────────────────────────

/// Start the harness thread. Nothing is launched until the first order arrives.
pub fn init(app: &AppHandle) {
    let saved = settings::get(app);
    let shared = Arc::new(Mutex::new(Shared::new(saved.session, saved.model)));
    let (commands, orders) = mpsc::unbounded();
    app.manage(Harness { shared: shared.clone(), commands });

    let handle = app.clone();
    let spawned = std::thread::Builder::new().name("mini-harness".into()).spawn(move || {
        futures::executor::block_on(run(handle, shared, orders));
    });
    if let Err(e) = spawned {
        log!("could not start the harness thread: {e}");
    }
}

async fn run(app: AppHandle, shared: Arc<Mutex<Shared>>, mut orders: mpsc::UnboundedReceiver<Command>) {
    while let Some(first) = orders.next().await {
        if let Err(reason) = serve(&app, &shared, first, &mut orders).await {
            log!("the harness connection ended: {reason}");
            // Every reply that was under way ends with it.
            let working: Vec<u64> = lock(&shared).held.iter().filter(|(_, held)| held.busy).map(|(which, _)| *which).collect();
            for which in working {
                fail_reply(&app, &shared, which, json!({ "type": "error", "message": reason, "code": null, "details": null }));
            }
        }
    }
}

/// Answer an order that cannot be carried out because there is no harness to give it to.
fn refuse(app: &AppHandle, shared: &Arc<Mutex<Shared>>, command: Command, reason: &str) {
    match command {
        Command::Send { which, .. } => {
            fail_reply(app, shared, which, json!({ "type": "error", "message": reason, "code": null, "details": null }));
        }
        Command::Models { reply } => {
            let _ = reply.send(Err(reason.to_string()));
        }
        Command::SetModel { reply, .. } => {
            let _ = reply.send(Err(reason.to_string()));
        }
        Command::Conversations { reply } => {
            let _ = reply.send(Err(reason.to_string()));
        }
        Command::OpenEarlier { reply, .. } => {
            let _ = reply.send(Err(reason.to_string()));
        }
        Command::Close { reply, .. } => {
            let _ = reply.send(());
        }
        Command::Interrupt { .. } | Command::Restart | Command::CheckModel { .. } => {}
    }
}

/// Start the harness and carry out orders on it until it goes away or the app quits.
async fn serve(
    app: &AppHandle,
    shared: &Arc<Mutex<Shared>>,
    first: Command,
    orders: &mut mpsc::UnboundedReceiver<Command>,
) -> Result<(), String> {
    if matches!(first, Command::Restart | Command::CheckModel { .. }) {
        return Ok(()); // nothing is running; the next order starts a fresh process anyway
    }
    // No harness is running, so none holds the conversation. It is not started to be asked.
    let first = match first {
        Command::Close { reply, .. } => {
            let _ = reply.send(());
            return Ok(());
        }
        other => other,
    };
    let Some((binary, origin)) = engine::in_use(app) else {
        refuse(app, shared, first, &format!("{HARNESS_NAME} is not installed"));
        return Ok(());
    };
    let mut argv = vec![binary.display().to_string()];
    argv.extend(extra_args());
    argv.extend(launch_args(&binary));
    argv.extend(settings_args(app, &binary));
    argv.push("acp".into());
    let whose = if origin == Origin::BuiltIn { "the built-in harness" } else { "the user's own harness" };
    log!("starting {whose}: {}", argv.join(" "));
    let agent = AcpAgent::from_args(argv).map_err(|e| e.to_string())?;

    let updates = (app.clone(), shared.clone());
    let approvals = (app.clone(), shared.clone());
    let outcome = Client
        .builder()
        .name("null-mini")
        .on_receive_notification(
            async move |notification: SessionNotification, _cx| {
                on_update(&updates.0, &updates.1, &notification);
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request: RequestPermissionRequest, responder, _cx| {
                on_permission(&approvals.0, &approvals.1, &request, responder);
                Ok(())
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(agent, async |cx: ConnectionTo<Agent>| Ok(converse(cx, app, shared, first, orders).await))
        .await;

    // Whatever this process held is gone with it.
    lock(shared).let_go();
    match outcome {
        Ok(result) => result,
        Err(error) => Err(describe(&error)),
    }
}

/// The settings Null hands the harness at every start. Always that it is not to
/// look for a newer version of itself: Null runs the version it chose, and moves
/// on only with a new Null. And the backup order, when there is one.
pub fn own_settings(backups: Option<Value>) -> Value {
    let mut settings = json!({ "startup": { "checkUpdate": false } });
    if let (Some(all), Some(Value::Object(more))) = (settings.as_object_mut(), backups) {
        all.extend(more);
    }
    settings
}

/// What to add when starting the harness so that it reads those settings. They
/// go in a file of Null's own beside Null's settings, written afresh each time;
/// the user's own Oh-my-pi settings are not touched. The text is JSON, which the
/// harness's settings format accepts.
fn settings_args(app: &AppHandle, binary: &Path) -> Vec<String> {
    let Some(dir) = settings::dir(app) else { return Vec::new() };
    let path = dir.join(settings::file_name("harness", "yml"));
    let _ = std::fs::create_dir_all(&dir);
    // The file had another name while the backup order was all it held.
    let _ = std::fs::remove_file(dir.join(settings::file_name("backups", "yml")));
    match std::fs::write(&path, own_settings(backups::harness_settings(app, binary)).to_string()) {
        Ok(()) => vec!["--config".into(), path.display().to_string()],
        Err(e) => {
            log!("could not write the harness's settings to {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// Arguments every run of the harness gets. `NULL_MINI_PROFILE` names an isolated
/// harness profile, so that sign-in and first-run behaviour can be checked
/// without touching the user's real sign-ins. A run on a folder of its own
/// (`NULL_MINI_FOLDER`) names no profile: the folder is what keeps it apart.
pub fn extra_args() -> Vec<String> {
    if settings::own_folder().is_some() {
        return Vec::new();
    }
    match std::env::var("NULL_MINI_PROFILE") {
        Ok(profile) if !profile.is_empty() => vec!["--profile".into(), profile],
        _ => Vec::new(),
    }
}

/// What the harness is set to do about approvals: "always-ask", "write", or
/// "yolo", which lets the agent act without asking. None when it does not say.
pub fn approval_mode(binary: &Path) -> Option<String> {
    let output = std::process::Command::new(binary).args(extra_args()).args(["config", "get", "tools.approvalMode"]).output();
    let mode = output.ok().map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string()).unwrap_or_default();
    matches!(mode.as_str(), "always-ask" | "write" | "yolo").then_some(mode)
}

/// Carry the approval mode the user set for the terminal into the protocol, which ignores it.
fn launch_args(binary: &Path) -> Vec<String> {
    match approval_mode(binary) {
        Some(mode) => vec!["--approval-mode".into(), mode],
        None => Vec::new(),
    }
}

/// The MCP servers the harness loads in a terminal, in the form `session/new` accepts.
fn mcp_servers(capabilities: &Value) -> Vec<Value> {
    // A run on a folder of its own reads that folder's servers: none, unless a check put some there.
    let path = match (settings::harness_folder(), std::env::var_os("HOME")) {
        (Some(folder), _) => folder.join("mcp.json"),
        (None, Some(home)) => PathBuf::from(home).join(".omp/agent/mcp.json"),
        (None, None) => return Vec::new(),
    };
    let Some(config) = std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok()) else {
        return Vec::new();
    };
    let mut servers = Vec::new();
    for (name, entry) in config.get("mcpServers").and_then(Value::as_object).into_iter().flatten() {
        match translate::mcp_server_from_config(name, entry, capabilities) {
            Some(server) => servers.push(server),
            None => log!("MCP server {name} cannot be attached over the protocol; skipped"),
        }
    }
    servers
}

/// Where a conversation works: a folder of its own under the app's support directory.
pub fn workspace(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = match settings::own_folder() {
        Some(folder) => folder.join("null"),
        None => app.path().app_data_dir().map_err(|e| e.to_string())?,
    }
    .join("Workspace");
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

// ── One connection ────────────────────────────────────────────────────────

struct Connection<'a> {
    cx: ConnectionTo<Agent>,
    app: &'a AppHandle,
    shared: &'a Arc<Mutex<Shared>>,
    mcp_capabilities: Value,
    can_load: bool,
    /// Whether the harness lists the conversations it keeps. If not, `/history` has nothing to show.
    can_list: bool,
    /// Whether the harness lets go of a conversation when asked.
    can_close: bool,
    /// The harness's answer to the greeting, until what it can do has been noted
    /// in the log: once for each version met.
    unnoted: Option<Value>,
}

async fn converse(
    cx: ConnectionTo<Agent>,
    app: &AppHandle,
    shared: &Arc<Mutex<Shared>>,
    first: Command,
    orders: &mut mpsc::UnboundedReceiver<Command>,
) -> Result<(), String> {
    let request: InitializeRequest = typed(json!({
        "protocolVersion": 1,
        "clientCapabilities": {},
        "clientInfo": { "name": "null-mini", "title": "Null", "version": env!("CARGO_PKG_VERSION") },
    }))?;
    let init = as_json(&cx.send_request(request).block_task().await.map_err(|e| describe(&e))?);
    if init.get("protocolVersion") != Some(&json!(1)) {
        return Err(format!("{HARNESS_NAME} speaks protocol version {}; Null speaks version 1", init["protocolVersion"]));
    }
    let version = init.pointer("/agentInfo/version").and_then(Value::as_str).unwrap_or("?").to_string();
    log!("harness ready: {} {version}", init.pointer("/agentInfo/name").and_then(Value::as_str).unwrap_or("?"));

    // A harness Null has not met in this run is found out about, not assumed.
    let unnoted = {
        let mut shared = lock(shared);
        let unnoted = shared.noted.as_deref() != Some(version.as_str());
        if unnoted {
            shared.noted = Some(version.clone());
            // Null was checked with the built-in version, which counts tokens. Any other
            // has to be seen to, once: what was seen is remembered between runs.
            shared.counts_tokens = engine::checked(app, &version) || settings::get(app).counts_tokens.as_deref() == Some(version.as_str());
        }
        unnoted
    };

    let mut connection = Connection {
        cx,
        app,
        shared,
        mcp_capabilities: init.pointer("/agentCapabilities/mcpCapabilities").cloned().unwrap_or(Value::Null),
        can_load: init.pointer("/agentCapabilities/loadSession") == Some(&json!(true)),
        can_list: translate::lists_conversations(&init),
        can_close: translate::closes_conversations(&init),
        unnoted: unnoted.then_some(init),
    };

    let mut next = Some(first);
    loop {
        let command = match next.take() {
            Some(command) => command,
            None => {
                futures::select! {
                    command = orders.next() => match command {
                        Some(command) => command,
                        None => return Ok(()), // the app is quitting
                    },
                    _ = connection.cx.incoming_closed().fuse() => return Err(format!("{HARNESS_NAME} exited")),
                }
            }
        };
        if matches!(command, Command::Restart) {
            log!("letting the harness process go; the next order starts a fresh one");
            return Ok(());
        }
        connection.handle(command).await?;
    }
}

impl Connection<'_> {
    /// Carry out one order. An error here means the connection itself is no good.
    async fn handle(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Send { which, text, files } => {
                let session = match self.open(which).await {
                    Ok(session) => session,
                    Err(reason) => {
                        fail_reply(self.app, self.shared, which, json!({ "type": "error", "message": reason, "code": null, "details": null }));
                        return Ok(());
                    }
                };
                history::said(self.app, &session, &text);
                let request: PromptRequest = typed(json!({ "sessionId": session, "prompt": attach::blocks(&text, &files) }))?;
                let with_files = !files.is_empty();
                let (app, shared) = (self.app.clone(), self.shared.clone());
                self.cx
                    .send_request(request)
                    .on_receiving_result(async move |result| {
                        match result {
                            Ok(response) => {
                                let response = as_json(&response);
                                let reason = response["stopReason"].as_str().map(translate::stop_reason).unwrap_or("completed");
                                // The harness reports a provider's refusal as a reply like any other.
                                let (failed, learned) = {
                                    let mut shared = lock(&shared);
                                    let (failed, counts) = translate::judge_reply(&response, shared.counts_tokens);
                                    let learned = (counts && !shared.counts_tokens).then(|| shared.noted.clone()).flatten();
                                    shared.counts_tokens = counts;
                                    (failed, learned)
                                };
                                if let Some(version) = learned {
                                    log!("this harness counts a reply's tokens; from now on a reply without a count is taken for a refusal");
                                    settings::update(&app, |settings| settings.counts_tokens = Some(version));
                                }
                                log!("reply ended: {reason}{}", if failed { ", with nothing from the model" } else { "" });
                                if failed {
                                    providers::forget(&app);
                                    publish(&app, &shared, which, json!({ "type": "reply_failed" }));
                                }
                                publish(&app, &shared, which, json!({ "type": "status_change", "status": "ready", "reason": null }));
                                publish(&app, &shared, which, json!({ "type": "message_done", "stop_reason": reason }));
                                finish(&shared, which);
                                // The user has now seen Null answer: the moment to ask for Full Disk Access.
                                if !failed && reason == "completed" {
                                    access::after_reply(&app);
                                }
                                let _ = order(&app.state::<Harness>(), Command::CheckModel { which });
                            }
                            Err(error) => fail_reply(&app, &shared, which, attach::refused(error_event(&error), with_files)),
                        }
                        Ok(())
                    })
                    .map_err(|e| describe(&e))?;
            }
            Command::Interrupt { which } => {
                // Only a reply under way, in a conversation this process holds, can be stopped.
                let session = with(self.shared, which, |held| held.session.clone().filter(|_| held.loaded && held.busy)).flatten();
                if let Some(session) = session {
                    // The protocol requires every open permission request to be answered "cancelled".
                    let pending: Vec<_> = with(self.shared, which, |held| held.pending.drain().map(|(_, responder)| responder).collect()).unwrap_or_default();
                    for responder in pending {
                        let _ = responder.respond(typed(json!({ "outcome": { "outcome": "cancelled" } }))?);
                    }
                    let cancel: CancelNotification = typed(json!({ "sessionId": session }))?;
                    self.cx.send_notification(cancel).map_err(|e| describe(&e))?;
                }
            }
            Command::Restart => {} // taken by the loop that calls this
            Command::Models { reply } => {
                let which = lock(self.shared).open;
                let result = self.open(which).await.map(|_| {
                    let mut shared = lock(self.shared);
                    ModelList { current: shared.of(which).and_then(|held| held.model.clone()), models: shared.models.clone() }
                });
                let _ = reply.send(result);
            }
            Command::SetModel { which, id, reply } => {
                let result = self.set_model(which, &id).await;
                let _ = reply.send(result);
            }
            Command::CheckModel { which } => self.check_model(which).await,
            Command::Conversations { reply } => {
                let _ = reply.send(self.conversations().await);
            }
            Command::OpenEarlier { id, reply } => {
                let _ = reply.send(self.open_earlier(&id).await);
            }
            Command::Close { id, reply } => {
                self.close(&id).await;
                let _ = reply.send(());
            }
        }
        Ok(())
    }

    /// Have the harness let go of a conversation. One it does not hold is
    /// closed like any other: the harness says nothing either way. If this
    /// run knows the conversation, the process holds it no longer, and the
    /// next message to it loads it back.
    async fn close(&mut self, id: &str) {
        if !self.can_close {
            return;
        }
        let Ok(request) = typed::<CloseSessionRequest>(json!({ "sessionId": id })) else { return };
        if let Err(error) = self.cx.send_request(request).block_task().await {
            log!("could not close the conversation {id}: {}", describe(&error));
        }
        let mut shared = lock(self.shared);
        if let Some(held) = shared.named(id).and_then(|which| shared.of(which)) {
            held.loaded = false;
        }
    }

    /// Every conversation the harness keeps, each as the harness lists it, the
    /// most recently used first. The list comes a page at a time. No list when
    /// this harness does not give one.
    async fn conversations(&mut self) -> Result<Option<Vec<Value>>, String> {
        if !self.can_list {
            return Ok(None);
        }
        let mut listed = Vec::new();
        let mut next: Option<String> = None;
        for _ in 0..MAX_PAGES {
            let request: ListSessionsRequest = typed(json!({ "cursor": next }))?;
            let answer = as_json(&self.cx.send_request(request).block_task().await.map_err(|e| describe(&e))?);
            let (kept, more) = history::page(&answer);
            if kept.is_empty() {
                break;
            }
            listed.extend(kept);
            next = more;
            if next.is_none() {
                break;
            }
        }
        Ok(Some(listed))
    }

    /// The folder a conversation works in, by the harness's own list. The
    /// harness loads a conversation back only in that folder, and one made in
    /// a terminal is not in Null's.
    async fn folder_of(&mut self, session: &str) -> Option<PathBuf> {
        let listed = self.conversations().await.ok().flatten()?;
        history::folder_of(&listed, session)
    }

    /// Open an earlier conversation in the box, in place of the one that is
    /// open, and show what was said in it. What the harness replays is kept
    /// back until the harness has said that it loaded, so a conversation that
    /// will not load leaves the box, and the conversation open in it, as they were.
    async fn open_earlier(&mut self, id: &str) -> Result<(), String> {
        if lock(self.shared).open_one().busy {
            return Err("stop the reply before opening another conversation".into());
        }
        if !self.can_load {
            return Err("this harness does not load a conversation back".into());
        }
        let folder = self.folder_of(id).await.ok_or("the harness no longer lists it")?;
        let servers = mcp_servers(&self.mcp_capabilities);
        let request: LoadSessionRequest = typed(json!({ "sessionId": id, "cwd": folder, "mcpServers": servers }))?;
        lock(self.shared).opening = Some((id.to_string(), Vec::new()));
        let loaded = self.cx.send_request(request).block_task().await;
        let replayed = lock(self.shared).opening.take().map(|(_, updates)| updates).unwrap_or_default();
        let response = match loaded {
            Ok(response) => as_json(&response),
            // The harness's own words for it are no use to the person: they go in the log.
            Err(error) => {
                log!("could not open the earlier conversation {id}: {}", describe(&error));
                return Err("the harness would not load it".into());
            }
        };

        // The box shows this one from now on, until told otherwise on the model it showed before.
        let (which, before) = {
            let mut shared = lock(self.shared);
            let before = shared.open_one().model.clone();
            (shared.turn_to(Conversation { session: Some(id.to_string()), loaded: true, model: before.clone(), ..Default::default() }), before)
        };
        settings::update(self.app, |settings| settings.session = Some(id.to_string()));
        self.absorb_options(which, &response["configOptions"]);
        log!("opened the earlier conversation {id} in {}", folder.display());

        let events = history::replayed(&replayed);
        history::opened(self.app, id, &events);
        // An earlier conversation is on the model it was left on, which may be another.
        let model = with(self.shared, which, |held| held.model.clone()).flatten().filter(|model| before.as_ref() != Some(model));
        publish(self.app, self.shared, which, json!({ "type": "conversation_opened", "events": events, "model": model }));
        Ok(())
    }

    /// Learn which model a conversation is on, and say so if the harness moved it.
    async fn check_model(&mut self, which: u64) {
        let asked = with(self.shared, which, |held| (held.session.clone(), held.ask.clone(), held.loaded));
        let Some((Some(session), Some((id, value)), true)) = asked else { return };
        let Ok(request) = typed::<SetSessionConfigOptionRequest>(json!({ "sessionId": session, "configId": id, "value": value })) else { return };
        match self.cx.send_request(request).block_task().await {
            Ok(response) => {
                let (_, current) = translate::models_from_config_options(&as_json(&response)["configOptions"]);
                moved(self.app, self.shared, which, current);
            }
            Err(error) => log!("could not ask which model the conversation is on: {}", describe(&error)),
        }
    }

    /// The harness's id for a conversation, opening a new one or loading the saved one back as needed.
    async fn open(&mut self, which: u64) -> Result<String, String> {
        with(self.shared, which, |held| held.settling = true);
        let opened = self.open_quietly(which).await;
        with(self.shared, which, |held| held.settling = false);
        opened
    }

    async fn open_quietly(&mut self, which: u64) -> Result<String, String> {
        let (saved, loaded) = with(self.shared, which, |held| (held.session.clone(), held.loaded)).ok_or("that conversation is not held any more")?;
        if let (Some(session), true) = (&saved, loaded) {
            return Ok(session.clone());
        }
        let cwd = workspace(self.app)?;
        let servers = mcp_servers(&self.mcp_capabilities);

        if let (Some(session), true) = (saved, self.can_load) {
            // One opened from `/history` may have been made in a terminal, in a folder of its own.
            let folder = self.folder_of(&session).await.unwrap_or_else(|| cwd.clone());
            with(self.shared, which, |held| held.replay = if held.events.is_empty() { Replay::Show } else { Replay::Drop });
            let request: LoadSessionRequest = typed(json!({ "sessionId": session, "cwd": folder, "mcpServers": servers }))?;
            let loaded = self.cx.send_request(request).block_task().await;
            with(self.shared, which, |held| held.replay = Replay::Live);
            match loaded {
                Ok(response) => {
                    self.absorb_options(which, &as_json(&response)["configOptions"]);
                    with(self.shared, which, |held| held.loaded = true);
                    log!("loaded the conversation {session} back");
                    return Ok(session);
                }
                // The harness no longer has it. Start a new conversation instead.
                Err(error) => log!("could not load the conversation {session}: {}", describe(&error)),
            }
        }

        let request: NewSessionRequest = typed(json!({ "cwd": cwd, "mcpServers": servers }))?;
        let response = as_json(&self.cx.send_request(request).block_task().await.map_err(|e| describe(&e))?);
        let session = response["sessionId"].as_str().ok_or("the harness opened a session without an id")?.to_string();
        let wanted = with(self.shared, which, |held| held.model.clone()).flatten();
        self.absorb_options(which, &response["configOptions"]);
        let open = {
            let mut shared = lock(self.shared);
            if let Some(held) = shared.of(which) {
                held.session = Some(session.clone());
                held.loaded = true;
            }
            shared.open == which
        };
        // Null remembers which conversation the box showed, to show it again after a restart.
        if open {
            settings::update(self.app, |settings| settings.session = Some(session.clone()));
        }
        log!("opened the conversation {session} in {}", cwd.display());

        // New conversations start on the model last chosen, when the harness still offers it.
        if let Some(wanted) = wanted {
            let (offered, current) = {
                let mut shared = lock(self.shared);
                (shared.models.iter().any(|model| model.id == wanted), shared.of(which).and_then(|held| held.model.clone()))
            };
            if offered && current.as_deref() != Some(&wanted) {
                if let Err(reason) = self.apply_model(which, &session, &wanted).await {
                    log!("staying on {}: {reason}", current.as_deref().unwrap_or("the default model"));
                }
            } else if !offered {
                log!("{wanted} is no longer offered; staying on {}", current.as_deref().unwrap_or("the default model"));
                settings::update(self.app, |settings| settings.model = current.clone());
            }
        }
        Ok(session)
    }

    /// Move a conversation to another model. It keeps what was said so far.
    async fn set_model(&mut self, which: u64, id: &str) -> Result<(), String> {
        if with(self.shared, which, |held| held.busy).unwrap_or(false) {
            return Err("stop the reply before switching models".into());
        }
        let session = self.open(which).await?;
        self.apply_model(which, &session, id).await
    }

    async fn apply_model(&mut self, which: u64, session: &str, id: &str) -> Result<(), String> {
        {
            let mut shared = lock(self.shared);
            if shared.of(which).is_some_and(|held| held.model.as_deref() == Some(id)) {
                return Ok(());
            }
            if !shared.models.is_empty() && !shared.models.iter().any(|model| model.id == id) {
                return Err(format!("{HARNESS_NAME} has no model named {id}"));
            }
        }
        let request: SetSessionConfigOptionRequest = typed(json!({ "sessionId": session, "configId": "model", "value": id }))?;
        let was_settling = with(self.shared, which, |held| std::mem::replace(&mut held.settling, true)).unwrap_or(false);
        let response = self.cx.send_request(request).block_task().await;
        if let Ok(response) = &response {
            self.absorb_options(which, &as_json(response)["configOptions"]);
            with(self.shared, which, |held| held.model = Some(id.to_string()));
        }
        with(self.shared, which, |held| held.settling = was_settling);
        response.map_err(|e| describe(&e))?;
        settings::update(self.app, |settings| settings.model = Some(id.to_string()));
        publish(self.app, self.shared, which, json!({ "type": "model_changed", "model": id }));
        Ok(())
    }

    /// Take in a conversation's settings as the harness gives them: the models
    /// offered, which are the same for every conversation, and for this one
    /// the model it is on and a setting to ask about that model with.
    fn absorb_options(&mut self, which: u64, options: &Value) {
        if let Some(greeting) = self.unnoted.take() {
            log!("what this harness can do: {}", translate::abilities(&greeting, options));
        }
        let (models, current) = translate::models_from_config_options(options);
        if models.is_empty() {
            return;
        }
        let mut shared = lock(self.shared);
        shared.models = models;
        if let Some(held) = shared.of(which) {
            if current.is_some() {
                held.model = current;
            }
            held.ask = translate::other_option(options);
        }
    }
}

// ── Calls from the harness ────────────────────────────────────────────────

/// Take note of the model the harness says a conversation is on. When that
/// is not the model Null left it on, the harness has moved to a backup, and
/// the box says so: a switch is never silent.
fn moved(app: &AppHandle, shared: &Arc<Mutex<Shared>>, which: u64, current: Option<String>) {
    let Some(current) = current else { return };
    let from = {
        let mut shared = lock(shared);
        let Some(held) = shared.of(which) else { return };
        if held.model.as_deref() == Some(current.as_str()) {
            return;
        }
        let from = held.model.replace(current.clone());
        if held.settling {
            return;
        }
        from
    };
    log!("the harness moved the conversation from {} to {current}", from.as_deref().unwrap_or("its first model"));
    publish(app, shared, which, json!({ "type": "model_switched", "from": from, "to": current }));
}

fn on_update(app: &AppHandle, shared: &Arc<Mutex<Shared>>, notification: &SessionNotification) {
    let notification = as_json(notification);
    let update = &notification["update"];
    let session = notification["sessionId"].as_str().unwrap_or_default();
    let which = {
        let mut shared = lock(shared);
        // A conversation being opened from the list: nothing of it is shown
        // until the harness has said that it loaded.
        if let Some(opening) = shared.opening.as_mut().filter(|opening| opening.0 == session) {
            opening.1.push(update.clone());
            return;
        }
        // What is said of a conversation this run does not hold is nobody's.
        let Some(which) = shared.named(session) else { return };
        which
    };
    if update["sessionUpdate"] == "config_option_update" {
        let (models, current) = translate::models_from_config_options(&update["configOptions"]);
        if !models.is_empty() {
            lock(shared).models = models;
        }
        moved(app, shared, which, current);
        return;
    }
    // The box already shows a conversation that is being loaded back after the harness went away.
    let event = with(shared, which, |held| if held.replay == Replay::Drop { None } else { translate::event_from_update(update, &mut held.tools) }).flatten();
    if let Some(event) = event {
        publish(app, shared, which, event);
    }
}

fn on_permission(
    app: &AppHandle,
    shared: &Arc<Mutex<Shared>>,
    request: &RequestPermissionRequest,
    responder: Responder<RequestPermissionResponse>,
) {
    let request = as_json(request);
    let call_id = request.pointer("/toolCall/toolCallId").and_then(Value::as_str).unwrap_or_default().to_string();
    let (which, request_id, title) = {
        let mut shared = lock(shared);
        // Only a conversation that is working has somebody there to ask.
        let asking = shared.named(request["sessionId"].as_str().unwrap_or_default()).filter(|which| shared.held.get(which).is_some_and(|held| held.busy));
        let Some(which) = asking else {
            drop(shared);
            if let Ok(response) = typed::<RequestPermissionResponse>(json!({ "outcome": { "outcome": "cancelled" } })) {
                let _ = responder.respond(response);
            }
            return;
        };
        shared.approvals += 1;
        let request_id = format!("approval-{}", shared.approvals);
        let held = shared.held.entry(which).or_default();
        let title = request
            .pointer("/toolCall/title")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| held.tools.get(&call_id).and_then(|tool| tool["title"].as_str()).map(str::to_string))
            .unwrap_or_else(|| "Permission needed".to_string());
        held.pending.insert(request_id.clone(), responder);
        (which, request_id, title)
    };
    let options: Vec<Value> = request["options"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|option| json!({ "option_id": option["optionId"], "label": option["name"], "kind": option["kind"] }))
        .collect();
    publish(app, shared, which, json!({ "type": "status_change", "status": "needs_input", "reason": null }));
    publish(app, shared, which, json!({ "type": "approval_request", "request_id": request_id, "title": title, "options": options, "call_id": call_id }));
}

// ── What the page may ask ─────────────────────────────────────────────────

fn order(harness: &Harness, command: Command) -> Result<(), String> {
    harness.commands.unbounded_send(command).map_err(|_| "the harness thread has stopped".to_string())
}

/// True while a reply is in flight, in any conversation.
pub fn busy(app: &AppHandle) -> bool {
    lock(&app.state::<Harness>().shared).held.values().any(|held| held.busy)
}

/// Let the running harness process go. The next order starts a fresh one and
/// loads the conversation back; a fresh process sees a provider just signed in to.
pub fn restart(app: &AppHandle) {
    let _ = order(&app.state::<Harness>(), Command::Restart);
}

/// Send a message to the open conversation. The reply arrives as events; the
/// number returned is the event to read after.
pub fn send_text(app: &AppHandle, text: String) -> Result<u64, String> {
    let harness = app.state::<Harness>();
    let (which, cursor) = {
        let mut shared = lock(&harness.shared);
        let (which, cursor) = (shared.open, shared.seq);
        let open = shared.open_one();
        if open.busy {
            return Err("still answering the previous message".into());
        }
        open.busy = true;
        (which, cursor)
    };
    // What is attached goes with this message. A file that has gone stops it.
    let files = attach::take(app).inspect_err(|_| {
        with(&harness.shared, which, |held| held.busy = false);
    })?;
    publish(app, &harness.shared, which, attach::said(&text, &files));
    publish(app, &harness.shared, which, json!({ "type": "status_change", "status": "running", "reason": null }));
    if let Err(reason) = order(&harness, Command::Send { which, text, files }) {
        fail_reply(app, &harness.shared, which, json!({ "type": "error", "message": reason, "code": null, "details": null }));
    }
    Ok(cursor)
}

#[tauri::command]
pub fn send(app: AppHandle, text: String) -> Result<u64, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("nothing to send".into());
    }
    send_text(&app, text)
}

/// Stop the open conversation's reply in progress. The conversation stays open.
#[tauri::command]
pub fn interrupt(app: AppHandle) -> Result<(), String> {
    let harness = app.state::<Harness>();
    let which = lock(&harness.shared).open;
    order(&harness, Command::Interrupt { which })
}

/// Answer an approval request: the id of the option the user picked.
#[tauri::command]
pub fn respond(app: AppHandle, request_id: String, answer: String) -> Result<(), String> {
    let harness = app.state::<Harness>();
    let (which, responder) = lock(&harness.shared).waiting(&request_id).ok_or("that approval request is no longer waiting")?;
    let response: RequestPermissionResponse = typed(json!({ "outcome": { "outcome": "selected", "optionId": answer } }))?;
    responder.respond(response).map_err(|e| describe(&e))?;
    publish(&app, &harness.shared, which, json!({ "type": "status_change", "status": "running", "reason": null }));
    Ok(())
}

/// The models the conversation can use, across every provider the harness is signed in to.
#[tauri::command]
pub async fn models(app: AppHandle) -> Result<ModelList, String> {
    let (reply, answer) = oneshot::channel();
    order(&app.state::<Harness>(), Command::Models { reply })?;
    answer.await.map_err(|_| "the harness went away".to_string())?
}

/// Move the open conversation to another model, on any provider. It keeps what was said so far.
#[tauri::command]
pub async fn set_model(app: AppHandle, id: String) -> Result<(), String> {
    let (reply, answer) = oneshot::channel();
    let which = lock(&app.state::<Harness>().shared).open;
    order(&app.state::<Harness>(), Command::SetModel { which, id, reply })?;
    answer.await.map_err(|_| "the harness went away".to_string())?
}

/// Start over. The box turns to a new conversation, which the next message
/// opens on the model last chosen. Gives the number that conversation is known
/// by, so that the page knows which events are its own from now on.
#[tauri::command]
pub fn new_conversation(app: AppHandle) -> Result<u64, String> {
    let which = {
        let harness = app.state::<Harness>();
        let mut shared = lock(&harness.shared);
        if shared.open_one().busy {
            return Err("stop the reply before starting over".into());
        }
        shared.start_over()
    };
    settings::update(&app, |settings| settings.session = None);
    Ok(which)
}

/// Have the harness let go of a conversation, if one is running and holds it.
pub async fn close(app: &AppHandle, id: String) {
    let (reply, answer) = oneshot::channel();
    if order(&app.state::<Harness>(), Command::Close { id, reply }).is_ok() {
        let _ = answer.await;
    }
}

/// The harness's id for the conversation open in the box, once it has one.
pub fn open_id(app: &AppHandle) -> Option<String> {
    lock(&app.state::<Harness>().shared).open_one().session.clone()
}

/// The conversations the harness keeps, each as the harness lists it, and the
/// id of the one open in the box. No list when this harness cannot give one.
pub async fn conversations(app: &AppHandle) -> Result<(Option<Vec<Value>>, Option<String>), String> {
    let (reply, answer) = oneshot::channel();
    order(&app.state::<Harness>(), Command::Conversations { reply })?;
    let listed = answer.await.map_err(|_| "the harness went away".to_string())??;
    Ok((listed, open_id(app)))
}

/// Open an earlier conversation in the box. It arrives as an event, with what
/// was said in it, and the next message continues it.
pub async fn open_earlier(app: &AppHandle, id: String) -> Result<(), String> {
    if lock(&app.state::<Harness>().shared).open_one().busy {
        return Err("stop the reply before opening another conversation".into());
    }
    let (reply, answer) = oneshot::channel();
    order(&app.state::<Harness>(), Command::OpenEarlier { id, reply })?;
    answer.await.map_err(|_| "the harness went away".to_string())?
}

#[derive(Serialize)]
pub struct Snapshot {
    /// The number of the conversation the box shows, which all of this is about.
    conversation: u64,
    busy: bool,
    model: Option<String>,
    last_seq: u64,
    events: Vec<Value>,
}

/// Everything the open conversation said after event `after`, for a page that
/// has just loaded or been shown again.
#[tauri::command]
pub fn events_since(app: AppHandle, after: u64) -> Snapshot {
    lock(&app.state::<Harness>().shared).since(after)
}

/// Send one message and print what comes back, then quit. For checking the
/// harness connection from a script, with no window involved.
///
/// `NULL_MINI_SMOKE_MODEL` switches to that model first, and
/// `NULL_MINI_SMOKE_STOP_AFTER` interrupts the reply after that many seconds.
pub fn smoke(app: &AppHandle, text: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Ok(model) = std::env::var("NULL_MINI_SMOKE_MODEL") {
            let listed = tauri::async_runtime::block_on(models(app.clone()));
            log!("smoke: {} models offered, on {:?}", listed.as_ref().map(|list| list.models.len()).unwrap_or(0), listed.as_ref().ok().and_then(|list| list.current.clone()));
            match tauri::async_runtime::block_on(set_model(app.clone(), model.clone())) {
                Ok(()) => log!("smoke: switched to {model}"),
                Err(reason) => log!("smoke: could not switch to {model}: {reason}"),
            }
        }
        let stop_after = std::env::var("NULL_MINI_SMOKE_STOP_AFTER").ok().and_then(|text| text.parse::<f32>().ok());
        let mut stopped = false;
        let started = std::time::Instant::now();
        let mut cursor = match send_text(&app, text) {
            Ok(cursor) => cursor,
            Err(reason) => {
                log!("smoke: could not send: {reason}");
                app.exit(1);
                return;
            }
        };
        let mut reply = String::new();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let (Some(after), false) = (stop_after, stopped) {
                if started.elapsed().as_secs_f32() >= after {
                    stopped = true;
                    log!("smoke: interrupting: {:?}", interrupt(app.clone()));
                }
            }
            let snapshot = events_since(app.clone(), cursor);
            for item in &snapshot.events {
                cursor = item["seq"].as_u64().unwrap_or(cursor);
                let event = &item["event"];
                match event["type"].as_str().unwrap_or_default() {
                    "text_delta" if event["thinking"] == false => reply.push_str(event["text"].as_str().unwrap_or_default()),
                    "text_delta" | "user_message" => {}
                    "tool_activity" => log!("smoke: tool {} [{}]", event["title"], event["status"]),
                    "message_done" | "error" => {
                        log!("smoke: {event}");
                        log!("smoke: reply after {:.1} s: {}", started.elapsed().as_secs_f32(), reply.trim());
                        // Which model answered is asked for once the reply is over.
                        std::thread::sleep(std::time::Duration::from_millis(1500));
                        for later in &events_since(app.clone(), item["seq"].as_u64().unwrap_or(cursor)).events {
                            log!("smoke: after the reply: {}", later["event"]);
                        }
                        app.exit(if event["type"] == "error" { 1 } else { 0 });
                        return;
                    }
                    _ => log!("smoke: {event}"),
                }
            }
        }
    });
}

/// What `selftest` runs in the page. Each line goes into the field and is
/// entered, as a person would, once the page has stopped working: after a
/// reply, when a list has opened, or when a sign-in asks for something. In a
/// list, a line narrows it and chooses the first entry left. When the last
/// line has been dealt with, the page says what it shows.
const SELFTEST_TYPING: &str = r#"async (lines) => {
  const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const settled = async () => {
    let quiet = 0;
    for (let waited = 0; waited < 120000 && quiet < 500; waited += 50) {
      await pause(50);
      quiet = busy || caret.classList.contains('working') ? 0 : quiet + 50;
    }
  };
  for (const text of lines) {
    await settled();
    // An empty line only presses Enter, on whatever is chosen or typed already.
    if (text) {
      input.value = text;
      input.dispatchEvent(new Event('input'));
    }
    await pause(150);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
  }
  await settled();
  invoke('report', {
    shown: out.innerText,
    list: pick.style.display === 'none' ? '' : pick.innerText,
    notice: noticeLine.style.display === 'none' ? '' : noticeLine.textContent,
    arrow: caret.className,
    height: lastHeight,
    busy,
  });
}"#;

/// How long the page gets for one line before the self-test gives up on it.
const SELFTEST_TIME_LIMIT: std::time::Duration = std::time::Duration::from_secs(150);

/// Type into the real page, line by line, and have the page report what it is
/// showing, then quit. Checks the whole path a keystroke takes except the
/// keystroke: page, command, harness, events, page. A message, a command, a
/// choice from a list and an answer to a sign-in can follow one another, one
/// to a line. Nothing of what is typed is logged here.
pub fn selftest(app: &AppHandle, text: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(window) = app.get_webview_window(panel::LABEL) else {
            log!("selftest: no window");
            app.exit(1);
            return;
        };
        let lines: Vec<&str> = text.lines().collect();
        if let Err(e) = window.eval(format!("({SELFTEST_TYPING})({})", json!(lines))) {
            log!("selftest: could not type into the page: {e}");
            app.exit(1);
            return;
        }
        // The page reports by itself, and the app quits then. This is for a page that never does.
        std::thread::sleep(SELFTEST_TIME_LIMIT * (lines.len() as u32 + 1));
        log!("selftest: the page never said what it shows");
        app.exit(1);
    });
}

/// The page's answer to `selftest`.
#[tauri::command]
pub fn report(app: AppHandle, shown: String, list: String, notice: String, arrow: String, height: f64, busy: bool) {
    log!("selftest: the page shows: {}", shown.replace('\n', " | "));
    if !list.is_empty() {
        log!("selftest: the list shows: {}", list.replace('\n', " | "));
    }
    if !notice.is_empty() {
        log!("selftest: the notice says: {notice}");
    }
    log!("selftest: arrow is \"{arrow}\", window height asked {height}, busy {busy}");
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_harness_is_always_told_not_to_look_for_a_newer_version_of_itself() {
        assert_eq!(own_settings(None), json!({ "startup": { "checkUpdate": false } }));
    }

    #[test]
    fn the_backup_order_goes_in_the_same_settings() {
        let backups = json!({ "retry": { "fallbackChains": { "default": ["a/one"] } } });
        assert_eq!(own_settings(Some(backups)), json!({ "startup": { "checkUpdate": false }, "retry": { "fallbackChains": { "default": ["a/one"] } } }));
    }

    /// A conversation the harness has opened, as this run holds it.
    fn opened(session: &str) -> Conversation {
        Conversation { session: Some(session.into()), loaded: true, ..Default::default() }
    }

    #[test]
    fn what_a_conversation_says_is_kept_with_that_conversation() {
        let mut shared = Shared::new(Some("one".into()), None);
        let (first, second) = (shared.open, shared.hold(opened("two")));
        let numbers = [shared.record(first, json!("a")), shared.record(second, json!("b")), shared.record(first, json!("c"))];
        assert_eq!(numbers, [Some(1), Some(2), Some(3)], "one count for every conversation");
        let kept = |shared: &mut Shared, which| shared.of(which).map(|held| held.events.iter().cloned().collect::<Vec<_>>());
        assert_eq!(kept(&mut shared, first), Some(vec![(1, json!("a")), (3, json!("c"))]));
        assert_eq!(kept(&mut shared, second), Some(vec![(2, json!("b"))]));
        assert_eq!(shared.record(99, json!("d")), None, "nothing is kept for a conversation this run does not hold");
        assert_eq!(shared.seq, 3, "and it takes no number");
    }

    #[test]
    fn a_page_is_given_only_what_the_open_conversation_said() {
        let mut shared = Shared::new(None, Some("a/model".into()));
        let (open, other) = (shared.open, shared.hold(opened("two")));
        shared.record(open, json!("a"));
        shared.record(other, json!("b"));
        shared.record(open, json!("c"));
        shared.open_one().busy = true;
        let snapshot = shared.since(1);
        assert_eq!((snapshot.conversation, snapshot.busy, snapshot.model.as_deref(), snapshot.last_seq), (open, true, Some("a/model"), 3));
        assert_eq!(snapshot.events, [json!({ "seq": 3, "conversation": open, "event": "c" })]);
    }

    #[test]
    fn a_conversation_is_found_by_the_harness_s_id_for_it() {
        let mut shared = Shared::new(None, None);
        assert_eq!(shared.named("one"), None, "a new conversation has no id until the harness has opened it");
        let which = shared.hold(opened("one"));
        assert_eq!(shared.named("one"), Some(which));
        assert_eq!(shared.named("two"), None);
    }

    #[test]
    fn starting_over_leaves_one_new_conversation_on_the_same_model() {
        let mut shared = Shared::new(Some("one".into()), Some("a/model".into()));
        let was = shared.open;
        shared.record(was, json!("a"));
        let now = shared.start_over();
        assert!(now != was && shared.open == now && shared.held.len() == 1, "the box shows the new one, and the last is held no longer");
        let open = shared.open_one();
        assert_eq!((open.session.as_deref(), open.loaded, open.events.len(), open.model.as_deref()), (None, false, 0, Some("a/model")));
        assert_eq!(shared.record(was, json!("late")), None, "what the last one says after that is not kept");
    }

    #[test]
    fn nothing_is_loaded_once_the_harness_has_gone() {
        let mut shared = Shared::new(None, None);
        let which = shared.hold(opened("one"));
        shared.of(which).unwrap().tools.insert("call".into(), json!({}));
        shared.let_go();
        let held = shared.of(which).unwrap();
        assert!(!held.loaded && held.tools.is_empty() && held.session.as_deref() == Some("one"), "it is still known, and loads back with the next message");
    }
}
