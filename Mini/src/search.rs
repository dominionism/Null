//! Searching inside conversations: `/history` finds one by something said in it.
//!
//! The harness offers no search. It keeps each conversation as one file, a
//! line for each thing that happened, with what was said in it as plain words
//! (`history.rs`). So Null looks through those files as it is asked, and keeps
//! nothing: no index, and nothing of what it finds in its log.
//!
//! The look is in two steps, so that it is over before it is noticed. First
//! every file is looked through whole for the words, on every core: fast, and
//! it leaves few files. Then only those are read line by line, for the words
//! in what the owner wrote and what the agent replied. What a tool read or
//! gave back is not searched: it would bury what was said.
//!
//! Capitals do not matter for the letters A to Z. A conversation is found when
//! every word was said in it, wherever and in whatever order. The more often
//! and the more tellingly, the higher it stands: its title counts most, then
//! what the owner wrote, then what the agent replied.
//!
//! Like deleting, this leans on how the harness keeps its files, which it
//! promises nobody. The harness check asks it of every version, and on a
//! harness that keeps them some other way a search finds nothing here and the
//! list is narrowed by its titles alone, as before.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use memchr::memmem;
use serde::Serialize;
use serde_json::Value;

/// How many conversations a search gives back at most: the best of them.
const MOST: usize = 40;
/// How much of the line a word was found in is shown: this many letters before it, and after.
const BEFORE: usize = 24;
const AFTER: usize = 60;

/// How much one mention counts for, by where it is.
const IN_TITLE: f64 = 5.0;
const BY_OWNER: f64 = 3.0;
const BY_AGENT: f64 = 1.0;

/// A conversation in which the words were said.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Found {
    pub id: String,
    /// A short piece of what was said, around the first of the words.
    pub piece: String,
    /// How well it answers the search. Higher is better.
    pub rank: f64,
}

/// What was typed, as the words to look for: in small letters, without the
/// spaces between them. Nothing to look for when it is too little to mean
/// anything: one letter is in every conversation.
pub fn words_of(typed: &str) -> Vec<String> {
    let words: Vec<String> = typed.split_whitespace().map(|word| word.to_ascii_lowercase()).collect();
    if words.iter().map(|word| word.chars().count()).sum::<usize>() < 2 {
        return Vec::new();
    }
    words
}

/// Every conversation the harness keeps under `sessions`, each with its id:
/// the files `<the folder it works in>/<when>_<id>.jsonl`. What lies deeper
/// belongs to a conversation's helpers, and is not a conversation of the list.
fn conversations(sessions: &Path) -> Vec<(String, PathBuf)> {
    let mut kept = Vec::new();
    for folder in std::fs::read_dir(sessions).into_iter().flatten().flatten() {
        for file in std::fs::read_dir(folder.path()).into_iter().flatten().flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let id = name.strip_suffix(".jsonl").filter(|_| !name.starts_with('.')).and_then(|stem| stem.split_once('_')).map(|(_, id)| id.to_string());
            if let Some(id) = id.filter(|id| !id.is_empty()) {
                kept.push((id, file.path()));
            }
        }
    }
    kept
}

/// How often `word` is in `text`. Both are in small letters already.
fn times(text: &str, word: &str) -> usize {
    memmem::find_iter(text.as_bytes(), word.as_bytes()).count()
}

/// What was said on one line of a conversation's file, and by whom: the title
/// the harness gave it, something the owner wrote, or something the agent
/// replied. None for every other line, a tool's among them.
fn said(line: &str) -> Option<(f64, String)> {
    let entry: Value = serde_json::from_str(line).ok()?;
    match entry.get("type")?.as_str()? {
        "title" => Some((IN_TITLE, entry.get("title")?.as_str()?.to_string())),
        "message" => {
            let message = entry.get("message")?;
            let weight = match message.get("role")?.as_str()? {
                "user" => BY_OWNER,
                "assistant" => BY_AGENT,
                _ => return None,
            };
            let words = match message.get("content")? {
                Value::String(text) => text.clone(),
                Value::Array(parts) => {
                    let texts = parts.iter().filter(|part| part.get("type").and_then(Value::as_str) == Some("text"));
                    texts.filter_map(|part| part.get("text")?.as_str()).collect::<Vec<_>>().join("\n")
                }
                _ => return None,
            };
            Some((weight, words))
        }
        _ => None,
    }
}

/// A short piece of `text` around the place a word was found at, on one line,
/// with a mark at an end that was cut.
fn piece(text: &str, at: usize, length: usize) -> String {
    let mut from = at.saturating_sub(BEFORE);
    while !text.is_char_boundary(from) {
        from -= 1;
    }
    let mut to = (at + length + AFTER).min(text.len());
    while !text.is_char_boundary(to) {
        to += 1;
    }
    let shown = text[from..to].split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}{shown}{}", if from > 0 { "…" } else { "" }, if to < text.len() { "…" } else { "" })
}

