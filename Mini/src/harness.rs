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

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
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
use crate::{access, attach, backups, computer, panel, providers, routing, rules, settings, verify};

/// What the box says when the harness has to be started again while conversations are working.
const RESTART_WAITS: &str = "that takes effect once nothing is working: the harness starts again then";

/// What the box says to a message that would be one conversation too many at work.
pub const TOO_MANY: &str = "four conversations are already working. wait for one to end, or stop one";

/// The harness this app drives. One for now; the name is also its CLI.
pub const HARNESS: &str = "omp";
pub const HARNESS_NAME: &str = "Oh-my-pi";

/// Events kept for a page that reloads or reopens. A long reply is a few hundred.
const MAX_EVENTS: usize = 5000;

/// How many conversations may work at once. Each costs memory, and they share
/// one provider's limit.
const MAX_WORKING: usize = 4;

/// How many pages of the harness's list of conversations are read, at most.
/// Oh-my-pi gives fifty to a page. A list that never ended would never be shown.
const MAX_PAGES: usize = 200;

#[derive(Clone, Debug, Serialize)]
pub struct ModelList {
    pub current: Option<String>,
    pub models: Vec<ModelChoice>,
}

enum Command {
    /// `after` is what the box is told once the conversation is open, when that
    /// had to wait for what was said in it before (see `send_text`).
    Send { which: u64, text: String, files: Vec<PathBuf>, after: Vec<Value> },
    Interrupt { which: u64 },
    /// Let the harness process go, so that the next order starts a fresh one.
    Restart,
    Models { reply: oneshot::Sender<Result<ModelList, String>> },
    SetModel { which: u64, id: String, reply: oneshot::Sender<Result<(), String>> },
    /// Ask the harness which model a conversation is on now. It can move to a
    /// backup by itself in the middle of a reply, and does not always say so.
    /// `counted` is what the reply that has just ended cost: it is kept once
    /// that is known, so that it is counted on the model that answered it.
    CheckModel { which: u64, counted: Option<routing::Reply> },
    /// A reply has ended: have the second model verify the answer, when the
    /// person asked for that and this is a reply to verify (`verify.rs`).
    Verify { which: u64 },
    /// The second model has answered: put the conversation back on its own model.
    Verified { which: u64 },
    /// Have the harness say what it can carry out. It says so when a session
    /// starts, so this is also what opens one: a box that has asked nothing yet
    /// has not been told, and the list it offers would be the box's own only.
    /// The answer arrives as an event like any other.
    Commands { reply: oneshot::Sender<Result<(), String>> },
    /// List the conversations the harness keeps, for `/history`: each as the
    /// harness gives it. No list when this harness cannot give one.
    Conversations { reply: oneshot::Sender<Result<Option<Vec<Value>>, String>> },
    /// Open one of them in the box, in place of the conversation that is open.
    /// Answers with the conversation's number when this run already held it.
    OpenEarlier { id: String, reply: oneshot::Sender<Result<Option<u64>, String>> },
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

/// How a conversation's last reply ended.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ended {
    Done,
    /// With an error, or with a provider's or a model's refusal.
    Failed,
    /// By the person.
    Stopped,
    /// By Null or the harness going away while it was working.
    CutOff,
}

/// One conversation with something to say for itself, as the page is told of it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Work {
    /// The number Null knows it by.
    conversation: u64,
    /// The harness's id for it, once it has one: what `/history` lists it under.
    id: Option<String>,
    /// How it stands: working, asking, done, failed, stopped or cut off.
    standing: &'static str,
    /// It ended while the box showed another conversation, and has not been shown since.
    unseen: bool,
    /// It is the one the box shows.
    open: bool,
    /// The title Null keeps for it, when it keeps one.
    title: Option<String>,
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
    /// The model the person has it on: the one it opened on, or the one they
    /// chose with `/model`. A backup taking over leaves it as it is, and so
    /// does Null moving the conversation for one reply. None until it is open.
    chosen: Option<String>,
    tools: HashMap<String, Value>,
    pending: HashMap<String, Responder<RequestPermissionResponse>>,
    replay: Replay,
    /// True while Null itself is opening the conversation or setting its model. A
    /// change of model then is Null's doing, not the harness moving to a backup.
    settling: bool,
    /// A setting of the conversation other than its model, and its value. Setting
    /// it to that same value changes nothing and answers with the model in use.
    ask: Option<(String, Value)>,
    /// How hard its model can be set to think, as the harness names the levels.
    /// None of them when the harness gives the conversation no such setting.
    levels: Vec<String>,
    /// How many steps the reply under way has taken, or the last one took.
    steps: u32,
    /// The steps of the reply under way that failed, by the harness's id for each.
    failed: HashSet<String>,
    /// When the reply under way began. None while none is.
    began: Option<std::time::Instant>,
    /// The reply under way is the second model verifying the answer before it
    /// (`verify.rs`), sent by Null and not by the person.
    verifying: bool,
    /// The model the conversation was moved from for that, to be put back on
    /// once the second model has answered. None when it was not moved.
    back: Option<String>,
    /// Its reply ended while the box showed another conversation, and it has
    /// not been shown since. It is kept until it has been.
    unseen: bool,
    /// How its last reply ended. None until one has.
    ended: Option<Ended>,
    /// The folder it works in, when that is not the box's own. It is chosen
    /// when the conversation starts and does not change.
    folder: Option<PathBuf>,
    /// The names of the commands the harness last said it can carry out here.
    commands: Vec<String>,
}

impl Conversation {
    /// Whether `text` is one of the harness's own commands: one it carries out
    /// itself, and not a skill, which is a message to the model under another
    /// name. The harness answers such a command without a model, so the answer
    /// counts no tokens and is still no refusal.
    fn answers_itself(&self, text: &str) -> bool {
        let name = text.strip_prefix('/').and_then(|rest| rest.split(char::is_whitespace).next()).unwrap_or_default();
        !name.is_empty() && !name.starts_with("skill:") && self.commands.iter().any(|command| command == name)
    }

    /// How it stands, in the box's own words. A reply under way is working,
    /// or asking while it waits for the person's answer. Otherwise it is as
    /// its last reply ended. None for one that nothing has been asked of yet.
    fn standing(&self) -> Option<&'static str> {
        if self.busy {
            return Some(if self.pending.is_empty() { "working" } else { "asking" });
        }
        self.ended.map(|ended| match ended {
            Ended::Done => "done",
            Ended::Failed => "failed",
            Ended::Stopped => "stopped",
            Ended::CutOff => "cut off",
        })
    }

    /// A reply begins: it is working, and is counted from here (`routing.rs`).
    fn begin(&mut self) {
        self.busy = true;
        (self.steps, self.began) = (0, Some(std::time::Instant::now()));
        self.failed.clear();
    }

    /// Count what the harness says of the reply under way: a step the agent
    /// takes, and a step that failed. Each is counted once, however often the
    /// harness says so. A replay of old ones is not counted. This comes before
    /// the update is taken in, while `tools` still holds what was said last.
    fn count(&mut self, update: &Value) {
        if !self.busy || self.replay != Replay::Live {
            return;
        }
        let (Some(kind @ ("tool_call" | "tool_call_update")), Some(id)) = (update["sessionUpdate"].as_str(), update["toolCallId"].as_str()) else { return };
        if kind == "tool_call" && !self.tools.contains_key(id) {
            self.steps += 1;
        }
        if update["status"] == "failed" {
            self.failed.insert(id.to_string());
        }
    }

    /// What the reply that has just ended cost, in numbers (`routing.rs`).
    /// None when no reply had begun.
    fn counted(&mut self) -> Option<routing::Reply> {
        let seconds = self.began.take()?.elapsed().as_secs_f64();
        Some(routing::Reply {
            at: routing::now(),
            model: self.model.clone(),
            thinking: self.level(),
            steps: self.steps,
            failed: self.failed.len() as u32,
            seconds: (seconds * 10.0).round() / 10.0,
            ended: self.standing()?.to_string(),
            own_folder: self.folder.is_none(),
            verification: self.verifying,
        })
    }

    /// Whether what was said in it before this run is still to be shown. The
    /// harness keeps it and has not loaded it back yet, and the box holds
    /// nothing of it: loading it is what shows it (`open_quietly`).
    fn comes_back(&self) -> bool {
        self.session.is_some() && !self.loaded && self.events.is_empty()
    }
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
    /// How the conversations stood when the page was last told, and how many
    /// times it has been told: a page that hears two tellings out of their
    /// order keeps the later one.
    told: (u64, Vec<Work>),
    /// The harness is to be started again as soon as nothing is working: after
    /// a sign-in, or a change of harness or of backups, made while something was.
    restart_due: bool,
    /// Null is quitting at the person's word. What is still working then is
    /// stopped by them, and is not written down as work that was cut off.
    quitting: bool,
}

impl Shared {
    /// A box with one conversation in it: the one that was open last, to be
    /// loaded back, or a new one that the first message opens.
    fn new(session: Option<String>, model: Option<String>) -> Shared {
        let mut shared = Shared { held: HashMap::new(), open: 0, numbered: 0, seq: 0, models: Vec::new(), approvals: 0, noted: None, counts_tokens: false, opening: None, told: (0, Vec::new()), restart_due: false, quitting: false };
        shared.open = shared.hold(Conversation { session, model, ..Default::default() });
        shared
    }

