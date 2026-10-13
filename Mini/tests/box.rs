//! The box check: what a person does in the box, tried in the real app.
//!
//! Each test is one thing Null does for the person using it, and is named for
//! that thing, so a failure says what broke. It starts the real app on a folder
//! of its own, where a stand-in provider answers and nothing of the user's is
//! read or changed, has it carry out a few lines as a person would (`selftest`
//! in `src/harness.rs` says what a line can be), and looks at what the page had.
//!
//! Every run is also held to three things the box promises everywhere: the
//! window is never asked to be taller than the tallest it gets, the transcript
//! never taller than its ten lines, and no message a person typed is in the log.
//!
//! They run only when asked (`cargo test -- --ignored`), and one at a time,
//! since each opens the box on the screen and takes the keyboard for a few
//! seconds. Leave the Mac alone and unlocked meanwhile: a page behind a locked
//! screen does not move. One of them moves a made-up conversation to the Trash,
//! which is the only way to see that deleting does that.

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

/// The tallest the window gets, and the transcript's ten lines (`src/panel.rs`).
const MAX_HEIGHT: f64 = 314.0;
const TEN_LINES: f64 = 212.0;

/// How long one run of the app is waited for.
const PATIENCE: Duration = Duration::from_secs(150);

/// One box on the screen at a time, however many tests are asked for at once.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

const WORDS: &str = "alder birch cedar dogwood elm fir ginkgo hazel ivy juniper kapok larch maple nutmeg oak pine quince rowan spruce teak upas vine willow yew";

/// A message the slow model takes about six seconds to say back, `times` over.
fn long(times: usize) -> String {
    vec![WORDS; times].join(" ")
}

/// The version of Oh-my-pi this Null carries.
fn carried() -> &'static str {
    let named = include_str!("../Engine.toml").lines().find_map(|line| line.strip_prefix("version = \""));
    named.and_then(|rest| rest.strip_suffix('"')).expect("Engine.toml names a version")
}

// ── A Mac of the test's own ───────────────────────────────────────────────

/// A folder that stands for a Mac: the harness's folder, Null's own files and
/// the log are all under it. It is thrown away when the test is over.
struct Mac {
    folder: PathBuf,
    with: Vec<(&'static str, String)>,
}

impl Mac {
    /// One that has never had Null.
    fn new() -> Mac {
        static MADE: AtomicU32 = AtomicU32::new(0);
        let folder = std::env::temp_dir().join(format!("null-box-check-{}-{}", std::process::id(), MADE.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(folder.join("null")).unwrap();
        Mac { folder, with: Vec::new() }
    }

    /// One where Null has been opened before, on one of the stand-in's models:
    /// `ok` answers "pong", `slow` says the message back a word at a time, and
    /// `limit`, `auth` and `noaccess` are refused as a provider refuses.
    fn on(model: &str) -> Mac {
        let mac = Mac::new();
        mac.keep("null/settings.json", &format!(r#"{{"model":"standin-anthropic/{model}","welcomed":true,"asked_full_disk":true}}"#));
        mac
    }

    /// Something in the app's surroundings for its next runs, as `main.rs` lists them.
    fn with(mut self, name: &'static str, value: String) -> Mac {
        self.with.push((name, value));
        self
    }

    fn place(&self, file: &str) -> String {
        self.folder.join(file).display().to_string()
    }

    fn keep(&self, file: &str, holds: &str) {
        let path = self.folder.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, holds).unwrap();
    }

    fn kept(&self, file: &str) -> String {
        std::fs::read_to_string(self.folder.join(file)).unwrap_or_default()
    }

    /// Start Null here, have it carry out `lines`, and give back what it printed.
    fn run(&self, lines: &[&str]) -> Seen {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|held| held.into_inner());
        let printed = self.folder.join("printed.log");
        let mut null = Command::new(env!("CARGO_BIN_EXE_null-mini"))
            .env("NULL_MINI_FOLDER", &self.folder)
            .env("NULL_MINI_STANDIN", "1")
            .env("NULL_MINI_NO_SHORTCUT", "1")
            .env("NULL_MINI_SELFTEST", lines.join("\n"))
            .envs(self.with.iter().map(|(name, value)| (*name, value.as_str())))
            .stdout(Stdio::null())
            .stderr(File::create(&printed).unwrap())
            .spawn()
            .expect("Null starts");
        let began = Instant::now();
        let ended = loop {
            if let Some(how) = null.try_wait().unwrap() {
                break Some(how);
            }
            if began.elapsed() > PATIENCE {
                let _ = null.kill();
                let _ = null.wait();
                break None;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let printed = std::fs::read_to_string(&printed).unwrap_or_default();
        match ended {
            Some(how) => assert!(how.success(), "Null ended badly ({how}):\n{printed}"),
            None => panic!("Null was still going after {PATIENCE:?}. Is the screen locked?\n{printed}"),
        }
        Seen::read(printed, lines)
    }
}

impl Drop for Mac {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

// ── What a run showed ─────────────────────────────────────────────────────

/// What one run printed: its log, and what the page had at each `<show name>`.
struct Seen {
    /// The log as the app writes it for a person, without what a scripted run adds.
    log: String,
    pages: HashMap<String, Value>,
}

impl Seen {
    fn read(printed: String, lines: &[&str]) -> Seen {
        let mut seen = Seen { log: String::new(), pages: HashMap::new() };
        for line in printed.lines().map(|line| line.strip_prefix("[mini] ").unwrap_or(line)) {
            let Some(said) = line.strip_prefix("selftest: ") else {
                seen.log.push_str(line);
                seen.log.push('\n');
                continue;
            };
            let Some((name, page)) = said.strip_prefix("at ").and_then(|rest| rest.split_once(": ")) else { continue };
            // A wait that was never met, or a press on nothing, is said in place of a page.
            let page: Value = serde_json::from_str(page).unwrap_or_else(|_| panic!("the run did not go as written, at {name}: {page}\n{printed}"));
            seen.pages.insert(name.to_string(), page);
        }
        // What the box promises in every state, whatever the test is about.
        for (name, page) in &seen.pages {
            assert!(page["height"].as_f64().unwrap_or(0.0) <= MAX_HEIGHT, "at {name} the box asked for a window taller than the window gets: {page}");
            assert!(page["transcript"].as_f64().unwrap_or(0.0) <= TEN_LINES, "at {name} the transcript is taller than its ten lines: {page}");
        }
        // A message is a line of several words: one word alone is a choice from a list.
        for typed in lines.iter().filter(|line| !line.starts_with('<') && !line.starts_with('/') && line.contains(' ')) {
            let message = typed.strip_suffix(" &").unwrap_or(typed);
            assert!(!seen.log.contains(message), "a message that was typed is in the log: {message}\n{}", seen.log);
        }
        seen
    }

    /// What the page had at `<show name>`.
    fn at(&self, name: &str) -> Look<'_> {
        let page = self.pages.get(name).unwrap_or_else(|| panic!("the run never came to {name}:\n{}", self.log));
        Look { name: name.to_string(), page }
    }

    /// Whether the app's log has these words.
    fn logged(&self, words: &str) -> bool {
        self.log.contains(words)
    }
}

/// The page at one moment: the field, the arrow, the pet, the notice, the open
/// list, the transcript (`shows`) and its parts (`lines`).
struct Look<'a> {
    name: String,
    page: &'a Value,
}

impl Look<'_> {
    fn text(&self, part: &str) -> &str {
        self.page[part].as_str().unwrap_or_else(|| panic!("{part} is no text {self}"))
    }

    /// Whether a part of the page holds these words. A heading is drawn in capitals, so case is let be.
    fn has(&self, part: &str, words: &str) -> bool {
        self.text(part).to_lowercase().contains(&words.to_lowercase())
    }

    fn is(&self, part: &str) -> bool {
        self.page[part].as_bool().unwrap_or_else(|| panic!("{part} is no yes or no {self}"))
    }

    fn number(&self, part: &str) -> f64 {
        self.page[part].as_f64().unwrap_or_else(|| panic!("{part} is no number {self}"))
    }

    /// The parts of the transcript in order, each by its kind: `you`, `note`,
    /// `steps`, `reply`, or `failed reply`. The page names more about a part
    /// than its kind, in an order that depends on when each thing happened.
    fn parts(&self) -> String {
        fn kind(part: &str) -> &str {
            let named: Vec<&str> = part.split_whitespace().collect();
            match (named.contains(&"reply"), named.contains(&"failed")) {
                (true, true) => "failed reply",
                (true, false) => "reply",
                _ => named.first().copied().unwrap_or(""),
            }
        }
        self.text("lines").split(',').map(kind).collect::<Vec<_>>().join(",")
    }

    /// How many of one kind of element the transcript is drawn with.
    fn drawn(&self, tag: &str) -> u64 {
        self.page["tags"][tag].as_u64().unwrap_or(0)
    }

    /// How many rows of the open list are for this command.
    fn rows(&self, command: &str) -> usize {
        self.text("list").lines().filter(|row| row.trim_start().split_whitespace().next() == Some(command)).count()
    }
}

impl fmt::Display for Look<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at {}: {}", self.name, self.page)
    }
}

// ── Talking ───────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn a_message_is_answered_and_the_arrow_and_the_pet_come_to_rest() {
    let seen = Mac::on("slow").run(&["hello there", "<settle>", "<show answered>"]);
    let page = seen.at("answered");
    assert_eq!(page.parts(), "you,note,reply", "{page}");
    assert_eq!(page.text("shows").matches("hello there").count(), 2, "the message and what the model said back {page}");
    assert!(page.has("shows", "now on standin-anthropic/slow"), "{page}");
    assert!(!page.is("busy") && page.text("arrow") == "caret", "{page}");
    assert!(matches!(page.text("pet"), "pet done" | "pet rest"), "{page}");
    assert!(seen.logged("reply ended: completed"));
}

