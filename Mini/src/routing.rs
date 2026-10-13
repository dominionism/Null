//! What each reply cost, in numbers: how many steps it took, how many of them
//! failed, how long it ran, on which model, and how it ended.
//!
//! Nothing is done with the numbers yet. They are what is to say when a reply
//! is stuck, and on which model that happens, before any reply is ever moved
//! to another model on their word. Until now that could only be found by
//! reading a conversation's file by hand.
//!
//! `harness.rs` hears every step and the end of every reply, and hands the
//! count over here, where the last few hundred are kept in a file of Null's
//! own, beside the settings. It holds numbers and the names of models and
//! nothing else: no word that was typed, shown or read, and no conversation's
//! id, title or folder. It stays on this Mac.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::log::log;
use crate::settings;

/// How many replies are kept. The oldest goes first.
const KEPT: usize = 500;

/// One reply, as it is kept.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reply {
    /// When it ended, in seconds since 1970.
    pub at: u64,
    /// The model the conversation was on when it ended. None when it never came to be on one.
    pub model: Option<String>,
    /// How hard that model was set to think, as the harness names the level.
    pub thinking: Option<String>,
    /// How many steps it took.
    pub steps: u32,
    /// How many of them failed.
    pub failed: u32,
    /// How long it ran, from the message being sent to the reply ending.
    pub seconds: f64,
    /// How it ended: done, failed, stopped or cut off.
    pub ended: String,
    /// The conversation works in the box's own folder, and not in one chosen with `/folder`.
    pub own_folder: bool,
    /// It was the second model verifying the answer before it (`verify.rs`), asked by Null.
    pub verification: bool,
}

/// The file as it is written.
#[derive(Default, Deserialize)]
#[serde(default)]
struct File {
    replies: Vec<Reply>,
}

/// Replies end on more than one thread, and each is added to what the file holds.
static WRITING: Mutex<()> = Mutex::new(());

/// Now, as a reply's `at`.
pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or_default()
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    settings::dir(app).map(|dir| dir.join(settings::file_name("routing", "json")))
}

/// The replies a file's text holds, the oldest first. None when it cannot be read.
fn read(text: &str) -> Vec<Reply> {
    serde_json::from_str::<File>(text).map(|file| file.replies).unwrap_or_default()
}

/// The file's text for these replies: one to a line, so that it can be read by eye.
fn written(replies: &[Reply]) -> String {
    let lines: Vec<String> = replies.iter().filter_map(|reply| serde_json::to_string(reply).ok()).collect();
    format!("{{\"replies\":[\n{}\n]}}\n", lines.join(",\n"))
}

/// The replies with one more, and no more of them than are kept.
fn with(mut replies: Vec<Reply>, reply: Reply) -> Vec<Reply> {
    replies.push(reply);
    let over = replies.len().saturating_sub(KEPT);
    replies.drain(..over);
    replies
}

/// Keep the count of a reply that has ended.
pub fn count(app: &AppHandle, reply: Reply) {
    let on = reply.model.as_deref().unwrap_or("no model");
    log!("reply counted: {} step(s), {} failed, {} s, {}, on {on}", reply.steps, reply.failed, reply.seconds, reply.ended);
    let Some(path) = path(app) else { return };
    let _one = WRITING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let replies = with(std::fs::read_to_string(&path).map(|text| read(&text)).unwrap_or_default(), reply);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Written beside the file and moved into its place, so that a file cut short is never the one read.
    let beside = path.with_extension("json.part");
    let kept = std::fs::write(&beside, written(&replies)).and_then(|()| std::fs::rename(&beside, &path));
    if let Err(e) = kept {
        log!("could not keep the count of a reply: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(steps: u32) -> Reply {
        Reply { at: 1_791_900_000, model: Some("a/b".into()), thinking: Some("high".into()), steps, failed: 1, seconds: 12.3, ended: "done".into(), own_folder: true, verification: false }
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let replies = vec![reply(1), Reply { model: None, thinking: None, ended: "cut off".into(), ..reply(2) }];
        assert_eq!(read(&written(&replies)), replies);
        assert_eq!(read(&written(&[])), []);
        assert_eq!(written(&replies).lines().count(), 4, "one reply to a line, between the two that hold them");
    }

    #[test]
    fn a_reply_is_kept_as_numbers_a_model_s_name_and_how_it_ended_and_nothing_else() {
        let kept = serde_json::to_value(reply(3)).unwrap();
        let mut names: Vec<&str> = kept.as_object().unwrap().keys().map(String::as_str).collect();
        names.sort_unstable();
        // Whatever is added here is kept of every reply, of everyone: nothing typed, shown or read, and no conversation's id.
        assert_eq!(names, ["at", "ended", "failed", "model", "own_folder", "seconds", "steps", "thinking", "verification"]);
    }

    #[test]
    fn the_oldest_goes_when_there_are_more_than_are_kept() {
        let mut replies = Vec::new();
        for steps in 0..KEPT as u32 + 2 {
            replies = with(replies, reply(steps));
        }
        assert_eq!(replies.len(), KEPT);
        assert_eq!((replies[0].steps, replies[KEPT - 1].steps), (2, KEPT as u32 + 1), "the newest are the ones kept, in their order");
    }

    #[test]
    fn a_file_that_cannot_be_read_starts_from_nothing_and_an_older_one_still_loads() {
        assert_eq!(read("not a file of replies"), []);
        assert_eq!(read(""), []);
        let older = read(r#"{"replies":[{"steps":4,"ended":"done"}]}"#);
        assert_eq!(older, [Reply { steps: 4, ended: "done".into(), ..Default::default() }]);
    }
}
