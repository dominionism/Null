//! The browser link: the harness's way into the person's own Chrome, offered
//! by Null under its own name.
//!
//! The link is the harness's own: an extension the person loads in Chrome,
//! and a relay on this Mac that the harness starts when its browser needs it.
//! Null has the engine in use write that extension into a folder of Null's,
//! `BrowserLink`, and changes what Chrome shows of it: the name, the toolbar's
//! words and icon, the heading of its settings page, and the title of the tab
//! group the agent's tabs are gathered in. Nothing else of it is changed, and
//! the harness's licence and notices stay beside it. Chrome loads it from that
//! folder and goes on loading it from there, so the folder never moves: the
//! extension of a newer engine is written into the same place.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::log::log;
use crate::{engine, harness, panel, settings};

/// The port the harness's relay listens on, and where the extension looks for it.
const PORT: u16 = 9224;

/// How long set-up waits for Chrome to load the extension.
const WAIT: Duration = Duration::from_secs(120);

/// The places Google Chrome is installed in.
const CHROME: [&str; 2] = ["/Applications/Google Chrome.app", "Applications/Google Chrome.app"];

/// The name Chrome shows for the extension, wherever it shows one.
pub const NAME: &str = "Null";

/// What Chrome says the extension is for.
const DESCRIPTION: &str = "Lets the agent in Null work your own Chrome tabs.";

/// Null's own icon, at the two sizes Chrome asks for.
const ICONS: [(&str, &str, &[u8]); 2] = [("32", "null-32.png", include_bytes!("../Icons/32x32.png")), ("128", "null-128.png", include_bytes!("../Icons/128x128.png"))];

/// Each change to the extension's own files: the file, what stands there, and
/// what goes in its place. The tab group's title is the relay's to give, and
/// the extension is made to keep its own instead. Its toolbar badge says "on"
/// while the agent is linked and nothing otherwise: "off" read as broken,
/// though it only means the agent is not working in Chrome just now.
const CHANGES: [(&str, &str, &str); 4] = [
    ("background.js", "async function groupTabs(tabIds, title, color) {", "async function groupTabs(tabIds, _title, color) {\n  const title = \"Null\";"),
    ("background.js", "chrome.action.setBadgeText({ text: connected ? \"on\" : \"off\" });", "chrome.action.setBadgeText({ text: connected ? \"on\" : \"\" });"),
    ("options.html", "<title>OMP Browser Relay settings</title>", "<title>Null settings</title>"),
    ("options.html", "<h1>OMP Browser Relay</h1>", "<h1>Null</h1>"),
];

/// The folder Null keeps its copy of the extension in, which Chrome loads.
pub fn folder(dir: &Path) -> PathBuf {
    dir.join(settings::folder_name("BrowserLink"))
}

/// Make Null's copy of the extension in `folder`, from the one `binary` writes.
/// It is written beside the folder and put in its place only once it is whole
/// and renamed, so that Chrome never finds half an extension there. An
/// extension that is not the one Null knows how to rename is not put there
/// at all: the link is never offered half under the harness's name.
pub fn make(binary: &Path, folder: &Path) -> Result<(), String> {
    let fresh = folder.with_extension("new");
    let _ = std::fs::remove_dir_all(&fresh);
    let made = write(binary, &fresh).and_then(|()| rename_in(&fresh));
    if let Err(why) = made {
        let _ = std::fs::remove_dir_all(&fresh);
        return Err(why);
    }
    let _ = std::fs::remove_dir_all(folder);
    std::fs::rename(&fresh, folder).map_err(|e| format!("could not put Null's copy of the extension in place: {e}"))
}

/// Null's copy is made one at a time: the harness's start, `/browser` and setting
/// Chrome up can each ask at the same moment, and two makings at once would spoil
/// each other's. The second finds the copy done.
static MAKING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// What the box says, once, when Null's copy was made again for another engine.
/// Chrome goes on running the copy it loaded until it is told to read it again.
pub const MADE_AGAIN: &str = "Chrome's copy of Null was updated: press reload on Null in Chrome's extensions page";

