//! Which Oh-my-pi the app runs.
//!
//! Null carries its own: one version, named in `Engine.toml`, which the build
//! takes into the app and puts beside Null's own program. That is the one it
//! starts, so nothing has to be installed first, and an update of an Oh-my-pi
//! elsewhere on the Mac changes nothing here. Someone who would sooner run their
//! own chooses it with `/harness`; the box then says which version that is, and
//! that Null was not checked with it when it is another one.
//!
//! `/update` moves the built-in harness forward when the user asks: to the
//! newest version the repository's harness check has passed, never to whatever
//! is newest. That version is kept in Null's support folder, outside the app,
//! whose signature covers the program it came with.
//!
//! Only the program is Null's. The folder it works from, with the user's
//! sign-ins, settings, skills and MCP servers, is Oh-my-pi's ordinary one
//! whichever program runs.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::harness::{self, HARNESS, HARNESS_NAME};
use crate::log::log;
use crate::{panel, providers, settings, translate};

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

/// Where `/update` reads which version the repository has checked.
/// `NULL_MINI_UPDATES` names another place, for a scripted check with a made-up file.
const UPDATES: &str = "https://raw.githubusercontent.com/dominionism/Null/main/Mini/Engine.toml";
const RELEASES: &str = "https://github.com/can1357/oh-my-pi/releases/download";
/// Oh-my-pi's name for the program for the kind of Mac Null is built for.
const PUBLISHED: &str = "omp-darwin-arm64";

/// Where `/update` learns which Null is the newest. The page of the newest
/// release sends on to that release, whose address ends in its tag; asked this
/// way, GitHub sets no limit on how often. `NULL_MINI_RELEASE` names another
/// address, for a scripted check with a made-up one.
const NEWEST_NULL: &str = "https://github.com/dominionism/Null/releases/latest";
/// What gets the newest Null, for the box to show. Null does not replace itself.
const INSTALL: &str = "curl -fsSL https://raw.githubusercontent.com/dominionism/Null/main/Mini/Scripts/null | sh -s install";

/// The version of Oh-my-pi that Null carries and was checked with.
pub fn carried_version() -> &'static str {
    named(include_str!("../Engine.toml"), "version").unwrap_or_default()
}

/// One value of an `Engine.toml`: the version, the Null it needs, or a file's checksum.
fn named<'a>(toml: &'a str, name: &str) -> Option<&'a str> {
    toml.lines().find_map(|line| {
        let rest = line.strip_prefix('"').unwrap_or(line).strip_prefix(name)?;
        let rest = rest.strip_prefix('"').unwrap_or(rest).trim_start().strip_prefix('=')?.trim_start();
        rest.strip_prefix('"')?.split('"').next()
    })
}

/// Whether one version is newer than another, by their numbers: 18.10.0 is
/// newer than 18.9.2. A version that is not all numbers is never newer.
fn newer(one: &str, other: &str) -> bool {
    let numbers = |version: &str| version.split('.').map(|part| part.parse::<u64>().ok()).collect::<Option<Vec<_>>>();
    matches!((numbers(one), numbers(other)), (Some(one), Some(other)) if one > other)
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
pub fn carried() -> Option<PathBuf> {
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
        None => carried().expect("Mini/Scripts/engine has fetched Oh-my-pi"),
    }
}

/// Where a version that `/update` fetched is kept: a folder of its own in Null's
/// support folder.
fn fetched_dir(app: &AppHandle, version: &str) -> Option<PathBuf> {
    Some(settings::dir(app)?.join(settings::folder_name("Engine")).join(version))
}

/// The version `/update` fetched, and its program, while that is the built-in
/// harness: until a Null arrives that carries that version or a newer one.
fn fetched(app: &AppHandle) -> Option<(String, PathBuf)> {
    let version = settings::get(app).engine?;
    let program = fetched_dir(app, &version)?.join(HARNESS);
    (newer(&version, carried_version()) && translate::runnable(&program)).then_some((version, program))
}

