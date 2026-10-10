//! Which Oh-my-pi the app runs.
//!
//! Null carries its own: one version, named in `Engine.toml`, which the build
//! takes into the app and puts beside Null's own program. That is the one it
//! starts, so nothing has to be installed first, and an update of an Oh-my-pi
//! elsewhere on the Mac changes nothing here. Someone who would sooner run their
//! own chooses it with `/harness`; the box then says which version that is, and
//! that Null was not checked with it when it is another one.
//!
//! Only the program is Null's. The folder it works from, with the user's
//! sign-ins, settings, skills and MCP servers, is Oh-my-pi's ordinary one
//! whichever program runs.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

use serde::Serialize;
use tauri::AppHandle;

use crate::harness::{self, HARNESS, HARNESS_NAME};
use crate::log::log;
use crate::{providers, settings, translate};

/// Where a program came from.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// It came with Null.
    BuiltIn,
    /// It is the user's own.
    Own,
}

/// One Oh-my-pi the box can run, as `/harness` lists it.
#[derive(Clone, Debug, Serialize)]
pub struct Choice {
    pub origin: Origin,
    pub path: String,
    /// What it says its version is, when it says.
    pub version: Option<String>,
    /// Whether that is the version Null carries and was checked with.
    pub checked: bool,
    pub current: bool,
}

/// The version of Oh-my-pi that Null carries and was checked with.
pub fn carried_version() -> &'static str {
    version_named(include_str!("../Engine.toml")).unwrap_or_default()
}

/// The version `Engine.toml` names.
fn version_named(toml: &str) -> Option<&str> {
    toml.lines().find_map(|line| line.strip_prefix("version")?.trim_start().strip_prefix('=')?.trim_start().strip_prefix('"')?.split('"').next())
}

/// The version in what a program prints for `--version`: `omp/18.4.3`.
fn version_from(printed: &str) -> Option<String> {
    let version = printed.split_whitespace().next()?.rsplit('/').next()?;
    version.starts_with(|c: char| c.is_ascii_digit()).then(|| version.to_string())
}

fn version_of(binary: &Path) -> Option<String> {
    let output = Command::new(binary).arg("--version").output().ok()?;
    version_from(&String::from_utf8_lossy(&output.stdout))
}

/// The Oh-my-pi that came with Null, when it is there to run. The build puts it
/// beside Null's own program, in the app and in a development run alike.
pub fn built_in() -> Option<PathBuf> {
    // A test is a program of its own in another folder; it takes the copy the build was given.
    #[cfg(test)]
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/Engine/omp-", env!("TAURI_ENV_TARGET_TRIPLE")));
    #[cfg(not(test))]
    let path = std::env::current_exe().ok()?.parent()?.join(HARNESS);
    translate::runnable(&path).then_some(path)
}

/// The program the live checks run (`cargo test -- --ignored`): the one
/// `NULL_MINI_ENGINE` names, for trying a version Null does not carry yet, else
/// the one it carries.
#[cfg(test)]
pub fn under_test() -> PathBuf {
    match std::env::var_os("NULL_MINI_ENGINE").map(PathBuf::from) {
        Some(named) => {
            assert!(translate::runnable(&named), "NULL_MINI_ENGINE names {}, which cannot be run", named.display());
            named
        }
        None => built_in().expect("Mini/Scripts/engine has fetched Oh-my-pi"),
    }
}

/// An Oh-my-pi of the user's own on this Mac: on the `PATH`, or where it installs itself.
fn found() -> Option<PathBuf> {
    translate::find_installed(HARNESS, None)
}

/// The program to run and where it came from: the user's own when they chose
/// one and it is still there, else the one that came with Null. None when there
/// is none at all.
pub fn in_use(app: &AppHandle) -> Option<(PathBuf, Origin)> {
    if let Some(own) = settings::get(app).harness.map(PathBuf::from).filter(|path| translate::runnable(path)) {
        return Some((own, Origin::Own));
    }
    if let Some(carried) = built_in() {
        return Some((carried, Origin::BuiltIn));
    }
    // Only a Null that was put together without its Oh-my-pi gets this far.
    let own = found()?;
    static SAID: Once = Once::new();
    SAID.call_once(|| log!("no {HARNESS_NAME} came with this Null; using the one at {}", own.display()));
    Some((own, Origin::Own))
}

