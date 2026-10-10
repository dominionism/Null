//! Attached files: what the person hands the agent with their next message.
//!
//! Null uploads nothing and reads nothing. A file stays where it is on the
//! Mac. What goes to the harness with the message is a link to it, in the
//! protocol's own form, and the agent reads the file with its own tools, as it
//! would in a terminal. So a file's content goes where the conversation goes,
//! to the provider the model runs on, once the agent reads it, and never
//! through Null: nothing here opens a file for what is in it, and no name or
//! path is written to the log or the settings.
//!
//! Why a link and not the content, found out on Oh-my-pi 18.8.7: of a link it
//! gives the model the link's title and nothing else, so the title says where
//! the file is; its own tool then reads text by line, the words of a PDF, a
//! picture as a picture, a folder, an archive. An embedded text arrives
//! without its name, and for it and for a picture Null would have to read the
//! content itself. The protocol requires every harness to take a link, so no
//! ability decides this, and a harness that refuses one costs the message that
//! carried it (`refused`), not the box.
//!
//! This module holds what is attached until it is sent, lists a folder for the
//! box's list, and writes the link.

use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::log::log;
use crate::panel;

/// How a link's title begins. The harness hands the title to the model, so it
/// says in plain words what the path after it is. A conversation loaded back
/// carries it in the message's text, where `split` finds it again.
pub const LEAD: &str = "attached file: ";

/// The most entries of one folder handed to the page. The list is narrowed by
/// typing; a folder larger than this says that it was cut.
const MOST: usize = 20_000;

struct Held {
    /// What goes with the next message, in the order it was attached.
    files: Vec<PathBuf>,
    /// The folder the list last showed. It opens there the next time.
    folder: Option<PathBuf>,
}

static HELD: Mutex<Held> = Mutex::new(Held { files: Vec::new(), folder: None });

fn held() -> MutexGuard<'static, Held> {
    HELD.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

// ── Paths ─────────────────────────────────────────────────────────────────

/// A path as the box shows it: the home folder is `~`.
fn shown(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

fn name(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

/// A path without `.` and `..` in it, worked out from its words alone.
fn tidy(path: &Path) -> PathBuf {
    let mut tidy = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                tidy.pop();
            }
            Component::CurDir => {}
            other => tidy.push(other),
        }
    }
    tidy
}

/// The path a person typed, as one that holds from anywhere: `~` is the home
/// folder, and a path that does not start at the root starts at `base`. Quotes
/// around it are taken off. Everything else is the path, spaces included.
pub fn resolve(typed: &str, base: &Path, home: &Path) -> PathBuf {
    let typed = typed.trim();
    let typed = ['"', '\''].iter().find_map(|quote| typed.strip_prefix(*quote)?.strip_suffix(*quote)).unwrap_or(typed);
    let path = match typed.strip_prefix('~') {
        Some("") => home.to_path_buf(),
        Some(rest) if rest.starts_with('/') => home.join(rest.trim_start_matches('/')),
        _ => base.join(typed),
    };
    tidy(&path)
}

/// A path copied out of a terminal, with its backslashes taken off.
fn unescaped(typed: &str) -> String {
    let mut plain = String::new();
    let mut letters = typed.chars();
    while let Some(letter) = letters.next() {
        plain.push(if letter == '\\' { letters.next().unwrap_or(letter) } else { letter });
    }
    plain
}

#[derive(Debug, PartialEq)]
enum Kind {
    File,
    Folder,
}

/// What is at a path, or in one plain line why it cannot be attached. Nothing
/// of the file is read: it is opened, to learn whether it can be, and closed.
fn kind(path: &Path, home: &Path) -> Result<Kind, String> {
    let said = shown(path, home);
    // A line break in a name would let the name pass for words of the message.
    if said.chars().any(char::is_control) {
        return Err("cannot attach a file whose name has a line break in it".into());
    }
    let about = match std::fs::metadata(path) {
        Ok(about) => about,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(format!("no such file: {said}")),
        Err(_) => return Err(format!("cannot read {said}")),
    };
    if about.is_dir() {
        return Ok(Kind::Folder);
    }
    if !about.is_file() {
        return Err(format!("not a file: {said}"));
    }
    match std::fs::File::open(path) {
        Ok(_) => Ok(Kind::File),
        Err(_) => Err(format!("cannot read {said}")),
    }
}

// ── What the harness is handed ────────────────────────────────────────────

/// A `file://` address for a path.
fn address(file: &Path) -> String {
    let mut address = String::from("file://");
    for byte in file.as_os_str().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => address.push(*byte as char),
            _ => address.push_str(&format!("%{byte:02X}")),
        }
    }
    address
}