#[test]
#[ignore]
fn markdown_is_drawn_and_nothing_a_model_writes_becomes_html() {
    let said = "**bold** and *soft* and `code` and <b>raw</b> <script>alert(1)</script> <img src=x onerror=alert(2)>";
    let seen = Mac::on("slow").run(&[said, "<settle>", "<show drawn>"]);
    let page = seen.at("drawn");
    assert_eq!((page.drawn("STRONG"), page.drawn("EM"), page.drawn("CODE")), (1, 1, 1), "{page}");
    assert_eq!((page.drawn("B"), page.drawn("SCRIPT"), page.drawn("IMG")), (0, 0, 0), "{page}");
    assert!(page.has("shows", "bold and soft and code and <b>raw</b> <script>alert(1)</script>"), "the marks are gone and the html is words {page}");
}

#[test]
#[ignore]
fn the_up_arrow_brings_the_last_message_back_and_an_empty_enter_sends_nothing() {
    let seen = Mac::on("slow").run(&["hello there", "<settle>", "<key ArrowUp>", "<show recalled>", "<type >", "<key Enter>", "<show nothing>"]);
    assert_eq!(seen.at("recalled").text("field"), "hello there");
    let page = seen.at("nothing");
    assert_eq!(page.parts(), "you,note,reply", "{page}");
    assert!(!page.is("busy"), "{page}");
}

#[test]
#[ignore]
fn a_long_reply_scrolls_inside_ten_lines() {
    let seen = Mac::on("slow").run(&[&long(3), "<settle>", "<show long>"]);
    let page = seen.at("long");
    assert!(page.is("scrolls"), "{page}");
    assert!(page.number("transcript") > 200.0, "the transcript has grown to its ten lines {page}");
    assert_eq!(page.number("height"), 288.0, "the box with a full transcript and no notice {page}");
}

// ── The list of commands ──────────────────────────────────────────────────

