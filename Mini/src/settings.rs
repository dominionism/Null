//! The few things the app remembers between runs. The harness keeps the
//! conversations; this holds only where the box was, which model was chosen and
//! which conversation was last open.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::log::log;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Top-left corner of the box, in physical pixels.
    pub position: Option<(i32, i32)>,
    pub model: Option<String>,
    pub session: Option<String>,
    /// Whether Null has already sent the user to switch on Full Disk Access. It asks once.
    pub asked_full_disk: bool,
    /// The models to carry on with when the one in use stops answering, in the order to try them.
    pub backups: Vec<String>,
    /// The user's own Oh-my-pi, by its path, when they chose it over the one Null carries
    /// (`/harness`). Nothing here means the one Null carries.
    pub harness: Option<String>,
}

pub struct Store(Mutex<Settings>);

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join(file_name("settings", "json")))
}

/// The name of one of the app's own files. A run under a harness profile
/// (`NULL_MINI_PROFILE`, for scripted checks) keeps files of its own, so that
/// a check never changes what the installed app remembers.
pub fn file_name(stem: &str, extension: &str) -> String {
    let profile: String = std::env::var("NULL_MINI_PROFILE").unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')).collect();
    if profile.is_empty() {
        format!("{stem}.{extension}")
    } else {
        format!("{stem}.{profile}.{extension}")
    }
}

/// Read the settings file, or start from nothing when it is missing or unreadable.
pub fn init(app: &AppHandle) {
    let settings = path(app)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    app.manage(Store(Mutex::new(settings)));
}

pub fn get(app: &AppHandle) -> Settings {
    let store = app.state::<Store>();
    let settings = store.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    settings.clone()
}

/// Change the settings and write them out. Does nothing to the file when the change is a no-op.
pub fn update(app: &AppHandle, change: impl FnOnce(&mut Settings)) {
    let store = app.state::<Store>();
    let mut settings = store.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = settings.clone();
    change(&mut settings);
    if *settings == before {
        return;
    }
    let Some(path) = path(app) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let written = serde_json::to_string_pretty(&*settings)
        .map_err(|e| e.to_string())
        .and_then(|text| std::fs::write(&path, text).map_err(|e| e.to_string()));
    if let Err(e) = written {
        log!("could not save settings: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_from_an_older_version_still_loads() {
        let settings: Settings = serde_json::from_str(r#"{"model":"a/b"}"#).unwrap();
        assert_eq!(settings.model.as_deref(), Some("a/b"));
        assert_eq!(settings.position, None);
    }

    #[test]
    fn settings_survive_a_round_trip() {
        let settings = Settings {
            position: Some((40, -12)),
            model: Some("a/b".into()),
            session: Some("s1".into()),
            asked_full_disk: true,
            backups: vec!["c/d".into()],
            harness: Some("/opt/own/omp".into()),
        };
        let text = serde_json::to_string(&settings).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&text).unwrap(), settings);
    }
}
