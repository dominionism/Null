//! What each provider has left.
//!
//! Null knows nothing about any one provider. The harness does: its usage report
//! (`omp usage --json`) says, for every account it is signed in to, how much of
//! each limit is used and when it starts over. This module reads that report and
//! sets it beside the providers whose models the conversation can use.
//!
//! The report is asked for with account names redacted, and nothing that names an
//! account is kept. The harness is never asked for a key or a token.

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::log::log;
use crate::{engine, harness, signin};

/// How long one reading of the report is good for. Asking takes about a second
/// and goes out to the providers.
const FRESH_FOR: Duration = Duration::from_secs(60);
const REPORT_TIME_LIMIT: Duration = Duration::from_secs(15);

/// One limit of a provider, as the harness reports it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Limit {
    /// The harness's own name for it, e.g. "5 hours".
    pub label: String,
    /// The share used, from 0 to 1, when the report gives one.
    pub used: Option<f64>,
    /// When it starts over, in milliseconds since 1970.
    pub resets_at: Option<i64>,
    /// The harness's word for it: "ok", "warning", "exhausted" or "unknown".
    pub status: String,
    /// False for a limit on part of the provider only, such as one tier of models.
    pub whole_provider: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProviderState {
    /// The harness's id for the provider, e.g. "openai-codex".
    pub provider: String,
    /// False when the harness has no report for it: a local model, or a key alone.
    pub reported: bool,
    pub limits: Vec<Limit>,
    /// Whether nothing is left. None when the harness does not say.
    pub limit_reached: Option<bool>,
}

// ── Reading the report ────────────────────────────────────────────────────

/// The state of every provider in a usage report. Whatever cannot be read is
/// left out or marked unknown; a strange report is never a failure.
pub fn states_from_report(report: &Value) -> Vec<ProviderState> {
    let mut states: Vec<ProviderState> = Vec::new();
    for entry in report.get("reports").and_then(Value::as_array).into_iter().flatten() {
        let Some(state) = state_from_account(entry) else { continue };
        match states.iter_mut().find(|known| known.provider == state.provider) {
            // Several accounts on one provider: it has room while any of them has.
            Some(known) => {
                if room(&state) > room(known) {
                    *known = state;
                }
            }
            None => states.push(state),
        }
    }
    for entry in report.get("accountsWithoutUsage").and_then(Value::as_array).into_iter().flatten() {
        let Some(provider) = entry.get("provider").and_then(Value::as_str) else { continue };
        if !states.iter().any(|known| known.provider == provider) {
            states.push(unreported(provider));
        }
    }
    states
}

fn unreported(provider: &str) -> ProviderState {
    ProviderState { provider: provider.to_string(), reported: false, limits: Vec::new(), limit_reached: None }
}

/// Orders accounts by how sure it is that something is left.
fn room(state: &ProviderState) -> u8 {
    match state.limit_reached {
        Some(false) => 2,
        None => 1,
        Some(true) => 0,
    }
}

fn state_from_account(entry: &Value) -> Option<ProviderState> {
    let provider = entry.get("provider")?.as_str()?.to_string();
    let limits: Vec<Limit> = entry.get("limits").and_then(Value::as_array).into_iter().flatten().filter_map(limit_from).collect();
    let said = entry.pointer("/metadata/limitReached").and_then(Value::as_bool);
    let used_up = limits.iter().any(|limit| limit.whole_provider && limit.status == "exhausted");
    let limit_reached = match said {
        Some(true) => Some(true),
        _ if used_up => Some(true),
        Some(false) => Some(false),
        None if limits.iter().any(|limit| limit.whole_provider && limit.status != "unknown") => Some(false),
        None => None,
    };
    Some(ProviderState { provider, reported: true, limits, limit_reached })
}

fn limit_from(entry: &Value) -> Option<Limit> {
    let label = entry.get("label").or_else(|| entry.pointer("/window/label"))?.as_str()?.to_string();
    let used = entry.pointer("/amount/usedFraction").and_then(Value::as_f64);
    let status = match (used, entry.get("status").and_then(Value::as_str)) {
        (Some(share), _) if share >= 1.0 => "exhausted",
        (_, Some(word)) => word,
        _ => "unknown",
    };
    let scope = entry.get("scope");
    let narrowed = scope.and_then(|scope| scope.get("tier")).is_some_and(|tier| !tier.is_null())
        || scope.and_then(|scope| scope.get("shared")).and_then(Value::as_bool) == Some(false);
    Some(Limit {
        label,
        used,
        resets_at: entry.pointer("/window/resetsAt").and_then(Value::as_i64),
        status: status.to_string(),
        whole_provider: !narrowed,
    })
}

