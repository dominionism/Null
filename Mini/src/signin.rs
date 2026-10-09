//! Signing in to a provider, from the box.
//!
//! Null knows nothing about any one provider. The harness does: its own sign-in
//! (`omp login`) lists the providers, says which page to open and what to paste,
//! and checks the result. This module runs that sign-in on ordinary pipes, passes
//! what it says to the box, and passes the user's answers back.
//!
//! An answer may be a key. It is written to the harness's sign-in and nowhere
//! else: nothing here logs or keeps what the user types or what the harness says.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::log::log;
use crate::{harness, panel};

/// One entry of the harness's own list of providers.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Provider {
    /// The number the harness's sign-in expects for this entry.
    pub number: u32,
    pub label: String,
}

/// Something the sign-in said, as the box shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum Said {
    Line(String),
    /// The harness is waiting for something typed.
    Question(String),
}

// ── Reading what the sign-in says ─────────────────────────────────────────

/// The harness's list of providers, from what `omp login` prints before it asks for a number.
pub fn providers_from(output: &str) -> Vec<Provider> {
    output.lines().filter_map(list_entry).collect()
}

fn list_entry(line: &str) -> Option<Provider> {
    let (number, label) = line.trim().split_once(". ")?;
    let number = number.parse().ok()?;
    let label = label.trim();
    (!label.is_empty()).then(|| Provider { number, label: label.to_string() })
}

/// Text without the escape sequences and control characters a terminal would act on.
fn clean(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut kept = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.next() {
                // A control sequence ends at its first letter or symbol from @ to ~.
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                // A title or link sequence ends at a bell.
                Some(']') => {
                    for c in chars.by_ref() {
                        if c == '\u{7}' {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if c == '\t' || !c.is_control() {
            kept.push(c);
        }
    }
    kept.trim().to_string()
}

/// Reads the sign-in's output as it arrives. A question ("Paste your key: ") comes
/// with no line end, so waiting for whole lines would never show it.
#[derive(Default)]
pub struct Feed {
    pending: Vec<u8>,
    /// How much of `pending` has already been shown as a question.
    asked: usize,
}

impl Feed {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Said> {
        self.pending.extend_from_slice(chunk);
        let mut said = Vec::new();
        while let Some(end) = self.pending.iter().position(|&byte| byte == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let text = clean(&line[self.asked.min(line.len())..]);
            self.asked = 0;
            if !text.is_empty() {
                said.push(Said::Line(text));
            }
        }
        let rest = String::from_utf8_lossy(&self.pending[self.asked.min(self.pending.len())..]).into_owned();
        if rest.ends_with(": ") || rest.ends_with("? ") {
            let text = clean(rest.as_bytes());
            self.asked = self.pending.len();
            if !text.is_empty() {
                said.push(Said::Question(text));
            }
        }
        said
    }
}

// ── Running the sign-in ───────────────────────────────────────────────────

/// A sign-in under way.
pub struct Running {
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
}

struct Progress {
    list_seen: bool,
    /// The harness has asked which entry of its list ("Enter number (1-78): ").
    asked_which: bool,
    last_output: Instant,
    chosen: bool,
}

impl Running {
    /// Pass one typed line to the sign-in.
    pub fn answer(&self, text: &str) -> Result<(), String> {
        let mut stdin = self.stdin.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let pipe = stdin.as_mut().ok_or("the sign-in is no longer listening")?;
        pipe.write_all(text.as_bytes()).and_then(|_| pipe.write_all(b"\n")).and_then(|_| pipe.flush()).map_err(|e| e.to_string())
    }

    pub fn cancel(&self) {
        let _ = self.child.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).kill();
    }
}

/// Start the harness's sign-in and choose entry `number` from its list.
///
/// `tell` gets every line and question that follows the choice, and `finished` is
/// called once, with whether the harness reported success.
pub fn start(
    binary: &Path,
    extra: &[String],
    number: u32,
    tell: impl Fn(Said) + Send + Sync + 'static,
    finished: impl FnOnce(bool) + Send + 'static,
) -> Result<Arc<Running>, String> {
    let mut child = Command::new(binary)
        .args(extra)
        .arg("login")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start the sign-in: {e}"))?;
    let stdout = child.stdout.take().ok_or("the sign-in has no output")?;
    let stderr = child.stderr.take().ok_or("the sign-in has no output")?;
    let stdin = child.stdin.take();
    let running = Arc::new(Running { child: Mutex::new(child), stdin: Mutex::new(stdin) });
    let progress = Arc::new(Mutex::new(Progress { list_seen: false, asked_which: false, last_output: Instant::now(), chosen: false }));
    let tell: Arc<dyn Fn(Said) + Send + Sync> = Arc::new(tell);

    // What it prints. Until the choice is made that is its list and its question
    // about the list, which the box has already shown and answered in its own way.
    let read = |mut pipe: Box<dyn Read + Send>, progress: Arc<Mutex<Progress>>, tell: Arc<dyn Fn(Said) + Send + Sync>| {
        std::thread::spawn(move || {
            let mut feed = Feed::default();
            let mut buffer = [0u8; 4096];
            while let Ok(count) = pipe.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                for said in feed.push(&buffer[..count]) {
                    let mut progress = progress.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    progress.last_output = Instant::now();
                    if !progress.chosen {
                        match &said {
                            Said::Line(text) if list_entry(text).is_some() || text.starts_with("Select a provider") => {
                                progress.list_seen = true;
                                continue;
                            }
                            Said::Question(_) => {
                                progress.asked_which = true;
                                continue;
                            }
                            Said::Line(_) => {}
                        }
                    }
                    drop(progress);
                    tell(said);
                }
            }
        });
    };
    read(Box::new(stdout), progress.clone(), tell.clone());
    read(Box::new(stderr), progress.clone(), tell.clone());

    // The choice. An answer written before the question is asked is lost, so wait
    // for the harness to ask which entry; failing that, for its list to have been
    // printed and the output to have gone quiet.
    let chooser = running.clone();
    let chosen = progress.clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(50));
            let progress = chosen.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let quiet = progress.last_output.elapsed() >= Duration::from_millis(300);
            if progress.asked_which || (progress.list_seen && quiet) || started.elapsed() >= Duration::from_secs(5) {
                break;
            }
        }
        chosen.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).chosen = true;
        let _ = chooser.answer(&number.to_string());
    });

    // The end.
    let waiter = running.clone();
    std::thread::spawn(move || {
        let ok = loop {
            std::thread::sleep(Duration::from_millis(100));
            match waiter.child.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).try_wait() {
                Ok(Some(status)) => break status.success(),
                Ok(None) => {}
                Err(_) => break false,
            }
        };
        // Let the last lines reach the box before it is told the sign-in is over.
        std::thread::sleep(Duration::from_millis(200));
        finished(ok);
    });

    Ok(running)
}

