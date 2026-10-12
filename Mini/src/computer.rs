//! Computer use: whether the agent may see the screen and work the Mac's apps.
//!
//! The harness does the seeing and the pressing. It is the harness's own
//! ability, and it is off until the harness's settings say otherwise. This
//! module keeps the choice the person made in the box (`/computer`) and puts
//! it in the form of the harness's settings, which `harness.rs` hands over at
//! start in a file of Null's own, beside the person's. Their own Oh-my-pi
//! settings are read, never written. Nothing of the screen passes through
//! here: Null neither looks at it nor keeps what the agent saw.

use std::path::Path;
use std::sync::Mutex;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::access::{self, Need, Permission};
use crate::log::log;
use crate::{engine, harness, settings};

/// The harness settings that switch computer use on or off. None when the
/// person never used the switch: the harness is then left to its own settings.
pub fn overlay(chosen: Option<bool>) -> Option<Value> {
    chosen.map(|on| json!({ "computer": { "enabled": on } }))
}

/// What the harness's own settings say of computer use, out of what it prints
/// when asked for them. None when it does not know the setting, as an
/// Oh-my-pi from before it had computer use does not.
pub fn own_read(printed: &[u8]) -> Option<bool> {
    serde_json::from_slice::<Value>(printed).ok()?.get("value")?.as_bool()
}

/// What the harness's own settings say of computer use, or none when it has none.
fn own(binary: &Path) -> Option<bool> {
    let output = std::process::Command::new(binary).args(harness::extra_args()).args(["config", "get", "computer.enabled", "--json"]).output().ok()?;
    own_read(&output.stdout)
}

/// How computer use stands, for the box to say.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Standing {
    /// Whether the harness in use has computer use at all.
    pub known: bool,
    /// Whether it is switched on: by the person in the box, or, when they
    /// never used the switch, by the harness's own settings.
    pub on: bool,
    /// Whether the harness acts without asking. Set to ask, this harness puts
    /// no question about computer use to the box and refuses it (the harness
    /// check, `set_to_ask_the_harness_refuses_a_code_run_and_never_runs_it_unasked`).
    pub freely: bool,
    /// How macOS's two permissions for it stand: without the first the agent can
    /// neither read a window nor press in it, without the second it has no
    /// pictures of the screen.
    pub needs: Vec<Need>,
}

/// How it stands, from the person's choice, the harness's own setting, what the
/// harness is set to do about asking, and what macOS has given Null.
pub fn standing(chosen: Option<bool>, own: Option<bool>, asking: Option<&str>, needs: Vec<Need>) -> Standing {
    Standing { known: own.is_some(), on: own.is_some() && chosen.or(own).unwrap_or(false), freely: !matches!(asking, Some("always-ask" | "write")), needs }
}

/// The harness settings for computer use as the person chose it, for the
/// harness about to be started. None when they never chose, and when that
/// harness does not know the setting: one handed a setting it does not know
/// might not start.
pub fn harness_settings(app: &AppHandle, binary: &Path) -> Option<Value> {
    let chosen = settings::get(app).computer?;
    own(binary)?;
    overlay(Some(chosen))
}

/// How computer use stands with the harness in use.
#[tauri::command(async)]
pub fn computer(app: AppHandle) -> Result<Standing, String> {
    let (binary, _) = engine::in_use(&app).ok_or(format!("{} is not installed", harness::HARNESS_NAME))?;
    Ok(standing(settings::get(&app).computer, own(&binary), harness::approval_mode(&binary).as_deref(), access::needs()))
}

/// Switch computer use on or off. The harness reads it when it starts, so the
/// one that is running is let go; the next message starts another and the
/// conversation is loaded back.
#[tauri::command(async)]
pub fn set_computer(app: AppHandle, on: bool) -> Result<Standing, String> {
    let (binary, _) = engine::in_use(&app).ok_or(format!("{} is not installed", harness::HARNESS_NAME))?;
    if own(&binary).is_none() {
        return Err(format!("this {} has no computer use", harness::HARNESS_NAME));
    }
    settings::update(&app, |settings| settings.computer = Some(on));
    log!("computer use: {}", if on { "on" } else { "off" });
    harness::restart(&app);
    let now = computer(app.clone())?;
    // Switching it on is the person's own act, so this is when macOS is asked: for the
    // first permission it has not given. The other has its row in the list.
    if let (true, Some(missing)) = (on, access::first_missing(&now.needs)) {
        access::ask(&app, missing);
    }
    Ok(now)
}

