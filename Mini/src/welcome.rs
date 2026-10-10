//! The first opening, for someone who has never used Null.
//!
//! A newcomer's Mac has no provider signed in. A message sent then brings back
//! only a provider's error, or the answer of a small local model that happened
//! to be running. So Null looks once, at its first start: when the harness's own
//! report lists no account, the box opens by itself, says so and shows the
//! sign-in list. After that it is the user's decision, as with Full Disk Access.
//!
//! Null still knows nothing about any one provider. "Signed in" is what the
//! harness's usage report says: an account is there or it is not. A local model
//! is not an account, and neither is a key the harness was handed some other way.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter};

use crate::log::log;
use crate::providers::ProviderState;
use crate::{engine, harness, panel, providers, settings};

/// How long the first opening waits for the page, which loads a moment after the app starts.
const PAGE_TIME_LIMIT: Duration = Duration::from_secs(20);

static PAGE_READY: AtomicBool = AtomicBool::new(false);

/// The page has loaded and listens, so the first opening can be sent to it.
pub fn page_ready() {
    PAGE_READY.store(true, Ordering::SeqCst);
}

/// Whether nobody is signed in, by the harness's report. None when there was
/// no report to read: that says nothing either way, and nothing is done.
fn nobody_signed_in(report: Option<&[ProviderState]>) -> Option<bool> {
    report.map(|accounts| accounts.is_empty())
}

/// At start: look, once, at whether any provider is signed in. When none is,
/// open the box, say so and show the harness's sign-in list.
pub fn check(app: &AppHandle) {
    if settings::get(app).welcomed {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let Some((binary, _)) = engine::in_use(&app) else { return };
        let report = providers::read_report(&binary, &harness::extra_args());
        match nobody_signed_in(report.as_deref()) {
            None => log!("could not tell whether a provider is signed in; Null looks again at its next start"),
            Some(false) => {
                log!("a provider is signed in, so there is no first opening");
                settings::update(&app, |settings| settings.welcomed = true);
            }
            Some(true) => {
                log!("no provider is signed in; opening the box to say so, this once");
                settings::update(&app, |settings| settings.welcomed = true);
                // What a newcomer also has to know: whether the agent acts without asking.
                let freely = harness::approval_mode(&binary).as_deref() == Some("yolo");
                let waiting = Instant::now();
                while !PAGE_READY.load(Ordering::SeqCst) && waiting.elapsed() < PAGE_TIME_LIMIT {
                    std::thread::sleep(Duration::from_millis(50));
                }
                panel::show(&app);
                let _ = app.emit_to(panel::LABEL, "mini:welcome", json!({ "freely": freely }));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(provider: &str) -> ProviderState {
        ProviderState { provider: provider.into(), reported: false, limits: Vec::new(), limit_reached: None }
    }

    #[test]
    fn a_report_with_no_account_means_nobody_is_signed_in() {
        assert_eq!(nobody_signed_in(Some(&[])), Some(true));
    }

    #[test]
    fn an_account_with_or_without_a_usage_report_is_a_sign_in() {
        assert_eq!(nobody_signed_in(Some(&[account("opencode-go")])), Some(false));
    }

    #[test]
    fn a_report_that_could_not_be_read_says_nothing() {
        assert_eq!(nobody_signed_in(None), None);
    }
}
