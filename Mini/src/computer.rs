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

use serde::Serialize;
use serde_json::{json, Value};
use tauri::AppHandle;

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
}

/// How it stands, from the person's choice, the harness's own setting and what
/// the harness is set to do about asking.
pub fn standing(chosen: Option<bool>, own: Option<bool>, asking: Option<&str>) -> Standing {
    Standing { known: own.is_some(), on: own.is_some() && chosen.or(own).unwrap_or(false), freely: !matches!(asking, Some("always-ask" | "write")) }
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
    Ok(standing(settings::get(&app).computer, own(&binary), harness::approval_mode(&binary).as_deref()))
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
    computer(app)
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
        assert!(standing(Some(true), Some(false), Some("yolo")).on);
        assert!(!standing(Some(false), Some(true), Some("yolo")).on);
        assert!(standing(None, Some(true), Some("yolo")).on, "never chosen in the box, the harness's own setting holds");
        assert!(!standing(None, Some(false), None).on);
    }

    #[test]
    fn a_harness_without_computer_use_has_none_whatever_was_chosen() {
        let none = standing(Some(true), None, Some("yolo"));
        assert!(!none.known && !none.on);
    }

    #[test]
    fn only_a_harness_that_acts_freely_can_use_it() {
        assert!(standing(Some(true), Some(false), Some("yolo")).freely);
        assert!(standing(Some(true), Some(false), None).freely, "a harness that does not say is left to its own way, which is to act freely");
        assert!(!standing(Some(true), Some(false), Some("always-ask")).freely);
        assert!(!standing(Some(true), Some(false), Some("write")).freely);
    }
}