#[test]
#[ignore]
fn a_slash_lists_the_box_s_commands_and_the_harness_s_each_once() {
    let seen = Mac::on("slow").run(&["<type />", "<until list /context>", "<show listed>", "<key Escape>", "<until no list>", "<show put-away>"]);
    let page = seen.at("listed");
    assert!(page.has("list", "the box") && page.has("list", "the harness"), "{page}");
    for own in ["/model", "/backup", "/usage", "/login", "/harness", "/update", "/attach", "/history", "/rename", "/pet", "/folder", "/new", "/quit"] {
        assert_eq!(page.rows(own), 1, "{own} {page}");
    }
    assert_eq!(page.rows("/context"), 1, "{page}");
    assert!(page.is("tinted"), "a slash is the start of a command {page}");
    let page = seen.at("put-away");
    assert!(page.text("list").is_empty() && page.text("field").is_empty(), "{page}");
}

#[test]
#[ignore]
fn a_word_narrows_the_list_tab_writes_the_name_out_and_enter_carries_it_out() {
    let lines = ["<type />", "<until list /context>", "<type /mo>", "<show narrowed>", "<key Tab>", "<show written>", "<key Enter>", "<until list standin-openai/ok>", "<show carried-out>", "<key Escape>"];
    let seen = Mac::on("slow").run(&lines);
    let page = seen.at("narrowed");
    assert!(page.text("chosen").trim_start().starts_with("/model"), "{page}");
    assert!(page.is("tinted"), "{page}");
    let page = seen.at("written");
    assert_eq!(page.text("field"), "/model", "{page}");
    assert!(page.is("tinted") && !page.text("list").is_empty(), "the name is written and the list stays {page}");
    assert!(seen.at("carried-out").has("list", "standin-anthropic/slow"), "the list of models");
}

#[test]
#[ignore]
fn a_word_that_is_no_command_is_not_sent() {
    let seen = Mac::on("slow").run(&["/zzz", "<until list no command like /zzz>", "<show slip>", "<key Escape>"]);
    let page = seen.at("slip");
    assert!(!page.is("tinted"), "{page}");
    assert!(!page.parts().contains("you") && !page.has("shows", "/zzz"), "nothing was sent {page}");
    assert_eq!(page.rows("/model"), 1, "the commands are there to choose from {page}");
}

// ── Models and providers ──────────────────────────────────────────────────

#[test]
#[ignore]
fn a_model_is_chosen_from_the_list_and_answers() {
    let lines = [
        "/model",
        "<until list standin-codex/ok>",
        "<show models>",
        "<key Escape>",
        "/model openai",
        "<until list standin-openai/ok>",
        "<show narrowed>",
        "<key Escape>",
        "<type >",
        "/model",
        "<until list standin-anthropic/ok>",
        "ok",
        "<until shows now on standin-anthropic/ok>",
        "say something",
        "<settle>",
        "<show answered>",
    ];
    let seen = Mac::on("slow").run(&lines);
    let page = seen.at("models");
    assert!(page.has("list", "STANDIN-ANTHROPIC") && page.has("list", "STANDIN-OPENAI") && page.has("list", "STANDIN-CODEX"), "{page}");
    assert!(page.has("chosen", "standin-anthropic/slow"), "the model in use is the one chosen {page}");
    let page = seen.at("narrowed");
    assert!(page.has("list", "standin-openai/slow") && !page.has("list", "standin-anthropic"), "{page}");
    assert!(seen.at("answered").has("shows", "pong"), "the model chosen answers");
}

#[test]
#[ignore]
fn a_refusal_is_shown_as_a_failed_reply_and_is_not_sent_again() {
    let lines = [
        "hello",
        "<settle>",
        "<show limit>",
        "<key ArrowUp>",
        "<show recalled>",
        "<type >",
        "/model",
        "<until list standin-anthropic/auth>",
        "auth",
        "<until shows now on standin-anthropic/auth>",
        "hello again",
        "<settle>",
        "<show auth>",
        "/model",
        "<until list standin-anthropic/noaccess>",
        "noaccess",
        "<until shows now on standin-anthropic/noaccess>",
        "hello once more",
        "<settle>",
        "<show noaccess>",
    ];
    let seen = Mac::on("limit").run(&lines);
    let page = seen.at("limit");
    assert_eq!(page.parts(), "you,note,failed reply,note", "sent once, and marked {page}");
    assert!(page.has("shows", "rate limit") && page.has("shows", "no answer from the provider. /model picks another model"), "{page}");
    assert_eq!(page.text("pet"), "pet fail", "{page}");
    assert_eq!(seen.at("recalled").text("field"), "hello", "it is there to send again by hand");
    let page = seen.at("auth");
    assert!(page.has("shows", "invalid x-api-key") && page.parts().ends_with("you,failed reply,note"), "{page}");
    let page = seen.at("noaccess");
    assert!(page.has("shows", "403") && page.parts().ends_with("you,failed reply,note"), "{page}");
}