/// The report's providers set beside the providers of the model list, in the
/// model list's order. A provider with models and no report is there, unreported.
pub fn beside_models(reported: Vec<ProviderState>, model_ids: &[String]) -> Vec<ProviderState> {
    let mut states: Vec<ProviderState> = Vec::new();
    for provider in model_ids.iter().filter_map(|id| id.split_once('/').map(|(provider, _)| provider)) {
        if states.iter().any(|known| known.provider == provider) {
            continue;
        }
        let state = reported.iter().find(|state| state.provider == provider).cloned();
        states.push(state.unwrap_or_else(|| unreported(provider)));
    }
    let rest: Vec<ProviderState> = reported.into_iter().filter(|state| !states.iter().any(|known| known.provider == state.provider)).collect();
    states.extend(rest);
    states
}

// ── Asking the harness ────────────────────────────────────────────────────

#[derive(Default)]
pub struct Providers(Mutex<Option<(Instant, Vec<ProviderState>)>>);

pub fn init(app: &AppHandle) {
    app.manage(Providers::default());
}

/// Drop what was read, so the next asking goes to the harness: after a sign-in,
/// or a reply that failed.
pub fn forget(app: &AppHandle) {
    *app.state::<Providers>().0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
}

fn remembered(app: &AppHandle) -> Option<Vec<ProviderState>> {
    let state = app.state::<Providers>();
    let kept = state.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    kept.as_ref().filter(|(read, _)| read.elapsed() < FRESH_FOR).map(|(_, states)| states.clone())
}

/// Run the harness's usage report and read it. None when there is no report to
/// read; nothing of what the harness printed is logged.
fn read_report(binary: &Path, extra: &[String]) -> Option<Vec<ProviderState>> {
    let mut command = Command::new(binary);
    command.args(extra).args(["usage", "--json", "--redact"]);
    let printed = signin::run_briefly(command, REPORT_TIME_LIMIT).ok()?;
    let start = printed.find('{')?;
    match serde_json::from_str::<Value>(&printed[start..]) {
        Ok(report) => Some(states_from_report(&report)),
        Err(error) => {
            log!("the usage report could not be read: {error}");
            None
        }
    }
}

