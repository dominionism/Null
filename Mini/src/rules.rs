//! Null's rules: what Null tells the agent of how to work this Mac well.
//!
//! The words are Null's own, the same for every person, carried inside the
//! app, and about the ways in and the habits that make the agent quick and
//! careful, never about one site. They are handed to the harness at start
//! as text to add to what it tells the model, each part only while what it is
//! about is switched on: computer use (`/computer`), and the browser link
//! (`/browser`). They go in a file of Null's own beside its settings, written
//! afresh at every start, so the person can read what the agent is told and
//! the log never holds the words. The person's own rules page in a folder is
//! theirs and is left alone; the harness reads it as before.

use std::path::Path;

use tauri::AppHandle;

use crate::log::log;
use crate::settings;

/// How to work this Mac: the route, few turns, the browser, the apps, and the person's things.
pub const COMPUTER: &str = include_str!("../Rules/Computer.md");

/// How to work the person's own Chrome through the browser link.
pub const BROWSER: &str = include_str!("../Rules/Browser.md");

/// What Null tells the agent, part by part: the rules for computer use while
/// it is on, and those for the browser link while it is on.
pub fn parts(computer: bool, browser: bool) -> Vec<(&'static str, &'static str)> {
    [(computer, "computer use", COMPUTER), (browser, "the browser link", BROWSER)].into_iter().filter(|(on, ..)| *on).map(|(_, name, words)| (name, words)).collect()
}

/// What to add when starting the harness so that it tells the model Null's
/// rules: the file they are written to. Nothing when there is nothing to tell,
/// and then the file of an earlier start is taken away.
pub fn args(app: &AppHandle, computer: bool, browser: bool) -> Vec<String> {
    let Some(dir) = settings::dir(app) else { return Vec::new() };
    handed(&dir, &parts(computer, browser))
}

/// Write the rules into `dir` and name the file, or take the file away when there are none.
fn handed(dir: &Path, parts: &[(&str, &str)]) -> Vec<String> {
    let path = dir.join(settings::file_name("rules", "md"));
    if parts.is_empty() {
        let _ = std::fs::remove_file(&path);
        return Vec::new();
    }
    let words: Vec<&str> = parts.iter().map(|(_, words)| *words).collect();
    let _ = std::fs::create_dir_all(dir);
    match std::fs::write(&path, words.join("\n")) {
        Ok(()) => {
            let names: Vec<&str> = parts.iter().map(|(name, _)| *name).collect();
            log!("the agent is told Null's rules for {}", names.join(" and "));
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
    fn each_part_of_the_rules_is_handed_over_only_while_what_it_is_about_is_on() {
        let dir = std::env::temp_dir().join(format!("null-rules-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(settings::file_name("rules", "md"));
        let told = |computer, browser| {
            let args = handed(&dir, &parts(computer, browser));
            (args, std::fs::read_to_string(&path).ok())
        };

        let (args, words) = told(true, false);
        assert_eq!(args, vec!["--append-system-prompt".to_string(), path.display().to_string()]);
        assert_eq!(words.as_deref(), Some(COMPUTER));

        let (_, words) = told(false, true);
        assert_eq!(words.as_deref(), Some(BROWSER), "the link's rules go without computer use's");

        let words = told(true, true).1.unwrap_or_default();
        assert!(words.starts_with(COMPUTER) && words.ends_with(BROWSER), "both, computer use's first: {words}");

        let (args, words) = told(false, false);
        assert!(args.is_empty(), "with both off, the start line names rules");
        assert_eq!(words, None, "with both off, the rules of an earlier start are left for the harness");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