#[test]
#[ignore]
fn a_backup_takes_over_from_a_model_that_refuses() {
    let mac = Mac::on("limit");
    let lines = [
        "/backup",
        "<until list standin-openai/ok>",
        "<show choosing>",
        "standin-openai",
        "<until list backups, in order>",
        "<show added>",
        "<key Escape>",
        "<until shows backups: standin-openai/ok>",
        "hello",
        "<settle>",
        "<until shows did not answer>",
        "<show after>",
    ];
    let seen = mac.run(&lines);
    assert_eq!(seen.at("choosing").text("notice"), "enter adds or removes a backup. esc is done");
    let page = seen.at("added");
    assert!(page.has("list", "1 Stand-in ok standin-openai/ok"), "{page}");
    let page = seen.at("after");
    assert!(page.has("shows", "pong") && page.has("shows", "standin-anthropic/limit did not answer. now on standin-openai/ok"), "{page}");
    assert!(!page.parts().contains("failed"), "the backup's answer is the answer {page}");
    let settings: Value = serde_json::from_str(&mac.kept("null/settings.json")).unwrap();
    assert_eq!(settings["backups"], serde_json::json!(["standin-openai/ok"]));
    assert!(mac.kept("null/harness.yml").contains(r#""fallbackChains":{"default":["standin-openai/ok"]}"#), "the harness is handed the order");
}

#[test]
#[ignore]
fn usage_names_every_provider_and_says_which_has_no_report() {
    let seen = Mac::on("slow").run(&["/usage", "<until list no usage report>", "<show usage>", "<key Enter>", "<until no list>", "<show put-away>"]);
    let page = seen.at("usage");
    for provider in ["STANDIN-ANTHROPIC", "STANDIN-OPENAI", "STANDIN-CODEX"] {
        assert!(page.has("list", provider), "{provider} {page}");
    }
    assert_eq!(page.text("list").matches("no usage report").count(), 3, "{page}");
    assert!(seen.at("put-away").text("list").is_empty());
}

// ── The harness ───────────────────────────────────────────────────────────

#[test]
#[ignore]
fn one_of_the_harness_s_own_commands_is_answered_and_a_skill_goes_by_its_short_name() {
    let mac = Mac::on("ok");
    // A skill of the run's own, so that this does not rest on the user's.
    mac.keep("agent/skills/box-check-echo/SKILL.md", "---\nname: box-check-echo\ndescription: Say the words back. Made by the box check.\n---\n\nSay back the words you were given.\n");
    let lines = [
        "<type />",
        "<until list /box-check-echo>",
        "<key Escape>",
        "<until no list>",
        "/context",
        "<until shows Context window>",
        "<settle>",
        "<show own>",
        "/box-check-echo these words",
        "<settle>",
        "<until shows pong>",
        "<show skill>",
    ];
    let seen = mac.run(&lines);
    let page = seen.at("own");
    assert_eq!(page.parts(), "you,reply", "an answer, and not a failed one {page}");
    assert!(!page.has("shows", "no answer from the provider"), "{page}");
    assert!(!page.text("shows").contains('\u{1b}') && !page.text("shows").contains("[38;"), "no colour codes in the words {page}");
    assert_ne!(page.text("pet"), "pet fail", "{page}");
    let page = seen.at("skill");
    assert!(page.has("shows", "/skill:box-check-echo these words"), "sent as the harness spells it, with what followed {page}");
    assert!(page.parts().ends_with("you,reply"), "{page}");
}

/// A second model looks at an answer again when the person has named one
/// (`/verify`). The answer is on the page first. Then the box says who verifies
/// it, that model's reply follows, and the conversation is back on its own
/// model afterwards, with the person's choice of model as it was. An answer
/// that took no step is left alone: it saw nothing to be checked against.
#[test]
#[ignore]
fn an_answer_that_took_a_step_is_verified_by_the_model_named_for_it() {
    let mac = Mac::on("ok");
    let lines = [
        "/verify",
        "<until list nobody>",
        "<show list>",
        "<key Escape>",
        "<until no list>",
        "/verify openai ok",
        "<until list standin-openai/ok>",
        "<key Enter>",
        "<until shows each answer is verified by standin-openai/ok>",
        "these words take no step",
        "<settle>",
        "<until shows pong>",
        "<show plain>",
        "ask the shell: true &",
        "<until shows verifying the answer with standin-openai/ok>",
        "<settle>",
        "<show verified>",
        "and these come after it",
        "<settle>",
        "<show after>",
    ];
    let seen = mac.run(&lines);
    let page = seen.at("list");
    assert!(page.has("list", "nobody") && page.has("list", "STANDIN-OPENAI"), "nobody first, then the models {page}");
    assert!(page.has("chosen", "nobody"), "nobody verifies until somebody is named {page}");

    let page = seen.at("plain");
    assert!(!page.has("shows", "verifying the answer"), "an answer that took no step was verified {page}");

    let page = seen.at("verified");
    assert!(page.parts().ends_with("note,reply"), "the box says who verifies, and that model's reply follows {page}");
    assert!(!page.is("busy"), "{page}");
    assert!(seen.logged("the answer is verified by standin-openai/ok"), "{}", seen.log);
    assert!(seen.logged("the conversation is back on standin-anthropic/ok"), "{}", seen.log);

    // The message after it is answered by the model the person chose, and not verified: it took no step.
    assert!(seen.at("after").parts().ends_with("you,reply"), "{}", seen.at("after"));
    let settings: Value = serde_json::from_str(&mac.kept("null/settings.json")).unwrap();
    assert_eq!(settings["model"], "standin-anthropic/ok", "the person's own model is still the one chosen {settings}");
    assert_eq!(settings["verifier"], "standin-openai/ok", "{settings}");
}

/// The harness starts every conversation thinking at its own level. The level
/// a person sets with the harness's own `/effort` is where their next
/// conversations start, in this run and once Null has been started again.
#[test]
#[ignore]
fn a_thinking_level_set_once_is_where_new_conversations_start() {
    // The stand-in's models say they can think here, so that there is a level to set.
    let mac = Mac::on("ok").with("NULL_MINI_STANDIN", "thinking".into());
    // The harness answers its command itself. The next conversation is not started before it has.
    let seen = mac.run(&["/effort low", "<until shows Thinking set to low>", "<settle>", "/new", "these words open a conversation", "<settle>", "<until shows pong>", "<show new>"]);
    assert!(seen.logged("thinking level low, set in the box"), "what the person set is not remembered:\n{}", seen.log);
    assert!(seen.logged("the conversation starts on thinking level low"), "the next conversation did not start there:\n{}", seen.log);
    let page = seen.at("new");
    assert!(page.parts().ends_with("you,reply"), "an answer, and not a failed one {page}");
    let settings: Value = serde_json::from_str(&mac.kept("null/settings.json")).unwrap();
    assert_eq!(settings["thinking"], "low", "{settings}");

    let seen = mac.run(&["/new", "and these open another", "<settle>", "<until shows pong>"]);
    assert!(seen.logged("the conversation starts on thinking level low"), "started again, Null did not start a conversation there:\n{}", seen.log);
    assert!(!seen.logged("thinking level low, set in the box"), "nobody set a level in this run:\n{}", seen.log);
}

#[test]
#[ignore]
fn the_box_says_which_harness_it_runs_and_switches_to_the_user_s_own() {
    let mac = Mac::on("slow");
    let seen = mac.run(&["/harness", "<until list built in>", "<show harnesses>", "<key Escape>"]);
    let page = seen.at("harnesses");
    assert!(page.has("list", &format!("built in Oh-my-pi {}", carried())), "{page}");
    assert!(seen.logged("starting the built-in harness"));
    // The rest needs an Oh-my-pi of the user's own on this Mac, which a hosted runner has not.
    if !page.has("list", "yours") {
        return;
    }
    assert_eq!(page.text("notice"), "built in stays until Null is updated. yours follows omp update", "{page}");
    let seen = mac.run(&["/harness", "<until list yours>", "<key ArrowDown>", "<key Enter>", "<until shows now on your Oh-my-pi>", "hello over there", "<settle>", "<show own>"]);
    assert!(seen.logged("starting the user's own harness"));
    assert_eq!(seen.at("own").text("shows").matches("hello over there").count(), 2, "the user's own answers");
}

#[test]
#[ignore]
fn computer_use_is_off_until_it_is_switched_on_and_the_harness_is_then_handed_it() {
    let mac = Mac::on("slow");
    let lines = [
        "/",
        "<until list /computer>",
        "<show commands>",
        "<key Escape>",
        "/computer",
        "<until list computer use>",
        "<show before>",
        "<key ArrowDown>",
        "<key Enter>",
        "<until shows computer use is on>",
        "<show on>",
        "hello with it on",
        "<settle>",
        "/computer",
        "<until list computer use>",
        "<show during>",
        "<key ArrowUp>",
        "<key Enter>",
        "<until shows computer use is off>",
        "<show off>",
    ];
    let seen = mac.run(&lines);
    assert_eq!(seen.at("commands").rows("/computer"), 1, "the box's own command, listed once and the harness's of that name not beside it {}", seen.at("commands"));

    let page = seen.at("before");
    assert!(page.has("list", "● off the agent cannot see the screen or work your apps"), "off is what is in force {page}");
    assert!(page.has("list", "on the agent may see the screen and work your apps, without asking you first"), "{page}");

    let page = seen.at("on");
    assert!(page.has("shows", "computer use is on: the agent may see the screen and work your apps, without asking you first"), "{page}");
    assert!(seen.logged("computer use: on"));

    let page = seen.at("during");
    assert!(page.has("list", "● on"), "on is in force now {page}");
    assert!(seen.at("off").has("shows", "computer use is off"), "{}", seen.at("off"));
    assert!(seen.logged("computer use: off"));

    // The harness is told when it next starts, in the file Null hands it: the message sent with
    // it on started one. Switched off since, the choice is kept for the start after this.
    assert!(mac.kept("null/harness.yml").contains(r#""computer":{"enabled":true}"#), "what the harness was handed: {}", mac.kept("null/harness.yml"));
    let kept: serde_json::Value = serde_json::from_str(&mac.kept("null/settings.json")).expect("Null's settings");
    assert_eq!(kept["computer"], false, "what Null keeps: {kept}");
}

#[test]
#[ignore]
fn with_computer_use_on_the_list_says_how_the_two_permissions_of_macos_stand() {
    let mac = Mac::on("slow");
    let lines = ["/computer", "<until list computer use>", "<show off>", "<key ArrowDown>", "<key Enter>", "<until shows computer use is on>", "<show switched>", "/computer", "<until list what macOS allows Null>", "<show on>", "<key Escape>"];
    let seen = mac.run(&lines);
    let page = seen.at("off");
    assert!(!page.has("list", "Accessibility") && !page.has("list", "Screen Recording"), "while it is off, what macOS allows is not the question {page}");

    // Each permission has a row that says how it stands, whatever the Mac this runs on has given.
    let page = seen.at("on");
    for name in ["Accessibility", "Screen Recording"] {
        let row = page.text("list").lines().find(|row| row.trim_start().starts_with(name)).unwrap_or_else(|| panic!("no row for {name} {page}")).to_string();
        let stands = row.trim_start().trim_start_matches(name).trim_start();
        assert!(stands == "on" || stands.starts_with("off · needed"), "the row for {name} says how it stands: {row}");
    }

    // What is missing is said on the notice line, and a run on a folder of its own asks macOS for
    // nothing and opens nothing: no dialog, no System Settings.
    let missing: Vec<&str> = ["Accessibility", "Screen Recording"].into_iter().filter(|name| page.text("list").lines().any(|row| row.trim_start().starts_with(name) && row.contains("off · needed"))).collect();
    let page = seen.at("switched");
    match missing.as_slice() {
        [] => assert!(!page.has("notice", "turn on Null"), "{page}"),
        missing => {
            assert_eq!(page.text("notice"), format!("turn on Null under {} in System Settings", missing.join(" and under ")), "{page}");
            assert!(seen.logged(&format!("{} is off; a run on a folder of its own asks macOS for nothing", missing[0])));
        }
    }
    assert!(!seen.logged("asking macOS for it"), "a scripted run asked macOS for a permission");
}

#[test]
#[ignore]
fn set_to_ask_the_box_shows_the_agent_s_question_and_carries_the_answer() {
    let mac = Mac::on("ok");
    // The harness of this run is set to ask before a tool acts, and Null passes that on.
    mac.keep("agent/config.yml", "tools:\n  approvalMode: always-ask\n");
    // The stand-in is told to ask the harness's shell for a command, as a model would.
    let lines = [
        "ask the shell: touch one &",
        "<until shows Your agent asks: touch one>",
        "<show asked>",
        "<key 3>",
        "<settle>",
        "<show refused>",
        "ask the shell: touch two &",
        "<until shows Your agent asks: touch two>",
        "<key 1>",
        "<settle>",
        "<show allowed>",
    ];
    let seen = mac.run(&lines);
    let made = |file: &str| std::path::Path::new(&mac.place(&format!("null/Workspace/{file}"))).exists();

    let page = seen.at("asked");
    assert!(page.text("arrow").contains("asking") && page.is("busy"), "the arrow says that the agent waits for an answer {page}");
    assert!(page.text("pet").contains("ask"), "and so does the pet {page}");
    for answer in ["1 Allow once", "2 Always allow", "3 Reject", "4 Always reject"] {
        assert!(page.has("shows", answer), "the answers the harness offers, numbered: {answer} {page}");
    }

    // Answered no with its number, the command is not run, and the reply ends.
    let page = seen.at("refused");
    assert!(!made("one"), "answered no, the shell acted all the same {page}");
    assert!(page.has("shows", "pong") && !page.is("busy"), "{page}");

    // Answered yes, it is.
    let page = seen.at("allowed");
    assert!(made("two"), "answered yes, the shell did not act {page}");
    assert!(!page.is("busy"), "{page}");
}

#[test]
#[ignore]
fn computer_use_is_not_changed_while_a_reply_is_under_way() {
    let working = format!("{} &", long(2));
    let lines = [working.as_str(), "<type /computer>", "<key Enter>", "<until notice changing computer use>", "<show computer>", "<type >", "<key ctrl+c>", "<settle>"];
    let seen = Mac::on("slow").run(&lines);
    let page = seen.at("computer");
    assert_eq!(page.text("notice"), "stop the reply with ctrl+c before changing computer use", "{page}");
    assert!(page.text("list").is_empty() && page.is("busy"), "no list is opened, and the reply goes on {page}");
}

#[test]
#[ignore]
fn update_says_when_nothing_is_newer_and_when_a_newer_null_is_out() {
    let mac = Mac::on("slow");
    mac.keep("null-v9.9.9", "");
    // Made-up places to ask, so that nothing outside this Mac is reached.
    let mac = mac.with("NULL_MINI_UPDATES", format!("file://{}/Engine.toml", env!("CARGO_MANIFEST_DIR")));
    let newest = format!("file://{}", mac.place("null-v9.9.9"));
    let mac = mac.with("NULL_MINI_RELEASE", newest);
    let seen = mac.run(&["/update", "<until shows Null 9.9.9 is out>", "<show update>"]);
    let page = seen.at("update");
    assert!(page.has("shows", &format!("already on the newest checked Oh-my-pi, {}", carried())), "{page}");
    assert!(page.has("shows", &format!("Null 9.9.9 is out, and this is Null {}", env!("CARGO_PKG_VERSION"))), "{page}");
    assert!(page.has("shows", "Mini/Scripts/null | sh -s install"), "what gets it {page}");
}

// ── Files and folders ─────────────────────────────────────────────────────

#[test]
#[ignore]
fn a_file_goes_with_a_message_and_the_agent_reads_it() {
    let mac = Mac::on("ok");
    mac.keep("note.txt", "The marked word is quillfeather.\n");
    let attach = format!("/attach {}", mac.place("note.txt"));
    let lines = [
        attach.as_str(),
        "<until notice attached: note.txt>",
        "what does the note say",
        "<settle>",
        "<show sent>",
        "/attach /no/such/file.txt",
        "<until notice no such file>",
        "<show missing>",
        "<type >",
        "<click #attach>",
        "<until list note.txt>",
        "<show choosing>",
        "<key Escape>",
    ];
    let seen = mac.run(&lines);
    let page = seen.at("sent");
    assert_eq!(page.parts(), "you,steps,reply", "{page}");
    assert!(page.has("shows", "attached: note.txt") && page.has("shows", "read the attached file") && page.has("shows", "pong"), "{page}");
    let page = seen.at("missing");
    assert_eq!(page.text("notice"), "no such file: /no/such/file.txt", "{page}");
    assert_eq!(page.text("field"), "/attach /no/such/file.txt", "left to be put right {page}");
    assert_eq!(seen.at("choosing").text("notice"), "enter opens a folder or attaches a file. esc is done");
    assert!(seen.logged("one file attached to the message") && !seen.logged("note.txt") && !seen.logged("quillfeather"), "the log says a file went, and not which");
}

#[test]
#[ignore]
fn a_conversation_is_started_in_a_folder_and_history_says_where() {
    let mac = Mac::on("slow");
    mac.keep("proj/README.md", "# A project\n");
    let folder = format!("/folder {}", mac.place("proj"));
    let lines = [
        "first in the workspace",
        "<settle>",
        folder.as_str(),
        "<until notice /proj>",
        "<show started>",
        "second in the project",
        "<settle>",
        "/history",
        "<until list second in the project>",
        "<show history>",
        "<key Tab>",
        "<show by-folder>",
        "<key Escape>",
        "<until no list>",
        "/folder /no/such/folder",
        "<until notice is not a folder>",
        "<show missing>",
    ];
    let seen = mac.run(&lines);
    let page = seen.at("started");
    assert!(page.text("notice").starts_with("in ") && page.text("notice").ends_with("/proj"), "the box says where {page}");
    assert!(page.text("shows").is_empty(), "a new conversation {page}");
    let page = seen.at("history");
    assert!(page.has("list", "proj ·") && page.has("list", "first in the workspace"), "{page}");
    assert_eq!(page.text("notice"), "tab sorts by folder", "{page}");
    let page = seen.at("by-folder");
    assert!(page.has("list", "the box") && page.has("list", "/proj"), "{page}");
    assert_eq!(page.text("notice"), "tab sorts by when", "{page}");
    assert_eq!(seen.at("missing").text("notice"), "/no/such/folder is not a folder");
    assert!(seen.log.lines().any(|line| line.starts_with("opened the conversation") && line.ends_with("/proj")), "the agent works in that folder:\n{}", seen.log);
}

// ── Earlier conversations ─────────────────────────────────────────────────

#[test]
#[ignore]
fn earlier_conversations_are_listed_renamed_and_found_by_what_was_said_in_them() {
    let mac = Mac::on("slow");
    let lines = [
        "alpha first line",
        "<settle>",
        "the hidden word is quillon",
        "<settle>",
        "/new",
        "beta other talk",
        "<settle>",
        "/rename Maple talk",
        "<until notice renamed: Maple talk>",
        "/history",
        "<until list alpha first line>",
        "<show listed>",
        "<key ArrowDown>",
        "<key meta+r>",
        "<until notice enter keeps the title>",
        "<show renaming>",
        "<type Cedar talk>",
        "<key Enter>",
        "<until list Cedar talk>",
        "<show renamed>",
        "<type quillon>",
        "<until list said inside>",
        "<show found>",
        "<type zzzznothing>",
        "<until list no conversation matches>",
        "<key Escape>",
    ];
    let seen = mac.run(&lines);
    let page = seen.at("listed");
    assert!(page.has("list", "today") && page.has("list", "alpha first line"), "{page}");
    assert!(page.has("chosen", "Maple talk"), "the open one stands first, under the title it was given {page}");
    assert_eq!(seen.at("renaming").text("field"), "alpha first line", "the title to change is in the field");
    let page = seen.at("renamed");
    assert!(page.has("list", "Cedar talk") && !page.has("list", "alpha first line"), "{page}");
    let page = seen.at("found");
    assert!(page.has("list", "Cedar talk") && page.has("list", "the hidden word is quillon"), "found by a word that is in no title {page}");
    let kept: Value = serde_json::from_str(&mac.kept("null/history.json")).unwrap();
    let renamed: Vec<&str> = kept["renamed"].as_object().unwrap().values().filter_map(Value::as_str).collect();
    assert!(renamed.contains(&"Maple talk") && renamed.contains(&"Cedar talk"), "the titles are kept: {kept}");
    for typed in ["quillon", "zzzznothing", "Maple talk", "Cedar talk"] {
        assert!(!seen.logged(typed), "a title or a word looked for is in the log: {typed}\n{}", seen.log);
    }
}

#[test]
#[ignore]
fn a_conversation_is_moved_to_the_trash_and_leaves_the_list() {
    let lines = [
        "alpha cedar message",
        "<settle>",
        "/new",
        "beta maple message",
        "<settle>",
        "/history",
        "<until list alpha cedar message>",
        "<key ArrowDown>",
        "<key meta+Backspace>",
        "<until notice moved to the Trash: alpha cedar message>",
        "<show deleted>",
        "<key Escape>",
    ];
    let seen = Mac::on("slow").run(&lines);
    let page = seen.at("deleted");
    assert!(!page.has("list", "alpha cedar message") && page.has("list", "beta maple message"), "{page}");
    assert!(seen.logged("a conversation was moved to the Trash"));
}

/// The harness lists a conversation from the first 4 KB of its file and counts
/// the messages it finds there (Oh-my-pi 18.8.7). A conversation begun with a
/// skill has the skill's words first, and with a skill as long as most are,
/// that 4 KB holds no message: it is counted at none, and Null took that for a
/// conversation in which nothing was said. It went from `/history` while it
/// was still working, and could not be opened again.
#[test]
#[ignore]
fn a_conversation_begun_with_a_skill_is_in_history() {
    let mac = Mac::on("ok");
    // A skill of the run's own, longer than the harness reads of a conversation to list it.
    let words = vec![WORDS; 40].join("\n");
    mac.keep("agent/skills/box-check-long/SKILL.md", &format!("---\nname: box-check-long\ndescription: A long skill. Made by the box check.\n---\n\nSay pong.\n\n{words}\n"));
    let lines = ["<type />", "<until list /box-check-long>", "<key Escape>", "<until no list>", "/box-check-long cedar maple", "<settle>", "/new", "beta maple message", "<settle>", "/history", "<until list beta maple message>", "<show listed>", "<key Escape>"];
    let seen = mac.run(&lines);
    let page = seen.at("listed");
    assert!(page.has("list", "box-check-long cedar maple"), "the conversation begun with a skill is listed {page}");
}

// ── Work in the background ────────────────────────────────────────────────

#[test]
#[ignore]
fn a_conversation_left_working_goes_on_and_history_lists_it_first() {
    let working = format!("{} &", long(3));
    let lines = [working.as_str(), "/new", "<until notice still working. /history shows it>", "<show left>", "/history", "<until list working>", "<show history>", "<key Escape>"];
    let seen = Mac::on("slow").run(&lines);
    let page = seen.at("left");
    assert!(!page.is("busy") && !page.parts().contains("you"), "a new conversation, at rest {page}");
    assert_eq!(page.text("pet"), "pet work", "the pet speaks for the one left working {page}");
    let page = seen.at("history");
    assert!(page.text("list").trim_start().to_lowercase().starts_with("now"), "what is working comes first {page}");
    assert!(page.has("chosen", "working"), "{page}");
}

#[test]
#[ignore]
fn a_fifth_conversation_is_not_set_to_work_and_its_message_is_kept() {
    let working = format!("{} &", long(5));
    let lines = [working.as_str(), "/new", working.as_str(), "/new", working.as_str(), "/new", working.as_str(), "/new", "fifth message", "<until notice four conversations are already working>", "<show fifth>"];
    let seen = Mac::on("slow").run(&lines);
    let page = seen.at("fifth");
    assert_eq!(page.text("notice"), "four conversations are already working. wait for one to end, or stop one", "{page}");
    assert_eq!(page.text("field"), "fifth message", "nothing was sent, so it is there to send later {page}");
    assert!(!page.is("busy"), "{page}");
}

#[test]
#[ignore]
fn a_message_typed_while_a_reply_is_under_way_is_kept() {
    let working = format!("{} &", long(2));
    let lines = [working.as_str(), "<type queued while busy>", "<key Enter>", "<until notice still working. ctrl+c stops it>", "<show typed>", "<type >", "<key ctrl+c>", "<settle>"];
    let seen = Mac::on("slow").run(&lines);
    let page = seen.at("typed");
    assert_eq!(page.text("field"), "queued while busy", "{page}");
    assert!(page.is("busy") && !page.has("shows", "queued while busy"), "nothing was sent {page}");
}

#[test]
#[ignore]
fn neither_the_model_nor_the_backups_are_changed_while_a_reply_is_under_way() {
    let working = format!("{} &", long(2));
    let lines = [
        working.as_str(),
        "<type /model>",
        "<key Enter>",
        "<until notice switching models>",
        "<show model>",
        "<type /backup>",
        "<key Enter>",
        "<until notice changing the backups>",
        "<show backups>",
        "<type >",
        "<key ctrl+c>",
        "<settle>",
    ];
    let seen = Mac::on("slow").run(&lines);
    let page = seen.at("model");
    assert_eq!(page.text("notice"), "stop the reply with ctrl+c before switching models", "{page}");
    assert!(page.text("list").is_empty() && page.is("busy"), "no list is opened, and the reply goes on {page}");
    let page = seen.at("backups");
    assert_eq!(page.text("notice"), "stop the reply with ctrl+c before changing the backups", "{page}");
    assert!(page.text("list").is_empty() && page.is("busy"), "{page}");
}

#[test]
#[ignore]
fn work_cut_off_by_a_restart_is_found_again() {
    let mac = Mac::on("slow");
    let working = format!("{} &", long(3));
    // Null goes away while a conversation is working, as at a restart.
    mac.run(&[working.as_str(), "/new", "<until notice still working. /history shows it>"]);
    let seen = mac.run(&["/history", "<until list cut off>", "<show history>", "<key Enter>", "<settle>", "<until shows alder birch>", "<show opened>"]);
    let page = seen.at("history");
    assert!(page.text("list").trim_start().to_lowercase().starts_with("now") && page.has("chosen", "cut off"), "{page}");
    assert_eq!(page.text("pet"), "pet fail", "the pet says so before the list is opened {page}");
    assert_eq!(seen.at("opened").parts(), "you", "what was asked is there, and the reply that was cut off is not");
}

// ── Stopping, quitting, starting again ────────────────────────────────────

#[test]
#[ignore]
fn ctrl_c_stops_a_reply_and_with_nothing_running_quits() {
    let mac = Mac::on("slow");
    let working = format!("{} &", long(2));
    let seen = mac.run(&[working.as_str(), "<until shows now on standin-anthropic/slow>", "<wait 1000>", "<key ctrl+c>", "<settle>", "<show stopped>", "<key ctrl+c>"]);
    assert!(!seen.at("stopped").is("busy"));
    assert!(seen.logged("reply ended: cancelled") && seen.logged("quit from the box; the conversation is closed"), "{}", seen.log);
    let settings: Value = serde_json::from_str(&mac.kept("null/settings.json")).unwrap();
    assert!(settings["session"].is_null(), "the next start is a new conversation: {settings}");
}

#[test]
#[ignore]
fn quitting_asks_first_when_a_conversation_is_still_working() {
    let working = format!("{} &", long(3));
    let seen = Mac::on("slow").run(&[working.as_str(), "/new", "/quit", "<until notice /quit again>", "<show asked>", "/quit"]);
    assert_eq!(seen.at("asked").text("notice"), "1 conversation is still working. /quit again stops it and quits.");
    assert!(seen.logged("quit from the box; the conversation is closed, and what was working is stopped"), "{}", seen.log);
}

#[test]
#[ignore]
fn after_a_restart_the_conversation_is_shown_before_a_new_message() {
    let mac = Mac::on("slow");
    mac.run(&["remember the word maple", "<settle>"]);
    let seen = mac.run(&["what was the word", "<settle>", "<show after>"]);
    let page = seen.at("after");
    assert_eq!(page.parts(), "you,reply,you,reply", "{page}");
    assert!(page.text("shows").starts_with("remember the word maple"), "what was said before stands first {page}");
    assert!(seen.logged("loaded the conversation"));
}

// ── The first opening and signing in ──────────────────────────────────────

#[test]
#[ignore]
fn the_first_opening_leads_to_the_sign_in_once() {
    let mac = Mac::new();
    let seen = mac.run(&["<until list sign in to>", "<show welcome>", "<key Escape>", "<until no list>", "<show later>"]);
    let page = seen.at("welcome");
    assert_eq!(page.text("notice"), "no provider is signed in. choose one, or esc for later", "{page}");
    assert!(page.has("list", "Anthropic"), "{page}");
    let page = seen.at("later");
    assert!(page.has("shows", "no provider is signed in. /login signs in") && page.has("shows", "the agent may act without asking you first"), "{page}");
    assert!(seen.logged("no provider is signed in; opening the box to say so, this once"));
    let seen = mac.run(&["<wait 2500>", "<show again>"]);
    assert!(seen.at("again").text("list").is_empty(), "it is said once");
    assert!(!seen.logged("opening the box to say so"));
}

#[test]
#[ignore]
fn the_list_of_providers_keeps_the_transcript_behind_it_and_narrows_as_a_name_is_typed() {
    let lines = ["/login", "<until list sign in to>", "<until shows now on standin-anthropic/slow>", "<show providers>", "<type deep>", "<until list deepseek>", "<show narrowed>", "<key Escape>", "<until no list>", "<show cancelled>"];
    let seen = Mac::on("slow").run(&lines);
    let page = seen.at("providers");
    assert_eq!(page.number("transcript"), 0.0, "a line that arrives meanwhile stays behind the list {page}");
    let page = seen.at("narrowed");
    assert!(page.has("list", "DeepSeek") && !page.has("list", "Anthropic"), "{page}");
    let page = seen.at("cancelled");
    assert!(page.number("transcript") > 0.0 && page.has("shows", "now on standin-anthropic/slow"), "{page}");
}

// ── The pet ───────────────────────────────────────────────────────────────

#[test]
#[ignore]
fn the_pet_is_put_away_and_shown_again() {
    let mac = Mac::on("slow");
    let seen = mac.run(&["<show first>", "/pet", "<show away>"]);
    assert!(seen.at("first").is("petShown") && !seen.at("away").is("petShown"));
    assert!(seen.logged("the pet is put away"));
    // It stays away at the next start, and comes back when asked.
    let seen = mac.run(&["<show still-away>", "/pet", "<show back>"]);
    assert!(!seen.at("still-away").is("petShown") && seen.at("back").is("petShown"));
    assert!(seen.logged("the pet is shown"));
}

/// The pet staying alone is a prototype (`src/panel.rs`). This goes with it, or
/// is written again, when that is decided.
#[test]
#[ignore]
fn with_the_box_put_away_the_pet_stays_while_work_goes_on() {
    let working = format!("{} &", long(2));
    let seen = Mac::on("slow").run(&[working.as_str(), "<wait 800>", "<away>", "<show alone>", "<pet>", "<show back>", "<key ctrl+c>", "<settle>"]);
    assert_eq!(seen.at("alone").text("pet"), "pet work");
    assert!(seen.logged("the box is put away (esc); the pet stays") && seen.logged("the pet stands alone") && seen.logged("shown, back from the pet alone"), "{}", seen.log);
}
