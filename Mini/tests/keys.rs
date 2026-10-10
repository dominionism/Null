//! A key typed during a sign-in goes to the harness's own sign-in and nowhere
//! else: not into Null's log, not into its settings, and not onto the page.
//!
//! This starts the real app on a folder of its own and has it type `/login`,
//! choose a provider that asks for a key, and type a marked key that is no
//! key. The provider refuses it. Then everything Null wrote and printed is
//! searched for the mark.
//!
//! It runs only when asked (`cargo test -- --ignored`). It opens the box on
//! the screen for a few seconds, and it reaches DeepSeek once, with the dummy.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Every file under `folder`.
fn files(folder: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files(&path));
        } else {
            found.push(path);
        }
    }
    found
}

fn holds(file: &Path, mark: &str) -> bool {
    std::fs::read(file).is_ok_and(|bytes| String::from_utf8_lossy(&bytes).contains(mark))
}

#[test]
#[ignore]
fn a_key_typed_during_a_sign_in_reaches_neither_the_log_nor_the_settings() {
    let folder = std::env::temp_dir().join(format!("null-key-check-{}", std::process::id()));
    let own = folder.join("null");
    std::fs::create_dir_all(&own).unwrap();
    // Not a first opening: the sign-in is to be asked for by typing, as a person does later.
    std::fs::write(own.join("settings.json"), r#"{"welcomed":true}"#).unwrap();
    let key = format!("sk-null-marked-{}-not-a-key", std::process::id());

    let run = Command::new(env!("CARGO_BIN_EXE_null-mini"))
        .env("NULL_MINI_FOLDER", &folder)
        .env("NULL_MINI_NO_SHORTCUT", "1")
        .env("NULL_MINI_SELFTEST", format!("/login\ndeepseek\n{key}"))
        .output()
        .expect("Null starts");
    let printed = String::from_utf8_lossy(&run.stderr).into_owned() + &String::from_utf8_lossy(&run.stdout);

    // The sign-in really ran, and the key really went to the harness, which refused it.
    assert!(printed.contains("a sign-in started"), "no sign-in was started:\n{printed}");
    assert!(printed.contains("the sign-in ended: not signed in"), "the sign-in did not end with the key refused:\n{printed}");
    assert!(printed.contains("selftest: the page shows:"), "the page never said what it shows:\n{printed}");

    // What Null printed, which is also its log and what the page showed.
    assert!(!printed.contains(&key), "the key is in what Null printed");
    // What Null keeps: its log, its settings, the settings it hands the harness.
    let kept = files(&own);
    assert!(kept.iter().any(|file| file.ends_with("mini.log")), "Null's log is not in {}", own.display());
    for file in &kept {
        assert!(!holds(file, &key), "the key is in {}", file.display());
    }

    let _ = std::fs::remove_dir_all(&folder);
}
