//! Earlier conversations: what `/history` lists, and the titles Null makes.
//!
//! The harness keeps every conversation and lists them: the id of each, the
//! folder it works in, when it was last used, and a title when it has one. The
//! box draws that list as it comes, with the conversations made in a terminal
//! in other folders among its own, and opens one by having the harness load it
//! back in its own folder (`harness.rs`).
//!
//! The harness gives a conversation a title only in a terminal. For one made
//! in the box Null makes the title: the first line of its first message, cut
//! short. And the owner can give any conversation a title of their own
//! (`/rename`), which the box then shows in place of any other. Those titles
//! are the one thing Null keeps about a conversation besides which was open.
//! They are in `history.json` beside Null's settings, and a title goes when
//! the harness no longer lists its conversation. No other word of a
//! conversation is kept here, and the harness's own file is never changed:
//! a terminal goes on showing the title it had.
//!
//! The harness offers no way to delete a conversation either. It keeps each
//! as one file in its own folder and stops listing one whose file is gone, so
//! to delete a conversation Null moves that file to the Mac's Trash, where it
//! can be put back. This leans on where the harness keeps its files, which it
//! promises nobody: the harness check asks it of every version.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::log::log;
use crate::{harness, search, settings, translate};

/// How long a title Null makes may be, in characters. A row of the list shows about sixty.
const TITLE_LENGTH: usize = 80;

/// One earlier conversation, as the box lists it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    pub id: String,
    /// What it was about: the harness's title, else the one Null made. None when there is neither.
    pub title: Option<String>,
    /// The folder it works in.
    pub folder: String,
    /// Whether that is the folder the box's own conversations work in: made in the box, not in a terminal.
    pub own: bool,
    /// When it was last used, as the harness wrote it.
    pub used: Option<String>,
    /// Whether it is the one open in the box.
    pub open: bool,
}

/// What `/history` shows.
#[derive(Clone, Debug, Serialize)]
pub struct Listed {
    /// False when this harness does not list its conversations.
    pub lists: bool,
    /// False when Null cannot find where this harness keeps them, and so has
    /// nothing to move to the Trash.
    pub deletes: bool,
    pub conversations: Vec<Entry>,
}

/// The titles Null keeps, by conversation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Titles {
    /// Made by Null, from the first line of a conversation's first message.
    made: BTreeMap<String, String>,
    /// Given by the owner. One of these comes before any other title.
    given: BTreeMap<String, String>,
}

impl Titles {
    /// The owner calls a conversation something: kept as Null's own titles
    /// are, on one line and cut short. Nothing to call it takes the owner's
    /// title away again, and the title it had before shows once more. Gives
    /// the title as it was kept.
    fn rename(&mut self, id: &str, title: &str) -> Option<String> {
        match title_from(title) {
            Some(title) => {
                self.given.insert(id.to_string(), title.clone());
                Some(title)
            }
            None => {
                self.given.remove(id);
                None
            }
        }
    }

    /// A title does not outlive its conversation.
    fn keep_only(&mut self, listed: &HashSet<&str>) {
        self.made.retain(|id, _| listed.contains(id.as_str()));
        self.given.retain(|id, _| listed.contains(id.as_str()));
    }
}

/// A title out of words: their first line that has any, on one line, with
/// nothing in it but what can be read, cut to `TITLE_LENGTH` characters.
pub fn title_from(text: &str) -> Option<String> {
    let readable = |line: &str| line.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>();
    let line = text.lines().map(|line| readable(line).split_whitespace().collect::<Vec<_>>().join(" ")).find(|line| !line.is_empty())?;
    if line.chars().count() <= TITLE_LENGTH {
        return Some(line);
    }
    let cut: String = line.chars().take(TITLE_LENGTH - 1).collect();
    Some(format!("{}…", cut.trim_end()))
}

/// One page of the harness's list: the conversations on it, and what to ask
/// for the next page with. Nothing to ask with means it was the last.
pub fn page(answer: &Value) -> (Vec<Value>, Option<String>) {
    let kept = answer.get("sessions").and_then(Value::as_array).cloned().unwrap_or_default();
    let next = answer.get("nextCursor").and_then(Value::as_str).filter(|next| !next.is_empty()).map(str::to_string);
    (kept, next)
}

