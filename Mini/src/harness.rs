//! The connection to the user's agent harness, over the Agent Client Protocol.
//!
//! The harness is the agent CLI behind the box (Oh-my-pi today): the copy Null
//! carries, or the user's own when they chose it (`engine.rs`). This module
//! starts it, keeps one conversation open on it, and turns what it reports into
//! the events the box shows. The harness keeps its own sign-in, tools and
//! transcript; nothing here touches a provider token.
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
use crate::{access, backups, panel, providers, settings};

/// The harness this app drives. One for now; the name is also its CLI.
pub const HARNESS: &str = "omp";
pub const HARNESS_NAME: &str = "Oh-my-pi";

/// Events kept for a page that reloads or reopens. A long reply is a few hundred.
const MAX_EVENTS: usize = 5000;

#[derive(Clone, Debug, Serialize)]
pub struct ModelList {
    pub current: Option<String>,
    pub models: Vec<ModelChoice>,
}

enum Command {
    Send { text: String },
    Interrupt,
    NewConversation,
    /// Let the harness process go, so that the next order starts a fresh one.
    Restart,
    Models { reply: oneshot::Sender<Result<ModelList, String>> },
    SetModel { id: String, reply: oneshot::Sender<Result<(), String>> },
    /// Ask the harness which model the conversation is on now. It can move to a
    /// backup by itself in the middle of a reply, and does not always say so.
    CheckModel,
}

/// What to do with the updates a harness replays while a conversation is loaded back.
#[derive(Clone, Copy, PartialEq)]
enum Replay {
    /// Not loading: every update is live.
    Live,
    /// The box already shows this conversation; drop the replay.
    Drop,
    /// The box is empty (the app was restarted); show the conversation again.
    Show,
}

struct Shared {
    events: VecDeque<(u64, Value)>,
    seq: u64,
    busy: bool,
    session: Option<String>,
    model: Option<String>,
    models: Vec<ModelChoice>,
    tools: HashMap<String, Value>,
    pending: HashMap<String, Responder<RequestPermissionResponse>>,
    approvals: u64,
    replay: Replay,
    /// True while Null itself is opening a conversation or setting its model. A
    /// change of model then is Null's doing, not the harness moving to a backup.
    settling: bool,
    /// A setting of the conversation other than its model, and its value. Setting
    /// it to that same value changes nothing and answers with the model in use.
    ask: Option<(String, Value)>,
    /// The version of the harness whose abilities were last noted. Another
    /// version is found out about afresh.
    noted: Option<String>,
    /// Whether this harness has been seen to count a reply's tokens. Until it
    /// has, a reply without a count is not taken for a provider's refusal.
    counts_tokens: bool,
}

/// The app's handle on the harness thread.
pub struct Harness {
    shared: Arc<Mutex<Shared>>,
    commands: mpsc::UnboundedSender<Command>,
}

fn lock(shared: &Arc<Mutex<Shared>>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Record an event and tell the page.
fn publish(app: &AppHandle, shared: &Arc<Mutex<Shared>>, event: Value) {
    let seq = {
        let mut shared = lock(shared);
        shared.seq += 1;
        let seq = shared.seq;
        shared.events.push_back((seq, event.clone()));
        while shared.events.len() > MAX_EVENTS {
            shared.events.pop_front();
        }
        seq
    };
    let _ = app.emit_to(panel::LABEL, "mini:event", json!({ "seq": seq, "event": event }));
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

/// End the reply in flight, if there is one, with an error the box can show.
fn fail_reply(app: &AppHandle, shared: &Arc<Mutex<Shared>>, event: Value) {
    if !lock(shared).busy {
        return;
    }
    publish(app, shared, json!({ "type": "status_change", "status": "blocked", "reason": event["message"] }));
    publish(app, shared, event);
    finish(shared);
}

/// The reply is over: free the conversation and answer any approval still open as cancelled.
fn finish(shared: &Arc<Mutex<Shared>>) {
    let pending: Vec<_> = {
        let mut shared = lock(shared);
        shared.busy = false;
        shared.pending.drain().map(|(_, responder)| responder).collect()
    };
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
    let shared = Arc::new(Mutex::new(Shared {
        events: VecDeque::new(),
        seq: 0,
        busy: false,
        session: saved.session,
        model: saved.model,
        models: Vec::new(),
        tools: HashMap::new(),
        pending: HashMap::new(),
        approvals: 0,
        replay: Replay::Live,
        settling: false,
        ask: None,
        noted: None,
        counts_tokens: false,
    }));
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
            fail_reply(&app, &shared, json!({ "type": "error", "message": reason, "code": null, "details": null }));
        }
    }
}

