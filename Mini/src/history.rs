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
//! short. Those titles are the one thing Null keeps about a conversation
//! besides which was open. They are in `history.json` beside Null's settings,
//! and a title goes when the harness no longer lists its conversation. No
//! other word of a conversation is kept here.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::log::log;
use crate::{harness, settings, translate};

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
    pub conversations: Vec<Entry>,
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
/// back to. `titles` are the ones Null made, `own` is the folder the box's
/// conversations work in, and `open` is the conversation open now.
pub fn entries(listed: &[Value], titles: &BTreeMap<String, String>, own: &Path, open: Option<&str>) -> Vec<Entry> {
    let entry = |kept: &Value| {
        let id = kept.get("sessionId")?.as_str()?;
        if kept.pointer("/_meta/messageCount").and_then(Value::as_u64) == Some(0) {
            return None;
        }
        let folder = kept.get("cwd").and_then(Value::as_str).unwrap_or_default();
        let title = kept.get("title").and_then(Value::as_str).and_then(title_from).or_else(|| titles.get(id).cloned());
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

// ── The titles Null made ──────────────────────────────────────────────────

/// By conversation. Read from the file the first time they are wanted.
static TITLES: Mutex<Option<BTreeMap<String, String>>> = Mutex::new(None);

fn file(app: &AppHandle) -> Option<PathBuf> {
    settings::dir(app).map(|dir| dir.join(settings::file_name("history", "json")))
}

fn read(app: &AppHandle) -> BTreeMap<String, String> {
    let kept = file(app).and_then(|file| std::fs::read_to_string(file).ok()).and_then(|text| serde_json::from_str::<Value>(&text).ok()).unwrap_or(Value::Null);
    let titles = kept.get("titles").and_then(Value::as_object).into_iter().flatten();
    titles.filter_map(|(id, title)| Some((id.clone(), title.as_str()?.to_string()))).collect()
}

/// Look at the titles, or change them. A change is written out.
fn titles<T>(app: &AppHandle, with: impl FnOnce(&mut BTreeMap<String, String>) -> T) -> T {
    let mut kept = TITLES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let titles = kept.get_or_insert_with(|| read(app));
    let before = titles.clone();
    let result = with(titles);
    if *titles != before {
        let written = file(app).ok_or_else(|| "Null has no folder of its own".to_string()).and_then(|file| {
            let text = serde_json::to_string_pretty(&json!({ "titles": &*titles })).map_err(|e| e.to_string())?;
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
        if !titles.contains_key(id) {
            titles.extend(title_from(text).map(|title| (id.to_string(), title)));
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
        return Ok(Listed { lists: false, conversations: Vec::new() });
    };
    let own = harness::workspace(&app)?;
    let conversations = titles(&app, |titles| {
        // A title does not outlive its conversation. An empty list is not taken
        // for every conversation being gone: it may be a harness that had a bad moment.
        if !listed.is_empty() {
            let kept: HashSet<&str> = listed.iter().filter_map(|kept| kept.get("sessionId")?.as_str()).collect();
            titles.retain(|id, _| kept.contains(id.as_str()));
        }
        entries(&listed, titles, &own, open.as_deref())
    });
    Ok(Listed { lists: true, conversations })
}

/// Open one of them in the box, in place of the conversation that is open. It
/// arrives as an event, with what was said in it; when it will not load, the
/// box is left as it was.
#[tauri::command]
pub async fn open_conversation(app: AppHandle, id: String) -> Result<(), String> {
    harness::open_earlier(&app, id).await
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
        let titles = BTreeMap::from([("c2".to_string(), "what is the capital of France?".to_string()), ("c3".to_string(), "never shown".to_string())]);
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
        let shown = entries(&[json!({ "sessionId": "bare" }), json!({ "cwd": "/nameless" })], &BTreeMap::new(), Path::new("/own"), None);
        assert_eq!(shown, vec![Entry { id: "bare".into(), title: None, folder: String::new(), own: false, used: None, open: false }]);
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
