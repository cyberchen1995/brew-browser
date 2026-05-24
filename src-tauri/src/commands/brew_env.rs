//! `brew_env` — Settings-driven brew environment toggles.
//!
//! Phase 12b additions (Settings shell):
//! - `brew_get_analytics()` shells `brew analytics state` and parses the first
//!   line of stdout into a boolean.
//! - `brew_set_analytics(enabled)` shells `brew analytics on|off` to flip the
//!   `HOMEBREW_NO_ANALYTICS` posture for the user.
//! - `app_version()` returns the application's own semver from
//!   `tauri::App::package_info()`. Kept in this module to avoid a near-empty
//!   `commands/app_info.rs`.
//!
//! Security gate per `memory-bank/scans/phase12-security-review.md` § 12b:
//! the analytics parser reads ONLY the first line of stdout with strict
//! match. `brew` can prepend deprecation banners and other chatter; regex-
//! searching the whole output would let a future banner like
//! "Warning: HOMEBREW_NO_ANALYTICS is disabled in your env" flip the result.

use tauri::{AppHandle, Runtime, State};

use crate::brew::exec::run_brew_capture;
use crate::error::BrewError;
use crate::state::AppState;

/// Parse the first line of `brew analytics state` into a boolean.
///
/// Strict match per the Phase 12 security review (§ 12b): only two
/// exact strings are accepted, anything else is treated as an internal
/// error so the UI can surface "unexpected output" rather than guessing.
///
/// Split out as a private fn so it can be exercised without a live
/// `brew` install — see `tests::*` below.
fn parse_analytics_state(stdout: &str) -> Result<bool, BrewError> {
    let first = stdout.lines().next().map(str::trim).unwrap_or("");
    match first {
        "Analytics are enabled." | "Analytics are enabled" => Ok(true),
        "Analytics are disabled." | "Analytics are disabled" => Ok(false),
        other => Err(BrewError::Internal {
            message: format!("unexpected analytics output: {other}"),
        }),
    }
}

/// Read the user's Homebrew analytics posture.
///
/// Shells `brew analytics state`. Returns `true` when analytics are
/// enabled (Homebrew's default), `false` when disabled. Uses a read of
/// brew state — no write lock needed.
#[tauri::command]
pub async fn brew_get_analytics(state: State<'_, AppState>) -> Result<bool, BrewError> {
    let brew = state.require_brew_path().await?;
    let out = run_brew_capture(&brew, &["analytics", "state"], "brew analytics state").await?;
    parse_analytics_state(&out)
}

/// Flip the user's Homebrew analytics posture.
///
/// Shells `brew analytics on` or `brew analytics off`. Takes the write
/// lock — this is a state mutation (brew rewrites `~/.homebrew/...` and
/// the future `HOMEBREW_NO_ANALYTICS` env will be re-read).
#[tauri::command]
pub async fn brew_set_analytics(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), BrewError> {
    let brew = state.require_brew_path().await?;
    let verb = if enabled { "on" } else { "off" };
    // Write lock for state mutations, per the AppState contract.
    let _guard = state.brew_write_lock.lock().await;
    let display = format!("brew analytics {verb}");
    run_brew_capture(&brew, &["analytics", verb], &display).await?;
    Ok(())
}

/// Return the app's version string from the Tauri package info. Source of
/// truth is `Cargo.toml` (`tauri.conf.json` mirrors it). Avoids reading
/// `package.json` from the renderer.
#[tauri::command]
pub fn app_version<R: Runtime>(app: AppHandle<R>) -> String {
    app.package_info().version.to_string()
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_enabled_with_period() {
        assert!(parse_analytics_state("Analytics are enabled.\n").unwrap());
    }

    #[test]
    fn parses_enabled_without_period() {
        // Older brew versions don't terminate the sentence with a period.
        assert!(parse_analytics_state("Analytics are enabled\n").unwrap());
    }

    #[test]
    fn parses_disabled_with_period() {
        assert!(!parse_analytics_state("Analytics are disabled.\n").unwrap());
    }

    #[test]
    fn parses_disabled_without_period() {
        assert!(!parse_analytics_state("Analytics are disabled\n").unwrap());
    }

    #[test]
    fn ignores_trailing_banner_lines() {
        // Brew sometimes prints a deprecation / migration banner AFTER the
        // status line. We must only consider the first line, otherwise a
        // future banner mentioning "enabled" could flip the result.
        let stdout = "Analytics are disabled.\nWarning: HOMEBREW_NO_ANALYTICS is set in your environment.\nAnalytics are enabled in your config.\n";
        assert!(!parse_analytics_state(stdout).unwrap());
    }

    #[test]
    fn rejects_unexpected_first_line() {
        // Anything outside the strict allowlist must be an Internal error,
        // not a silent default — the UI should show "unexpected output"
        // so the user knows brew is misbehaving.
        let err = parse_analytics_state("Some other thing\n").unwrap_err();
        match err {
            BrewError::Internal { message } => {
                assert!(message.contains("unexpected analytics output"));
                assert!(message.contains("Some other thing"));
            }
            other => panic!("expected Internal, got {:?}", other),
        }
    }

    #[test]
    fn rejects_empty_stdout() {
        // Empty output → empty first line → no match → Internal error.
        let err = parse_analytics_state("").unwrap_err();
        assert!(matches!(err, BrewError::Internal { .. }));
    }

    #[test]
    fn trims_whitespace_on_first_line() {
        // Leading whitespace on the first line shouldn't trip the matcher.
        assert!(parse_analytics_state("  Analytics are enabled.  \n").unwrap());
    }
}