    /// At a start of Null: what was written down of the conversations it held
    /// and had not shown. One that was working then was cut off, by Null or
    /// the Mac going away; one that had ended is as it ended. Each waits to be
    /// seen. The harness has what was said in them; Null has kept none of it.
    fn recall(&mut self, stood: BTreeMap<String, String>) {
        for (id, word) in stood {
            let ended = match word.as_str() {
                "done" => Ended::Done,
                "failed" => Ended::Failed,
                "stopped" => Ended::Stopped,
                _ => Ended::CutOff,
            };
            match self.named(&id).and_then(|which| self.of(which)) {
                Some(open) => (open.ended, open.unseen) = (Some(ended), true),
                None => {
                    self.hold(Conversation { session: Some(id), ended: Some(ended), unseen: true, ..Default::default() });
                }
            }
        }
        // This is how things stand at the start, and what a page that loads is
        // given. Whatever changes from here is news, the first change too.
        self.told = (0, self.work());
    }

    /// A conversation recalled at the start that the harness no longer has:
    /// nothing is left to open, and it says so no longer. One this run has
    /// held since, working or with something of it kept, is not touched. The
    /// box's open one keeps its place and loses only what was recalled of it.
    /// True when anything was let go.
    fn let_go_of_recalled(&mut self, id: &str) -> bool {
        let Some(which) = self.named(id) else { return false };
        let open = which == self.open;
        let Some(held) = self.of(which) else { return false };
        if held.busy || !held.events.is_empty() || held.ended.is_none() {
            return false;
        }
        if open {
            (held.ended, held.unseen) = (None, false);
        } else {
            self.held.remove(&which);
        }
        true
    }

    /// What is written down of the conversations this run holds: for one that
    /// is working, that it was, and for one that waits to be seen, how it
    /// ended. Quitting at the person's word leaves the working ones out.
    fn stands(&self) -> BTreeMap<String, String> {
        let word = |one: &Work| if one.unseen { one.standing } else { "working" };
        self.work().iter().filter(|one| one.unseen || !self.quitting).filter_map(|one| Some((one.id.clone()?, word(one).to_string()))).collect()
    }

    /// The harness is wanted afresh. True when that can be now; with
    /// something working it is put off until nothing is.
    fn restart_now(&mut self) -> bool {
        self.restart_due = self.working() > 0;
        !self.restart_due
    }

    /// A reply has ended. True when the harness was waiting for that to be started again.
    fn restart_at_last(&mut self) -> bool {
        let now = self.restart_due && self.working() == 0;
        if now {
            self.restart_due = false;
        }
        now
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
        // What the harness carries out itself is kept by name as well: the list
        // of events is cut at its old end, and this is asked at every message.
        if event["type"] == "commands" {
            let named = event["commands"].as_array().into_iter().flatten().filter_map(|command| command["name"].as_str());
            conversation.commands = named.map(str::to_string).collect();
        }
        self.seq += 1;
        conversation.events.push_back((self.seq, event));
        while conversation.events.len() > MAX_EVENTS {
            conversation.events.pop_front();
        }
        Some(self.seq)
    }

    /// The box turns to a conversation this run holds. The one it showed goes
    /// on if it is working. At rest there is nothing more to keep of it, and it
    /// is held no longer: the harness's id for it is then given back, when the
    /// harness process has it in memory and can let it go as well.
    fn turn_to(&mut self, which: u64) -> Option<String> {
        if let Some(shown) = self.of(which) {
            shown.unseen = false;
        }
        let was = std::mem::replace(&mut self.open, which);
        if was == which || self.held.get(&was).is_some_and(|left| left.busy) {
            return None;
        }
        self.held.remove(&was).and_then(|left| left.session.filter(|_| left.loaded))
    }

    /// Start over: the box turns to a new conversation in its own folder, which
    /// the next message opens on the model the person had the last one on. Gives
    /// its number, and what `turn_to` gives of the one that was left.
    fn start_over(&mut self) -> (u64, Option<String>) {
        self.start_in(None)
    }

    /// The same, in a folder of the person's choosing when one is given.
    fn start_in(&mut self, folder: Option<PathBuf>) -> (u64, Option<String>) {
        // The model the person has the open one on: not a backup answering in
        // its place, and not the harness's default while it is being opened.
        let open = self.open_one();
        let model = open.chosen.clone().or_else(|| open.model.clone());
        let which = self.hold(Conversation { model, folder, ..Default::default() });
        (which, self.turn_to(which))
    }

    /// Every conversation with something to say for itself: one that is working
    /// or asking, and one that waits to be seen. The most pressing first: one
    /// that asks, then one that failed or was cut off, one that is done, one
    /// that is working.
    /// Among equals, the newer first.
    fn work(&self) -> Vec<Work> {
        let mut work: Vec<Work> = self
            .held
            .iter()
            .filter(|(_, held)| held.busy || held.unseen)
            .filter_map(|(which, held)| Some(Work { conversation: *which, id: held.session.clone(), standing: held.standing()?, unseen: held.unseen, open: *which == self.open, title: None }))
            .collect();
        let pressing = |standing: &str| ["asking", "failed", "cut off", "done", "working"].iter().position(|known| *known == standing).unwrap_or(5);
        work.sort_by_key(|one| (pressing(one.standing), std::cmp::Reverse(one.conversation)));
        work
    }

    /// How many conversations are working.
    fn working(&self) -> usize {
        self.held.values().filter(|held| held.busy).count()
    }