/// Look for the words in one conversation's file, already read. Gives how
/// well it answers and a piece of what was said, when every word is said in it.
fn look(file: &str, words: &[String]) -> Option<(f64, String)> {
    let mut counts = vec![0.0; words.len()];
    // The piece comes from what counts most: the owner's own words before the agent's.
    let mut best: Option<(f64, String)> = None;
    for line in file.lines() {
        // Most lines are a tool's, and long. One that has none of the words is not read.
        let small = line.to_ascii_lowercase();
        if !words.iter().any(|word| memmem::find(small.as_bytes(), word.as_bytes()).is_some()) {
            continue;
        }
        let Some((weight, text)) = said(line) else { continue };
        let small = text.to_ascii_lowercase();
        for (count, word) in counts.iter_mut().zip(words) {
            *count += weight * times(&small, word) as f64;
        }
        let found = words.iter().find_map(|word| memmem::find(small.as_bytes(), word.as_bytes()).map(|at| (at, word.len())));
        if let Some((at, length)) = found {
            if weight < IN_TITLE && best.as_ref().is_none_or(|(held, _)| weight > *held) {
                best = Some((weight, piece(&text, at, length)));
            }
        }
    }
    if counts.iter().any(|count| *count == 0.0) {
        return None;
    }
    let rank = counts.iter().map(|count| (1.0 + count).ln()).sum();
    Some((rank, best.map(|(_, piece)| piece).unwrap_or_default()))
}