/// Answer an order that cannot be carried out because there is no harness to give it to.
fn refuse(app: &AppHandle, shared: &Arc<Mutex<Shared>>, command: Command, reason: &str) {
    match command {
        Command::Send { .. } => {
            fail_reply(app, shared, json!({ "type": "error", "message": reason, "code": null, "details": null }));
        }
        Command::Models { reply } => {
            let _ = reply.send(Err(reason.to_string()));
        }
        Command::SetModel { reply, .. } => {
            let _ = reply.send(Err(reason.to_string()));
        }
        Command::Interrupt | Command::NewConversation | Command::Restart | Command::CheckModel => {}
    }
}

/// Start the harness and carry out orders on it until it goes away or the app quits.
async fn serve(
    app: &AppHandle,
    shared: &Arc<Mutex<Shared>>,
    first: Command,
    orders: &mut mpsc::UnboundedReceiver<Command>,
) -> Result<(), String> {
    if matches!(first, Command::Restart | Command::CheckModel) {
        return Ok(()); // nothing is running; the next order starts a fresh process anyway
    }
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
    lock(shared).tools.clear();
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
fn workspace(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = match settings::own_folder() {
        Some(folder) => folder.join("null"),
        None => app.path().app_data_dir().map_err(|e| e.to_string())?,
    }
    .join("Workspace");
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

// ── One connection ────────────────────────────────────────────────────────

struct Conversation<'a> {
    cx: ConnectionTo<Agent>,
    app: &'a AppHandle,
    shared: &'a Arc<Mutex<Shared>>,
    mcp_capabilities: Value,
    can_load: bool,
    /// Whether this harness process holds the conversation in memory.
    loaded: bool,
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

    let mut conversation = Conversation {
        cx,
        app,
        shared,
        mcp_capabilities: init.pointer("/agentCapabilities/mcpCapabilities").cloned().unwrap_or(Value::Null),
        can_load: init.pointer("/agentCapabilities/loadSession") == Some(&json!(true)),
        loaded: false,
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
                    _ = conversation.cx.incoming_closed().fuse() => return Err(format!("{HARNESS_NAME} exited")),
                }
            }
        };
        if matches!(command, Command::Restart) {
            log!("letting the harness process go; the next order starts a fresh one");
            return Ok(());
        }
        conversation.handle(command).await?;
    }
}