/// The folder the harness says a conversation works in. It loads one back
/// only there.
pub fn folder_of(listed: &[Value], id: &str) -> Option<PathBuf> {
    let kept = listed.iter().find(|kept| kept.get("sessionId").and_then(Value::as_str) == Some(id))?;
    kept.get("cwd").and_then(Value::as_str).filter(|folder| !folder.is_empty()).map(PathBuf::from)
}

/// The harness's list as the box shows it, in the harness's order. A
/// conversation in which nothing was said is left out: there is nothing to go
/// back to. `titles` are the ones Null keeps, `own` is the folder the box's
/// conversations work in, and `open` is the conversation open now. A title
/// the owner gave comes first, then the harness's, then the one Null made.
pub fn entries(listed: &[Value], titles: &Titles, own: &Path, open: Option<&str>) -> Vec<Entry> {
    let entry = |kept: &Value| {
        let id = kept.get("sessionId")?.as_str()?;
        if kept.pointer("/_meta/messageCount").and_then(Value::as_u64) == Some(0) {
            return None;
        }
        let folder = kept.get("cwd").and_then(Value::as_str).unwrap_or_default();
        let title = titles.given.get(id).cloned().or_else(|| kept.get("title").and_then(Value::as_str).and_then(title_from)).or_else(|| titles.made.get(id).cloned());
        let used = kept.get("updatedAt").and_then(Value::as_str).map(str::to_string);
        Some(Entry { id: id.to_string(), title, folder: folder.to_string(), own: Path::new(folder) == own, used, open: open == Some(id) })
    };
    listed.iter().filter_map(entry).collect()
}

/// What the harness replays of a conversation it loads back, as the events
/// the box draws a reply from. What a tool was given and gave back is left
/// out, and so is the model's thinking: the box shows neither.
pub fn replayed(updates: &[Value]) -> Vec<Value> {
    let mut tools = HashMap::new();
    let mut events = Vec::new();
    for mut event in updates.iter().filter_map(|update| translate::event_from_update(update, &mut tools)) {
        if event["type"] == "text_delta" && event["thinking"] == true {
            continue;
        }
        if event["type"] == "tool_activity" {
            event["raw_input"] = Value::Null;
            event["raw_output"] = Value::Null;
        }
        events.push(event);
    }
    events
}

/// The first thing the owner said in a replayed conversation.
fn first_said(events: &[Value]) -> Option<&str> {
    events.iter().find(|event| event["type"] == "user_message").and_then(|event| event["text"].as_str())
}

// ── The titles Null keeps ─────────────────────────────────────────────────

/// Read from the file the first time they are wanted.
static TITLES: Mutex<Option<Titles>> = Mutex::new(None);

fn file(app: &AppHandle) -> Option<PathBuf> {
    settings::dir(app).map(|dir| dir.join(settings::file_name("history", "json")))
}

/// The titles as the file holds them: the ones Null made under `titles`, as
/// they were before there was any other kind, and the owner's under `renamed`.
fn parse(kept: &Value) -> Titles {
    let under = |name: &str| {
        let titles = kept.get(name).and_then(Value::as_object).into_iter().flatten();
        titles.filter_map(|(id, title)| Some((id.clone(), title.as_str()?.to_string()))).collect()
    };
    Titles { made: under("titles"), given: under("renamed") }
}

fn written(titles: &Titles) -> Value {
    json!({ "titles": &titles.made, "renamed": &titles.given })
}

fn read(app: &AppHandle) -> Titles {
    parse(&file(app).and_then(|file| std::fs::read_to_string(file).ok()).and_then(|text| serde_json::from_str::<Value>(&text).ok()).unwrap_or(Value::Null))
}