/// Make Null's copy in `folder` again, in place, from the one `binary` writes.
/// True when it came out different from the copy that was there, which is
/// when Chrome has to reload it. One that cannot be made leaves that copy as
/// it was.
pub fn make_again(binary: &Path, folder: &Path) -> Result<bool, String> {
    let before = held(folder);
    make(binary, folder)?;
    Ok(held(folder) != before)
}

/// Every file a copy holds, by name, with what it holds.
fn held(folder: &Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    let mut files: Vec<_> = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|file| (file.file_name(), std::fs::read(file.path()).unwrap_or_default())).collect();
    files.sort();
    files
}

/// Whether Null's copy has to be made again: it is there, and the engine in
/// use is not the version it was made from. A copy made before Null noted the
/// version is made again once.
fn behind(made: bool, made_from: Option<&str>, in_use: Option<&str>) -> bool {
    made && in_use.is_some() && made_from != in_use
}

/// The engine in use may not be the one Null's copy was made from: `/update`
/// or `/harness` changed it, a newer Null came with a newer one, or the
/// person updated their own Oh-my-pi. So whenever the harness starts or
/// `/browser` is asked, the versions are compared, and when the link is set up
/// and they differ, the copy is made again in the folder Chrome loads, to
/// speak the protocol of the relay this engine starts: the relay refuses an
/// extension of another ("out of date"). What the box is to say of it, when
/// there is anything to say: that Chrome has to reload the copy, or that this
/// engine's extension is not one Null can name, and then the agent is no
/// longer told to use the link until Chrome is set up again.
pub fn follow_engine(app: &AppHandle) -> Option<String> {
    let _turn = MAKING.lock().unwrap_or_else(|held| held.into_inner());
    let folder = folder_of(app).ok()?;
    let made = folder.join("manifest.json").is_file();
    let (binary, _) = engine::in_use(app)?;
    let version = made.then(|| engine::version_of(&binary)).flatten();
    if !behind(made, settings::get(app).browser_engine.as_deref(), version.as_deref()) {
        return None;
    }
    let said = match make_again(&binary, &folder) {
        Ok(false) => None,
        Ok(true) => {
            log!("Null's copy of the browser link's extension is made again for the harness now in use");
            Some(MADE_AGAIN.to_string())
        }
        Err(why) => {
            log!("Null's copy of the browser link's extension could not be made again: {why}");
            settings::update(app, |settings| settings.browser_linked = false);
            return Some(format!("Chrome's copy of Null could not be made from this Oh-my-pi, so the agent no longer uses your Chrome: {why}"));
        }
    };
    settings::update(app, |settings| settings.browser_engine = version.clone());
    said
}

/// The same when the harness starts, out of its way: the box says it on its notice line.
pub fn follow_engine_aside(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Some(said) = follow_engine(&app) {
            let _ = app.emit_to(panel::LABEL, "mini:notice", json!({ "text": said, "stay": true }));
        }
    });
}

/// Whether the agent is told the link's rules when the harness starts: while
/// the person has it on and Chrome was seen to load the copy.
pub fn told(kept: &settings::Settings) -> bool {
    kept.browser == Some(true) && kept.browser_linked
}

/// Make a copy for a Chrome of its own, such as the bench's, from `engine`, and
/// say how it went (`NULL_MINI_LINK_COPY`). The exit status for the process: 0
/// when the copy is there, 2 when it is not.
pub fn copy(engine: Option<&Path>, folder: &Path) -> i32 {
    let made = engine.ok_or_else(|| format!("no {} to write the extension with", harness::HARNESS_NAME)).and_then(|binary| make(binary, folder));
    match made {
        Ok(()) => {
            log!("Null's copy of the browser link's extension is in {}", folder.display());
            0
        }
        Err(why) => {
            log!("no copy of the browser link's extension was made: {why}");
            2
        }
    }
}