impl Conversation<'_> {
    /// Carry out one order. An error here means the connection itself is no good.
    async fn handle(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Send { text } => {
                let session = match self.open().await {
                    Ok(session) => session,
                    Err(reason) => {
                        fail_reply(self.app, self.shared, json!({ "type": "error", "message": reason, "code": null, "details": null }));
                        return Ok(());
                    }
                };
                let request: PromptRequest = typed(json!({ "sessionId": session, "prompt": [{ "type": "text", "text": text }] }))?;
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
                                    publish(&app, &shared, json!({ "type": "reply_failed" }));
                                }
                                publish(&app, &shared, json!({ "type": "status_change", "status": "ready", "reason": null }));
                                publish(&app, &shared, json!({ "type": "message_done", "stop_reason": reason }));
                                finish(&shared);
                                // The user has now seen Null answer: the moment to ask for Full Disk Access.
                                if !failed && reason == "completed" {
                                    access::after_reply(&app);
                                }
                                let _ = order(&app.state::<Harness>(), Command::CheckModel);
                            }
                            Err(error) => fail_reply(&app, &shared, error_event(&error)),
                        }
                        Ok(())
                    })
                    .map_err(|e| describe(&e))?;
            }
            Command::Interrupt => {
                let session = lock(self.shared).session.clone();
                if let (Some(session), true) = (session, self.loaded) {
                    // The protocol requires every open permission request to be answered "cancelled".
                    let busy = lock(self.shared).busy;
                    if busy {
                        let pending: Vec<_> = lock(self.shared).pending.drain().map(|(_, responder)| responder).collect();
                        for responder in pending {
                            let _ = responder.respond(typed(json!({ "outcome": { "outcome": "cancelled" } }))?);
                        }
                        let cancel: CancelNotification = typed(json!({ "sessionId": session }))?;
                        self.cx.send_notification(cancel).map_err(|e| describe(&e))?;
                    }
                }
            }
            Command::Restart => {} // taken by the loop that calls this
            Command::NewConversation => {
                {
                    let mut shared = lock(self.shared);
                    shared.session = None;
                    shared.tools.clear();
                }
                self.loaded = false;
                settings::update(self.app, |settings| settings.session = None);
            }
            Command::Models { reply } => {
                let result = self.open().await.map(|_| {
                    let shared = lock(self.shared);
                    ModelList { current: shared.model.clone(), models: shared.models.clone() }
                });
                let _ = reply.send(result);
            }
            Command::SetModel { id, reply } => {
                let result = self.set_model(&id).await;
                let _ = reply.send(result);
            }
            Command::CheckModel => self.check_model().await,
        }
        Ok(())
    }

    /// Learn which model the conversation is on, and say so if the harness moved it.
    async fn check_model(&mut self) {
        let (session, ask) = {
            let shared = lock(self.shared);
            (shared.session.clone(), shared.ask.clone())
        };
        let (Some(session), Some((id, value)), true) = (session, ask, self.loaded) else { return };
        let Ok(request) = typed::<SetSessionConfigOptionRequest>(json!({ "sessionId": session, "configId": id, "value": value })) else { return };
        match self.cx.send_request(request).block_task().await {
            Ok(response) => {
                let (_, current) = translate::models_from_config_options(&as_json(&response)["configOptions"]);
                moved(self.app, self.shared, current);
            }
            Err(error) => log!("could not ask which model the conversation is on: {}", describe(&error)),
        }
    }

    /// The conversation's id, opening a new conversation or loading the saved one back as needed.
    async fn open(&mut self) -> Result<String, String> {
        lock(self.shared).settling = true;
        let opened = self.open_quietly().await;
        lock(self.shared).settling = false;
        opened
    }

    async fn open_quietly(&mut self) -> Result<String, String> {
        let saved = lock(self.shared).session.clone();
        if let (Some(session), true) = (&saved, self.loaded) {
            return Ok(session.clone());
        }
        let cwd = workspace(self.app)?;
        let servers = mcp_servers(&self.mcp_capabilities);

        if let (Some(session), true) = (saved, self.can_load) {
            {
                let mut shared = lock(self.shared);
                shared.replay = if shared.events.is_empty() { Replay::Show } else { Replay::Drop };
            }
            let request: LoadSessionRequest = typed(json!({ "sessionId": session, "cwd": cwd, "mcpServers": servers }))?;
            let loaded = self.cx.send_request(request).block_task().await;
            lock(self.shared).replay = Replay::Live;
            match loaded {
                Ok(response) => {
                    self.absorb_options(&as_json(&response)["configOptions"]);
                    self.loaded = true;
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
        let wanted = lock(self.shared).model.clone();
        self.absorb_options(&response["configOptions"]);
        lock(self.shared).session = Some(session.clone());
        self.loaded = true;
        settings::update(self.app, |settings| settings.session = Some(session.clone()));
        log!("opened the conversation {session} in {}", cwd.display());

        // New conversations start on the model last chosen, when the harness still offers it.
        if let Some(wanted) = wanted {
            let (offered, current) = {
                let shared = lock(self.shared);
                (shared.models.iter().any(|model| model.id == wanted), shared.model.clone())
            };
            if offered && current.as_deref() != Some(&wanted) {
                if let Err(reason) = self.apply_model(&session, &wanted).await {
                    log!("staying on {}: {reason}", current.as_deref().unwrap_or("the default model"));
                }
            } else if !offered {
                log!("{wanted} is no longer offered; staying on {}", current.as_deref().unwrap_or("the default model"));
                settings::update(self.app, |settings| settings.model = current.clone());
            }
        }
        Ok(session)
    }

    /// Move the conversation to another model. It keeps what was said so far.
    async fn set_model(&mut self, id: &str) -> Result<(), String> {
        if lock(self.shared).busy {
            return Err("stop the reply before switching models".into());
        }
        let session = self.open().await?;
        self.apply_model(&session, id).await
    }

    async fn apply_model(&mut self, session: &str, id: &str) -> Result<(), String> {
        {
            let shared = lock(self.shared);
            if shared.model.as_deref() == Some(id) {
                return Ok(());
            }
            if !shared.models.is_empty() && !shared.models.iter().any(|model| model.id == id) {
                return Err(format!("{HARNESS_NAME} has no model named {id}"));
            }
        }
        let request: SetSessionConfigOptionRequest = typed(json!({ "sessionId": session, "configId": "model", "value": id }))?;
        let was_settling = std::mem::replace(&mut lock(self.shared).settling, true);
        let response = self.cx.send_request(request).block_task().await;
        if let Ok(response) = &response {
            self.absorb_options(&as_json(response)["configOptions"]);
            lock(self.shared).model = Some(id.to_string());
        }
        lock(self.shared).settling = was_settling;
        response.map_err(|e| describe(&e))?;
        settings::update(self.app, |settings| settings.model = Some(id.to_string()));
        publish(self.app, self.shared, json!({ "type": "model_changed", "model": id }));
        Ok(())
    }

    fn absorb_options(&mut self, options: &Value) {
        if let Some(greeting) = self.unnoted.take() {
            log!("what this harness can do: {}", translate::abilities(&greeting, options));
        }
        let (models, current) = translate::models_from_config_options(options);
        if models.is_empty() {
            return;
        }
        let mut shared = lock(self.shared);
        shared.models = models;
        if current.is_some() {
            shared.model = current;
        }
        shared.ask = translate::other_option(options);
    }
}

// ── Calls from the harness ────────────────────────────────────────────────

/// Take note of the model the harness says the conversation is on. When that
/// is not the model Null left it on, the harness has moved to a backup, and
/// the box says so: a switch is never silent.
fn moved(app: &AppHandle, shared: &Arc<Mutex<Shared>>, current: Option<String>) {
    let Some(current) = current else { return };
    let from = {
        let mut shared = lock(shared);
        if shared.model.as_deref() == Some(current.as_str()) {
            return;
        }
        let from = shared.model.replace(current.clone());
        if shared.settling {
            return;
        }
        from
    };
    log!("the harness moved the conversation from {} to {current}", from.as_deref().unwrap_or("its first model"));
    publish(app, shared, json!({ "type": "model_switched", "from": from, "to": current }));
}

fn on_update(app: &AppHandle, shared: &Arc<Mutex<Shared>>, notification: &SessionNotification) {
    let notification = as_json(notification);
    let update = &notification["update"];
    if update["sessionUpdate"] == "config_option_update" {
        let (models, current) = translate::models_from_config_options(&update["configOptions"]);
        if !models.is_empty() {
            lock(shared).models = models;
        }
        moved(app, shared, current);
        return;
    }
    let event = {
        let mut shared = lock(shared);
        if shared.replay == Replay::Drop || Some(notification["sessionId"].as_str().unwrap_or_default()) != shared.session.as_deref() && shared.replay == Replay::Live {
            return;
        }
        translate::event_from_update(update, &mut shared.tools)
    };
    if let Some(event) = event {
        publish(app, shared, event);
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
    let (request_id, title) = {
        let mut shared = lock(shared);
        if !shared.busy {
            drop(shared);
            // Nobody is there to ask.
            if let Ok(response) = typed::<RequestPermissionResponse>(json!({ "outcome": { "outcome": "cancelled" } })) {
                let _ = responder.respond(response);
            }
            return;
        }
        shared.approvals += 1;
        let request_id = format!("approval-{}", shared.approvals);
        let title = request
            .pointer("/toolCall/title")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| shared.tools.get(&call_id).and_then(|tool| tool["title"].as_str()).map(str::to_string))
            .unwrap_or_else(|| "Permission needed".to_string());
        shared.pending.insert(request_id.clone(), responder);
        (request_id, title)
    };
    let options: Vec<Value> = request["options"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|option| json!({ "option_id": option["optionId"], "label": option["name"], "kind": option["kind"] }))
        .collect();
    publish(app, shared, json!({ "type": "status_change", "status": "needs_input", "reason": null }));
    publish(app, shared, json!({ "type": "approval_request", "request_id": request_id, "title": title, "options": options, "call_id": call_id }));
}

// ── What the page may ask ─────────────────────────────────────────────────

fn order(harness: &Harness, command: Command) -> Result<(), String> {
    harness.commands.unbounded_send(command).map_err(|_| "the harness thread has stopped".to_string())
}

/// True while a reply is in flight.
pub fn busy(app: &AppHandle) -> bool {
    lock(&app.state::<Harness>().shared).busy
}

/// Let the running harness process go. The next order starts a fresh one and
/// loads the conversation back; a fresh process sees a provider just signed in to.
pub fn restart(app: &AppHandle) {
    let _ = order(&app.state::<Harness>(), Command::Restart);
}

/// Send a message. The reply arrives as events; the number returned is the event to read after.
pub fn send_text(app: &AppHandle, text: String) -> Result<u64, String> {
    let harness = app.state::<Harness>();
    let cursor = {
        let mut shared = lock(&harness.shared);
        if shared.busy {
            return Err("still answering the previous message".into());
        }
        shared.busy = true;
        shared.seq
    };
    publish(app, &harness.shared, json!({ "type": "user_message", "text": text }));
    publish(app, &harness.shared, json!({ "type": "status_change", "status": "running", "reason": null }));
    if let Err(reason) = order(&harness, Command::Send { text }) {
        fail_reply(app, &harness.shared, json!({ "type": "error", "message": reason, "code": null, "details": null }));
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

/// Stop the reply in progress. The conversation stays open.
#[tauri::command]
pub fn interrupt(app: AppHandle) -> Result<(), String> {
    order(&app.state::<Harness>(), Command::Interrupt)
}

/// Answer an approval request: the id of the option the user picked.
#[tauri::command]
pub fn respond(app: AppHandle, request_id: String, answer: String) -> Result<(), String> {
    let harness = app.state::<Harness>();
    let responder = lock(&harness.shared).pending.remove(&request_id).ok_or("that approval request is no longer waiting")?;
    let response: RequestPermissionResponse = typed(json!({ "outcome": { "outcome": "selected", "optionId": answer } }))?;
    responder.respond(response).map_err(|e| describe(&e))?;
    publish(&app, &harness.shared, json!({ "type": "status_change", "status": "running", "reason": null }));
    Ok(())
}

/// The models the conversation can use, across every provider the harness is signed in to.
#[tauri::command]
pub async fn models(app: AppHandle) -> Result<ModelList, String> {
    let (reply, answer) = oneshot::channel();
    order(&app.state::<Harness>(), Command::Models { reply })?;
    answer.await.map_err(|_| "the harness went away".to_string())?
}

/// Move the conversation to another model, on any provider. It keeps what was said so far.
#[tauri::command]
pub async fn set_model(app: AppHandle, id: String) -> Result<(), String> {
    let (reply, answer) = oneshot::channel();
    order(&app.state::<Harness>(), Command::SetModel { id, reply })?;
    answer.await.map_err(|_| "the harness went away".to_string())?
}

/// Start over. The next message opens a new conversation, on the model last chosen.
#[tauri::command]
pub fn new_conversation(app: AppHandle) -> Result<(), String> {
    let harness = app.state::<Harness>();
    if lock(&harness.shared).busy {
        return Err("stop the reply before starting over".into());
    }
    lock(&harness.shared).events.clear();
    order(&harness, Command::NewConversation)
}

#[derive(Serialize)]
pub struct Snapshot {
    busy: bool,
    model: Option<String>,
    last_seq: u64,
    events: Vec<Value>,
}

/// Everything after event `after`, for a page that has just loaded or been shown again.
#[tauri::command]
pub fn events_since(app: AppHandle, after: u64) -> Snapshot {
    let harness = app.state::<Harness>();
    let shared = lock(&harness.shared);
    Snapshot {
        busy: shared.busy,
        model: shared.model.clone(),
        last_seq: shared.seq,
        events: shared.events.iter().filter(|(seq, _)| *seq > after).map(|(seq, event)| json!({ "seq": seq, "event": event })).collect(),
    }
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

/// Type a message into the real page, wait for the reply, and have the page
/// report what it is showing, then quit. Checks the whole path a keystroke
/// takes except the keystroke: page, command, harness, events, page.
pub fn selftest(app: &AppHandle, text: String) {
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(window) = app.get_webview_window(panel::LABEL) else {
            log!("selftest: no window");
            app.exit(1);
            return;
        };
        if let Err(e) = window.eval(format!("send({})", Value::String(text))) {
            log!("selftest: could not type into the page: {e}");
            app.exit(1);
            return;
        }
        let mut cursor = 0;
        let mut over = false;
        while !over {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let snapshot = events_since(app.clone(), cursor);
            for item in &snapshot.events {
                cursor = item["seq"].as_u64().unwrap_or(cursor);
                over |= matches!(item["event"]["type"].as_str(), Some("message_done" | "error"));
            }
        }
        // Give the page a moment to draw the last event before asking what it shows.
        std::thread::sleep(std::time::Duration::from_millis(400));
        let ask = "invoke('report', { shown: out.innerText, arrow: caret.className, height: lastHeight, busy })";
        if let Err(e) = window.eval(ask) {
            log!("selftest: could not ask the page: {e}");
            app.exit(1);
        }
    });
}

/// The page's answer to `selftest`.
#[tauri::command]
pub fn report(app: AppHandle, shown: String, arrow: String, height: f64, busy: bool) {
    log!("selftest: the page shows: {}", shown.replace('\n', " | "));
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
}