/// Look at the titles, or change them. A change is written out.
fn titles<T>(app: &AppHandle, with: impl FnOnce(&mut Titles) -> T) -> T {
    let mut kept = TITLES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let titles = kept.get_or_insert_with(|| read(app));
    let before = titles.clone();
    let result = with(titles);
    if *titles != before {
        let written = file(app).ok_or_else(|| "Null has no folder of its own".to_string()).and_then(|file| {
            let text = serde_json::to_string_pretty(&written(titles)).map_err(|e| e.to_string())?;
            std::fs::write(file, text).map_err(|e| e.to_string())
        });
        if let Err(e) = written {
            log!("could not save the titles of earlier conversations: {e}");
        }
    }
    result
}

/// A message is on its way to a conversation. If that conversation has no
/// title of Null's making yet, this is its first message from the box, and
/// its first line is the title.
pub fn said(app: &AppHandle, id: &str, text: &str) {
    titles(app, |titles| {
        if !titles.made.contains_key(id) {
            titles.made.extend(title_from(text).map(|title| (id.to_string(), title)));
        }
    });
}

/// A conversation was opened from the list and replayed. One the harness gave
/// no title, and Null has none for, gets its title from what was said first.
pub fn opened(app: &AppHandle, id: &str, events: &[Value]) {
    if let Some(first) = first_said(events) {
        said(app, id, first);
    }
}

// ── What the page may ask ─────────────────────────────────────────────────

/// `/history`: the earlier conversations, for the box to list.
#[tauri::command]
pub async fn history(app: AppHandle) -> Result<Listed, String> {
    let (listed, open) = harness::conversations(&app).await?;
    let Some(listed) = listed else {
        return Ok(Listed { lists: false, deletes: false, conversations: Vec::new() });
    };
    let own = harness::workspace(&app)?;
    let conversations = titles(&app, |titles| {
        // A title does not outlive its conversation. An empty list is not taken
        // for every conversation being gone: it may be a harness that had a bad moment.
        if !listed.is_empty() {
            titles.keep_only(&listed.iter().filter_map(|kept| kept.get("sessionId")?.as_str()).collect());
        }
        entries(&listed, titles, &own, open.as_deref())
    });
    Ok(Listed { lists: true, deletes: sessions_folder().is_some_and(|sessions| sessions.is_dir()), conversations })
}

/// Open one of them in the box, in place of the conversation that is open. It
/// arrives as an event, with what was said in it; when it will not load, the
/// box is left as it was.
#[tauri::command]
pub async fn open_conversation(app: AppHandle, id: String) -> Result<(), String> {
    harness::open_earlier(&app, id).await
}

// ── Deleting ──────────────────────────────────────────────────────────────

/// Where the harness keeps its conversations: `sessions` in its own folder.
/// None for a run under a profile of the harness's, whose folder Null does
/// not know.
fn sessions_folder() -> Option<PathBuf> {
    if !harness::extra_args().is_empty() {
        return None;
    }
    let agent = match settings::harness_folder() {
        Some(folder) => folder,
        None => PathBuf::from(std::env::var_os("HOME")?).join(".omp/agent"),
    };
    Some(agent.join("sessions"))
}

/// The file the harness keeps a conversation in, found by its id: one each,
/// `sessions/<the folder it works in>/<when>_<id>.jsonl`. None unless there is
/// exactly one, which is a harness that keeps them some other way, and none
/// for an id that could not be one.
pub fn file_of(sessions: &Path, id: &str) -> Option<PathBuf> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return None;
    }
    let ending = format!("_{id}.jsonl");
    let mut found = Vec::new();
    for folder in std::fs::read_dir(sessions).ok()?.flatten() {
        for file in std::fs::read_dir(folder.path()).into_iter().flatten().flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            if name.ends_with(&ending) && !name.starts_with('.') {
                found.push(file.path());
            }
        }
    }
    (found.len() == 1).then(|| found.remove(0))
}

/// What the harness keeps beside a conversation's file, when it keeps
/// anything: a folder of the same name, for the conversations of the helpers
/// it started. It goes where the conversation goes.
fn beside(file: &Path) -> Option<PathBuf> {
    let folder = file.with_extension("");
    folder.is_dir().then_some(folder)
}