/// A message as the protocol's `session/prompt` takes it: the words, then a
/// link for each attached file.
pub fn blocks(text: &str, files: &[PathBuf]) -> Value {
    let mut blocks = vec![json!({ "type": "text", "text": text })];
    for file in files {
        blocks.push(json!({ "type": "resource_link", "uri": address(file), "name": name(file), "title": format!("{LEAD}{}", file.display()) }));
    }
    Value::Array(blocks)
}

/// The owner's message as the box shows it, with what went with it.
pub fn said(text: &str, files: &[PathBuf]) -> Value {
    let mut event = json!({ "type": "user_message", "text": text });
    if !files.is_empty() {
        event["files"] = files.iter().map(|file| json!({ "name": name(file), "path": file.display().to_string() })).collect();
    }
    event
}

/// A message's words and the files that went with it, out of the text a
/// harness gives back when it loads a conversation again: the words, then each
/// link's title in a paragraph of its own.
pub fn split(text: &str) -> (&str, Vec<PathBuf>) {
    let mut words = text;
    let mut files = Vec::new();
    while let Some((before, last)) = words.rsplit_once("\n\n") {
        match last.strip_prefix(LEAD) {
            Some(path) if path.starts_with('/') && !path.contains('\n') => files.insert(0, PathBuf::from(path)),
            _ => break,
        }
        words = before;
    }
    (words, files)
}

/// A message the harness replays, shown as it was when it was sent.
pub fn said_again(text: &str) -> Value {
    let (words, files) = split(text);
    said(words, &files)
}

/// The protocol's code for a request the other side could not make sense of.
const NOT_UNDERSTOOD: i64 = -32602;

/// The error to show when a harness answered a message with one. A harness
/// that could not make sense of a message that carried files does not take
/// files: that costs the message, and is said in one line. The harness's own
/// words stay out of the log, since they may quote what it was sent.
pub fn refused(event: Value, with_files: bool) -> Value {
    if !with_files || event["code"] != json!(NOT_UNDERSTOOD) {
        return event;
    }
    log!("the harness could not make sense of a message that carried files");
    json!({ "type": "error", "message": "this harness does not take attached files. send the message again without them", "code": event["code"], "details": null })
}

// ── What is attached ──────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Attached {
    pub name: String,
    pub path: String,
    /// The folder it is in, as the box shows it.
    pub within: String,
}

fn listed(files: &[PathBuf], home: &Path) -> Vec<Attached> {
    files
        .iter()
        .map(|file| Attached { name: name(file), path: file.display().to_string(), within: shown(file.parent().unwrap_or(Path::new("/")), home) })
        .collect()
}

/// Tell the page what is attached now.
fn tell(app: &AppHandle) {
    let now = listed(&held().files, &home());
    let _ = app.emit_to(panel::LABEL, "mini:attached", now);
}

/// What goes with the message being sent. A file that has gone since it was
/// attached stops the message, so that nothing is sent half: the person takes
/// it off or puts it back.
pub fn take(app: &AppHandle) -> Result<Vec<PathBuf>, String> {
    let files = {
        let mut held = held();
        if let Some(gone) = held.files.iter().find(|file| !file.is_file()) {
            return Err(format!("{} is no longer there. /attach takes it off", name(gone)));
        }
        std::mem::take(&mut held.files)
    };
    if !files.is_empty() {
        log!("{} attached to the message", if files.len() == 1 { "one file".to_string() } else { format!("{} files", files.len()) });
        tell(app);
    }
    Ok(files)
}

// ── What the page may ask ─────────────────────────────────────────────────

#[derive(Debug, PartialEq, Serialize)]
pub struct Entry {
    name: String,
    path: String,
    folder: bool,
}

#[derive(Serialize)]
pub struct Listing {
    path: String,
    /// The folder as the box shows it.
    shown: String,
    /// The folder above it. None at the top.
    parent: Option<String>,
    entries: Vec<Entry>,
    /// How many entries were left out of a folder too large to hand over whole.
    cut: usize,
}

/// What a folder holds, by name, with capitals and small letters together.
fn entries(folder: &Path, home: &Path) -> Result<Vec<Entry>, String> {
    let read = std::fs::read_dir(folder).map_err(|_| format!("cannot open {}", shown(folder, home)))?;
    let mut entries: Vec<Entry> = read
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            // A link is whatever it points to. One that points nowhere is left out.
            let folder = match entry.file_type().ok()? {
                kind if kind.is_symlink() => std::fs::metadata(&path).ok()?.is_dir(),
                kind => kind.is_dir(),
            };
            Some(Entry { name: entry.file_name().to_string_lossy().into_owned(), path: path.display().to_string(), folder })
        })
        .collect();
    entries.sort_by_cached_key(|entry| (entry.name.to_lowercase(), entry.name.clone()));
    Ok(entries)
}

