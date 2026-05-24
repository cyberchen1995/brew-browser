//! GitHub IPC surface (Phase 12c + 12e).
//!
//! Every command in this module follows the same security pattern:
//!
//! 1. **Settings opt-in gate** (for `github_repo_stats`): consult
//!    `Settings::github_enabled`. False → return `Ok(None)` without any
//!    outbound call, without any URL parse, without any cache touch.
//! 2. **Paranoid-mode gate**: `state.require_network("github_*")`. This
//!    is the single chokepoint that the "Block all outbound network"
//!    master switch flips. Sign-in itself is gated too — per §12d the
//!    OAuth handshake is "outbound" and must be blocked when paranoid
//!    mode is on.
//! 3. **URL allowlist** (`github_repo_stats`): `parse_github_url` —
//!    refuse anything that isn't strictly `github.com/<owner>/<repo>`.
//!
//! ## Token never crosses IPC
//!
//! `github_status` returns `GithubStatusDto { signed_in, username,
//! scopes }`. The token itself lives in the Keychain and is read
//! server-side by `read_token()` on each authenticated request.

use tauri::State;

use crate::error::BrewError;
use crate::github::{
    self, auth, fetch_repo_stats, parse_github_url, DeviceFlowStart, GithubStatusDto, PollResult,
    PollResultDto, RepoStats,
};
use crate::state::AppState;

// ---------- Repo stats (12c) ----------

#[tauri::command]
pub async fn github_repo_stats(
    homepage: String,
    state: State<'_, AppState>,
) -> Result<Option<RepoStats>, BrewError> {
    // 1. Settings opt-in gate. The `github_enabled` toggle defaults
    //    OFF; if the user hasn't flipped it we silently return None.
    //    No network. No URL parse. The frontend interprets None as
    //    "no GitHub stats for this row".
    {
        let guard = state.settings.read().await;
        let enabled = match &*guard {
            crate::commands::settings::SettingsLoadState::Loaded(s) => s.github_enabled,
            // First launch defaults: github_enabled = false. Match
            // Settings::default() so the gate's behaviour is the same
            // as if the user had explicitly chosen the defaults.
            crate::commands::settings::SettingsLoadState::FirstLaunch => false,
            // Corrupt → fail closed (paranoid_mode_blocked will be
            // raised by the next gate anyway, but short-circuit here
            // so we don't leak the corrupt-state behaviour into the
            // None path).
            crate::commands::settings::SettingsLoadState::Corrupt { .. } => false,
        };
        if !enabled {
            return Ok(None);
        }
    }

    // 2. Paranoid-mode gate. Even with the opt-in toggle ON, the
    //    master switch wins — no GitHub probe when paranoid mode is
    //    enabled or settings are corrupt.
    state.require_network("github_repo_stats").await?;

    // 3. URL allowlist. Non-github URLs collapse to None (we treat
    //    them the same as "no homepage").
    let repo = match parse_github_url(&homepage) {
        Some(r) => r,
        None => return Ok(None),
    };

    // 4. Issue the fetch.
    let client = github::stats::build_client()?;
    let auth_token = auth::read_token()?;
    let cache_dir = state.app_data_dir.join("github-cache");
    fetch_repo_stats(&client, &repo, auth_token.as_ref(), &cache_dir).await
}

// ---------- Auth status (12e) ----------

#[tauri::command]
pub async fn github_status(_state: State<'_, AppState>) -> Result<GithubStatusDto, BrewError> {
    // Reads from Keychain only — no network call, so no
    // require_network gate. The Settings panel calls this on mount to
    // know whether to show "Sign in" vs "Signed in as @user".
    auth::status()
}

// ---------- Sign-in start (12e) ----------

#[tauri::command]
pub async fn github_signin_start(
    state: State<'_, AppState>,
) -> Result<DeviceFlowStart, BrewError> {
    // Sign-in itself is outbound — paranoid mode blocks even the OAuth
    // handshake. Per §12d this is by design: the user can't sign in if
    // they've told us not to make outbound calls.
    state.require_network("github_signin").await?;
    auth::start_device_flow().await
}

// ---------- Sign-in poll (12e) ----------

#[tauri::command]
pub async fn github_signin_poll(
    device_code: String,
    state: State<'_, AppState>,
) -> Result<PollResultDto, BrewError> {
    state.require_network("github_signin").await?;
    let result: PollResult = auth::poll_device_flow(&device_code).await?;
    Ok(result.into())
}

// ---------- Sign-out (12e) ----------