/// Every conversation under `sessions` in which all of `words` were said, the
/// best first, and no more than the best few. `wanted` is asked between files:
/// a search that is no longer wanted stops and gives nothing.
pub fn search(sessions: &Path, words: &[String], wanted: &(dyn Fn() -> bool + Sync)) -> Vec<Found> {
    if words.is_empty() {
        return Vec::new();
    }
    let files = conversations(sessions);
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map(|cores| cores.get()).unwrap_or(4).min(files.len().max(1));
    let mut found: Vec<Found> = std::thread::scope(|scope| {
        let working: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut found = Vec::new();
                    while wanted() {
                        let Some((id, file)) = files.get(next.fetch_add(1, Ordering::Relaxed)) else { break };
                        let Ok(mut bytes) = std::fs::read(file) else { continue };
                        // The whole file at once, in small letters: has it every word at all?
                        bytes.make_ascii_lowercase();
                        if !words.iter().all(|word| memmem::find(&bytes, word.as_bytes()).is_some()) {
                            continue;
                        }
                        drop(bytes);
                        // One of the few that has: read again as it was written, for who said what.
                        let Ok(whole) = std::fs::read(file) else { continue };
                        if let Some((rank, piece)) = look(&String::from_utf8_lossy(&whole), words) {
                            found.push(Found { id: id.clone(), piece, rank });
                        }
                    }
                    found
                })
            })
            .collect();
        working.into_iter().flat_map(|worker| worker.join().unwrap_or_default()).collect()
    });
    if !wanted() {
        return Vec::new();
    }
    found.sort_by(|a, b| b.rank.total_cmp(&a.rank).then_with(|| b.id.cmp(&a.id)));
    found.truncate(MOST);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of the test's own, with conversations as Oh-my-pi 18.8.7 writes them.
    fn kept(name: &str, files: &[(&str, &[Value])]) -> PathBuf {
        let sessions = std::env::temp_dir().join(format!("null-search-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&sessions);
        for (file, lines) in files {
            let file = sessions.join(file);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, lines.iter().map(|line| format!("{line}\n")).collect::<String>()).unwrap();
        }
        sessions
    }

    fn owner(text: &str) -> Value {
        serde_json::json!({ "type": "message", "id": "m", "message": { "role": "user", "content": [{ "type": "text", "text": text }] } })
    }

    fn agent(text: &str) -> Value {
        serde_json::json!({ "type": "message", "id": "m", "message": { "role": "assistant", "content": [{ "type": "thinking", "thinking": "a linker thought" }, { "type": "text", "text": text }] } })
    }

    fn tool(text: &str) -> Value {
        serde_json::json!({ "type": "message", "id": "m", "message": { "role": "toolResult", "content": [{ "type": "text", "text": text }] } })
    }

    fn title(text: &str) -> Value {
        serde_json::json!({ "type": "title", "v": 1, "title": text })
    }

    fn found(sessions: &Path, typed: &str) -> Vec<String> {
        search(sessions, &words_of(typed), &|| true).into_iter().map(|found| found.id).collect()
    }

    #[test]
    fn what_was_typed_is_the_words_to_look_for() {
        assert_eq!(words_of("  Linker   ERROR "), ["linker", "error"]);
        // One letter is in every conversation: it is nothing to look for.
        assert_eq!(words_of("a"), Vec::<String>::new());
        assert_eq!(words_of("  "), Vec::<String>::new());
        assert_eq!(words_of("a b"), ["a", "b"]);
    }

    #[test]
    fn a_conversation_is_found_by_what_was_said_in_it_and_not_by_what_a_tool_read() {
        let sessions = kept(
            "said",
            &[
                ("-w-/2026-10-10T09-00-00-000Z_c1.jsonl", &[title(""), owner("Why does the build fail with a LINKER error?"), agent("The linker cannot find libfoo.")]),
                ("-w-/2026-10-09T09-00-00-000Z_c2.jsonl", &[owner("what is the capital of France?"), agent("Paris."), tool("linker linker linker linker")]),
                ("-site-/2026-10-08T09-00-00-000Z_c3.jsonl", &[owner("list the files"), agent("Two files: linker.ld and main.c")]),
                ("-site-/2026-10-08T09-00-00-000Z_c3/2026-10-08T09-01-00-000Z_helper.jsonl", &[owner("a helper was told about the linker")]),
                ("-site-/.2026-10-08T09-00-00-000Z_c3.jsonl.lock.os", &[]),
            ],
        );
        // Said by the owner and by the agent: found, the one where it was said more first. A tool's words do not count.
        assert_eq!(found(&sessions, "linker"), ["c1", "c3"]);
        assert_eq!(found(&sessions, "LINKER"), ["c1", "c3"]);
        // Every word, anywhere and in any order.
        assert_eq!(found(&sessions, "error linker"), ["c1"]);
        assert_eq!(found(&sessions, "capital france"), ["c2"]);
        assert_eq!(found(&sessions, "linker france"), Vec::<String>::new());
        assert_eq!(found(&sessions, "nowhere"), Vec::<String>::new());
        // The agent's thinking is not something it said.
        assert_eq!(found(&sessions, "thought"), Vec::<String>::new());
        // Nothing to look for finds nothing, and so does a folder that is not there.
        assert_eq!(found(&sessions, "a"), Vec::<String>::new());
        assert_eq!(found(&sessions.join("nowhere"), "linker"), Vec::<String>::new());
        let _ = std::fs::remove_dir_all(sessions);
    }

    #[test]
    fn what_stands_first_is_where_the_words_count_most() {
        let sessions = kept(
            "rank",
            &[
                ("-w-/2026-10-10T09-00-00-000Z_agent.jsonl", &[owner("hello"), agent("marmalade")]),
                ("-w-/2026-10-10T09-00-00-000Z_owner.jsonl", &[owner("marmalade"), agent("yes")]),
                ("-w-/2026-10-10T09-00-00-000Z_titled.jsonl", &[title("Marmalade recipes"), owner("hello"), agent("yes")]),
                ("-w-/2026-10-10T09-00-00-000Z_often.jsonl", &[owner("marmalade marmalade marmalade marmalade"), agent("marmalade, yes")]),
            ],
        );
        // Said often by the owner, then in a title, then once by the owner, then once by the agent.
        assert_eq!(found(&sessions, "marmalade"), ["often", "titled", "owner", "agent"]);
        let _ = std::fs::remove_dir_all(sessions);
    }

    #[test]
    fn a_piece_of_what_was_said_is_shown_around_the_word() {
        let long = format!("{} the word is marmalade, {}", "before ".repeat(20), "after ".repeat(30));
        let sessions = kept(
            "piece",
            &[
                ("-w-/2026-10-10T09-00-00-000Z_short.jsonl", &[owner("the word is\n  marmalade")]),
                ("-w-/2026-10-10T09-00-00-000Z_long.jsonl", &[agent("marmalade was said here first"), owner(&long)]),
                ("-w-/2026-10-10T09-00-00-000Z_title.jsonl", &[title("marmalade"), owner("hello"), agent("yes")]),
                ("-w-/2026-10-10T09-00-00-000Z_wide.jsonl", &[owner(&format!("{}marmalade{}", "é".repeat(40), "ü".repeat(80)))]),
            ],
        );
        let pieces: std::collections::HashMap<String, String> = search(&sessions, &words_of("marmalade"), &|| true).into_iter().map(|found| (found.id, found.piece)).collect();
        // On one line, whole when it is short.
        assert_eq!(pieces["short"], "the word is marmalade");
        // The owner's words before the agent's, cut at both ends, with a mark where it was cut.
        assert!(pieces["long"].starts_with('…') && pieces["long"].ends_with('…') && pieces["long"].contains("the word is marmalade, after"), "{}", pieces["long"]);
        assert!(pieces["long"].chars().count() <= BEFORE + "marmalade".len() + AFTER + 2, "{}", pieces["long"]);
        // Found in its title alone, it has no piece: the title is on the row already.
        assert_eq!(pieces["title"], "");
        // Cut between letters, never through one.
        assert!(pieces["wide"].contains("marmalade"), "{}", pieces["wide"]);
        let _ = std::fs::remove_dir_all(sessions);
    }

    #[test]
    fn a_search_that_is_no_longer_wanted_gives_nothing() {
        let sessions = kept("unwanted", &[("-w-/2026-10-10T09-00-00-000Z_c1.jsonl", &[owner("marmalade")])]);
        assert_eq!(search(&sessions, &words_of("marmalade"), &|| false), Vec::new());
        assert_eq!(search(&sessions, &words_of("marmalade"), &|| true).len(), 1);
        let _ = std::fs::remove_dir_all(sessions);
    }
}