/// Have the harness write its extension into `folder`.
fn write(binary: &Path, folder: &Path) -> Result<(), String> {
    let written = Command::new(binary)
        .args(harness::extra_args())
        .args(["browser-relay", "install", "--dir"])
        .arg(folder)
        .output()
        .map_err(|e| format!("{} could not be started: {e}", harness::HARNESS_NAME))?;
    if !written.status.success() || !folder.join("manifest.json").is_file() {
        return Err(format!("{} would not write its browser extension", harness::HARNESS_NAME));
    }
    Ok(())
}

/// Give the extension in `folder` Null's name, words and icon.
fn rename_in(folder: &Path) -> Result<(), String> {
    let read = |file: &str| std::fs::read_to_string(folder.join(file)).map_err(|_| unknown(file));
    let put = |file: &str, text: &[u8]| std::fs::write(folder.join(file), text).map_err(|e| format!("could not write {file}: {e}"));
    put("manifest.json", renamed_manifest(&read("manifest.json")?)?.as_bytes())?;
    for (file, from, to) in CHANGES {
        put(file, changed(&read(file)?, from, to, file)?.as_bytes())?;
    }
    for (_, file, bytes) in ICONS {
        put(file, bytes)?;
    }
    Ok(())
}

/// The extension's manifest with Null's name, words and icon in place of the
/// harness's. Everything else in it, what it asks Chrome for above all, is as
/// the harness wrote it.
fn renamed_manifest(text: &str) -> Result<String, String> {
    let mut manifest: Value = serde_json::from_str(text).map_err(|_| unknown("manifest.json"))?;
    let icons: Value = ICONS.iter().map(|(size, file, _)| (size.to_string(), json!(file))).collect::<serde_json::Map<_, _>>().into();
    let fields = manifest.as_object_mut().ok_or_else(|| unknown("manifest.json"))?;
    if !fields.contains_key("name") {
        return Err(unknown("manifest.json"));
    }
    fields.insert("name".into(), json!(NAME));
    fields.insert("description".into(), json!(DESCRIPTION));
    fields.insert("icons".into(), icons.clone());
    let action = fields.get_mut("action").and_then(Value::as_object_mut).ok_or_else(|| unknown("manifest.json"))?;
    action.insert("default_title".into(), json!(NAME));
    action.insert("default_icon".into(), icons);
    serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())
}

/// `text` with `from` replaced by `to`, where `from` stands exactly once.
fn changed(text: &str, from: &str, to: &str, file: &str) -> Result<String, String> {
    match text.matches(from).count() {
        1 => Ok(text.replacen(from, to, 1)),
        _ => Err(unknown(file)),
    }
}

/// What is said when the harness's extension is not the one Null knows.
fn unknown(file: &str) -> String {
    format!("this {}'s browser extension is not the one Null knows how to name ({file})", harness::HARNESS_NAME)
}

/// How the browser link stands, for the box to say.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Standing {
    /// Whether Google Chrome is on this Mac. The link is Chrome's only.
    pub chrome: bool,
    /// Whether Null's copy of the extension is in its folder.
    pub made: bool,
    /// Whether Chrome was seen to load it, the last time it was set up.
    pub linked: bool,
    /// Whether the agent may use it (`/browser`).
    pub on: bool,
    /// Whether the harness acts without asking. Set to ask, it asks before each step, link or not.
    pub freely: bool,
    /// The folder Chrome is to load, for the steps the box shows.
    pub folder: String,
    /// What the box is to say once of Null's copy, made again for another harness.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub said: Option<String>,
}

/// Whether Google Chrome is on this Mac, in the system's Applications or the person's own.
fn chrome() -> bool {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    CHROME.iter().any(|place| if place.starts_with('/') { Path::new(place).exists() } else { home.join(place).exists() })
}