fn same_file(one: &Path, other: &Path) -> bool {
    one == other || matches!((std::fs::canonicalize(one), std::fs::canonicalize(other)), (Ok(one), Ok(other)) if one == other)
}

fn choice(path: &Path, origin: Origin, current: bool) -> Choice {
    let version = version_of(path);
    Choice { origin, path: path.display().to_string(), checked: version.as_deref() == Some(carried_version()), version, current }
}

/// The programs the box can run: the one that came with Null, then the user's own.
#[tauri::command(async)]
pub fn harnesses(app: AppHandle) -> Vec<Choice> {
    let mut listed: Vec<(PathBuf, Origin)> = built_in().map(|path| (path, Origin::BuiltIn)).into_iter().collect();
    for own in settings::get(&app).harness.map(PathBuf::from).into_iter().chain(found()) {
        if translate::runnable(&own) && !listed.iter().any(|(known, _)| same_file(known, &own)) {
            listed.push((own, Origin::Own));
        }
    }
    let current = in_use(&app).map(|(path, _)| path);
    listed.iter().map(|(path, origin)| choice(path, *origin, current.as_deref() == Some(path))).collect()
}

/// Choose which Oh-my-pi the box runs: the user's own by its path, or with no
/// path the one that came with Null. The one that is running is let go; the next
/// message starts the chosen one and the conversation is loaded back.
#[tauri::command(async)]
pub fn set_harness(app: AppHandle, path: Option<String>) -> Result<Choice, String> {
    if harness::busy(&app) {
        return Err("stop the reply before changing the harness".into());
    }
    match &path {
        Some(own) if !translate::runnable(Path::new(own)) => return Err(format!("{own} is not a program that can be run")),
        None if built_in().is_none() => return Err(format!("no {HARNESS_NAME} came with this Null")),
        _ => {}
    }
    settings::update(&app, |settings| settings.harness = path.clone());
    harness::restart(&app);
    providers::forget(&app);
    let (binary, origin) = in_use(&app).ok_or(format!("{HARNESS_NAME} is not installed"))?;
    let chosen = choice(&binary, origin, true);
    log!("the harness is now {}, version {}", binary.display(), chosen.version.as_deref().unwrap_or("unknown"));
    Ok(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_null_carries_is_read_from_engine_toml() {
        assert_eq!(version_named("# a note\n\nversion = \"18.4.3\"\n\n[sha256]\nomp-darwin-arm64 = \"13\"\n"), Some("18.4.3"));
        assert_eq!(version_named("[sha256]\nomp-darwin-arm64 = \"13\"\n"), None);
        // The file in the repo names one, in numbers and dots.
        assert!(carried_version().split('.').count() >= 2 && carried_version().chars().all(|c| c.is_ascii_digit() || c == '.'), "{:?}", carried_version());
    }

    #[test]
    fn a_program_s_version_is_read_from_what_it_prints() {
        assert_eq!(version_from("omp/18.4.3\n").as_deref(), Some("18.4.3"));
        assert_eq!(version_from("18.8.7").as_deref(), Some("18.8.7"));
        assert_eq!(version_from("error: unknown option\n"), None);
        assert_eq!(version_from(""), None);
    }

    #[test]
    fn two_names_for_one_file_are_the_same_program() {
        let folder = std::env::temp_dir().join(format!("null-engine-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let program = folder.join("omp");
        std::fs::write(&program, "").unwrap();
        let link = folder.join("link");
        std::os::unix::fs::symlink(&program, &link).unwrap();

        assert!(same_file(&program, &link));
        assert!(!same_file(&program, &folder.join("missing")));
        std::fs::remove_dir_all(&folder).unwrap();
    }

    /// Needs the Oh-my-pi that `Mini/Scripts/engine` fetches, and runs it, so it
    /// runs only when asked: `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn the_program_the_build_was_given_is_the_version_engine_toml_names() {
        let carried = built_in().expect("Mini/Scripts/engine has fetched Oh-my-pi");
        assert_eq!(version_of(&carried).as_deref(), Some(carried_version()));
    }
}