/// Run a command that should end by itself, and give back what it printed.
fn run_briefly(mut command: Command, limit: Duration) -> Result<String, String> {
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().ok_or("no output")?;
    let reader = std::thread::spawn(move || {
        let mut printed = Vec::new();
        let _ = stdout.read_to_end(&mut printed);
        printed
    });
    let started = Instant::now();
    while matches!(child.try_wait(), Ok(None)) {
        if started.elapsed() >= limit {
            let _ = child.kill();
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let printed = reader.join().map_err(|_| "could not read the output".to_string())?;
    Ok(clean_lines(&printed))
}

fn clean_lines(bytes: &[u8]) -> String {
    bytes.split(|&byte| byte == b'\n').map(clean).collect::<Vec<_>>().join("\n")
}

// ── What the page may ask ─────────────────────────────────────────────────

#[derive(Default)]
pub struct Signin(Mutex<Option<Arc<Running>>>);

pub fn init(app: &AppHandle) {
    app.manage(Signin::default());
}

/// True while a sign-in is running. The user is then in their browser, or copying
/// a key, and the box has to stay where they can come back to it.
pub fn under_way(app: &AppHandle) -> bool {
    current(app).is_some()
}

fn current(app: &AppHandle) -> Option<Arc<Running>> {
    app.state::<Signin>().0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
}

/// The providers the harness can sign in to, as the harness lists them.
#[tauri::command(async)]
pub fn signin_providers() -> Result<Vec<Provider>, String> {
    let binary = harness::installed().ok_or(format!("{} is not installed", harness::HARNESS_NAME))?;
    let mut command = Command::new(binary);
    command.args(harness::extra_args()).arg("login");
    // With nothing to read it prints its list and gives up, which is all that is wanted.
    let printed = run_briefly(command, Duration::from_secs(10))?;
    let providers = providers_from(&printed);
    if providers.is_empty() {
        return Err(format!("{} did not list any providers", harness::HARNESS_NAME));
    }
    Ok(providers)
}

/// Begin signing in to entry `number` of that list. What follows arrives as `mini:signin` events.
#[tauri::command(async)]
pub fn signin_start(app: AppHandle, number: u32) -> Result<(), String> {
    let state = app.state::<Signin>();
    let mut slot = state.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if slot.is_some() {
        return Err("a sign-in is already under way".into());
    }
    let binary = harness::installed().ok_or(format!("{} is not installed", harness::HARNESS_NAME))?;
    let said = app.clone();
    let over = app.clone();
    let running = start(
        &binary,
        &harness::extra_args(),
        number,
        move |said_now| {
            let payload = match said_now {
                Said::Line(text) => json!({ "kind": "line", "text": text }),
                Said::Question(text) => json!({ "kind": "ask", "text": text }),
            };
            let _ = said.emit_to(panel::LABEL, "mini:signin", payload);
        },
        move |ok| {
            *over.state::<Signin>().0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
            log!("the sign-in ended: {}", if ok { "the harness reports success" } else { "not signed in" });
            if ok {
                // The harness process that is running was started before this
                // sign-in; a fresh one sees the new provider.
                harness::restart(&over);
            }
            let _ = over.emit_to(panel::LABEL, "mini:signin", json!({ "kind": "done", "ok": ok }));
        },
    )?;
    *slot = Some(running);
    log!("a sign-in started (entry {number} of the harness's list)");
    Ok(())
}

/// Pass what the user typed to the sign-in. It may be a key: it goes nowhere else.
#[tauri::command(async)]
pub fn signin_answer(app: AppHandle, text: String) -> Result<(), String> {
    current(&app).ok_or("no sign-in is under way")?.answer(&text)
}

#[tauri::command(async)]
pub fn signin_cancel(app: AppHandle) {
    if let Some(running) = current(&app) {
        running.cancel();
    }
}

/// Open a page the sign-in pointed to, in the user's browser.
#[tauri::command(async)]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("not a web address".into());
    }
    Command::new("/usr/bin/open").arg(&url).status().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_harness_list_is_read_into_numbers_and_labels() {
        let printed = "Select a provider:\n  1. ChatGPT Plus/Pro (Codex Subscription)\n  2. Anthropic (Claude Pro/Max)\n  19. ai&\n\nLogin failed: Login cancelled: stdin closed\n";
        assert_eq!(
            providers_from(printed),
            vec![
                Provider { number: 1, label: "ChatGPT Plus/Pro (Codex Subscription)".into() },
                Provider { number: 2, label: "Anthropic (Claude Pro/Max)".into() },
                Provider { number: 19, label: "ai&".into() },
            ]
        );
    }

    #[test]
    fn whole_lines_are_passed_on_as_they_complete() {
        let mut feed = Feed::default();
        assert_eq!(feed.push(b"Open this URL in your"), vec![]);
        assert_eq!(feed.push(b" browser:\nhttps://example.com/keys\n"), vec![Said::Line("Open this URL in your browser:".into()), Said::Line("https://example.com/keys".into())]);
    }

    #[test]
    fn a_question_is_shown_without_waiting_for_a_line_end() {
        let mut feed = Feed::default();
        assert_eq!(feed.push(b"Paste your DeepSeek API key (sk-...): "), vec![Said::Question("Paste your DeepSeek API key (sk-...):".into())]);
        // What follows on the same line is new; the question is not repeated.
        assert_eq!(feed.push(b"Validating API key...\nLogin failed: bad key\n"), vec![Said::Line("Validating API key...".into()), Said::Line("Login failed: bad key".into())]);
    }

    #[test]
    fn terminal_colour_codes_are_dropped() {
        let mut feed = Feed::default();
        assert_eq!(feed.push(b"\x1b[1mWaiting\x1b[0m for browser authentication...\r\n"), vec![Said::Line("Waiting for browser authentication...".into())]);
    }

    /// Needs Oh-my-pi installed and reaches a provider, so it runs only when asked:
    /// `cargo test -- --ignored`. It uses the probe profile, never the real sign-ins.
    #[test]
    #[ignore]
    fn a_wrong_key_is_refused_by_the_harness() {
        let binary = harness::installed().expect("Oh-my-pi is installed");
        let extra = vec!["--profile".to_string(), "null-probe".to_string()];
        let mut command = Command::new(&binary);
        command.args(&extra).arg("login");
        let listed = providers_from(&run_briefly(command, Duration::from_secs(10)).unwrap());
        let deepseek = listed.iter().find(|provider| provider.label == "DeepSeek").expect("DeepSeek is in the list");

        let heard = Arc::new(Mutex::new(Vec::new()));
        let (done, over) = std::sync::mpsc::channel();
        let record = heard.clone();
        let running = start(&binary, &extra, deepseek.number, move |said| record.lock().unwrap().push(said), move |ok| done.send(ok).unwrap()).unwrap();

        let asked = Instant::now();
        while !heard.lock().unwrap().iter().any(|said| matches!(said, Said::Question(_))) {
            assert!(asked.elapsed() < Duration::from_secs(15), "the harness never asked for the key: {:?}", heard.lock().unwrap());
            std::thread::sleep(Duration::from_millis(50));
        }
        running.answer("sk-null-dummy-key-0000").unwrap();

        assert!(!over.recv_timeout(Duration::from_secs(30)).unwrap(), "a dummy key was accepted");
        let heard = heard.lock().unwrap();
        assert!(heard.iter().any(|said| matches!(said, Said::Line(text) if text.starts_with("Login failed"))), "{heard:?}");
        assert!(!heard.iter().any(|said| matches!(said, Said::Line(text) | Said::Question(text) if text.contains("sk-null-dummy-key-0000"))), "the key was echoed");
    }
}