/// The programs that have a file open, by their numbers. A harness has the
/// file of each conversation it holds open for as long as it holds it. (The
/// lock file the harness leaves beside a conversation says nothing: it stays
/// when the harness has gone.)
pub fn held_by(file: &Path) -> Vec<u32> {
    let Ok(listed) = std::process::Command::new("/usr/sbin/lsof").arg("-t").arg("--").arg(file).output() else {
        return Vec::new();
    };
    let mut programs: Vec<u32> = String::from_utf8_lossy(&listed.stdout).lines().filter_map(|line| line.trim().parse().ok()).collect();
    programs.sort_unstable();
    programs.dedup();
    programs
}

/// Whether what `ps` writes for a program's terminal names one. A program a
/// person runs in a terminal window has one, `ttys003` and the like. One that
/// another program started in the background has none, written `??`.
fn names_a_terminal(written: &str) -> bool {
    let written = written.trim();
    !written.is_empty() && written.chars().any(|c| c.is_ascii_alphanumeric())
}

/// Whether a program is running in a terminal: someone has it in front of them.
pub fn in_a_terminal(program: u32) -> bool {
    let asked = std::process::Command::new("/bin/ps").args(["-o", "tty=", "-p"]).arg(program.to_string()).output();
    asked.is_ok_and(|answer| names_a_terminal(&String::from_utf8_lossy(&answer.stdout)))
}

/// Move a file or a folder to the Mac's Trash with macOS's own call for it,
/// which writes down where it came from, so that the Trash can put it back.
/// Gives where in the Trash it went.
pub fn to_trash(what: &Path) -> Result<PathBuf, String> {
    use objc::rc::autoreleasepool;
    use objc::runtime::{Object, BOOL, NO};
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::{CStr, CString};
    use std::os::raw::c_char;
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(what.as_os_str().as_bytes()).map_err(|_| "its name cannot be handed to macOS".to_string())?;
    autoreleasepool(|| unsafe {
        let words = |text: *mut Object| -> Option<String> {
            if text.is_null() {
                return None;
            }
            let utf8: *const c_char = msg_send![text, UTF8String];
            (!utf8.is_null()).then(|| CStr::from_ptr(utf8).to_string_lossy().into_owned())
        };
        let text: *mut Object = msg_send![class!(NSString), stringWithUTF8String: path.as_ptr()];
        let url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: text];
        let manager: *mut Object = msg_send![class!(NSFileManager), defaultManager];
        let mut landed: *mut Object = std::ptr::null_mut();
        let mut error: *mut Object = std::ptr::null_mut();
        let moved: BOOL = msg_send![manager, trashItemAtURL: url resultingItemURL: &mut landed error: &mut error];
        if moved == NO {
            let why: *mut Object = if error.is_null() { std::ptr::null_mut() } else { msg_send![error, localizedDescription] };
            return Err(words(why).unwrap_or_else(|| "macOS would not move it to the Trash".to_string()));
        }
        let place: *mut Object = if landed.is_null() { std::ptr::null_mut() } else { msg_send![landed, path] };
        Ok(words(place).map(PathBuf::from).unwrap_or_default())
    })
}