/// What each provider has left, for the providers the conversation can use.
/// `fresh` asks the harness again even if the last reading is recent.
#[tauri::command]
pub async fn providers(app: AppHandle, fresh: Option<bool>) -> Result<Vec<ProviderState>, String> {
    if fresh == Some(true) {
        forget(&app);
    }
    let reported = match remembered(&app) {
        Some(states) => states,
        None => {
            let binary = engine::in_use(&app).map(|(binary, _)| binary);
            let read = tauri::async_runtime::spawn_blocking(move || read_report(&binary?, &harness::extra_args())).await.map_err(|e| e.to_string())?;
            if let Some(states) = &read {
                *app.state::<Providers>().0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((Instant::now(), states.clone()));
            }
            // Asked for outright (`/usage`), a report this harness does not give is said in a line.
            if read.is_none() && fresh == Some(true) {
                return Err(format!("this {} gave no usage report that could be read", harness::HARNESS_NAME));
            }
            // Otherwise no report is not a failure: every provider is then simply unreported.
            read.unwrap_or_default()
        }
    };
    let model_ids: Vec<String> = match harness::models(app.clone()).await {
        Ok(list) => list.models.into_iter().map(|model| model.id).collect(),
        Err(_) => Vec::new(),
    };
    Ok(beside_models(reported, &model_ids))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The report as captured on 2026-10-08, with what names the account taken out.
    fn captured() -> Value {
        json!({
            "generatedAt": 1791519134292u64,
            "reports": [
                {
                    "provider": "openai-codex",
                    "fetchedAt": 1791519133030u64,
                    "limits": [
                        {
                            "id": "openai-codex:primary",
                            "label": "5 hours",
                            "scope": { "provider": "openai-codex", "windowId": "5h", "shared": true },
                            "window": { "id": "5h", "label": "5 hours", "durationMs": 18000000, "resetsAt": 1791537134000u64 },
                            "amount": { "used": 0, "limit": 100, "remaining": 100, "usedFraction": 0, "remainingFraction": 1, "unit": "percent" },
                            "status": "ok"
                        },
                        {
                            "id": "openai-codex:secondary",
                            "label": "7 days",
                            "scope": { "provider": "openai-codex", "windowId": "7d", "shared": true },
                            "window": { "id": "7d", "label": "7 days", "durationMs": 604800000, "resetsAt": 1792034151000u64 },
                            "amount": { "used": 2, "limit": 100, "remaining": 98, "usedFraction": 0.02, "remainingFraction": 0.98, "unit": "percent" },
                            "status": "ok"
                        }
                    ],
                    "metadata": { "planType": "plus", "allowed": true, "limitReached": false, "meterStates": { "chat": { "allowed": true, "limitReached": false } } }
                },
                {
                    "provider": "opencode-go",
                    "fetchedAt": 1791519133491u64,
                    "limits": [
                        {
                            "id": "rolling-5h",
                            "label": "5 Hour limit",
                            "scope": { "provider": "opencode-go", "windowId": "5h", "shared": true },
                            "window": { "id": "5h", "label": "5 Hour", "resetsAt": 1791533054000u64, "durationMs": 18000000 },
                            "amount": { "used": 0, "usedFraction": 0, "remainingFraction": 1, "unit": "percent" },
                            "status": "ok"
                        },
                        {
                            "id": "weekly",
                            "label": "Weekly limit",
                            "scope": { "provider": "opencode-go", "windowId": "7d", "shared": true },
                            "window": { "id": "7d", "label": "Weekly", "resetsAt": 1791763200000u64, "durationMs": 604800000 },
                            "amount": { "used": 27, "usedFraction": 0.27, "remainingFraction": 0.73, "unit": "percent" },
                            "status": "ok"
                        },
                        {
                            "id": "monthly",
                            "label": "Monthly limit",
                            "scope": { "provider": "opencode-go", "windowId": "monthly", "shared": true },
                            "window": { "id": "monthly", "label": "Monthly", "resetsAt": 1793245977000u64 },
                            "amount": { "used": 23, "usedFraction": 0.23, "remainingFraction": 0.77, "unit": "percent" },
                            "status": "ok"
                        }
                    ],
                    "metadata": { "planType": "OpenCode Go" }
                }
            ],
            "accountsWithoutUsage": [],
            "disabledCredentials": [],
            "capacity": {}
        })
    }

    fn account(provider: &str, limits: Value, metadata: Value) -> Value {
        json!({ "provider": provider, "limits": limits, "metadata": metadata })
    }

    fn limit(label: &str, used: f64, status: &str) -> Value {
        json!({ "label": label, "scope": { "shared": true }, "window": { "resetsAt": 1791537134000u64 }, "amount": { "usedFraction": used }, "status": status })
    }

    #[test]
    fn the_captured_report_is_read() {
        let states = states_from_report(&captured());
        assert_eq!(states.iter().map(|state| state.provider.as_str()).collect::<Vec<_>>(), ["openai-codex", "opencode-go"]);
        let codex = &states[0];
        assert!(codex.reported);
        assert_eq!(codex.limit_reached, Some(false));
        assert_eq!(
            codex.limits[1],
            Limit { label: "7 days".into(), used: Some(0.02), resets_at: Some(1792034151000), status: "ok".into(), whole_provider: true }
        );
        let go = &states[1];
        assert_eq!(go.limits.iter().map(|limit| limit.label.as_str()).collect::<Vec<_>>(), ["5 Hour limit", "Weekly limit", "Monthly limit"]);
        assert_eq!(go.limits[1].used, Some(0.27));
        // This provider's report never says "limit reached"; the limits themselves do.
        assert_eq!(go.limit_reached, Some(false));
    }

    #[test]
    fn a_profile_with_nothing_signed_in_has_no_providers() {
        let report = json!({ "generatedAt": 1791520169277u64, "reports": [], "accountsWithoutUsage": [], "disabledCredentials": [], "capacity": {} });
        assert!(states_from_report(&report).is_empty());
    }

    #[test]
    fn the_harness_saying_so_is_a_limit_reached() {
        let report = json!({ "reports": [account("openai-codex", json!([limit("5 hours", 1.0, "exhausted")]), json!({ "limitReached": true }))] });
        assert_eq!(states_from_report(&report)[0].limit_reached, Some(true));
        // Even when its limits have not caught up.
        let report = json!({ "reports": [account("openai-codex", json!([limit("5 hours", 0.4, "ok")]), json!({ "limitReached": true }))] });
        assert_eq!(states_from_report(&report)[0].limit_reached, Some(true));
    }

    #[test]
    fn a_used_up_limit_is_a_limit_reached_without_being_told() {
        let report = json!({ "reports": [account("opencode-go", json!([limit("5 Hour limit", 0.1, "ok"), limit("Weekly limit", 1.0, "ok")]), json!({}))] });
        let state = &states_from_report(&report)[0];
        assert_eq!(state.limits[1].status, "exhausted");
        assert_eq!(state.limit_reached, Some(true));
    }

    #[test]
    fn a_limit_on_part_of_a_provider_does_not_use_it_up() {
        let tier = json!({ "label": "Top tier, weekly", "scope": { "tier": "top", "shared": false }, "amount": { "usedFraction": 1.0 }, "status": "exhausted" });
        let report = json!({ "reports": [account("anthropic", json!([limit("5 hours", 0.2, "ok"), tier]), json!({}))] });
        let state = &states_from_report(&report)[0];
        assert!(!state.limits[1].whole_provider);
        assert_eq!(state.limit_reached, Some(false));
    }

    #[test]
    fn one_account_with_room_keeps_the_provider_usable() {
        let used_up = account("openai-codex", json!([limit("5 hours", 1.0, "exhausted")]), json!({ "limitReached": true }));
        let with_room = account("openai-codex", json!([limit("5 hours", 0.3, "ok")]), json!({ "limitReached": false }));
        for accounts in [json!([used_up.clone(), with_room.clone()]), json!([with_room, used_up])] {
            let states = states_from_report(&json!({ "reports": accounts }));
            assert_eq!(states.len(), 1);
            assert_eq!(states[0].limit_reached, Some(false));
            assert_eq!(states[0].limits[0].used, Some(0.3));
        }
    }

    #[test]
    fn what_cannot_be_read_is_unknown_and_never_a_failure() {
        assert!(states_from_report(&json!("not a report")).is_empty());
        assert!(states_from_report(&json!({ "reports": "nor this" })).is_empty());
        let report = json!({
            "reports": [
                { "limits": [] },
                { "provider": "odd", "limits": [{ "no": "label" }, { "label": "Daily", "amount": { "usedFraction": "a lot" } }] },
                { "provider": "bare" }
            ],
            "accountsWithoutUsage": [{ "provider": "anthropic" }, "nonsense"]
        });
        let states = states_from_report(&report);
        assert_eq!(states.iter().map(|state| state.provider.as_str()).collect::<Vec<_>>(), ["odd", "bare", "anthropic"]);
        assert_eq!(states[0].limits, [Limit { label: "Daily".into(), used: None, resets_at: None, status: "unknown".into(), whole_provider: true }]);
        assert_eq!(states[0].limit_reached, None);
        assert_eq!(states[1].limit_reached, None);
        assert!(!states[2].reported);
    }

    /// Runs Oh-my-pi itself, so it runs only when asked: `cargo test -- --ignored`.
    /// It uses the probe profile, which is signed in to nothing, never the real sign-ins.
    #[test]
    #[ignore]
    fn the_harness_report_is_read_for_a_profile_with_nothing_signed_in() {
        let extra = vec!["--profile".to_string(), "null-probe".to_string()];
        assert_eq!(read_report(&engine::under_test(), &extra), Some(Vec::new()));
    }

    #[test]
    fn providers_follow_the_model_list_and_a_local_model_is_unreported() {
        let models: Vec<String> = ["ollama/llama3.2:latest", "opencode-go/deepseek-v4.1-flash", "opencode-go/another", "anthropic/claude", "odd-one-without-a-provider"]
            .map(String::from)
            .to_vec();
        let states = beside_models(states_from_report(&captured()), &models);
        assert_eq!(states.iter().map(|state| state.provider.as_str()).collect::<Vec<_>>(), ["ollama", "opencode-go", "anthropic", "openai-codex"]);
        assert_eq!(states[0], unreported("ollama"));
        assert!(states[1].reported);
        assert_eq!(states[2].limit_reached, None);
    }
}
