//! Null's rules: what Null tells the agent of how to work this Mac well.
//!
//! The words are Null's own, the same for every person, carried inside the
//! app, and about the ways in and the habits that make the agent quick and
//! careful, never about one site. They are handed to the harness at start
//! as text to add to what it tells the model, and only while computer use is
//! on: they are about the screen, the apps and the browser. They go in a file
//! of Null's own beside its settings, written afresh at every start, so the
//! person can read what the agent is told and the log never holds the words.
//! The person's own rules page in a folder is theirs and is left alone; the
//! harness reads it as before.

use std::path::Path;

use tauri::AppHandle;

use crate::log::log;
use crate::settings;

/// How to work this Mac: the route, few turns, the browser, the apps, and the person's things.
pub const COMPUTER: &str = include_str!("../Rules/Computer.md");

/// What Null tells the agent: the rules for computer use while it is on, else nothing.
pub fn text(computer: bool) -> Option<&'static str> {
    computer.then_some(COMPUTER)
}

/// What to add when starting the harness so that it tells the model Null's
/// rules: the file they are written to. Nothing when there is nothing to tell,
/// and then the file of an earlier start is taken away.
pub fn args(app: &AppHandle, computer: bool) -> Vec<String> {
    let Some(dir) = settings::dir(app) else { return Vec::new() };
    handed(&dir, text(computer))
}

/// Write the rules into `dir` and name the file, or take the file away when there are none.
fn handed(dir: &Path, rules: Option<&str>) -> Vec<String> {
    let path = dir.join(settings::file_name("rules", "md"));
    let Some(rules) = rules else {
        let _ = std::fs::remove_file(&path);
        return Vec::new();
    };
    let _ = std::fs::create_dir_all(dir);
    match std::fs::write(&path, rules) {
        Ok(()) => {
            log!("the agent is told Null's rules for working this Mac");
            vec!["--append-system-prompt".into(), path.display().to_string()]
        }
        Err(e) => {
            log!("could not write Null's rules to {}: {e}", path.display());
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rules_are_handed_over_only_while_computer_use_is_on_and_go_with_it() {
        let dir = std::env::temp_dir().join(format!("null-rules-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let on = handed(&dir, text(true));
        let path = dir.join(settings::file_name("rules", "md"));
        assert_eq!(on, vec!["--append-system-prompt".to_string(), path.display().to_string()]);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), COMPUTER);
        assert!(handed(&dir, text(false)).is_empty(), "with computer use off, the start line names rules");
        assert!(!path.exists(), "with computer use off, the rules of an earlier start are left for the harness");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