/// The x on a row of the list, or Command+Delete: move a conversation to the
/// Trash. Refused for one whose reply is still running, and for one that is
/// open in a terminal. Null's own harness lets go of the conversation first,
/// so whoever still has its file open is someone else; if that is a program in
/// a terminal, a person has the conversation in front of them, and it is left
/// alone. A program in the background that still holds it does not stand in
/// the way: nobody can see it to close it, and it loses nothing, since a
/// harness goes on writing to a conversation's file wherever the file is. Says
/// whether it was the conversation open in the box, which then has a new one.
/// Why one was not deleted goes in the log, without saying which.
#[tauri::command]
pub async fn delete_conversation(app: AppHandle, id: String) -> Result<bool, String> {
    let refuse = |why: &str| {
        log!("a conversation was not deleted: {why}");
        Err(why.to_string())
    };
    let open = harness::open_id(&app).as_deref() == Some(id.as_str());
    if open && harness::busy(&app) {
        return refuse("stop the reply with ctrl+c before deleting this conversation");
    }
    let Some(file) = sessions_folder().and_then(|sessions| file_of(&sessions, &id)) else {
        return refuse("this harness keeps its conversations where Null cannot find them");
    };
    harness::close(&app, id.clone()).await;
    let holders = held_by(&file);
    if holders.iter().any(|program| in_a_terminal(*program)) {
        return refuse("that conversation is open in a terminal. close it there first");
    }
    if !holders.is_empty() {
        log!("a program in the background still holds the conversation that is being deleted");
    }
    let helpers = beside(&file);
    if let Err(why) = to_trash(&file) {
        return refuse(&format!("it could not be moved to the Trash: {why}"));
    }
    // The conversation is gone from the list now. What was beside it follows; if it will not, it only takes room.
    if let Some(Err(why)) = helpers.as_deref().map(to_trash) {
        log!("what was kept beside a deleted conversation stayed where it was: {why}");
    }
    log!("a conversation was moved to the Trash");
    if open {
        let _ = harness::new_conversation(app.clone());
    }
    Ok(open)
}

// ── Looking inside ────────────────────────────────────────────────────────

/// Typing in `/history`: the conversations in which those words were said,
/// the best first (`search.rs`). Nothing when there is too little to look
/// for, or when Null cannot find where this harness keeps its conversations.
/// A newer search ends an older one. The log gets how many were found and how
/// long it took, and never a word.
#[tauri::command]
pub async fn search_conversations(words: String) -> Result<Vec<search::Found>, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LATEST: AtomicU64 = AtomicU64::new(0);

    let mine = LATEST.fetch_add(1, Ordering::SeqCst) + 1;
    let words = search::words_of(&words);
    let Some(sessions) = sessions_folder().filter(|_| !words.is_empty()) else {
        return Ok(Vec::new());
    };
    let began = std::time::Instant::now();
    let found = tauri::async_runtime::spawn_blocking(move || search::search(&sessions, &words, &|| LATEST.load(Ordering::SeqCst) == mine)).await.map_err(|e| e.to_string())?;
    if LATEST.load(Ordering::SeqCst) == mine {
        log!("looked inside the conversations in {} ms: {} found", began.elapsed().as_millis(), found.len());
    }
    Ok(found)
}