/// A folder for the box's list: the one asked for, or the one the list last
/// showed, or the home folder. A folder that has gone since is not an error.
#[tauri::command(async)]
pub fn attach_list(folder: Option<String>) -> Result<Listing, String> {
    let home = home();
    let folder = match folder {
        Some(folder) => PathBuf::from(folder),
        None => held().folder.clone().filter(|folder| folder.is_dir()).unwrap_or_else(|| home.clone()),
    };
    let mut entries = entries(&folder, &home)?;
    let cut = entries.len().saturating_sub(MOST);
    entries.truncate(MOST);
    held().folder = Some(folder.clone());
    Ok(Listing {
        path: folder.display().to_string(),
        shown: shown(&folder, &home),
        parent: folder.parent().map(|parent| parent.display().to_string()),
        entries,
        cut,
    })
}

/// What became of a path handed to `/attach`.
#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Added {
    /// It was a file, and goes with the next message.
    File,
    /// It was a folder: the list opens there.
    Folder { path: String },
}

/// Attach the file at a path, typed after `/attach` or chosen from the list.
/// The error is the one line the box says.
#[tauri::command(async)]
pub fn attach_file(app: AppHandle, path: String) -> Result<Added, String> {
    let home = home();
    let base = held().folder.clone().unwrap_or_else(|| home.clone());
    let mut file = resolve(&path, &base, &home);
    if !file.exists() && path.contains('\\') {
        let plain = resolve(&unescaped(&path), &base, &home);
        if plain.exists() {
            file = plain;
        }
    }
    if kind(&file, &home)? == Kind::Folder {
        return Ok(Added::Folder { path: file.display().to_string() });
    }
    {
        let mut held = held();
        if !held.files.contains(&file) {
            held.files.push(file.clone());
        }
        held.folder = file.parent().map(Path::to_path_buf);
    }
    tell(&app);
    Ok(Added::File)
}

/// Take an attached file off again.
#[tauri::command]
pub fn detach_file(app: AppHandle, path: String) {
    held().files.retain(|file| file != Path::new(&path));
    tell(&app);
}