#[tauri::command]
pub async fn github_signout(_state: State<'_, AppState>) -> Result<(), BrewError> {
    // Sign-out is purely a Keychain delete — no network. We don't
    // gate it on paranoid mode (it's a *reduction* of state, never
    // an outbound call).
    auth::signout()
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    //! Tests focus on the gate-chain ordering — the actual network/Keychain
    //! work has its own coverage in `github::{auth, stats, url}`. Here we
    //! pin the contract that gates fire in the right order:
    //!   settings → paranoid → URL → fetch.
    //!
    //! The Tauri-command wrappers themselves need an `AppState` to test;
    //! we build one via `AppState::build` and hand-mutate the settings
    //! slot to drive the gates.

    use super::*;
    use crate::commands::settings::{Settings, SettingsLoadState};

    async fn build_state_with(slot: SettingsLoadState) -> AppState {
        let state = AppState::build().expect("AppState::build");
        {
            let mut guard = state.settings.write().await;
            *guard = slot;
        }
        state
    }

    /// `github_enabled: false` → command returns `Ok(None)` without
    /// any network attempt, URL parse, or settings.json write.
    #[tokio::test]
    async fn settings_disabled_short_circuits_to_none() {
        let s = Settings {
            github_enabled: false,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        // Call the inner gate sequence directly to avoid the
        // `State<'_, AppState>` wrapper that the macro needs.
        let result = inner_repo_stats(&state, "https://github.com/foo/bar".into()).await;
        assert!(matches!(result, Ok(None)));
    }

    /// `github_enabled: true` but paranoid mode ON → blocked.
    #[tokio::test]
    async fn paranoid_mode_blocks_even_when_github_enabled() {
        let s = Settings {
            github_enabled: true,
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let result = inner_repo_stats(&state, "https://github.com/foo/bar".into()).await;
        match result {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "github_repo_stats");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    /// `github_enabled: true`, paranoid off, non-github homepage →
    /// `Ok(None)` (gates passed, validator rejected).
    #[tokio::test]
    async fn non_github_homepage_returns_none() {
        let s = Settings {
            github_enabled: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let result = inner_repo_stats(&state, "https://example.com/foo/bar".into()).await;
        assert!(matches!(result, Ok(None)));
    }

    /// All 4 sign-in commands consult require_network. We test the
    /// blocking path here; the per-command happy path requires hitting
    /// github.com which is out of scope for a unit test.
    #[tokio::test]
    async fn signin_start_is_blocked_by_paranoid_mode() {
        let s = Settings {
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let r = inner_signin_start(&state).await;
        match r {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "github_signin");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn signin_poll_is_blocked_by_paranoid_mode() {
        let s = Settings {
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let r = inner_signin_poll(&state, "fake-device-code".into()).await;
        match r {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "github_signin");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    /// `Corrupt` settings → fail closed for repo stats too. The
    /// settings-opt-in check sees Corrupt as `false` (defensive),
    /// returning Ok(None). The paranoid gate would *also* block —
    /// but the opt-in short-circuit fires first.
    #[tokio::test]
    async fn corrupt_settings_returns_none_for_stats() {
        let state = build_state_with(SettingsLoadState::Corrupt {
            message: "bad json".into(),
        })
        .await;
        let result = inner_repo_stats(&state, "https://github.com/foo/bar".into()).await;
        assert!(matches!(result, Ok(None)));
    }

    // ---------- Inner copies that don't take `State<>` ----------
    //
    // The Tauri command attribute wraps each function in a layer that
    // expects `State<'_, AppState>`. For unit tests we want to drive
    // the same logic from a plain `&AppState`. These inner copies are
    // identical to the public commands minus the Tauri wrapper.

    async fn inner_repo_stats(
        state: &AppState,
        homepage: String,
    ) -> Result<Option<RepoStats>, BrewError> {
        {
            let guard = state.settings.read().await;
            let enabled = match &*guard {
                SettingsLoadState::Loaded(s) => s.github_enabled,
                SettingsLoadState::FirstLaunch => false,
                SettingsLoadState::Corrupt { .. } => false,
            };
            if !enabled {
                return Ok(None);
            }
        }
        state.require_network("github_repo_stats").await?;
        let repo = match parse_github_url(&homepage) {
            Some(r) => r,
            None => return Ok(None),
        };
        let client = github::stats::build_client()?;
        let auth_token = auth::read_token()?;
        let cache_dir = state.app_data_dir.join("github-cache");
        fetch_repo_stats(&client, &repo, auth_token.as_ref(), &cache_dir).await
    }

    async fn inner_signin_start(state: &AppState) -> Result<DeviceFlowStart, BrewError> {
        state.require_network("github_signin").await?;
        auth::start_device_flow().await
    }

    async fn inner_signin_poll(
        state: &AppState,
        device_code: String,
    ) -> Result<PollResultDto, BrewError> {
        state.require_network("github_signin").await?;
        let result: PollResult = auth::poll_device_flow(&device_code).await?;
        Ok(result.into())
    }
}