    /// A message for the open conversation, which is working from now on.
    /// Refused while it still answers the last one, and when as many as may
    /// are working already. Gives its number, and the number of the last event
    /// before the message.
    fn begin_reply(&mut self) -> Result<(u64, u64), String> {
        let (which, cursor, working) = (self.open, self.seq, self.working());
        let open = self.open_one();
        if open.busy {
            return Err("still answering the previous message".into());
        }
        if working >= MAX_WORKING {
            return Err(TOO_MANY.into());
        }
        open.begin();
        open.unseen = false;
        Ok((which, cursor))
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
        Snapshot { conversation, busy: open.busy, model: open.model.clone(), folder: open.folder.as_deref().map(attach::said_path), last_seq, events }
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

/// Several events of one conversation, in their order.
fn tell(app: &AppHandle, shared: &Arc<Mutex<Shared>>, which: u64, events: Vec<Value>) {
    for event in events {
        publish(app, shared, which, event);
    }
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

/// Tell the page how the conversations stand, when that has changed since it
/// was last told: the pet speaks for all of them, and `/history` names each.
fn tell_work(app: &AppHandle, shared: &Arc<Mutex<Shared>>) {
    let (told, work) = {
        let mut shared = lock(shared);
        let work = shared.work();
        if work == shared.told.1 {
            return;
        }
        shared.told = (shared.told.0 + 1, work.clone());
        (shared.told.0, work)
    };
    // And it is written down, for a start of Null that finds work left behind.
    write_down(app, shared);
    let _ = app.emit_to(panel::LABEL, "mini:work", json!({ "told": told, "conversations": titled(app, work) }));
    // PROTOTYPE (item 15): the pet alone stays or goes by what there is to say.
    panel::settle(app);
}

/// PROTOTYPE (item 15). Whether any conversation is working, or asking the
/// person something: what the pet alone stays for. One that has ended and
/// waits to be seen does not keep it: the pet says how it ended and goes
/// (`panel::settle`), and "now" in `/history` and the notice line still name it.
pub fn anything_at_work(app: &AppHandle) -> bool {
    lock(&app.state::<Harness>().shared).working() > 0
}

/// PROTOTYPE (item 15). The box is up again: what ended in the open
/// conversation while it was away has been seen.
pub fn seen(app: &AppHandle) {
    let harness = app.state::<Harness>();
    lock(&harness.shared).open_one().unseen = false;
    tell_work(app, &harness.shared);
}

fn write_down(app: &AppHandle, shared: &Arc<Mutex<Shared>>) {
    let stands = lock(shared).stands();
    history::stand(app, stands);
}

/// The same with the title Null keeps for each, which is what the notice line calls it by.
fn titled(app: &AppHandle, mut work: Vec<Work>) -> Vec<Work> {
    for one in &mut work {
        one.title = one.id.as_deref().and_then(|id| history::title_of(app, id));
    }
    work
}

/// End a conversation's reply in flight, if there is one, with an error the box can show.
fn fail_reply(app: &AppHandle, shared: &Arc<Mutex<Shared>>, which: u64, event: Value) {
    end_reply(app, shared, which, event, Ended::Failed);
}

/// The same, saying how it ended: failed, or cut off when the harness went away under it.
fn end_reply(app: &AppHandle, shared: &Arc<Mutex<Shared>>, which: u64, event: Value, ended: Ended) {
    if !with(shared, which, |held| held.busy).unwrap_or(false) {
        return;
    }
    publish(app, shared, which, json!({ "type": "status_change", "status": "blocked", "reason": event["message"] }));
    publish(app, shared, which, event);
    if let Some(reply) = finish(shared, which, ended) {
        routing::count(app, reply);
    }
    tell_work(app, shared);
    restart_if_due(app, shared);
}

/// Keep the count of a reply that has ended (`routing.rs`), on the model its
/// conversation is on now, when the conversation is still held.
fn count_on(app: &AppHandle, shared: &Arc<Mutex<Shared>>, which: u64, counted: Option<routing::Reply>) {
    let Some(mut reply) = counted else { return };
    if let Some(on) = with(shared, which, |held| held.model.clone()).flatten() {
        reply.model = Some(on);
    }
    routing::count(app, reply);
}

/// A reply has ended: if the harness was waiting for nothing to be working, it is started again now.
fn restart_if_due(app: &AppHandle, shared: &Arc<Mutex<Shared>>) {
    if lock(shared).restart_at_last() {
        log!("nothing is working any more; the harness is started again, as was asked");
        let _ = order(&app.state::<Harness>(), Command::Restart);
    }
}

/// The harness has answered a message with how its reply ended: say so in the
/// box and free the conversation. `own` is a reply the harness gave itself,
/// with no model asked. Gives what the reply cost, for the caller to keep
/// (`routing.rs`): nothing for one the harness gave itself.
fn reply_ended(app: &AppHandle, shared: &Arc<Mutex<Shared>>, which: u64, response: &Value, own: bool) -> Option<routing::Reply> {
    let reason = response["stopReason"].as_str().map(translate::stop_reason).unwrap_or("completed");
    // The harness reports a provider's refusal as a reply like any other.
    let (failed, learned) = {
        let mut shared = lock(shared);
        let (failed, counts) = translate::judge_reply(response, shared.counts_tokens);
        let learned = (counts && !shared.counts_tokens).then(|| shared.noted.clone()).flatten();
        shared.counts_tokens = counts;
        // A reply without a count is a refusal only where a model was asked.
        (failed && !own, learned)
    };
    if let Some(version) = learned {
        log!("this harness counts a reply's tokens; from now on a reply without a count is taken for a refusal");
        settings::update(app, |settings| settings.counts_tokens = Some(version));
    }
    log!("reply ended: {reason}{}", if failed { ", with nothing from the model" } else { "" });
    if failed {
        providers::forget(app);
        publish(app, shared, which, json!({ "type": "reply_failed" }));
    }
    publish(app, shared, which, json!({ "type": "status_change", "status": "ready", "reason": null }));
    publish(app, shared, which, json!({ "type": "message_done", "stop_reason": reason }));
    let ended = match reason {
        _ if failed => Ended::Failed,
        "refused" => Ended::Failed,
        "cancelled" => Ended::Stopped,
        _ => Ended::Done,
    };
    let counted = finish(shared, which, ended).filter(|_| !own);
    tell_work(app, shared);
    restart_if_due(app, shared);
    // The user has now seen Null answer: the moment to ask for Full Disk Access.
    if !failed && reason == "completed" {
        access::after_reply(app);
    }
    counted
}

/// The reply is over, and this is how it ended: free the conversation and
/// answer any approval still open as cancelled. One that the box is not
/// showing waits to be seen. Gives what the reply cost, when one had begun.
fn finish(shared: &Arc<Mutex<Shared>>, which: u64, ended: Ended) -> Option<routing::Reply> {
    let (pending, counted): (Vec<_>, _) = {
        let mut shared = lock(shared);
        let open = shared.open;
        shared.of(which).map(|held| {
            held.busy = false;
            held.ended = Some(ended);
            // PROTOTYPE (item 15, the pet alone): with the box put away, a reply
            // that ends in the open conversation has not been seen either.
            held.unseen = which != open || panel::put_away();
            (held.pending.drain().map(|(_, responder)| responder).collect(), held.counted())
        })
    }
    .unwrap_or_default();
    for responder in pending {
        if let Ok(response) = typed::<RequestPermissionResponse>(json!({ "outcome": { "outcome": "cancelled" } })) {
            let _ = responder.respond(response);
        }
    }
    counted
}

// ── Start-up ──────────────────────────────────────────────────────────────

/// Start the harness thread. Nothing is launched until the first order arrives.
pub fn init(app: &AppHandle) {
    let saved = settings::get(app);
    // The one that was open is loaded back with the first message, unless a
    // person has it open in a terminal now: two harnesses writing one
    // conversation spoil it. The box starts on a new one instead.
    let session = saved.session.filter(|id| {
        let elsewhere = history::open_in_a_terminal(id);
        if elsewhere {
            log!("the conversation that was open is open in a terminal now; the box starts a new one");
            settings::update(app, |settings| settings.session = None);
        }
        !elsewhere
    });
    let mut shared = Shared::new(session, saved.model);
    // Work Null noted and the harness never wrote down, as when Null went away
    // between the harness opening the conversation and the message reaching
    // it, is nothing to go back to: it would say it was cut off at every start
    // and could not be opened. It is let go.
    let (kept, gone): (BTreeMap<_, _>, BTreeMap<_, _>) = history::stood(app).into_iter().partition(|(id, _)| history::anything_said(id) != Some(false));
    if !gone.is_empty() {
        log!("{} conversation(s) noted as working had nothing in them at the harness; let go", gone.len());
    }
    shared.recall(kept);
    let shared = Arc::new(Mutex::new(shared));
    // What was working when Null went away is written down as cut off from now on.
    write_down(app, &shared);
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
            // Every reply that was under way is cut off with it, and says so in one line.
            let working: Vec<u64> = lock(&shared).held.iter().filter(|(_, held)| held.busy).map(|(which, _)| *which).collect();
            for which in working {
                end_reply(&app, &shared, which, json!({ "type": "error", "message": reason, "code": null, "details": null }), Ended::CutOff);
            }
        }
    }
}

/// Answer an order that cannot be carried out because there is no harness to give it to.
fn refuse(app: &AppHandle, shared: &Arc<Mutex<Shared>>, command: Command, reason: &str) {
    match command {
        Command::Send { which, after, .. } => {
            tell(app, shared, which, after);
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
        Command::Commands { reply } => {
            let _ = reply.send(Err(reason.to_string()));
        }
        Command::OpenEarlier { reply, .. } => {
            let _ = reply.send(Err(reason.to_string()));
        }
        Command::Close { reply, .. } => {
            let _ = reply.send(());
        }
        Command::Interrupt { .. } | Command::Restart | Command::CheckModel { .. } | Command::Verify { .. } | Command::Verified { .. } => {}
    }
}

/// Start the harness and carry out orders on it until it goes away or the app quits.
async fn serve(
    app: &AppHandle,
    shared: &Arc<Mutex<Shared>>,
    first: Command,
    orders: &mut mpsc::UnboundedReceiver<Command>,
) -> Result<(), String> {
    let first = match first {
        // Nothing is running; the next order starts a fresh process anyway.
        Command::Restart | Command::Verify { .. } | Command::Verified { .. } => return Ok(()),
        // The same, and the reply it came after is counted on the model as it was known.
        Command::CheckModel { which, counted } => {
            count_on(app, shared, which, counted);
            return Ok(());
        }
        // No harness is running, so none holds the conversation. It is not started to be asked.
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
    let kept = settings::get(app);
    let computer = (kept.computer, computer::own(&binary));
    argv.extend(settings_args(app, &binary, computer));
    argv.extend(rules::args(app, computer::on(computer.0, computer.1), kept.browser == Some(true) && kept.browser_linked));
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
/// on only with a new Null. And `more`, whatever else there is to say to it,
/// in the harness's own form: the backup order, when there is one, and
/// computer use, when the person switched it.
pub fn own_settings(more: Option<Value>) -> Value {
    let mut settings = json!({ "startup": { "checkUpdate": false } });
    if let (Some(all), Some(Value::Object(more))) = (settings.as_object_mut(), more) {
        all.extend(more);
    }
    settings
}

/// Several things to say to the harness, each in its own form, as one. None
/// when there is nothing to say.
pub fn together(parts: impl IntoIterator<Item = Option<Value>>) -> Option<Value> {
    let mut all = serde_json::Map::new();
    for part in parts.into_iter().flatten() {
        if let Value::Object(part) = part {
            all.extend(part);
        }
    }
    (!all.is_empty()).then_some(Value::Object(all))
}

/// What to add when starting the harness so that it reads those settings. They
/// go in a file of Null's own beside Null's settings, written afresh each time;
/// the user's own Oh-my-pi settings are not touched. The text is JSON, which the
/// harness's settings format accepts. `computer` is the person's choice of
/// computer use and what the harness's own settings say of it.
fn settings_args(app: &AppHandle, binary: &Path, computer: (Option<bool>, Option<bool>)) -> Vec<String> {
    let Some(dir) = settings::dir(app) else { return Vec::new() };
    let path = dir.join(settings::file_name("harness", "yml"));
    let _ = std::fs::create_dir_all(&dir);
    // The file had another name while the backup order was all it held.
    let _ = std::fs::remove_file(dir.join(settings::file_name("backups", "yml")));
    let more = together([backups::harness_settings(app, binary), computer::harness_settings(computer.0, computer.1)]);
    match std::fs::write(&path, own_settings(more).to_string()) {
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
            Command::Send { which, text, files, after } => {
                let opened = self.open(which).await;
                // What was said before has been shown by now: the message goes under it.
                tell(self.app, self.shared, which, after);
                let session = match opened {
                    Ok(session) => session,
                    Err(reason) => {
                        fail_reply(self.app, self.shared, which, json!({ "type": "error", "message": reason, "code": null, "details": null }));
                        return Ok(());
                    }
                };
                self.put_back(which, &session).await;
                history::said(self.app, &session, &text);
                let request: PromptRequest = typed(json!({ "sessionId": session, "prompt": attach::blocks(&text, &files) }))?;
                let with_files = !files.is_empty();
                // One of the harness's own commands is answered by the harness, with no model asked.
                let own = with(self.shared, which, |held| held.answers_itself(&text)).unwrap_or(false);
                let (app, shared) = (self.app.clone(), self.shared.clone());
                self.cx
                    .send_request(request)
                    .on_receiving_result(async move |result| {
                        match result {
                            Ok(response) => {
                                let counted = reply_ended(&app, &shared, which, &as_json(&response), own);
                                let _ = order(&app.state::<Harness>(), Command::CheckModel { which, counted });
                                let _ = order(&app.state::<Harness>(), Command::Verify { which });
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
                    tell_work(self.app, self.shared); // it asks no longer
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
            Command::CheckModel { which, counted } => {
                self.check_model(which).await;
                count_on(self.app, self.shared, which, counted);
            }
            Command::Verify { which } => self.verify(which).await?,
            Command::Verified { which } => {
                let session = with(self.shared, which, |held| held.session.clone().filter(|_| held.loaded)).flatten();
                if let Some(session) = session {
                    self.put_back(which, &session).await;
                }
            }
            Command::Commands { reply } => {
                let which = lock(self.shared).open;
                // Opening the conversation is the whole of it: the harness says
                // what it can carry out as soon as the session is there, and that
                // arrives as an event, not as this answer.
                let _ = reply.send(self.open(which).await.map(|_| ()));
            }
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
    /// open, and show what was said in it. The one that was open goes on if it
    /// is working.
    ///
    /// A conversation this run holds, working or waiting to be seen, is not
    /// asked of the harness, which does not replay a reply that is under way:
    /// the box turns to it, and its number is given so that the page shows
    /// what Null kept of it. Any other is loaded by the harness. What the
    /// harness replays is kept back until it has said that it loaded, so a
    /// conversation that will not load leaves the box, and the conversation
    /// open in it, as they were.
    async fn open_earlier(&mut self, id: &str) -> Result<Option<u64>, String> {
        let held = {
            let mut shared = lock(self.shared);
            match shared.named(id) {
                // The open one, at rest, is loaded afresh: after a restart the box shows little of it.
                Some(which) if which == shared.open && !shared.open_one().busy => None,
                // One recalled from before Null was started: Null kept nothing of it, and the harness has it all.
                Some(which) if shared.of(which).is_some_and(|held| !held.busy && held.events.is_empty()) => None,
                Some(which) => Some((which, shared.turn_to(which))),
                None => None,
            }
        };
        if let Some((which, left)) = held {
            settings::update(self.app, |settings| settings.session = Some(id.to_string()));
            log!("the box turned to the conversation {id}, which this run held");
            tell_work(self.app, self.shared);
            if let Some(left) = left {
                self.close(&left).await;
            }
            return Ok(Some(which));
        }
        if !self.can_load {
            return Err("this harness does not load a conversation back".into());
        }
        let Some(folder) = self.folder_of(id).await else {
            // Recalled at the start, and gone from the harness since: deleted in
            // a terminal, or never written down. It is let go, so that it stops
            // saying it was cut off.
            if lock(self.shared).let_go_of_recalled(id) {
                log!("a conversation noted as cut off is gone from the harness; let go");
                tell_work(self.app, self.shared);
            }
            return Err("the harness no longer lists it".into());
        };
        // Said when it comes to the front, unless it is the box's own folder.
        let elsewhere = workspace(self.app).ok().filter(|own| *own != folder).map(|_| folder.clone());
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
        let (which, before, left) = {
            let mut shared = lock(self.shared);
            let before = shared.open_one().model.clone();
            // What was only recalled of it gives way to the conversation itself.
            if let Some(recalled) = shared.named(id).filter(|which| *which != shared.open) {
                shared.held.remove(&recalled);
            }
            let which = shared.hold(Conversation { session: Some(id.to_string()), loaded: true, model: before.clone(), folder: elsewhere.clone(), ..Default::default() });
            (which, before, shared.turn_to(which))
        };
        settings::update(self.app, |settings| settings.session = Some(id.to_string()));
        self.absorb_options(which, &response["configOptions"]);
        with(self.shared, which, |held| held.opened());
        log!("opened the earlier conversation {id} in {}", folder.display());

        let events = history::replayed(&replayed);
        history::opened(self.app, id, &events);
        // An earlier conversation is on the model it was left on, which may be another.
        let model = with(self.shared, which, |held| held.model.clone()).flatten().filter(|model| before.as_ref() != Some(model));
        publish(self.app, self.shared, which, json!({ "type": "conversation_opened", "events": events, "model": model, "folder": elsewhere.as_deref().map(attach::said_path) }));
        // What the harness can carry out is said as soon as a session starts,
        // which for a conversation loaded back from the list is while it is
        // still loading. It belongs to that one, and is kept, not replayed.
        if let Some(said) = replayed.iter().find_map(translate::commands_said) {
            publish(self.app, self.shared, which, said);
        }
        tell_work(self.app, self.shared);
        // The one that was left at rest, unless it is this same one, opened again.
        if let Some(left) = left.filter(|left| left != id) {
            self.close(&left).await;
        }
        Ok(None)
    }

    /// Learn which model a conversation is on, and say so if the harness moved it.
    async fn check_model(&mut self, which: u64) {
        let asked = with(self.shared, which, |held| (held.session.clone(), held.ask.clone(), held.loaded));
        let Some((Some(session), Some((id, value)), true)) = asked else { return };
        let Ok(request) = typed::<SetSessionConfigOptionRequest>(json!({ "sessionId": session, "configId": id, "value": value })) else { return };
        match self.cx.send_request(request).block_task().await {
            Ok(response) => {
                let options = &as_json(&response)["configOptions"];
                with(self.shared, which, |held| held.settings_said(options));
                let (_, current) = translate::models_from_config_options(options);
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
        // It works in the folder chosen for it when it was started, or else in the box's own.
        let own = workspace(self.app)?;
        let cwd = with(self.shared, which, |held| held.folder.clone()).flatten().unwrap_or_else(|| own.clone());
        let servers = mcp_servers(&self.mcp_capabilities);

        if let (Some(session), true) = (saved, self.can_load) {
            // One opened from `/history` may have been made in a terminal, in a folder of its own.
            let folder = self.folder_of(&session).await.unwrap_or_else(|| cwd.clone());
            with(self.shared, which, |held| held.folder = (folder != own).then(|| folder.clone()));
            with(self.shared, which, |held| held.replay = if held.events.is_empty() { Replay::Show } else { Replay::Drop });
            let request: LoadSessionRequest = typed(json!({ "sessionId": session, "cwd": folder, "mcpServers": servers }))?;
            let loaded = self.cx.send_request(request).block_task().await;
            with(self.shared, which, |held| held.replay = Replay::Live);
            match loaded {
                Ok(response) => {
                    self.absorb_options(which, &as_json(&response)["configOptions"]);
                    with(self.shared, which, |held| {
                        held.loaded = true;
                        held.opened();
                    });
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
        // The harness opens every conversation on its own default, and Null then
        // moves it to the model wanted. That model is the person's from now on, not
        // only once it is moved, so that a conversation started from this one in
        // between (`/new`) starts on it and not on the harness's default.
        let wanted = with(self.shared, which, |held| {
            held.opened();
            held.chosen.clone()
        })
        .flatten();
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
        // It has the harness's id now: what `/history` lists it under, and what is written down of it.
        tell_work(self.app, self.shared);
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
                    with(self.shared, which, |held| held.chosen = current.clone());
                }
            } else if !offered {
                log!("{wanted} is no longer offered; staying on {}", current.as_deref().unwrap_or("the default model"));
                settings::update(self.app, |settings| settings.model = current.clone());
                with(self.shared, which, |held| held.chosen = current.clone());
            }
        }
        with(self.shared, which, |held| held.opened());
        self.apply_level(which, &session).await;
        Ok(session)
    }

    /// Start a new conversation thinking as hard as the person last set with
    /// the harness's own `/effort`, when the model it is on can be set to
    /// that. The harness starts every conversation on its own level, so
    /// without this the person sets theirs again in each one.
    async fn apply_level(&mut self, which: u64, session: &str) {
        let Some(wanted) = settings::get(self.app).thinking else { return };
        let Some((offered, on)) = with(self.shared, which, |held| (held.levels.contains(&wanted), held.level())) else { return };
        if !offered || on.as_deref() == Some(wanted.as_str()) {
            return;
        }
        let Ok(request) = typed::<SetSessionConfigOptionRequest>(json!({ "sessionId": session, "configId": "thinking", "value": wanted })) else { return };
        let was_settling = with(self.shared, which, |held| std::mem::replace(&mut held.settling, true)).unwrap_or(false);
        let response = self.cx.send_request(request).block_task().await;
        match &response {
            Ok(response) => {
                self.absorb_options(which, &as_json(response)["configOptions"]);
                log!("the conversation starts on thinking level {wanted}, as last set in the box");
            }
            Err(error) => log!("could not start the conversation on thinking level {wanted}: {}", describe(error)),
        }
        with(self.shared, which, |held| held.settling = was_settling);
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
            // Already on it, as when a backup took over and the person then chooses that one.
            if let Some(held) = shared.of(which).filter(|held| held.model.as_deref() == Some(id)) {
                held.chosen = Some(id.to_string());
                return Ok(());
            }
            if !shared.models.is_empty() && !shared.models.iter().any(|model| model.id == id) {
                return Err(format!("{HARNESS_NAME} has no model named {id}"));
            }
        }
        self.move_to(which, session, id).await?;
        with(self.shared, which, |held| held.chosen = Some(id.to_string()));
        settings::update(self.app, |settings| settings.model = Some(id.to_string()));
        publish(self.app, self.shared, which, json!({ "type": "model_changed", "model": id }));
        Ok(())
    }

    /// Have the harness put a conversation on another model. It is Null's doing
    /// and is not taken for a backup taking over. Nothing is remembered of it
    /// and the box is not told: that is for whoever asked for the move.
    async fn move_to(&mut self, which: u64, session: &str, id: &str) -> Result<(), String> {
        let request: SetSessionConfigOptionRequest = typed(json!({ "sessionId": session, "configId": "model", "value": id }))?;
        let was_settling = with(self.shared, which, |held| std::mem::replace(&mut held.settling, true)).unwrap_or(false);
        let response = self.cx.send_request(request).block_task().await;
        if let Ok(response) = &response {
            self.absorb_options(which, &as_json(response)["configOptions"]);
            with(self.shared, which, |held| held.model = Some(id.to_string()));
        }
        with(self.shared, which, |held| held.settling = was_settling);
        response.map(|_| ()).map_err(|e| describe(&e))
    }

    /// Have a second model verify the answer a reply has just ended with, when
    /// the person named one (`/verify`) and this is a reply to verify. The
    /// conversation is moved to that model and asked in Null's own words; it
    /// is the same conversation, so the second model has before it everything
    /// the first saw and did. The answer is on the page already.
    async fn verify(&mut self, which: u64) -> Result<(), String> {
        let Some(verifier) = settings::get(self.app).verifier else { return Ok(()) };
        let offered = lock(self.shared).models.iter().any(|model| model.id == verifier);
        let Some((reply, session, on, on_a_backup)) = with(self.shared, which, |held| {
            let reply = verify::Reply { ended_well: !held.busy && held.ended == Some(Ended::Done), steps: held.steps, verification: held.verifying, own_folder: held.folder.is_none(), offered };
            (reply, held.session.clone().filter(|_| held.loaded), held.model.clone(), held.on_a_backup())
        }) else {
            return Ok(());
        };
        let (true, Some(session)) = (verify::due(&reply), session) else { return Ok(()) };
        // While a backup is answering the conversation is not moved: the model
        // that answered goes over its own answer, and when to go back to the
        // person's model is left to the harness, as it is when nothing verifies.
        let verifier = on.clone().filter(|_| on_a_backup).unwrap_or(verifier);
        with(self.shared, which, |held| {
            held.begin();
            held.verifying = true;
        });
        tell_work(self.app, self.shared);
        log!("the answer is verified by {verifier}");
        tell(self.app, self.shared, which, vec![json!({ "type": "verifying", "model": verifier }), json!({ "type": "status_change", "status": "running", "reason": null })]);
        if on.as_deref() != Some(verifier.as_str()) {
            if let Err(reason) = self.move_to(which, &session, &verifier).await {
                log!("could not move the conversation to {verifier}: {reason}");
                with(self.shared, which, |held| held.verifying = false);
                fail_reply(self.app, self.shared, which, json!({ "type": "error", "message": format!("the answer was not verified: {reason}"), "code": null, "details": null }));
                return Ok(());
            }
            with(self.shared, which, |held| held.back = on);
        }
        let request: PromptRequest = typed(json!({ "sessionId": session, "prompt": attach::blocks(verify::ASK, &[]) }))?;
        let (app, shared) = (self.app.clone(), self.shared.clone());
        self.cx
            .send_request(request)
            .on_receiving_result(async move |result| {
                match result {
                    // The second model's reply is counted as its own, on the model it was asked of.
                    Ok(response) => count_on(&app, &shared, which, reply_ended(&app, &shared, which, &as_json(&response), false)),
                    Err(error) => fail_reply(&app, &shared, which, error_event(&error)),
                }
                with(&shared, which, |held| held.verifying = false);
                let _ = order(&app.state::<Harness>(), Command::Verified { which });
                Ok(())
            })
            .map_err(|e| describe(&e))?;
        Ok(())
    }

    /// Put a conversation back on the model it was on before the second model
    /// was asked. A message the person sends meanwhile waits for this, so that
    /// it is answered by the model they chose.
    async fn put_back(&mut self, which: u64, session: &str) {
        let Some(back) = with(self.shared, which, |held| held.back.take()).flatten() else { return };
        match self.move_to(which, session, &back).await {
            Ok(()) => log!("the conversation is back on {back}"),
            Err(reason) => {
                log!("could not put the conversation back on {back}: {reason}");
                let on = with(self.shared, which, |held| held.model.clone()).flatten().unwrap_or_default();
                publish(self.app, self.shared, which, json!({ "type": "error", "message": format!("the conversation stays on {on}: it could not be put back on {back}"), "code": null, "details": reason }));
            }
        }
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
            held.levels = translate::thinking_from_config_options(options).0;
        }
    }
}

impl Conversation {
    /// The harness has said how the conversation's settings stand. The one Null
    /// asks with is kept at the value it has now. Null asks by setting it again,
    /// so asking with an older value would put that back: a thinking level the
    /// person set with the harness's own `/effort` would last until the next reply.
    fn settings_said(&mut self, options: &Value) {
        if let Some(ask) = translate::other_option(options) {
            self.ask = Some(ask);
        }
    }

    /// How hard its model thinks, as far as Null has been told.
    fn level(&self) -> Option<String> {
        self.ask.as_ref().filter(|(setting, _)| setting == "thinking").and_then(|(_, value)| value.as_str().map(str::to_string))
    }

    /// The thinking level the person has just set, when these settings say so:
    /// it is another than it was, on the same model, during a reply, which is
    /// how the harness's own `/effort` arrives. A backup taking over can bring
    /// another level with its model, and Null sets one itself as a conversation
    /// opens: neither is the person choosing, and neither is remembered.
    fn level_chosen(&self, options: &Value) -> Option<String> {
        if !self.busy || self.settling {
            return None;
        }
        let (_, model) = translate::models_from_config_options(options);
        if model.is_some() && model != self.model {
            return None;
        }
        let was = self.level()?;
        translate::thinking_from_config_options(options).1.filter(|now| *now != was)
    }

    /// The conversation is open. Unless the person has chosen a model for it
    /// since Null came to hold it, the one it is on now is the one they have
    /// it on.
    fn opened(&mut self) {
        if self.chosen.is_none() {
            self.chosen = self.model.clone();
        }
    }

    /// Whether a backup is answering in place of the model the person has it
    /// on: the harness moved the conversation and has not moved it back. Null
    /// then sets no model on it. Set on the backup by Null's own order, the
    /// conversation would stay there as if the person had chosen it; set on
    /// the person's model, the harness would try that one again and again
    /// before every reply while it still refuses.
    fn on_a_backup(&self) -> bool {
        matches!((&self.chosen, &self.model), (Some(chosen), Some(on)) if chosen != on)
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
        // One of the harness's own commands may have changed a setting, as `/effort` does.
        let chosen = with(shared, which, |held| {
            let chosen = held.level_chosen(&update["configOptions"]);
            held.settings_said(&update["configOptions"]);
            chosen
        });
        // A level the person set is where their next conversations start.
        if let Some(level) = chosen.flatten() {
            log!("thinking level {level}, set in the box: new conversations start on it");
            settings::update(app, |settings| settings.thinking = Some(level));
        }
        moved(app, shared, which, current);
        return;
    }
    // The box already shows a conversation that is being loaded back after the harness went away.
    let event = with(shared, which, |held| {
        held.count(update);
        if held.replay == Replay::Drop {
            None
        } else {
            translate::event_from_update(update, &mut held.tools)
        }
    })
    .flatten();
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
    tell_work(app, shared);
}

// ── What the page may ask ─────────────────────────────────────────────────

fn order(harness: &Harness, command: Command) -> Result<(), String> {
    harness.commands.unbounded_send(command).map_err(|_| "the harness thread has stopped".to_string())
}

/// True while the conversation the harness knows by this id is working.
pub fn working(app: &AppHandle, id: &str) -> bool {
    let harness = app.state::<Harness>();
    let mut shared = lock(&harness.shared);
    shared.named(id).and_then(|which| shared.of(which)).is_some_and(|held| held.busy)
}

/// A conversation is gone for good: if this run held it without showing it,
/// it is held no longer.
pub fn forget(app: &AppHandle, id: &str) {
    let harness = app.state::<Harness>();
    {
        let mut shared = lock(&harness.shared);
        if let Some(which) = shared.named(id).filter(|which| *which != shared.open) {
            shared.held.remove(&which);
        }
    }
    tell_work(app, &harness.shared);
}

/// Have the harness let go of a conversation the box has turned away from. Nobody waits for it.
fn let_go_of(harness: &Harness, left: Option<String>) {
    if let Some(id) = left {
        let (reply, _) = oneshot::channel();
        let _ = order(harness, Command::Close { id, reply });
    }
}

/// Let the running harness process go. The next order starts a fresh one and
/// loads the conversation back; a fresh process sees a provider just signed in
/// to. With conversations working, that would end them: it waits until
/// nothing is working, and the box says so.
pub fn restart(app: &AppHandle) {
    let harness = app.state::<Harness>();
    if lock(&harness.shared).restart_now() {
        let _ = order(&harness, Command::Restart);
    } else {
        log!("the harness is to be started again, once nothing is working");
        let _ = app.emit_to(panel::LABEL, "mini:notice", json!({ "text": RESTART_WAITS, "stay": false }));
    }
}

/// How many conversations are working.
pub fn working_now(app: &AppHandle) -> usize {
    lock(&app.state::<Harness>().shared).working()
}

/// Null is quitting at the person's word. Whatever is still working is
/// stopped by that, and what is written down says only what still waits to be seen.
pub fn quitting(app: &AppHandle) {
    let harness = app.state::<Harness>();
    lock(&harness.shared).quitting = true;
    write_down(app, &harness.shared);
}

/// Send a message to the open conversation. The reply arrives as events; the
/// number returned is the event to read after.
pub fn send_text(app: &AppHandle, text: String) -> Result<u64, String> {
    let harness = app.state::<Harness>();
    let (which, cursor) = lock(&harness.shared).begin_reply()?;
    // What is attached goes with this message. A file that has gone stops it.
    let files = attach::take(app).inspect_err(|_| {
        with(&harness.shared, which, |held| held.busy = false);
    })?;
    tell_work(app, &harness.shared);
    let saying = vec![attach::said(&text, &files), json!({ "type": "status_change", "status": "running", "reason": null })];
    // A conversation kept from before this run is shown as the harness loads it
    // back, and only while the box holds nothing of it. So the first message to
    // one waits to be shown until that is done, and then stands under it.
    let after = if with(&harness.shared, which, |held| held.comes_back()).unwrap_or(false) {
        saying
    } else {
        tell(app, &harness.shared, which, saying);
        Vec::new()
    };
    if let Err(reason) = order(&harness, Command::Send { which, text, files, after: after.clone() }) {
        tell(app, &harness.shared, which, after);
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
    tell_work(&app, &harness.shared);
    Ok(())
}

/// The models the conversation can use, across every provider the harness is signed in to.
#[tauri::command]
pub async fn models(app: AppHandle) -> Result<ModelList, String> {
    let (reply, answer) = oneshot::channel();
    order(&app.state::<Harness>(), Command::Models { reply })?;
    answer.await.map_err(|_| "the harness went away".to_string())?
}

/// Ask the harness to say what it can carry out, for the conversation the box
/// shows. The box cannot know until it has been told, and the harness says so
/// when a session starts: this is what starts one for a box that has asked
/// nothing yet. The list itself arrives as an event.
#[tauri::command]
pub async fn harness_commands(app: AppHandle) -> Result<(), String> {
    let (reply, answer) = oneshot::channel();
    order(&app.state::<Harness>(), Command::Commands { reply })?;
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
/// opens on the model last chosen. The one it showed goes on if it is working.
/// Gives the number the new one is known by, so that the page knows which
/// events are its own from now on.
#[tauri::command]
pub fn new_conversation(app: AppHandle) -> Result<u64, String> {
    let harness = app.state::<Harness>();
    let (which, left) = lock(&harness.shared).start_over();
    settings::update(&app, |settings| settings.session = None);
    let_go_of(&harness, left);
    tell_work(&app, &harness.shared);
    Ok(which)
}

/// The folders the harness's conversations work in, the most recently used
/// first, without the box's own: what `/folder` offers. Each as it is, and as
/// the box says it.
#[tauri::command]
pub async fn folders(app: AppHandle) -> Result<Vec<Value>, String> {
    let (listed, _) = conversations(&app).await?;
    let own = workspace(&app)?;
    Ok(history::folders(&listed.unwrap_or_default(), &own).iter().map(|folder| json!({ "path": folder, "shown": attach::said_path(folder) })).collect())
}

/// Start a conversation in a folder: the box turns to a new one, which the
/// next message opens there, and the one it showed goes on if it is working.
/// In that folder the agent has the project's own instructions and files, as
/// in a terminal. Gives the new one's number and the folder as the box says it.
#[tauri::command]
pub fn new_conversation_in(app: AppHandle, folder: String) -> Result<Value, String> {
    let folder = attach::folder_typed(&folder)?;
    let own = workspace(&app)?;
    let harness = app.state::<Harness>();
    let (which, left) = lock(&harness.shared).start_in((folder != own).then(|| folder.clone()));
    settings::update(&app, |settings| settings.session = None);
    let_go_of(&harness, left);
    tell_work(&app, &harness.shared);
    Ok(json!({ "conversation": which, "shown": attach::said_path(&folder) }))
}

/// How the conversations stand, for a page that has just loaded: the same as
/// `mini:work` tells it at every change.
#[tauri::command]
pub fn work(app: AppHandle) -> Value {
    let harness = app.state::<Harness>();
    let (told, work) = {
        let shared = lock(&harness.shared);
        (shared.told.0, shared.work())
    };
    json!({ "told": told, "conversations": titled(&app, work) })
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

/// Open an earlier conversation in the box, and the next message continues it.
/// One the harness loads arrives as an event, with what was said in it. One
/// this run already held, working or waiting to be seen, does not: its number
/// is given, and the page asks for what Null kept of it.
pub async fn open_earlier(app: &AppHandle, id: String) -> Result<Option<u64>, String> {
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
    /// The folder it works in, as the box says it, when that is not the box's own.
    folder: Option<String>,
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
/// list, a line narrows it and chooses the first entry left. A line that ends
/// in " &" is entered without that ending, and the next line does not wait
/// for its reply: that is how a conversation is left working. When the last
/// line has been dealt with, the page says what it shows.
///
/// A line in angle brackets is something a person does besides that, and none
/// of them waits for a reply: `<key ctrl+c>` presses a key, with `ctrl`, `meta`
/// or `shift` held when named; `<type /mo>` writes in the field without Enter;
/// `<click #history>` presses the first thing the selector finds; `<settle>`
/// waits until the page has stopped working; `<until list words>` waits for
/// words to stand in the list, and likewise in `shows` (the transcript), the
/// `reply` last begun (what the agent says, and not what was asked), the
/// `notice` or the `field`, or with `no` before it for that to be empty, which
/// is what keeps a run from resting on how fast the Mac is; and `<show name>`
/// logs what the page has at that moment, under that name, and the run goes on.
const SELFTEST_TYPING: &str = r#"async (lines) => {
  const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const settled = async () => {
    let quiet = 0;
    for (let waited = 0; waited < 120000 && quiet < 500; waited += 50) {
      await pause(50);
      quiet = busy || caret.classList.contains('working') ? 0 : quiet + 50;
    }
  };
  // What stands in a part of the page, as a person sees it: nothing when it is put away.
  const seen = (element) => (element && getComputedStyle(element).display !== 'none' ? element.innerText : '');
  let leftWorking = false;
  const pets = []; // PROTOTYPE (item 15): what the pet said at each of the lines below
  for (const line of lines) {
    // PROTOTYPE (item 15): "<away>" puts the box away as Esc does, "<pet>" is a
    // press on the pet alone, and "<wait 2000>" waits. None waits for a reply.
    const act = /^<(away|pet|wait)(?: (\d+))?>$/.exec(line);
    if (act) {
      if (act[1] === 'away') await invoke('hide_box');
      if (act[1] === 'pet') await invoke('show_box');
      await pause(Number(act[2]) || 700);
      pets.push(`${act[1]}: ${pet.getAttribute('class')}, ${busy ? 'working' : 'at rest'}`);
      continue;
    }
    // Waiting as a person does, with their eyes: for words in a part of the
    // page, or for that part to be empty. Half a minute, and then it is said.
    const until = /^<until (no )?(list|shows|reply|notice|field)(?: (.*))?>$/.exec(line);
    if (until) {
      const [, none, where, words = ''] = until;
      const there = () => {
        const replies = out.querySelectorAll('.reply');
        const text = where === 'list' ? seen(pick) : where === 'shows' ? out.innerText : where === 'reply' ? (replies.length ? replies[replies.length - 1].innerText : '') : where === 'notice' ? seen(noticeLine) : input.value;
        if (none) return !text.trim();
        return words ? text.toLowerCase().includes(words.toLowerCase()) : !!text.trim();
      };
      for (let waited = 0; waited < 30000 && !there(); waited += 50) await pause(50);
      if (!there()) await invoke('selftest_say', { name: line, state: 'NEVER SEEN' });
      continue;
    }
    // What a person does besides typing a line and Enter: a key, writing
    // without Enter, a press on something, a wait, and a look at the page.
    const more = /^<(key|type|click|settle|show)(?: (.*))?>$/.exec(line);
    if (more) {
      const [, what, rest = ''] = more;
      if (what === 'key') {
        const held = rest.split('+');
        const key = held.pop() || '+';
        const target = document.activeElement && document.activeElement !== document.body ? document.activeElement : input;
        target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ctrlKey: held.includes('ctrl'), metaKey: held.includes('meta'), shiftKey: held.includes('shift') }));
      }
      if (what === 'type') {
        input.value = rest;
        input.dispatchEvent(new Event('input'));
      }
      if (what === 'click') {
        const found = document.querySelector(rest);
        if (found) found.click();
        else await invoke('selftest_say', { name: rest, state: 'NOTHING TO CLICK' });
      }
      if (what === 'settle') await settled();
      else await pause(300);
      if (what === 'show') {
        await invoke('selftest_say', {
          name: rest,
          state: JSON.stringify({
            field: input.value,
            tinted: input.classList.contains('command'),
            masked: input.type,
            arrow: caret.className,
            pet: pet.getAttribute('class'),
            petShown: document.body.classList.contains('with-pet'),
            busy,
            height: lastHeight,
            transcript: out.clientHeight,
            scrolls: out.scrollHeight > out.clientHeight,
            focused: document.activeElement === input,
            notice: seen(noticeLine),
            chosen: (pick.querySelector('.selected') || { innerText: '' }).innerText,
            list: seen(pick),
            shows: out.innerText,
            lines: [...out.children].map((one) => one.className).join(','),
            tags: [...out.querySelectorAll('*')].reduce((count, one) => ({ ...count, [one.tagName]: (count[one.tagName] || 0) + 1 }), {}),
            // How far the page has moved sideways inside its window, and how wide that is.
            aside: document.scrollingElement.scrollLeft,
            wide: window.innerWidth,
          }),
        });
      }
      continue;
    }
    if (leftWorking) await pause(700); // long enough for the reply to have begun
    else await settled();
    leftWorking = line.endsWith(' &');
    const text = leftWorking ? line.slice(0, -2) : line;
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
    arrow: pets.length ? `${caret.className}; the pet said: ${pets.join(' > ')}` : caret.className,
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

/// What the page has part of the way through a `selftest` (`<show name>`),
/// logged and nothing more: the run goes on.
#[tauri::command]
pub fn selftest_say(name: String, state: String) {
    log!("selftest: at {name}: {}", state.replace('\n', " | "));
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

    #[test]
    fn what_null_has_to_say_to_the_harness_is_handed_over_as_one() {
        let backups = json!({ "retry": { "fallbackChains": { "default": ["a/one"] } } });
        let computer = json!({ "computer": { "enabled": true } });
        assert_eq!(together([None, None]), None, "nothing to say");
        assert_eq!(together([Some(backups.clone()), None]), Some(backups.clone()));
        assert_eq!(together([Some(backups), Some(computer)]), Some(json!({ "retry": { "fallbackChains": { "default": ["a/one"] } }, "computer": { "enabled": true } })));
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
    fn a_setting_the_harness_s_own_command_changed_is_asked_with_as_it_is_now() {
        let settings = |thinking: &str| json!([{ "id": "model", "category": "model", "currentValue": "a/b" }, { "id": "thinking", "currentValue": thinking }]);
        let mut shared = Shared::new(None, None);
        let held = shared.open_one();
        held.settings_said(&settings("high"));
        assert_eq!(held.ask, Some(("thinking".to_string(), json!("high"))));
        // `/effort low`, and the harness says so. Asking with "high" again would put it back.
        held.settings_said(&settings("low"));
        assert_eq!(held.ask, Some(("thinking".to_string(), json!("low"))));
        // Told nothing that can be asked with, what there was is kept.
        held.settings_said(&json!([{ "id": "model", "category": "model", "currentValue": "a/b" }]));
        assert_eq!(held.ask, Some(("thinking".to_string(), json!("low"))));
    }

    #[test]
    fn a_thinking_level_is_remembered_only_when_the_person_set_it() {
        let settings = |model: &str, thinking: &str| json!([{ "id": "model", "category": "model", "currentValue": model }, { "id": "thinking", "currentValue": thinking }]);
        let mut shared = Shared::new(None, None);
        let held = shared.open_one();
        held.model = Some("a/b".into());
        held.settings_said(&settings("a/b", "high"));
        assert_eq!(held.level().as_deref(), Some("high"));
        // No reply is under way: nobody typed `/effort`.
        assert_eq!(held.level_chosen(&settings("a/b", "low")), None);
        // `/effort low`, which the harness answers as it answers a message.
        held.busy = true;
        assert_eq!(held.level_chosen(&settings("a/b", "low")), Some("low".to_string()));
        assert_eq!(held.level_chosen(&settings("a/b", "high")), None, "said again as it already is");
        // A backup took over in the middle of a reply, and thinks at another level.
        assert_eq!(held.level_chosen(&settings("c/d", "low")), None);
        // Null is opening the conversation and setting its level itself.
        held.settling = true;
        assert_eq!(held.level_chosen(&settings("a/b", "low")), None);
        // A model that does not think has no level to have been changed.
        held.settling = false;
        held.ask = None;
        assert_eq!(held.level_chosen(&settings("a/b", "low")), None);
    }

    #[test]
    fn a_backup_is_answering_when_the_conversation_is_not_on_the_model_the_person_has_it_on() {
        let mut shared = Shared::new(None, None);
        let held = shared.open_one();
        assert!(!held.on_a_backup(), "nothing is known of a conversation that has not been opened");
        // It opens on a model, and nothing moves it.
        held.model = Some("a/own".into());
        held.opened();
        assert!(!held.on_a_backup());
        // A backup took over during a reply, which `moved` writes here.
        held.model = Some("b/backup".into());
        assert!(held.on_a_backup());
        // Loaded back while the backup answers, it is still the other that the person has it on.
        held.opened();
        assert_eq!(held.chosen.as_deref(), Some("a/own"));
        assert!(held.on_a_backup());
        // The harness goes back to their model by itself.
        held.model = Some("a/own".into());
        assert!(!held.on_a_backup());
        // The person chooses another with `/model`, which `apply_model` writes in both places.
        held.model = Some("c/chosen".into());
        held.chosen = Some("c/chosen".into());
        assert!(!held.on_a_backup());
    }

    #[test]
    fn a_new_conversation_starts_on_the_model_the_person_has_the_last_one_on() {
        let model = |shared: &mut Shared, which: u64| shared.of(which).and_then(|held| held.model.clone());
        // Never opened: the model last chosen, as it was kept.
        let mut shared = Shared::new(None, Some("a/own".into()));
        let (next, _) = shared.start_over();
        assert_eq!(model(&mut shared, next).as_deref(), Some("a/own"));
        // Being opened, as `open_quietly` does: the person's model is theirs before the
        // harness, which opened it on its own default, has been told to move it.
        let held = shared.open_one();
        held.opened();
        held.model = Some("z/default".into());
        let (next, _) = shared.start_over();
        assert_eq!(model(&mut shared, next).as_deref(), Some("a/own"), "not the harness's default");
        // A backup answering in the open one does not carry over either.
        let held = shared.open_one();
        held.opened();
        held.model = Some("b/backup".into());
        let (next, _) = shared.start_over();
        assert_eq!(model(&mut shared, next).as_deref(), Some("a/own"), "not the backup");
    }

    #[test]
    fn one_of_the_harness_s_own_commands_is_answered_without_a_model() {
        let mut shared = Shared::new(None, None);
        let open = shared.open;
        assert!(!shared.open_one().answers_itself("/context"), "nothing is the harness's own before it has said what it has");
        shared.record(open, json!({ "type": "commands", "commands": [{ "name": "context", "words": "", "hint": "" }, { "name": "skill:research", "words": "", "hint": "" }] }));
        let held = shared.open_one();
        assert!(held.answers_itself("/context"));
        assert!(held.answers_itself("/context and what follows it"));
        assert!(!held.answers_itself("/skill:research the box"), "a skill is a message to the model under another name");
        assert!(!held.answers_itself("/contexts"));
        assert!(!held.answers_itself("what does /context say"));
        assert!(!held.answers_itself("/"));
        // What it has is what it last said it has.
        shared.record(open, json!({ "type": "commands", "commands": [] }));
        assert!(!shared.open_one().answers_itself("/context"));
    }

    #[test]
    fn a_reply_s_steps_are_counted_once_each_and_so_are_the_ones_that_failed() {
        // As `on_update` takes an update in: it is counted, and then kept.
        fn hear(held: &mut Conversation, update: Value) {
            held.count(&update);
            translate::event_from_update(&update, &mut held.tools);
        }
        let begun = |id: &str| json!({ "sessionUpdate": "tool_call", "toolCallId": id, "title": "$ false", "kind": "execute", "status": "pending" });
        let said = |id: &str, status: &str| json!({ "sessionUpdate": "tool_call_update", "toolCallId": id, "status": status });
        let mut held = Conversation::default();
        // Nothing is counted of a conversation that is not answering.
        hear(&mut held, begun("before"));
        hear(&mut held, said("before", "failed"));
        assert_eq!((held.steps, held.failed.len()), (0, 0));

        held.begin();
        // A step that fails, as Oh-my-pi 18.8.7 says it: pending, in progress, failed.
        hear(&mut held, begun("c1"));
        hear(&mut held, said("c1", "in_progress"));
        hear(&mut held, said("c1", "failed"));
        // Said again, it is the same step and the same failure.
        hear(&mut held, begun("c1"));
        hear(&mut held, said("c1", "failed"));
        // One that goes well, and words between them, which are no step.
        hear(&mut held, json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "failed" } }));
        hear(&mut held, begun("c2"));
        hear(&mut held, said("c2", "completed"));
        // One the harness said nothing of before it failed.
        hear(&mut held, said("c3", "failed"));
        assert_eq!((held.steps, held.failed.len()), (2, 2));

        // Old steps replayed while a conversation is loaded back are not this reply's.
        held.replay = Replay::Show;
        hear(&mut held, begun("old"));
        hear(&mut held, said("old", "failed"));
        assert_eq!((held.steps, held.failed.len()), (2, 2));
        // The next reply is counted from nothing.
        held.replay = Replay::Live;
        held.begin();
        assert_eq!((held.steps, held.failed.len()), (0, 0));
    }

    #[test]
    fn a_reply_that_has_ended_gives_what_it_cost_and_one_that_never_began_gives_nothing() {
        let shared = Arc::new(Mutex::new(Shared::new(None, None)));
        let which = lock(&shared).begin_reply().expect("it may work").0;
        with(&shared, which, |held| {
            held.model = Some("a/b".into());
            held.ask = Some(("thinking".into(), json!("high")));
            held.count(&json!({ "sessionUpdate": "tool_call", "toolCallId": "c1", "status": "pending" }));
            held.count(&json!({ "sessionUpdate": "tool_call_update", "toolCallId": "c1", "status": "failed" }));
        });
        let counted = finish(&shared, which, Ended::Stopped).expect("a reply that began is counted");
        assert!(counted.at > 0 && counted.seconds >= 0.0);
        assert_eq!(counted, routing::Reply { model: Some("a/b".into()), thinking: Some("high".into()), steps: 1, failed: 1, ended: "stopped".into(), own_folder: true, verification: false, at: counted.at, seconds: counted.seconds });
        // It is given once, and a conversation nothing was asked of has nothing to count.
        assert_eq!(finish(&shared, which, Ended::Done), None);
        // One that works in a folder of the person's choosing says so, and so does a verification.
        let (elsewhere, _) = lock(&shared).start_in(Some(PathBuf::from("/tmp/elsewhere")));
        with(&shared, elsewhere, |held| {
            held.begin();
            held.verifying = true;
        });
        let counted = finish(&shared, elsewhere, Ended::Done).expect("it began");
        assert_eq!((counted.own_folder, counted.verification, counted.ended.as_str(), counted.steps), (false, true, "done", 0));
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
        let (now, left) = shared.start_over();
        assert!(now != was && shared.open == now && shared.held.len() == 1, "the box shows the new one, and the last is held no longer");
        assert_eq!(left, None, "the harness had not loaded the last one, so it has nothing to let go");
        let open = shared.open_one();
        assert_eq!((open.session.as_deref(), open.loaded, open.events.len(), open.model.as_deref()), (None, false, 0, Some("a/model")));
        assert_eq!(shared.record(was, json!("late")), None, "what the last one says after that is not kept");
    }

    #[test]
    fn a_conversation_left_working_goes_on_and_one_left_at_rest_is_let_go() {
        let mut shared = Shared::new(None, None);
        let first = shared.open;
        *shared.open_one() = opened("one");
        let (second, left) = shared.start_over();
        assert_eq!((left.as_deref(), shared.held.len(), shared.open), (Some("one"), 1, second), "left at rest: held no longer, and the harness is to let it go");
        assert!(shared.of(first).is_none());

        *shared.open_one() = Conversation { busy: true, ..opened("two") };
        let (third, left) = shared.start_over();
        assert_eq!((left, shared.held.len(), shared.open), (None, 2, third), "left working: it is kept, and the harness keeps it");
        assert!(shared.of(second).is_some_and(|held| held.busy));
    }

    #[test]
    fn a_reply_that_ends_while_the_box_shows_another_waits_to_be_seen() {
        let shared = Arc::new(Mutex::new(Shared::new(None, None)));
        let (working, shown) = {
            let mut shared = lock(&shared);
            *shared.open_one() = Conversation { busy: true, ..opened("one") };
            (shared.open, shared.start_over().0)
        };
        finish(&shared, working, Ended::Done);
        let mut shared = lock(&shared);
        assert!(shared.of(working).is_some_and(|held| !held.busy && held.unseen), "it is kept, at rest and not yet seen");

        assert_eq!(shared.turn_to(working), None, "the one the box showed had said nothing, and the harness never had it");
        assert!(shared.open == working && shared.of(shown).is_none());
        assert!(shared.of(working).is_some_and(|held| !held.unseen), "shown, it has been seen");
    }

    #[test]
    fn a_reply_that_ends_in_the_open_conversation_has_been_seen() {
        let shared = Arc::new(Mutex::new(Shared::new(None, None)));
        let open = lock(&shared).begin_reply().expect("it may work").0;
        finish(&shared, open, Ended::Done);
        assert!(lock(&shared).of(open).is_some_and(|held| !held.busy && !held.unseen));
    }

    #[test]
    fn a_fifth_conversation_is_not_set_to_work() {
        let mut shared = Shared::new(None, None);
        for _ in 0..MAX_WORKING {
            shared.begin_reply().expect("one of the four");
            shared.start_over();
        }
        assert_eq!(shared.begin_reply(), Err(TOO_MANY.to_string()));
        assert!(!shared.open_one().busy, "the fifth is not working, so the message can be sent again later");

        let one = *shared.held.keys().find(|which| **which != shared.open).unwrap();
        shared.turn_to(one);
        assert_eq!(shared.begin_reply(), Err("still answering the previous message".to_string()), "a second message to one that is working");

        shared.of(one).unwrap().busy = false;
        shared.start_over();
        assert!(shared.begin_reply().is_ok(), "one has ended, so there is room again");
    }

    #[test]
    fn the_page_is_told_of_what_works_asks_or_waits_to_be_seen_the_most_pressing_first() {
        let shared = Arc::new(Mutex::new(Shared::new(None, None)));
        // Four left behind: one done, one failed, one stopped by the person and one still working.
        let mut left = Vec::new();
        for (session, ended) in [("done", Some(Ended::Done)), ("failed", Some(Ended::Failed)), ("stopped", Some(Ended::Stopped)), ("working", None)] {
            let which = {
                let mut shared = lock(&shared);
                *shared.open_one() = Conversation { busy: true, ..opened(session) };
                let which = shared.open;
                shared.start_over();
                which
            };
            if let Some(ended) = ended {
                finish(&shared, which, ended);
            }
            left.push(which);
        }
        let mut shared = lock(&shared);
        let told = |shared: &Shared| shared.work().iter().map(|one| (one.id.clone().unwrap_or_default(), one.standing, one.unseen, one.open)).collect::<Vec<_>>();
        assert_eq!(told(&shared), [("failed".to_string(), "failed", true, false), ("done".to_string(), "done", true, false), ("working".to_string(), "working", false, false), ("stopped".to_string(), "stopped", true, false)]);

        // The new one the box shows has nothing to say until something is asked of it.
        let open = shared.begin_reply().expect("it may work").0;
        assert_eq!(told(&shared)[2], (String::new(), "working", false, true), "the newer of the two that work comes first");
        assert_eq!(shared.work()[2].conversation, open);

        // Shown, one has been seen; at rest, it has nothing more to say.
        shared.turn_to(left[1]);
        assert!(!told(&shared).iter().any(|one| one.0 == "failed"));
    }

    #[test]
    fn what_was_working_when_null_went_away_is_cut_off_and_waits_to_be_seen() {
        // What one run writes down.
        let shared = Arc::new(Mutex::new(Shared::new(None, None)));
        let ended = {
            let mut shared = lock(&shared);
            *shared.open_one() = Conversation { busy: true, ..opened("ended") };
            let which = shared.open;
            shared.start_over();
            *shared.open_one() = Conversation { busy: true, ..opened("working") };
            shared.start_over();
            which
        };
        finish(&shared, ended, Ended::Done);
        let written = lock(&shared).stands();
        assert_eq!(written, BTreeMap::from([("ended".to_string(), "done".to_string()), ("working".to_string(), "working".to_string())]));

        // What the next one makes of it, with the box on a conversation of its own.
        let mut next = Shared::new(Some("open".into()), None);
        next.recall(written.clone());
        let told: Vec<_> = next.work().iter().map(|one| (one.id.clone().unwrap_or_default(), one.standing, one.unseen, one.open)).collect();
        assert_eq!(told, [("working".to_string(), "cut off", true, false), ("ended".to_string(), "done", true, false)]);
        // That is where the page starts from, so that the first of them to be seen is news to it.
        assert_eq!(next.told.1, next.work());
        let seen = next.named("working").expect("it is held");
        next.turn_to(seen);
        assert_ne!(next.told.1, next.work(), "seen, it has nothing more to say, and the page has to hear that");
        let mut next = Shared::new(Some("open".into()), None);
        next.recall(written.clone());
        assert_eq!(next.stands(), BTreeMap::from([("ended".to_string(), "done".to_string()), ("working".to_string(), "cut off".to_string())]), "and it stays written down until it has been seen");

        // The one the box was on, if that was working: cut off too, and it is the open one.
        let mut next = Shared::new(Some("working".into()), None);
        next.recall(written);
        assert_eq!(next.held.len(), 2, "it is not held twice");
        assert!(next.work().iter().any(|one| one.open && one.standing == "cut off"));
        next.begin_reply().expect("it may work");
        assert!(next.work().iter().any(|one| one.open && one.standing == "working" && !one.unseen), "a message to it, and it has been seen");

        // Quitting at the person's word stops what works: only what waits to be seen stays written down.
        let mut shared = lock(&shared);
        shared.quitting = true;
        assert_eq!(shared.stands(), BTreeMap::from([("ended".to_string(), "done".to_string())]));
    }

    #[test]
    fn the_harness_is_started_again_only_once_nothing_is_working() {
        let shared = Arc::new(Mutex::new(Shared::new(None, None)));
        assert!(lock(&shared).restart_now(), "with nothing working it is started again at once");
        let working = lock(&shared).begin_reply().expect("it may work").0;
        assert!(!lock(&shared).restart_now(), "with something working it waits");
        assert!(!lock(&shared).restart_at_last(), "and goes on waiting while that works");
        finish(&shared, working, Ended::Done);
        assert!(lock(&shared).restart_at_last(), "until the reply has ended");
        assert!(!lock(&shared).restart_at_last(), "once");
    }

    #[test]
    fn a_conversation_started_in_a_folder_is_new_and_keeps_that_folder() {
        let mut shared = Shared::new(Some("one".into()), Some("a/model".into()));
        let (which, _) = shared.start_in(Some(PathBuf::from("/Users/someone/site")));
        let open = shared.open_one();
        assert_eq!((open.session.as_deref(), open.model.as_deref(), open.folder.as_deref()), (None, Some("a/model"), Some(Path::new("/Users/someone/site"))));
        assert_eq!(shared.since(0).conversation, which);
        assert!(shared.since(0).folder.is_some_and(|said| said.ends_with("site")), "a page is told where the open one works");

        // Starting over is in the box's own folder again, of which nothing is said.
        shared.start_over();
        assert_eq!((shared.open_one().folder.clone(), shared.since(0).folder), (None, None));
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

    #[test]
    fn a_conversation_kept_from_before_is_shown_before_the_first_message_to_it() {
        // Null starts again on the conversation it was left on. The harness keeps
        // it, and the box holds nothing of it until the harness has loaded it back.
        let mut shared = Shared::new(Some("kept".into()), None);
        assert!(shared.open_one().comes_back());
        // Once it is loaded, or the box holds something of it, a message is shown at once.
        shared.open_one().loaded = true;
        assert!(!shared.open_one().comes_back());
        shared.open_one().loaded = false;
        let open = shared.open;
        shared.record(open, json!("said"));
        assert!(!shared.open_one().comes_back());
        // A new conversation has nothing that could come back.
        assert!(!Shared::new(None, None).open_one().comes_back());
    }

    #[test]
    fn recalled_work_the_harness_no_longer_has_is_let_go_and_nothing_else_is() {
        let mut shared = Shared::new(Some("open".into()), None);
        shared.recall(BTreeMap::from([("gone".to_string(), "working".to_string()), ("open".to_string(), "done".to_string())]));
        assert_eq!(shared.work().len(), 2, "each says how it stands at the start");
        // One that is held only for what was noted of it goes, and with it what it said.
        assert!(shared.let_go_of_recalled("gone"));
        assert!(shared.named("gone").is_none());
        // The open one keeps its place in the box, without what was recalled of it.
        assert!(shared.let_go_of_recalled("open"));
        assert_eq!(shared.named("open"), Some(shared.open));
        assert!(shared.work().is_empty(), "nothing is left to say");
        // One working in this run, or with something of this run kept, is not touched.
        let working = shared.hold(Conversation { session: Some("working".into()), busy: true, ended: Some(Ended::CutOff), ..Default::default() });
        let spoken = shared.hold(Conversation { session: Some("spoken".into()), ended: Some(Ended::Done), unseen: true, ..Default::default() });
        shared.record(spoken, json!({ "type": "user_message", "text": "hello" }));
        assert!(!shared.let_go_of_recalled("working") && !shared.let_go_of_recalled("spoken"));
        assert!(shared.of(working).is_some() && shared.of(spoken).is_some());
        assert!(!shared.let_go_of_recalled("never held"));
    }
}