/// `/rename`, or the pencil in the list: the owner gives a conversation a
/// title, which the box shows from then on in place of any other. With no id
/// it is the conversation open in the box. A title with nothing in it takes
/// the owner's title away again. Gives the title as it was kept. What the
/// owner called it is theirs: it goes in `history.json` and never in the log.
#[tauri::command]
pub fn rename_conversation(app: AppHandle, id: Option<String>, title: String) -> Result<Option<String>, String> {
    let id = id.or_else(|| harness::open_id(&app)).ok_or("there is no conversation to rename yet")?;
    let kept = titles(&app, |titles| titles.rename(&id, &title));
    log!("{}", if kept.is_some() { "a conversation was given a title" } else { "a conversation's given title was taken away" });
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_the_first_line_that_says_anything_cut_short() {
        assert_eq!(title_from("why does the build fail on main?").as_deref(), Some("why does the build fail on main?"));
        assert_eq!(title_from("\n\n  fix   the\tlinker error  \nand then run the tests").as_deref(), Some("fix the linker error"));
        assert_eq!(title_from("a\u{1b}[31mb\u{0}c").as_deref(), Some("a [31mb c"));
        assert_eq!(title_from(" \n\t\n"), None);

        let long = title_from(&"word ".repeat(40)).unwrap();
        assert_eq!(long.chars().count(), TITLE_LENGTH);
        assert!(long.ends_with("word…"), "{long}");
        // Cut by characters, never through one.
        assert_eq!(title_from(&"é".repeat(200)).unwrap().chars().count(), TITLE_LENGTH);
    }

    /// As Oh-my-pi 18.8.7 answered `session/list` on 2026-10-10.
    fn listed() -> Vec<Value> {
        vec![
            json!({ "sessionId": "c3", "cwd": "/Users/me/Developer/site", "title": "Fix the linker error", "updatedAt": "2026-10-10T09:02:50.081Z", "_meta": { "messageCount": 6, "size": 2037 } }),
            json!({ "sessionId": "c2", "cwd": "/Users/me/Null/Workspace", "updatedAt": "2026-10-09T18:00:00.000Z", "_meta": { "messageCount": 2, "size": 2018 } }),
            json!({ "sessionId": "c1", "cwd": "/Users/me/Null/Workspace", "updatedAt": "2026-10-01T08:00:00.000Z", "_meta": { "messageCount": 0, "size": 811 } }),
            json!({ "sessionId": "c0", "cwd": "/Users/me/Developer/site", "updatedAt": "2026-09-12T08:00:00.000Z" }),
        ]
    }

    #[test]
    fn the_harness_s_list_is_shown_as_it_comes_with_the_titles_null_made() {
        let titles = Titles { made: BTreeMap::from([("c2".to_string(), "what is the capital of France?".to_string()), ("c3".to_string(), "never shown".to_string())]), given: BTreeMap::new() };
        let shown = entries(&listed(), &titles, Path::new("/Users/me/Null/Workspace"), Some("c2"));

        // The conversation nothing was said in is left out. The order is the harness's.
        assert_eq!(shown.iter().map(|entry| entry.id.as_str()).collect::<Vec<_>>(), ["c3", "c2", "c0"]);
        // The harness's title comes before Null's, and Null's stands in when the harness has none.
        assert_eq!(shown[0].title.as_deref(), Some("Fix the linker error"));
        assert_eq!(shown[1].title.as_deref(), Some("what is the capital of France?"));
        assert_eq!(shown[2].title, None);
        // Made in the box, or in a terminal somewhere else.
        assert_eq!(shown.iter().map(|entry| entry.own).collect::<Vec<_>>(), [false, true, false]);
        assert_eq!(shown[0].folder, "/Users/me/Developer/site");
        assert_eq!(shown[0].used.as_deref(), Some("2026-10-10T09:02:50.081Z"));
        assert_eq!(shown.iter().map(|entry| entry.open).collect::<Vec<_>>(), [false, true, false]);
    }

    #[test]
    fn a_harness_that_says_less_about_a_conversation_still_has_it_listed() {
        // No count of messages, no time, no folder: only that it exists.
        let shown = entries(&[json!({ "sessionId": "bare" }), json!({ "cwd": "/nameless" })], &Titles::default(), Path::new("/own"), None);
        assert_eq!(shown, vec![Entry { id: "bare".into(), title: None, folder: String::new(), own: false, used: None, open: false }]);
    }

    #[test]
    fn a_title_the_owner_gave_comes_before_the_harness_s_and_null_s() {
        let mut titles = Titles { made: BTreeMap::from([("c2".to_string(), "what is the capital of France?".to_string())]), given: BTreeMap::new() };
        let shown = |titles: &Titles| entries(&listed(), titles, Path::new("/own"), None).into_iter().map(|entry| entry.title).collect::<Vec<_>>();

        // One with the harness's title, one with Null's, one with none: each can be called something else.
        assert_eq!(titles.rename("c3", "  the arm64 build\nand more  ").as_deref(), Some("the arm64 build"));
        assert_eq!(titles.rename("c2", "capitals").as_deref(), Some("capitals"));
        assert_eq!(titles.rename("c0", &"long ".repeat(40)).map(|title| title.chars().count()), Some(TITLE_LENGTH));
        assert_eq!(shown(&titles)[..2], [Some("the arm64 build".to_string()), Some("capitals".to_string())]);
        assert!(shown(&titles)[2].as_deref().is_some_and(|title| title.starts_with("long long")));

        // Nothing to call it takes the owner's title away, and the one it had shows again.
        assert_eq!(titles.rename("c3", " \n "), None);
        assert_eq!(titles.rename("c2", ""), None);
        assert_eq!(shown(&titles)[..2], [Some("Fix the linker error".to_string()), Some("what is the capital of France?".to_string())]);
        // Taking away a title that was never given changes nothing.
        assert_eq!(titles.rename("c3", ""), None);
    }

    #[test]
    fn titles_are_kept_in_the_file_as_they_were_with_the_owner_s_beside_them() {
        // The file as an older Null wrote it holds only the titles Null made.
        let older = parse(&json!({ "titles": { "c2": "made by Null" } }));
        assert_eq!(older, Titles { made: BTreeMap::from([("c2".to_string(), "made by Null".to_string())]), given: BTreeMap::new() });

        let mut titles = older;
        titles.rename("c3", "given by the owner");
        assert_eq!(written(&titles), json!({ "titles": { "c2": "made by Null" }, "renamed": { "c3": "given by the owner" } }));
        assert_eq!(parse(&written(&titles)), titles);
        // Anything else in the file, or nothing at all, is no titles.
        assert_eq!(parse(&json!({ "titles": [1, 2], "renamed": "no" })), Titles::default());
        assert_eq!(parse(&Value::Null), Titles::default());
    }

    #[test]
    fn a_title_goes_with_its_conversation_whoever_gave_it() {
        let mut titles = Titles::default();
        titles.made.insert("gone".to_string(), "made".to_string());
        titles.made.insert("c2".to_string(), "made".to_string());
        titles.rename("gone", "given");
        titles.rename("c3", "given");
        titles.keep_only(&HashSet::from(["c2", "c3"]));
        assert_eq!(titles, Titles { made: BTreeMap::from([("c2".to_string(), "made".to_string())]), given: BTreeMap::from([("c3".to_string(), "given".to_string())]) });
    }

    /// A folder of the test's own, laid out as Oh-my-pi 18.8.7 lays out its conversations.
    fn kept(name: &str, files: &[&str]) -> PathBuf {
        let sessions = std::env::temp_dir().join(format!("null-history-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&sessions);
        for file in files {
            let file = sessions.join(file);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, "{}\n").unwrap();
        }
        sessions
    }

    #[test]
    fn a_conversation_s_file_is_found_by_its_id() {
        let sessions = kept(
            "found",
            &[
                "-Users-me-site-/2026-10-10T09-02-50-081Z_01a1-c3.jsonl",
                "-Users-me-site-/.2026-10-10T09-02-50-081Z_01a1-c3.jsonl.lock.os",
                "-Users-me-site-/2026-10-10T09-02-50-081Z_01a1-c3/2026-10-10T09-05-00-000Z_01a1-helper.jsonl",
                "-Users-me-Null-Workspace-/2026-10-09T18-00-00-000Z_01a1-c2.jsonl",
                "-Users-me-Null-Workspace-/2026-10-01T08-00-00-000Z_01a1-twice.jsonl",
                "-Users-me-site-/2026-10-02T08-00-00-000Z_01a1-twice.jsonl",
            ],
        );
        let found = file_of(&sessions, "01a1-c3").expect("the conversation's own file, not its lock");
        assert_eq!(found, sessions.join("-Users-me-site-/2026-10-10T09-02-50-081Z_01a1-c3.jsonl"));
        // What the harness keeps beside it, for the helpers it started, goes with it.
        assert_eq!(beside(&found), Some(sessions.join("-Users-me-site-/2026-10-10T09-02-50-081Z_01a1-c3")));
        assert_eq!(beside(&file_of(&sessions, "01a1-c2").unwrap()), None);
        // A helper's conversation is not one of the list's. Two files for one id is a layout Null does not know.
        assert_eq!(file_of(&sessions, "01a1-helper"), None);
        assert_eq!(file_of(&sessions, "01a1-twice"), None);
        assert_eq!(file_of(&sessions, "01a1-gone"), None);
        // Nothing that is no id is looked for, whatever it would match.
        for no_id in ["", "c3", "../01a1-c3", "01a1-c3.jsonl", "*"] {
            assert_eq!(file_of(&sessions, no_id), None, "{no_id:?}");
        }
        assert_eq!(file_of(&sessions.join("nowhere"), "01a1-c3"), None);
        let _ = std::fs::remove_dir_all(sessions);
    }

    #[test]
    fn a_program_in_a_terminal_is_told_from_one_in_the_background() {
        // As `ps -o tty=` writes it: a terminal's name, or that there is none.
        for terminal in ["ttys003", "ttys012\n", "  console  ", "pts/4"] {
            assert!(names_a_terminal(terminal), "{terminal:?}");
        }
        for none in ["??", "??\n", "", " \n", "-", "?"] {
            assert!(!names_a_terminal(none), "{none:?}");
        }
        // A program that is not running is in no terminal.
        assert!(!in_a_terminal(u32::MAX - 7));
    }

    #[test]
    fn a_file_that_a_program_has_open_is_known_to_be_held() {
        let sessions = kept("held", &["-w-/2026-10-10T09-02-50-081Z_01a1-c3.jsonl"]);
        let file = file_of(&sessions, "01a1-c3").unwrap();
        assert_eq!(held_by(&file), Vec::<u32>::new());
        let open = std::fs::File::open(&file).unwrap();
        assert_eq!(held_by(&file), vec![std::process::id()]);
        drop(open);
        assert_eq!(held_by(&file), Vec::<u32>::new());
        let _ = std::fs::remove_dir_all(sessions);
    }

    #[test]
    fn the_list_is_read_a_page_at_a_time() {
        let (kept, next) = page(&json!({ "sessions": listed(), "nextCursor": "50" }));
        assert_eq!((kept.len(), next.as_deref()), (4, Some("50")));
        // The last page says nothing about a next one.
        assert_eq!(page(&json!({ "sessions": [] })), (Vec::new(), None));
        assert_eq!(page(&json!({ "sessions": [], "nextCursor": null })), (Vec::new(), None));
        assert_eq!(page(&Value::Null), (Vec::new(), None));
    }

    #[test]
    fn a_conversation_is_loaded_back_in_the_folder_the_harness_names() {
        assert_eq!(folder_of(&listed(), "c3"), Some(PathBuf::from("/Users/me/Developer/site")));
        assert_eq!(folder_of(&listed(), "gone"), None);
        assert_eq!(folder_of(&[json!({ "sessionId": "bare" })], "bare"), None);
    }

    #[test]
    fn a_replay_is_drawn_from_what_was_said_and_done_and_nothing_else() {
        let updates = vec![
            json!({ "sessionUpdate": "available_commands_update", "availableCommands": [] }),
            json!({ "sessionUpdate": "user_message_chunk", "content": { "type": "text", "text": "list the files\nand count them" } }),
            json!({ "sessionUpdate": "agent_thought_chunk", "content": { "type": "text", "text": "the owner wants a listing" } }),
            json!({ "sessionUpdate": "tool_call", "toolCallId": "t1", "title": "$ ls", "status": "pending", "rawInput": { "command": "ls" } }),
            json!({ "sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed", "rawOutput": "a.txt\nb.txt" }),
            json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": "Two files." } }),
        ];
        let events = replayed(&updates);

        assert_eq!(events.iter().map(|event| event["type"].as_str().unwrap()).collect::<Vec<_>>(), ["user_message", "tool_activity", "tool_activity", "text_delta"]);
        assert_eq!(events[2]["title"], "$ ls");
        assert_eq!(events[2]["status"], "completed");
        assert!(events.iter().all(|event| event.get("raw_input").is_none_or(Value::is_null) && event.get("raw_output").is_none_or(Value::is_null)), "{events:?}");
        assert_eq!(events[3], json!({ "type": "text_delta", "text": "Two files.", "thinking": false }));
        // The title Null makes for it is the first line of what the owner said first.
        assert_eq!(first_said(&events).and_then(title_from).as_deref(), Some("list the files"));
        assert_eq!(first_said(&replayed(&[])), None);
    }
}