/// The folder of Null's copy, under Null's own.
fn folder_of(app: &AppHandle) -> Result<PathBuf, String> {
    settings::dir(app).map(|dir| folder(&dir)).ok_or_else(|| "Null has no folder of its own to keep the extension in".to_string())
}

/// How the link stands with the harness in use, and, once, what came of
/// making Null's copy again when that harness is not the one it was made from.
#[tauri::command(async)]
pub fn browser(app: AppHandle) -> Result<Standing, String> {
    let said = follow_engine(&app);
    standing(app).map(|standing| Standing { said, ..standing })
}

/// How the link stands with the harness in use.
fn standing(app: AppHandle) -> Result<Standing, String> {
    let (binary, _) = engine::in_use(&app).ok_or(format!("{} is not installed", harness::HARNESS_NAME))?;
    let folder = folder_of(&app)?;
    let kept = settings::get(&app);
    let made = folder.join("manifest.json").is_file();
    Ok(Standing {
        chrome: chrome(),
        made,
        linked: made && kept.browser_linked,
        on: kept.browser == Some(true),
        freely: !matches!(harness::approval_mode(&binary).as_deref(), Some("always-ask" | "write")),
        folder: folder.display().to_string(),
        said: None,
    })
}

/// Let the agent use the link, or not. Its rules reach the agent when the
/// harness starts, so the one that is running is let go. On needs the link
/// set up first: the box sets it up when it is not.
#[tauri::command(async)]
pub fn set_browser(app: AppHandle, on: bool) -> Result<Standing, String> {
    let now = standing(app.clone())?;
    if on && !now.linked {
        return Err("Chrome is not linked yet: choose set up Chrome first".into());
    }
    settings::update(&app, |settings| settings.browser = Some(on));
    log!("browser link: {}", if on { "on" } else { "off" });
    harness::restart(&app);
    standing(app)
}

/// Set Chrome up: make Null's copy of the extension from the harness in use,
/// put the folder's path on the clipboard, show the folder in Finder and open
/// Chrome on its extensions page, where the person loads it. Whether Chrome
/// did is for `browser_linked` to see.
#[tauri::command(async)]
pub fn link_browser(app: AppHandle) -> Result<Standing, String> {
    if !chrome() {
        return Err("the browser link needs Google Chrome, and it is not on this Mac".into());
    }
    let (binary, _) = engine::in_use(&app).ok_or(format!("{} is not installed", harness::HARNESS_NAME))?;
    let folder = folder_of(&app)?;
    let turn = MAKING.lock().unwrap_or_else(|held| held.into_inner());
    make(&binary, &folder)?;
    let version = engine::version_of(&binary);
    settings::update(&app, |settings| {
        settings.browser_linked = false;
        settings.browser_engine = version;
    });
    drop(turn);
    log!("Null's copy of the browser link's extension is made");
    if settings::own_folder().is_some() {
        // A scripted run on a folder of its own sends nobody anywhere.
        log!("a run on a folder of its own opens neither Chrome nor Finder");
    } else {
        let path = folder.display().to_string();
        if let Ok(mut copy) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
            use std::io::Write;
            let _ = copy.stdin.take().map(|mut input| input.write_all(path.as_bytes()));
            let _ = copy.wait();
        }
        // Finder first, so that Chrome, opened last, is the window in front.
        let _ = Command::new("open").arg("-R").arg(&folder).status();
        let _ = Command::new("open").args(["-a", "Google Chrome", "chrome://extensions"]).status();
    }
    standing(app)
}

