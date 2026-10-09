//! One line per event, to stderr and to a file under ~/Library/Logs, so what the
//! app did can be read afterwards without a terminal attached to it.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

static FILE: OnceLock<Option<PathBuf>> = OnceLock::new();

fn file() -> Option<&'static PathBuf> {
    FILE.get_or_init(|| {
        let home = std::env::var_os("HOME")?;
        let dir = PathBuf::from(home).join("Library/Logs/Null");
        std::fs::create_dir_all(&dir).ok()?;
        Some(dir.join("mini.log"))
    })
    .as_ref()
}

pub fn line(text: &str) {
    eprintln!("[mini] {text}");
    let Some(path) = file() else { return };
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs_f64())
        .unwrap_or(0.0);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{seconds:.3} {text}");
    }
}

macro_rules! log {
    ($($arg:tt)*) => { $crate::log::line(&format!($($arg)*)) };
}
pub(crate) use log;