/// What is attached, for a page that has just loaded.
#[tauri::command]
pub fn attached() -> Vec<Attached> {
    listed(&held().files, &home())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("null-mini-test-attach-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A name with a space, both kinds of quote and another alphabet in it.
    const AWKWARD: &str = "notes \"one\" it's ノート.txt";

    #[test]
    fn a_typed_path_holds_from_anywhere() {
        let (base, home) = (Path::new("/Users/someone/Documents"), Path::new("/Users/someone"));
        assert_eq!(resolve("~", base, home), home);
        assert_eq!(resolve("~/Desktop/a b.png", base, home), Path::new("/Users/someone/Desktop/a b.png"));
        assert_eq!(resolve("/tmp/x.txt", base, home), Path::new("/tmp/x.txt"));
        // A path that does not start at the root starts where the list last was.
        assert_eq!(resolve("plan.md", base, home), Path::new("/Users/someone/Documents/plan.md"));
        assert_eq!(resolve("../Desktop/./shot.png", base, home), Path::new("/Users/someone/Desktop/shot.png"));
        // Spaces belong to the path, and quotes around it do not.
        assert_eq!(resolve("  \"~/My Files/a.txt\" ", base, home), Path::new("/Users/someone/My Files/a.txt"));
        assert_eq!(resolve("'/tmp/it is.txt'", base, home), Path::new("/tmp/it is.txt"));
        // A name that only starts with the home sign is a name.
        assert_eq!(resolve("~draft.txt", base, home), Path::new("/Users/someone/Documents/~draft.txt"));
        assert_eq!(unescaped("/tmp/My\\ Files/a\\(1\\).txt"), "/tmp/My Files/a(1).txt");
    }

    #[test]
    fn a_path_is_shown_with_the_home_folder_short() {
        let home = Path::new("/Users/someone");
        assert_eq!(shown(Path::new("/Users/someone"), home), "~");
        assert_eq!(shown(Path::new("/Users/someone/Desktop/a.png"), home), "~/Desktop/a.png");
        assert_eq!(shown(Path::new("/Users/someone-else/a.png"), home), "/Users/someone-else/a.png");
    }

    #[test]
    fn a_file_goes_to_the_harness_as_a_link_that_says_where_it_is() {
        let file = PathBuf::from("/Users/someone/My Files").join(AWKWARD);
        let prompt = blocks("what is in this?", &[file.clone()]);
        assert_eq!(prompt[0], json!({ "type": "text", "text": "what is in this?" }));
        assert_eq!(prompt[1]["type"], "resource_link");
        assert_eq!(prompt[1]["name"], AWKWARD);
        assert_eq!(prompt[1]["title"], format!("attached file: /Users/someone/My Files/{AWKWARD}"));
        assert_eq!(prompt[1]["uri"], "file:///Users/someone/My%20Files/notes%20%22one%22%20it%27s%20%E3%83%8E%E3%83%BC%E3%83%88.txt");
        // Nothing of the file itself is in what is handed over: this one does not exist.
        assert_eq!(prompt.as_array().unwrap().len(), 2);
        // A message with nothing attached is the words alone, as before.
        assert_eq!(blocks("hello", &[]), json!([{ "type": "text", "text": "hello" }]));
    }

    #[test]
    fn the_sent_message_shows_what_went_with_it() {
        let file = PathBuf::from("/tmp").join(AWKWARD);
        assert_eq!(said("look", &[file]), json!({ "type": "user_message", "text": "look", "files": [{ "name": AWKWARD, "path": format!("/tmp/{AWKWARD}") }] }));
        assert_eq!(said("hello", &[]), json!({ "type": "user_message", "text": "hello" }));
    }

    #[test]
    fn a_message_loaded_back_shows_its_files_again() {
        // As Oh-my-pi 18.8.7 replays a message that carried two links.
        let files = [PathBuf::from("/tmp").join(AWKWARD), PathBuf::from("/Users/someone/shot.png")];
        let replayed = format!("look at these\n\nand say what you see\n\nattached file: /tmp/{AWKWARD}\n\nattached file: /Users/someone/shot.png");
        assert_eq!(said_again(&replayed), said("look at these\n\nand say what you see", &files));
        // Words that only look like it are words.
        assert_eq!(said_again("hello"), json!({ "type": "user_message", "text": "hello" }));
        assert_eq!(split("attached file: /tmp/a.txt"), ("attached file: /tmp/a.txt", Vec::new()));
        assert_eq!(split("see\n\nattached file: somewhere"), ("see\n\nattached file: somewhere", Vec::new()));
    }

    #[test]
    fn what_cannot_be_attached_is_said_in_one_line() {
        let root = scratch("kind");
        let home = root.join("home");
        std::fs::create_dir_all(home.join("Folder")).unwrap();
        std::fs::write(home.join(AWKWARD), "words").unwrap();

        assert_eq!(kind(&home.join(AWKWARD), &home), Ok(Kind::File));
        assert_eq!(kind(&home.join("Folder"), &home), Ok(Kind::Folder));
        assert_eq!(kind(&home.join("nothing.txt"), &home), Err("no such file: ~/nothing.txt".into()));
        assert_eq!(kind(&home.join("a\nb.txt"), &home), Err("cannot attach a file whose name has a line break in it".into()));

        use std::os::unix::fs::PermissionsExt;
        let locked = home.join("locked.txt");
        std::fs::write(&locked, "words").unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        assert_eq!(kind(&locked, &home), Err("cannot read ~/locked.txt".into()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_is_listed_by_name_with_its_folders_marked() {
        let root = scratch("list");
        std::fs::create_dir_all(root.join("beta")).unwrap();
        std::fs::write(root.join("Alpha.txt"), "").unwrap();
        std::fs::write(root.join(AWKWARD), "").unwrap();
        std::fs::write(root.join(".hidden"), "").unwrap();
        std::os::unix::fs::symlink(root.join("beta"), root.join("gamma")).unwrap();
        std::os::unix::fs::symlink(root.join("nowhere"), root.join("delta")).unwrap();

        let found = entries(&root, &root).unwrap();
        let names: Vec<(&str, bool)> = found.iter().map(|entry| (entry.name.as_str(), entry.folder)).collect();
        assert_eq!(names, [(".hidden", false), ("Alpha.txt", false), ("beta", true), ("gamma", true), (AWKWARD, false)]);
        assert_eq!(found[1].path, root.join("Alpha.txt").display().to_string());
        assert_eq!(entries(&root.join("nowhere"), &root), Err("cannot open ~/nowhere".into()));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_harness_that_cannot_make_sense_of_files_costs_only_that_message() {
        let not_understood = json!({ "type": "error", "message": "Invalid params", "code": -32602, "details": null });
        let said = refused(not_understood.clone(), true);
        assert_eq!(said["message"], "this harness does not take attached files. send the message again without them");
        // The same answer to a message without files is the harness's own error, and so is any other error.
        assert_eq!(refused(not_understood.clone(), false), not_understood);
        let other = json!({ "type": "error", "message": "Internal error", "code": -32603, "details": "boom" });
        assert_eq!(refused(other.clone(), true), other);
    }
}