/// The built-in harness: the newest checked program Null has, which is the one
/// `/update` fetched or else the one that came with the app.
pub fn built_in(app: &AppHandle) -> Option<PathBuf> {
    fetched(app).map(|(_, program)| program).or_else(carried)
}

/// Whether a version is one Null was checked with: the one it carries, or one
/// the repository checked and `/update` fetched.
pub fn checked(app: &AppHandle, version: &str) -> bool {
    version == carried_version() || fetched(app).is_some_and(|(fetched, _)| fetched == version)
}

/// Forget a version `/update` fetched once the app itself carries that version
/// or a newer one, and remove its folder.
pub fn tidy(app: &AppHandle) {
    let Some(version) = settings::get(app).engine else { return };
    if newer(&version, carried_version()) {
        return;
    }
    if let Some(folder) = fetched_dir(app, &version) {
        let _ = std::fs::remove_dir_all(folder);
    }
    settings::update(app, |settings| settings.engine = None);
    log!("the app now carries {HARNESS_NAME} {}; the {version} that /update fetched is removed", carried_version());
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
    if let Some(built_in) = built_in(app) {
        return Some((built_in, Origin::BuiltIn));
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

fn choice(app: &AppHandle, path: &Path, origin: Origin, current: bool) -> Choice {
    let version = version_of(path);
    Choice { origin, path: path.display().to_string(), checked: version.as_deref().is_some_and(|version| checked(app, version)), version, current }
}

/// The programs the box can run: the one that came with Null, then the user's own.
#[tauri::command(async)]
pub fn harnesses(app: AppHandle) -> Vec<Choice> {
    let mut listed: Vec<(PathBuf, Origin)> = built_in(&app).map(|path| (path, Origin::BuiltIn)).into_iter().collect();
    for own in settings::get(&app).harness.map(PathBuf::from).into_iter().chain(found()) {
        if translate::runnable(&own) && !listed.iter().any(|(known, _)| same_file(known, &own)) {
            listed.push((own, Origin::Own));
        }
    }
    let current = in_use(&app).map(|(path, _)| path);
    listed.iter().map(|(path, origin)| choice(&app, path, *origin, current.as_deref() == Some(path))).collect()
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
        None if built_in(&app).is_none() => return Err(format!("no {HARNESS_NAME} came with this Null")),
        _ => {}
    }
    settings::update(&app, |settings| settings.harness = path.clone());
    harness::restart(&app);
    providers::forget(&app);
    let (binary, origin) = in_use(&app).ok_or(format!("{HARNESS_NAME} is not installed"))?;
    let chosen = choice(&app, &binary, origin, true);
    log!("the harness is now {}, version {}", binary.display(), chosen.version.as_deref().unwrap_or("unknown"));
    Ok(chosen)
}

// ── /update ───────────────────────────────────────────────────────────────

/// What `/update` should do, given what the repository says it has checked.
#[derive(Debug, PartialEq)]
enum Step {
    /// Nothing: the built-in harness is already that version, or a newer one.
    Stay,
    /// The checked version needs a newer Null than this one.
    NeedsNull { version: String, null: String },
    /// Fetch this version, and keep it only if it has this checksum.
    Fetch { version: String, sha256: String },
}

/// Read the repository's `Engine.toml` against the version of the built-in
/// harness and the version of this Null.
fn step(toml: &str, built_in: &str, this_null: &str) -> Result<Step, String> {
    let version = named(toml, "version").ok_or("the repository names no checked version")?;
    if !newer(version, built_in) {
        return Ok(Step::Stay);
    }
    if let Some(null) = named(toml, "null").filter(|null| newer(null, this_null)) {
        return Ok(Step::NeedsNull { version: version.to_string(), null: null.to_string() });
    }
    let sha256 = named(toml, PUBLISHED).filter(|sum| sum.len() == 64 && sum.chars().all(|c| c.is_ascii_hexdigit()));
    let sha256 = sha256.ok_or(format!("the repository gives no checksum for {HARNESS_NAME} {version}"))?;
    Ok(Step::Fetch { version: version.to_string(), sha256: sha256.to_string() })
}

/// Run one of the Mac's own tools. Gives what it printed, or what it said went wrong.
fn tool(program: &str, arguments: &[&std::ffi::OsStr]) -> Result<String, String> {
    let output = Command::new(program).args(arguments).output().map_err(|e| format!("{program} could not be run: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Who signed a program: its Apple team, when it has one.
fn signer(program: &Path) -> Option<String> {
    let output = Command::new("/usr/bin/codesign").args(["-dv", "--verbose=2"]).arg(program).output().ok()?;
    let said = String::from_utf8_lossy(&output.stderr).into_owned();
    said.lines().find_map(|line| line.strip_prefix("TeamIdentifier=")).filter(|team| *team != "not set").map(str::to_string)
}

/// Fetch one version of Oh-my-pi into Null's support folder. It is kept only
/// if it has the checksum the repository gave, is signed by whoever signed the
/// program Null came with, and says it is that version.
fn fetch(app: &AppHandle, version: &str, sha256: &str) -> Result<PathBuf, String> {
    let folder = fetched_dir(app, version).ok_or("Null has no support folder")?;
    std::fs::create_dir_all(&folder).map_err(|e| format!("could not create {}: {e}", folder.display()))?;
    let (part, program) = (folder.join("omp.part"), folder.join(HARNESS));
    let kept = (|| {
        let address = format!("{RELEASES}/v{version}/{PUBLISHED}");
        tool("/usr/bin/curl", &["--fail", "--location", "--silent", "--show-error", "--retry", "2", "--max-time", "1800", "--output"].map(std::ffi::OsStr::new).iter().copied().chain([part.as_os_str(), address.as_ref()]).collect::<Vec<_>>())
            .map_err(|said| format!("{HARNESS_NAME} {version} could not be fetched: {said}"))?;
        let sum = tool("/usr/bin/shasum", &["-a".as_ref(), "256".as_ref(), part.as_os_str()])?;
        if sum.split_whitespace().next() != Some(sha256) {
            return Err(format!("what was fetched is not the {HARNESS_NAME} {version} the repository checked, so it was not kept"));
        }
        tool("/bin/chmod", &["755".as_ref(), part.as_os_str()])?;
        tool("/usr/bin/codesign", &["--verify".as_ref(), "--strict".as_ref(), part.as_os_str()]).map_err(|said| format!("the signature of what was fetched does not hold: {said}"))?;
        if let Some(expected) = carried().as_deref().and_then(signer) {
            if signer(&part).as_deref() != Some(expected.as_str()) {
                return Err(format!("what was fetched is not signed by whoever signed the {HARNESS_NAME} Null came with, so it was not kept"));
            }
        }
        if version_of(&part).as_deref() != Some(version) {
            return Err(format!("what was fetched does not say it is {HARNESS_NAME} {version}, so it was not kept"));
        }
        std::fs::rename(&part, &program).map_err(|e| e.to_string())
    })();
    if let Err(reason) = kept {
        let _ = std::fs::remove_dir_all(&folder);
        return Err(reason);
    }
    // One fetched version is kept, not every one there ever was.
    for other in folder.parent().and_then(|all| std::fs::read_dir(all).ok()).into_iter().flatten().flatten() {
        if other.path() != folder {
            let _ = std::fs::remove_dir_all(other.path());
        }
    }
    Ok(program)
}

/// The version in the address of one of Null's releases: `…/releases/tag/null-v0.2.0`.
/// The repository may hold releases of other things, so the tag has to be one of Null's.
fn null_version_from(address: &str) -> Option<&str> {
    let version = address.trim().rsplit('/').next()?.strip_prefix("null-v")?;
    (!version.is_empty() && version.split('.').all(|part| part.parse::<u64>().is_ok())).then_some(version)
}

/// A released Null that is newer than this one. None when there is none, and
/// none when GitHub could not be asked: that is not worth a word in the box.
fn newer_null(this_null: &str) -> Option<String> {
    let place = std::env::var("NULL_MINI_RELEASE").unwrap_or_else(|_| NEWEST_NULL.to_string());
    let asked = ["--fail", "--location", "--silent", "--head", "--output", "/dev/null", "--max-time", "15", "--write-out", "%{url_effective}", place.as_str()];
    let address = tool("/usr/bin/curl", &asked.map(std::ffi::OsStr::new)).ok()?;
    null_version_from(&address).filter(|version| newer(version, this_null)).map(str::to_string)
}

/// What `/update` did, for the box to say.
#[derive(Clone, Debug, Serialize)]
pub struct Update {
    /// `updated`, `current`, or `needs_null`.
    pub outcome: &'static str,
    /// The version the built-in harness is now, or the one that needs a newer Null.
    pub version: String,
    /// The Null that version needs, and the version of this one.
    pub null: Option<String>,
    pub this_null: &'static str,
    /// Whether the box is running the user's own harness, which `/update` leaves alone.
    pub on_own: bool,
    /// A released Null newer than this one, when there is one, and what gets it.
    pub newer_null: Option<String>,
    pub install: &'static str,
}

/// Move the built-in harness to the newest version the repository has checked,
/// if it is newer than the one Null has. The one that is running is let go; the
/// next message starts the new one and the conversation is loaded back.
#[tauri::command(async)]
pub fn update_harness(app: AppHandle) -> Result<Update, String> {
    if harness::busy(&app) {
        return Err("stop the reply before updating".into());
    }
    update(&app).inspect_err(|reason| log!("/update: nothing was changed: {reason}"))
}

fn update(app: &AppHandle) -> Result<Update, String> {
    let app = app.clone();
    let place = std::env::var("NULL_MINI_UPDATES").unwrap_or_else(|_| UPDATES.to_string());
    let toml = tool("/usr/bin/curl", &["--fail", "--location", "--silent", "--show-error", "--max-time", "30", place.as_str()].map(std::ffi::OsStr::new))
        .map_err(|said| format!("could not read which version is checked: {said}"))?;
    let built_in = fetched(&app).map(|(version, _)| version).unwrap_or_else(|| carried_version().to_string());
    let this_null = env!("CARGO_PKG_VERSION");
    let on_own = matches!(in_use(&app), Some((_, Origin::Own)));
    let did = |outcome, version, null| {
        // Whatever became of the harness, say when Null itself has a newer release.
        let newer_null = newer_null(this_null);
        if let Some(newer) = &newer_null {
            log!("/update: Null {newer} is out; this is Null {this_null}");
        }
        Update { outcome, version, null, this_null, on_own, newer_null, install: INSTALL }
    };
    match step(&toml, &built_in, this_null)? {
        Step::Stay => {
            log!("/update: already on the newest checked {HARNESS_NAME}, {built_in}");
            Ok(did("current", built_in, None))
        }
        Step::NeedsNull { version, null } => {
            log!("/update: {HARNESS_NAME} {version} needs Null {null}; this is Null {this_null}");
            Ok(did("needs_null", version, Some(null)))
        }
        Step::Fetch { version, sha256 } => {
            log!("/update: fetching {HARNESS_NAME} {version}");
            let _ = app.emit_to(panel::LABEL, "mini:notice", format!("fetching {HARNESS_NAME} {version}, about 200 MB"));
            let fetched = fetch(&app, &version, &sha256);
            let _ = app.emit_to(panel::LABEL, "mini:notice", "");
            let program = fetched?;
            settings::update(&app, |settings| settings.engine = Some(version.clone()));
            harness::restart(&app);
            providers::forget(&app);
            log!("/update: the built-in harness is now {}, version {version}", program.display());
            Ok(did("updated", version, None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_null_carries_is_read_from_engine_toml() {
        let toml = "# a note\n\nversion = \"18.4.3\"\nnull = \"0.1.0\"\n\n[sha256]\nomp-darwin-arm64 = \"13\"\n\"THIRD-PARTY-NOTICES.txt\" = \"d0\"\n";
        assert_eq!(named(toml, "version"), Some("18.4.3"));
        assert_eq!(named(toml, "null"), Some("0.1.0"));
        assert_eq!(named(toml, "omp-darwin-arm64"), Some("13"));
        assert_eq!(named(toml, "THIRD-PARTY-NOTICES.txt"), Some("d0"));
        // A name that only begins another is not that name.
        assert_eq!(named(toml, "omp-darwin"), None);
        assert_eq!(named("[sha256]\nomp-darwin-arm64 = \"13\"\n", "version"), None);
        // The file in the repo says which Null its version was checked with, and gives the program's checksum.
        let ours = include_str!("../Engine.toml");
        assert!(named(ours, "null").is_some() && named(ours, PUBLISHED).is_some_and(|sum| sum.len() == 64));
        // The file in the repo names one, in numbers and dots.
        assert!(carried_version().split('.').count() >= 2 && carried_version().chars().all(|c| c.is_ascii_digit() || c == '.'), "{:?}", carried_version());
    }

    #[test]
    fn a_version_is_newer_by_its_numbers_not_its_letters() {
        assert!(newer("18.10.0", "18.9.2"));
        assert!(newer("19.0.0", "18.99.99"));
        assert!(!newer("18.8.7", "18.8.7"));
        assert!(!newer("18.4.3", "18.8.7"));
        // A version that is not all numbers is never taken for newer.
        assert!(!newer("latest", "18.8.7"));
        assert!(!newer("18.9.0-beta", "18.8.7"));
    }

    #[test]
    fn update_moves_only_to_a_newer_version_the_repository_checked() {
        let sum = "c".repeat(64);
        let toml = format!("version = \"18.9.0\"\nnull = \"0.1.0\"\n[sha256]\nomp-darwin-arm64 = \"{sum}\"\n");

        // Newer than the built-in harness, and this Null is new enough: fetch it.
        assert_eq!(step(&toml, "18.8.7", "0.1.0"), Ok(Step::Fetch { version: "18.9.0".into(), sha256: sum.clone() }));
        // Already there, or ahead of what the repository has checked: nothing to do.
        assert_eq!(step(&toml, "18.9.0", "0.1.0"), Ok(Step::Stay));
        assert_eq!(step(&toml, "18.9.1", "0.1.0"), Ok(Step::Stay));
    }

    #[test]
    fn update_says_so_when_the_checked_version_needs_a_newer_null() {
        let toml = format!("version = \"18.9.0\"\nnull = \"0.3.0\"\n[sha256]\nomp-darwin-arm64 = \"{}\"\n", "c".repeat(64));
        assert_eq!(step(&toml, "18.8.7", "0.2.0"), Ok(Step::NeedsNull { version: "18.9.0".into(), null: "0.3.0".into() }));
        assert!(matches!(step(&toml, "18.8.7", "0.3.0"), Ok(Step::Fetch { .. })));
    }

    #[test]
    fn update_fetches_nothing_it_has_no_checksum_for() {
        assert!(step("null = \"0.1.0\"\n", "18.8.7", "0.1.0").is_err());
        assert!(step("version = \"18.9.0\"\n[sha256]\nomp-darwin-arm64 = \"short\"\n", "18.8.7", "0.1.0").is_err());
        assert!(step("version = \"18.9.0\"\n[sha256]\nomp-darwin-x64 = \"abc\"\n", "18.8.7", "0.1.0").is_err());
    }

    #[test]
    fn a_release_of_null_is_known_by_its_tag_and_nothing_else_is() {
        assert_eq!(null_version_from("https://github.com/dominionism/Null/releases/tag/null-v0.2.0\n"), Some("0.2.0"));
        // A release of something else in the same repository, and a page that is no release.
        assert_eq!(null_version_from("https://github.com/dominionism/Null/releases/tag/v0.9.0"), None);
        assert_eq!(null_version_from("https://github.com/dominionism/Null/releases"), None);
        assert_eq!(null_version_from("https://github.com/dominionism/Null/releases/tag/null-vnext"), None);
        assert_eq!(null_version_from(""), None);
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
        let carried = carried().expect("Mini/Scripts/engine has fetched Oh-my-pi");
        assert_eq!(version_of(&carried).as_deref(), Some(carried_version()));
    }
}