/// Wait for Chrome to load the extension: true once it reaches the link,
/// false after two minutes. Seen, the link is noted as set up.
#[tauri::command(async)]
pub fn browser_linked(app: AppHandle) -> Result<Standing, String> {
    let (binary, _) = engine::in_use(&app).ok_or(format!("{} is not installed", harness::HARNESS_NAME))?;
    let seen = if settings::own_folder().is_some() {
        // Another relay on this Mac would be the person's own Chrome reaching it.
        log!("a run on a folder of its own does not wait for Chrome");
        false
    } else {
        let mut relay = Command::new(&binary);
        relay.args(harness::extra_args()).args(["browser-relay", "--port", &PORT.to_string()]);
        wait_for_extension(relay, PORT, WAIT)
    };
    log!("{}", if seen { "Chrome loaded the browser link" } else { "Chrome was not seen to load the browser link" });
    settings::update(&app, |settings| settings.browser_linked = seen);
    standing(app)
}

/// A relay of Null's own, stopped however the wait ends.
struct Relay(Child);

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Wait up to `wait` for the extension to reach a relay on `port`. When the
/// harness's own relay is already there it is asked; when none is, Null
/// starts one with `relay` for the wait and stops it afterwards. A relay
/// started with no input quits at once, so it is given one that stays open.
pub fn wait_for_extension(mut relay: Command, port: u16, wait: Duration) -> bool {
    let _own = match std::net::TcpStream::connect(("127.0.0.1", port)) {
        Ok(_) => None,
        Err(_) => match relay.stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(child) => Some(Relay(child)),
            Err(e) => {
                log!("the browser link's relay could not be started: {e}");
                return false;
            }
        },
    };
    let until = Instant::now() + wait;
    while Instant::now() < until {
        if reached(&relay_says(port)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}

/// Whether what a relay says of itself shows an extension has reached it.
/// Until one has it answers 503 with `"extensionSeen": false`. Once Chrome's
/// has, it answers as Chrome would, with no `extensionSeen` at all, and names
/// the protocol the extension spoke (`ompExtensionDiscardedTabsProtocol`).
/// An extension that has said hello and no more is `"extensionSeen": true`.
fn reached(said: &Value) -> bool {
    said.get("extensionSeen") == Some(&Value::Bool(true)) || said.get("ompExtensionDiscardedTabsProtocol").is_some()
}

/// What a relay says of itself at `/json/version`, whatever its status: 503
/// until an extension has reached it. Nothing when none answers.
pub fn relay_says(port: u16) -> Value {
    use std::io::{Read, Write};
    let Ok(mut socket) = std::net::TcpStream::connect(("127.0.0.1", port)) else { return Value::Null };
    let _ = socket.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = write!(socket, "GET /json/version HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    let mut answer = String::new();
    let _ = socket.read_to_string(&mut answer);
    answer.find('{').and_then(|start| serde_json::from_str(&answer[start..answer.rfind('}')? + 1]).ok()).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_is_seen_to_reach_the_relay_by_what_the_relay_says() {
        // As the relay of Oh-my-pi 18.8.7 answered on 2026-10-14: before any extension, and
        // after Chrome's had reached it.
        let before = json!({ "ompRelayVersion": "18.8.7", "error": "relay extension is not connected", "extensionSeen": false, "uptimeMs": 3949 });
        let after = json!({ "Browser": "Chrome/155.0.0.0", "Protocol-Version": "1.3", "webSocketDebuggerUrl": "ws://127.0.0.1:9224/cdp", "ompRelayVersion": "18.8.7", "ompRelayDiscardedTabsProtocol": "1", "ompExtensionDiscardedTabsProtocol": "1" });
        assert!(!reached(&before));
        assert!(reached(&after), "Chrome's extension reached the relay and was not seen");
        assert!(reached(&json!({ "extensionSeen": true })), "an extension that has only said hello");
        assert!(!reached(&Value::Null), "no relay answered");
    }

    /// The extension as Oh-my-pi 18.8.7 writes it, kept whole, so that the
    /// renaming is tried on what an engine really writes. Its licence is the
    /// harness's own and stays with it.
    const KEPT: [(&str, &str); 4] = [
        ("manifest.json", include_str!("../tests/Data/BrowserLink/manifest.json")),
        ("background.js", include_str!("../tests/Data/BrowserLink/background.js")),
        ("options.html", include_str!("../tests/Data/BrowserLink/options.html")),
        ("LICENSE", include_str!("../tests/Data/BrowserLink/LICENSE")),
    ];
    const MANIFEST: &str = KEPT[0].1;

    /// A folder of the test's own, thrown away when it is over.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!("null-browser-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A stand-in for an engine whose extension is `files`: it writes them
    /// into the folder named after `--dir`, as `browser-relay install` does.
    fn engine_writing(scratch: &Scratch, name: &str, files: &[(&str, String)]) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let written = scratch.0.join(format!("{name}-writes"));
        std::fs::create_dir_all(&written).unwrap();
        for (file, text) in files {
            std::fs::write(written.join(file), text).unwrap();
        }
        let program = scratch.0.join(name);
        let script = format!("#!/bin/sh\nwhile [ $# -gt 0 ]; do [ \"$1\" = --dir ] && dir=\"$2\"; shift; done\nmkdir -p \"$dir\" && cp \"{}\"/* \"$dir\"/\n", written.display());
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        program
    }

    fn kept() -> Vec<(&'static str, String)> {
        KEPT.iter().map(|(file, text)| (*file, text.to_string())).collect()
    }

    #[test]
    fn the_extension_18_8_7_writes_is_put_under_null_s_name_whole() {
        let scratch = Scratch::new("whole");
        let link = scratch.0.join("BrowserLink");
        make(&engine_writing(&scratch, "omp", &kept()), &link).expect("Null knows how to name the extension 18.8.7 writes");
        let read = |file: &str| std::fs::read_to_string(link.join(file)).unwrap();
        let manifest: Value = serde_json::from_str(&read("manifest.json")).unwrap();
        assert_eq!((&manifest["name"], &manifest["action"]["default_title"]), (&json!("Null"), &json!("Null")));
        for (_, icon) in manifest["icons"].as_object().unwrap() {
            assert!(link.join(icon.as_str().unwrap()).is_file(), "the icon {icon} is in the copy");
        }
        let script = read("background.js");
        assert!(script.contains("async function groupTabs(tabIds, _title, color) {\n  const title = \"Null\";"), "every group the agent's tabs are gathered in is titled Null");
        assert!(script.contains("discardedTabsProtocol: 1"), "the protocol the relay checks is as the harness wrote it");
        let undone = CHANGES.iter().filter(|(file, ..)| *file == "background.js").fold(script.clone(), |text, (_, from, to)| text.replacen(to, from, 1));
        assert_eq!(undone, KEPT[1].1, "nothing of the script is changed but Null's own changes");
        let page = read("options.html");
        assert!(page.contains("<title>Null settings</title>") && page.contains("<h1>Null</h1>"));
        for file in ["manifest.json", "options.html"] {
            assert!(!read(file).contains("OMP Browser Relay"), "{file} still shows the harness's name");
        }
        assert_eq!(read("LICENSE"), KEPT[3].1, "the harness's licence stays with it");
    }

    #[test]
    fn an_extension_null_does_not_know_leaves_the_copy_chrome_loads_as_it_was() {
        let scratch = Scratch::new("unknown");
        let link = scratch.0.join("BrowserLink");
        make(&engine_writing(&scratch, "known", &kept()), &link).unwrap();
        let before = held(&link);
        for (file, from) in [("background.js", "async function groupTabs(tabIds, title, color) {"), ("options.html", "<h1>OMP Browser Relay</h1>")] {
            let moved: Vec<_> = kept().into_iter().map(|(name, text)| (name, if name == file { text.replace(from, "") } else { text })).collect();
            let why = make_again(&engine_writing(&scratch, &format!("moved-{file}"), &moved), &link).expect_err("a change that finds nothing to change stops the copy");
            assert!(why.contains(file), "{why}");
            assert_eq!(held(&link), before, "the copy Chrome loads is left as it was");
            assert!(!link.with_extension("new").exists(), "nothing is left beside it");
        }
    }

    #[test]
    fn the_copy_made_again_for_another_engine_is_said_only_when_it_changed() {
        let scratch = Scratch::new("again");
        let link = scratch.0.join("BrowserLink");
        let same = engine_writing(&scratch, "same", &kept());
        make(&same, &link).unwrap();
        assert_eq!(make_again(&same, &link), Ok(false), "the same engine again: nothing for Chrome to reload");
        let newer: Vec<_> = kept().into_iter().map(|(name, text)| (name, text.replace("discardedTabsProtocol: 1", "discardedTabsProtocol: 2"))).collect();
        assert_eq!(make_again(&engine_writing(&scratch, "newer", &newer), &link), Ok(true), "a newer protocol: Chrome has to reload the copy");
        assert!(std::fs::read_to_string(link.join("background.js")).unwrap().contains("discardedTabsProtocol: 2"), "the copy in the folder Chrome loads is the newer one");
        let manifest: Value = serde_json::from_str(&std::fs::read_to_string(link.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["name"], "Null", "and still under Null's name");
    }

    #[test]
    fn the_copy_is_made_again_only_when_the_engine_in_use_is_not_the_one_it_was_made_from() {
        assert!(!behind(true, Some("18.8.7"), Some("18.8.7")), "the same version");
        assert!(behind(true, Some("18.8.7"), Some("18.10.2")), "a newer Null, /update or the person's own omp update");
        assert!(behind(true, None, Some("18.8.7")), "a copy made before Null noted the version, made again once");
        assert!(!behind(false, Some("18.8.7"), Some("18.10.2")), "never set up: nothing to make");
        assert!(!behind(true, Some("18.8.7"), None), "an engine that does not say its version is left alone");
    }

    #[test]
    fn the_agent_is_told_the_link_s_rules_only_while_it_is_on_and_chrome_loaded_it() {
        let with = |browser, browser_linked| told(&settings::Settings { browser, browser_linked, ..Default::default() });
        assert!(with(Some(true), true));
        assert!(!with(Some(true), false), "on, but Chrome was not seen to load it");
        assert!(!with(Some(false), true), "switched off");
        assert!(!with(None, true), "never switched on");
        assert!(!with(None, false));
    }

    #[test]
    fn chrome_shows_null_s_name_words_and_icon_and_nothing_else_changes() {
        let renamed: Value = serde_json::from_str(&renamed_manifest(MANIFEST).unwrap()).unwrap();
        let before: Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(renamed["name"], "Null");
        assert_eq!(renamed["action"]["default_title"], "Null");
        assert_eq!(renamed["description"], DESCRIPTION);
        assert_eq!(renamed["icons"], json!({ "32": "null-32.png", "128": "null-128.png" }));
        assert_eq!(renamed["action"]["default_icon"], renamed["icons"]);
        for kept in ["manifest_version", "version", "permissions", "background", "options_page"] {
            assert_eq!(renamed[kept], before[kept], "{kept} is as the harness wrote it");
        }
    }

    #[test]
    fn a_manifest_null_does_not_know_is_not_renamed() {
        assert!(renamed_manifest("not json").is_err());
        assert!(renamed_manifest(r#"{ "name": "x" }"#).is_err(), "no toolbar button");
        assert!(renamed_manifest(r#"{ "action": {} }"#).is_err(), "no name");
    }

    #[test]
    fn a_change_is_made_only_where_its_words_stand_once() {
        assert_eq!(changed("a <h1>X</h1> b", "<h1>X</h1>", "<h1>Null</h1>", "f").unwrap(), "a <h1>Null</h1> b");
        assert!(changed("a b", "<h1>X</h1>", "<h1>Null</h1>", "f").is_err(), "not there");
        assert!(changed("<h1>X</h1><h1>X</h1>", "<h1>X</h1>", "<h1>Null</h1>", "f").is_err(), "there twice");
    }
}