/// Ask macOS for one of the two permissions, from its row in the `/computer` list.
#[tauri::command(async)]
pub fn ask_permission(app: AppHandle, permission: Permission) -> Result<Standing, String> {
    if !permission.given() {
        access::ask(&app, permission);
    }
    computer(app)
}

/// What macOS had given when Null last looked: Accessibility, then Screen Recording.
static SEEN: Mutex<Option<[bool; 2]>> = Mutex::new(None);

/// Whether what macOS gives has changed since the last look. The first look only notes it.
fn changed(seen: &mut Option<[bool; 2]>, now: [bool; 2]) -> bool {
    seen.replace(now).is_some_and(|was| was != now)
}

/// The box is coming up. With computer use switched on in the box, a
/// permission given or taken away since Null last looked means the harness is
/// started again, so that it works with what macOS allows now. The person
/// comes back from System Settings to a harness that has what they just gave.
pub fn look_again(app: &AppHandle) {
    let now = Permission::BOTH.map(Permission::given);
    let differs = changed(&mut SEEN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()), now);
    if differs && settings::get(app).computer == Some(true) {
        log!("macOS now has Accessibility {} and Screen Recording {} for Null; the harness is started again", on_or_off(now[0]), on_or_off(now[1]));
        harness::restart(app);
    }
}

fn on_or_off(given: bool) -> &'static str {
    if given {
        "on"
    } else {
        "off"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_handed_over_until_the_person_uses_the_switch() {
        assert_eq!(overlay(None), None);
        assert_eq!(overlay(Some(true)), Some(json!({ "computer": { "enabled": true } })));
        assert_eq!(overlay(Some(false)), Some(json!({ "computer": { "enabled": false } })), "switched off in the box, it is off whatever the harness's own settings say");
    }

    #[test]
    fn the_harness_s_own_setting_is_read_from_what_it_prints() {
        assert_eq!(own_read(br#"{"key":"computer.enabled","value":false,"type":"boolean"}"#), Some(false));
        assert_eq!(own_read(br#"{"key":"computer.enabled","value":true,"type":"boolean"}"#), Some(true));
        assert_eq!(own_read(b"Unknown setting: computer.enabled\n"), None, "a harness without computer use says so in words");
        assert_eq!(own_read(b""), None);
    }

    #[test]
    fn the_person_s_choice_stands_over_the_harness_s_own_setting() {
        assert!(standing(Some(true), Some(false), Some("yolo"), Vec::new()).on);
        assert!(!standing(Some(false), Some(true), Some("yolo"), Vec::new()).on);
        assert!(standing(None, Some(true), Some("yolo"), Vec::new()).on, "never chosen in the box, the harness's own setting holds");
        assert!(!standing(None, Some(false), None, Vec::new()).on);
    }

    #[test]
    fn a_harness_without_computer_use_has_none_whatever_was_chosen() {
        let none = standing(Some(true), None, Some("yolo"), Vec::new());
        assert!(!none.known && !none.on);
    }

    #[test]
    fn only_a_harness_that_acts_freely_can_use_it() {
        assert!(standing(Some(true), Some(false), Some("yolo"), Vec::new()).freely);
        assert!(standing(Some(true), Some(false), None, Vec::new()).freely, "a harness that does not say is left to its own way, which is to act freely");
        assert!(!standing(Some(true), Some(false), Some("always-ask"), Vec::new()).freely);
        assert!(!standing(Some(true), Some(false), Some("write"), Vec::new()).freely);
    }

    #[test]
    fn the_harness_is_started_again_only_when_what_macos_gives_has_changed() {
        let mut seen = None;
        assert!(!changed(&mut seen, [false, false]), "the first look only notes how things stand");
        assert!(!changed(&mut seen, [false, false]));
        assert!(changed(&mut seen, [true, false]), "Accessibility was given since");
        assert!(!changed(&mut seen, [true, false]));
        assert!(changed(&mut seen, [true, true]));
        assert!(changed(&mut seen, [false, true]), "and one taken away counts too");
    }
}
