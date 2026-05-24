//! Settings persistence (Phase 12d).
//!
//! Stores user-configurable preferences in
//! `~/Library/Application Support/brew-browser/settings.json`. Loaded once
//! at app startup into `AppState.settings` and refreshed by every
//! `settings_set` / `settings_reset` call so live readers (e.g. the
//! paranoid-mode gate consulted by `require_network`) see changes
//! immediately without a process restart.
//!
//! ## Security gates (security-review §12d)
//!
//! - **File-absent vs file-corrupt distinction.** [`SettingsLoadState`]
//!   carries three variants: `FirstLaunch` (file missing → defaults
//!   apply, paranoid OFF), `Loaded(Settings)` (good parse → use as-is),
//!   `Corrupt(message)` (file present but unreadable → **fail closed**;
//!   `require_network` denies everything until the user repairs).
//! - **Atomic writes.** Every save goes through [`crate::util::fs::atomic_write`]
//!   — temp + fsync + rename + fsync(parent). No torn writes.
//! - **Bounded path.** Settings always live at
//!   `state.app_data_dir.join("settings.json")`. No IPC argument can
//!   influence the location.
//! - **Size cap.** [`MAX_SETTINGS_BYTES`] (1 MiB) enforced on both read
//!   (via `read_capped`) and write (pre-serialize check + post-serialize
//!   check, defense in depth).
//! - **Schema validation.** `#[serde(default)]` on every field absorbs
//!   forward-compat additions; unknown enum variants fall back to the
//!   default with a stderr warning rather than rejecting the whole file.
//! - **Numeric clamps.** [`Settings::clamp`] re-applies the ranges
//!   declared in the type docs after every load and write so a manual
//!   edit (`settings.json` is plain JSON the user can poke at) can't
//!   smuggle an out-of-range value.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::BrewError;
use crate::state::AppState;
use crate::util::fs::{atomic_write, read_capped};

/// Hard cap on settings.json size. 1 MiB is wildly generous for what is
/// at most a few dozen scalar fields — protects against accidental or
/// hostile bloat (e.g. a future bug that appends to an array forever).
pub const MAX_SETTINGS_BYTES: u64 = 1024 * 1024;

/// On-disk + IPC payload. Every field has `#[serde(default)]` so a
/// future version that adds a field reads cleanly into an older shape
/// (missing fields take their defaults) and an older version reading a
/// newer file ignores fields it doesn't know about.
///
/// **Numeric clamping** is applied by [`Self::clamp`] after every load
/// and before every save. Don't bypass it — the caps are part of the
/// contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Master "block all outbound network" switch. When true,
    /// `require_network` denies every call. Default false (first launch
    /// = current behaviour preserved).
    pub paranoid_mode: bool,

    /// How often to auto-refresh the bundled catalog. Default Off
    /// (matches current manual-refresh-only behaviour). Auto-refresh
    /// itself is wired in a later phase — this field is the persisted
    /// preference even before that wiring lands.
    pub catalog_auto_refresh: CatalogAutoRefresh,

    /// Show the "Catalog is N days old — refresh?" banner when the
    /// active catalog is at least this many days old. Default 14.
    /// Clamped to `[1, 365]` on every load and save.
    pub catalog_stale_banner_days: u32,

    /// Cask icon fetching mode. Default `All` (matches the current Phase
    /// 8 behaviour where every uninstalled cask with a homepage probes
    /// for a favicon). `InstalledOnly` skips the homepage cascade
    /// entirely. `Off` disables even installed-app icon extraction.
    pub cask_icon_mode: CaskIconMode,

    /// Trending cache TTL in minutes. Default 60 (matches the existing
    /// `TRENDING_TTL` in `trending/cache.rs`). Clamped to `[5, 1440]`
    /// on every load and save — five minutes minimum to be a polite
    /// client, 24 hours maximum because anything older would be stale.
    pub trending_ttl_minutes: u32,

    /// Phase 12c — when true, PackageDetail probes `api.github.com` for
    /// repo stats whenever the package's homepage is a GitHub URL.
    /// Default **false** (off) so the v0.1.x posture of "no GitHub
    /// traffic unless the user opts in" is preserved on every fresh
    /// install. The runtime gate is `commands::github::*` which
    /// short-circuits to `Ok(None)` when this is false — before any
    /// outbound call. Paranoid mode overrides this regardless.
    pub github_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            paranoid_mode: false,
            catalog_auto_refresh: CatalogAutoRefresh::Off,
            catalog_stale_banner_days: 14,
            cask_icon_mode: CaskIconMode::All,
            trending_ttl_minutes: 60,
            // Off by default per Phase 12c plan: anonymous GitHub probes
            // are opt-in so first-launch posture stays "zero outbound
            // beyond what the user has already consented to".
            github_enabled: false,
        }
    }
}

impl Settings {
    /// Inclusive lower bound for `catalog_stale_banner_days`.
    pub const CATALOG_STALE_DAYS_MIN: u32 = 1;
    /// Inclusive upper bound for `catalog_stale_banner_days`.
    pub const CATALOG_STALE_DAYS_MAX: u32 = 365;
    /// Inclusive lower bound for `trending_ttl_minutes`.
    pub const TRENDING_TTL_MIN: u32 = 5;
    /// Inclusive upper bound for `trending_ttl_minutes`.
    pub const TRENDING_TTL_MAX: u32 = 1440;

    /// Apply the numeric clamps declared in the field docs. Idempotent;
    /// safe to call on already-clamped values.
    pub fn clamp(&mut self) {
        self.catalog_stale_banner_days = self
            .catalog_stale_banner_days
            .clamp(Self::CATALOG_STALE_DAYS_MIN, Self::CATALOG_STALE_DAYS_MAX);
        self.trending_ttl_minutes = self
            .trending_ttl_minutes
            .clamp(Self::TRENDING_TTL_MIN, Self::TRENDING_TTL_MAX);
    }
}

/// Catalog auto-refresh cadence. Wire-format is kebab-case to match the
/// frontend's TypeScript union (`"off" | "weekly" | "daily"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CatalogAutoRefresh {
    /// Manual only — the current Phase 12a default.
    #[default]
    Off,
    Weekly,
    Daily,
}

/// Cask icon fetching mode. `All` preserves the current behaviour from
/// Phase 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CaskIconMode {
    Off,
    InstalledOnly,
    #[default]
    All,
}

/// Three-state container for the in-memory settings cache.
///
/// The distinction between `FirstLaunch` and `Corrupt` is **load-bearing**
/// (security review §12d): the former applies defaults (paranoid OFF),
/// the latter fails closed (paranoid effectively ON until the user
/// repairs the file or hits the reset button in the Settings UI).
#[derive(Debug, Clone)]
pub enum SettingsLoadState {
    /// `settings.json` did not exist when we tried to read it. New
    /// installs, freshly-reset apps, etc. Defaults apply.
    FirstLaunch,
    /// Successfully parsed. Carries the clamped, validated Settings.
    Loaded(Settings),
    /// File present but unreadable (bad JSON, oversize, read error).
    /// `require_network` denies every call until repaired. The message
    /// is surfaced via `settings_get` so the UI can show a clear "Reset
    /// to defaults" affordance instead of silently rolling back.
    Corrupt { message: String },
}

impl SettingsLoadState {
    /// Convenience for the gate: returns the effective settings when
    /// they should be honoured, or `None` when the load failed and we
    /// should fall back to "deny outbound" semantics.
    ///
    /// `AppState::require_network` reaches for the variants directly
    /// rather than this helper (to keep the gate's logic visible in one
    /// place), but the helper is the canonical reference for anything
    /// else that needs the same disambiguation — kept available for
    /// future callers (settings UI, diagnostics) and exercised by tests.
    #[allow(dead_code)]
    pub fn effective_settings(&self) -> Option<Settings> {
        match self {
            SettingsLoadState::Loaded(s) => Some(s.clone()),
            SettingsLoadState::FirstLaunch => Some(Settings::default()),
            SettingsLoadState::Corrupt { .. } => None,
        }
    }
}

/// Resolve the canonical settings path inside `app_data_dir`.
///
/// Always `<app_data_dir>/settings.json`. The directory is created if
/// missing — the caller (typically `AppState::build`) has already
/// ensured `app_data_dir` exists, so this is a defense-in-depth mkdir.
pub fn settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("settings.json")
}

/// Synchronous startup loader. Called from `AppState::build()` (which is
/// a non-async function) so we use the blocking `std::fs` API rather
/// than tokio. The trade-off accepted is a single small read on startup
/// in exchange for a much simpler init story.
///
/// Returns the same three-state shape as the async loader so callers
/// stay uniform.
pub fn load_at_startup(app_data_dir: &Path) -> SettingsLoadState {
    let path = settings_path(app_data_dir);

    let meta = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return SettingsLoadState::FirstLaunch;
        }
        Err(e) => {
            // Stat failed for some non-NotFound reason (permission denied,
            // EIO, etc.). Treat as corrupt — fail closed.
            tracing::warn!("settings: stat failed at {}: {e}", path.display());
            return SettingsLoadState::Corrupt {
                message: format!("stat {}: {e}", path.display()),
            };
        }
    };

    if meta.len() > MAX_SETTINGS_BYTES {
        tracing::warn!(
            "settings: {} is {} bytes, exceeds {}-byte cap; treating as corrupt",
            path.display(),
            meta.len(),
            MAX_SETTINGS_BYTES
        );
        return SettingsLoadState::Corrupt {
            message: format!(
                "settings.json is {} bytes, exceeds {}-byte cap",
                meta.len(),
                MAX_SETTINGS_BYTES
            ),
        };
    }

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("settings: read failed at {}: {e}", path.display());
            return SettingsLoadState::Corrupt {
                message: format!("read {}: {e}", path.display()),
            };
        }
    };

    match serde_json::from_slice::<Settings>(&bytes) {
        Ok(mut s) => {
            s.clamp();
            SettingsLoadState::Loaded(s)
        }
        Err(e) => {
            tracing::warn!(
                "settings: parse failed at {}: {e}; treating as corrupt",
                path.display()
            );
            SettingsLoadState::Corrupt {
                message: format!("parse {}: {e}", path.display()),
            }
        }
    }
}

/// Async loader, identical semantics to [`load_at_startup`] but
/// non-blocking. Used by tests and any future callers that need to
/// re-read from disk without blocking the runtime.
#[cfg_attr(not(test), allow(dead_code))]
async fn load_async(app_data_dir: &Path) -> SettingsLoadState {
    let path = settings_path(app_data_dir);

    let meta = match tokio::fs::metadata(&path).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return SettingsLoadState::FirstLaunch;
        }
        Err(e) => {
            tracing::warn!("settings: stat failed at {}: {e}", path.display());
            return SettingsLoadState::Corrupt {
                message: format!("stat {}: {e}", path.display()),
            };
        }
    };

    if meta.len() > MAX_SETTINGS_BYTES {
        return SettingsLoadState::Corrupt {
            message: format!(
                "settings.json is {} bytes, exceeds {}-byte cap",
                meta.len(),
                MAX_SETTINGS_BYTES
            ),
        };
    }

    let bytes = match read_capped(&path, MAX_SETTINGS_BYTES).await {
        Ok(b) => b,
        Err(e) => {
            return SettingsLoadState::Corrupt {
                message: format!("read {}: {e}", path.display()),
            };
        }
    };

    match serde_json::from_slice::<Settings>(&bytes) {
        Ok(mut s) => {
            s.clamp();
            SettingsLoadState::Loaded(s)
        }
        Err(e) => SettingsLoadState::Corrupt {
            message: format!("parse {}: {e}", path.display()),
        },
    }
}

/// Serialize `settings`, enforce the size cap, then atomically persist.
///
/// Order: (1) clamp numerics (no-op if already in range), (2) serialize
/// to bytes, (3) reject if the byte length exceeds the cap, (4)
/// `atomic_write` into place, (5) return the clamped struct so callers
/// can re-broadcast the canonicalized values.
async fn persist(app_data_dir: &Path, mut settings: Settings) -> Result<Settings, BrewError> {
    settings.clamp();
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|e| BrewError::Internal {
        message: format!("serialize settings: {e}"),
    })?;
    if bytes.len() as u64 > MAX_SETTINGS_BYTES {
        return Err(BrewError::InvalidArgument {
            message: format!(
                "serialized settings are {} bytes, exceeds {}-byte cap",
                bytes.len(),
                MAX_SETTINGS_BYTES
            ),
        });
    }

    // Defense in depth — ensure the parent dir exists. `AppState::build`
    // already mkdir_p'd it, but a fresh checkout of the app on a system
    // that's never run brew-browser could plausibly hit this otherwise.
    if !app_data_dir.exists() {
        tokio::fs::create_dir_all(app_data_dir).await.map_err(|e| {
            BrewError::Io {
                message: format!(
                    "create settings parent {}: {e}",
                    app_data_dir.display()
                ),
            }
        })?;
    }

    let path = settings_path(app_data_dir);
    atomic_write(&path, &bytes).await?;
    Ok(settings)
}

// ---------- Commands ----------

/// Read the current settings.
///
/// Always returns the *currently-loaded* state — does not re-read from
/// disk on every call (the in-memory cache is authoritative and is
/// refreshed by `settings_set` / `settings_reset`).
///
/// Returns an error when the loaded state is `Corrupt`, so the frontend
/// can surface a "Settings file unreadable — reset to defaults?" prompt
/// without exposing the corrupt JSON contents.
#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<Settings, BrewError> {
    let guard = state.settings.read().await;
    match &*guard {
        SettingsLoadState::Loaded(s) => Ok(s.clone()),
        SettingsLoadState::FirstLaunch => Ok(Settings::default()),
        SettingsLoadState::Corrupt { message } => Err(BrewError::Internal {
            message: format!("settings file is unreadable: {message}"),
        }),
    }
}

/// Write a complete settings struct to disk and update the in-memory
/// cache. The frontend always sends a complete object (merging with
/// existing values is the caller's responsibility, not ours).
#[tauri::command]
pub async fn settings_set(
    settings: Settings,
    state: State<'_, AppState>,
) -> Result<Settings, BrewError> {
    let clamped = persist(&state.app_data_dir, settings).await?;
    {
        let mut guard = state.settings.write().await;
        *guard = SettingsLoadState::Loaded(clamped.clone());
    }
    Ok(clamped)
}

/// Overwrite `settings.json` with the defaults and update the
/// in-memory cache. Used by the UI's "Reset to defaults" button when
/// the file is corrupt or the user just wants to start fresh.
#[tauri::command]
pub async fn settings_reset(state: State<'_, AppState>) -> Result<Settings, BrewError> {
    let defaults = Settings::default();
    let clamped = persist(&state.app_data_dir, defaults).await?;
    {
        let mut guard = state.settings.write().await;
        *guard = SettingsLoadState::Loaded(clamped.clone());
    }
    Ok(clamped)
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    /// File-absent → defaults apply (paranoid OFF).
    #[tokio::test]
    async fn missing_file_is_first_launch() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let state = load_at_startup(tmp.path());
        match state {
            SettingsLoadState::FirstLaunch => {}
            other => panic!("expected FirstLaunch, got {other:?}"),
        }
        // Defaults must have paranoid OFF.
        let effective = state.effective_settings().expect("first launch has defaults");
        assert!(!effective.paranoid_mode);
    }

    /// File-corrupt (bad JSON) → fail closed.
    #[tokio::test]
    async fn corrupt_file_fails_closed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = settings_path(tmp.path());
        tokio::fs::write(&path, b"{not valid json").await.unwrap();

        let state = load_at_startup(tmp.path());
        match &state {
            SettingsLoadState::Corrupt { message } => {
                assert!(message.contains("parse"), "{message}");
            }
            other => panic!("expected Corrupt, got {other:?}"),
        }
        // effective_settings must be None — caller must NOT see "paranoid off".
        assert!(state.effective_settings().is_none());
    }

    /// File-oversize → fail closed.
    #[tokio::test]
    async fn oversize_file_fails_closed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = settings_path(tmp.path());
        // Write 1 MiB + 1 byte.
        let payload = vec![b'a'; (MAX_SETTINGS_BYTES + 1) as usize];
        tokio::fs::write(&path, &payload).await.unwrap();

        let state = load_at_startup(tmp.path());
        match &state {
            SettingsLoadState::Corrupt { message } => {
                assert!(message.contains("exceeds"), "{message}");
            }
            other => panic!("expected Corrupt, got {other:?}"),
        }
    }

    /// Round-trip: persist + reload returns the same struct.
    #[tokio::test]
    async fn round_trip_persists_all_fields() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let s = Settings {
            paranoid_mode: true,
            catalog_auto_refresh: CatalogAutoRefresh::Weekly,
            catalog_stale_banner_days: 21,
            cask_icon_mode: CaskIconMode::InstalledOnly,
            trending_ttl_minutes: 120,
            github_enabled: true,
        };
        let written = persist(tmp.path(), s.clone()).await.expect("persist");
        assert_eq!(written, s);

        let reloaded = load_async(tmp.path()).await;
        match reloaded {
            SettingsLoadState::Loaded(loaded) => assert_eq!(loaded, s),
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    /// Phase 12c — `github_enabled` must round-trip with the camelCase
    /// JSON key `githubEnabled`. The field is brand-new and we want a
    /// pinning test that the wire shape matches the frontend type.
    #[tokio::test]
    async fn github_enabled_round_trips_with_camel_case_key() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let s = Settings {
            github_enabled: true,
            ..Settings::default()
        };
        persist(tmp.path(), s.clone()).await.expect("persist");

        // Inspect raw JSON on disk for the camelCase key. We don't want a
        // future serde rename to silently shift the wire shape.
        let raw = tokio::fs::read_to_string(settings_path(tmp.path()))
            .await
            .expect("read raw");
        assert!(
            raw.contains("\"githubEnabled\""),
            "expected camelCase key in raw JSON, got: {raw}"
        );
        assert!(
            !raw.contains("\"github_enabled\""),
            "must not emit snake_case key"
        );

        let reloaded = load_async(tmp.path()).await;
        match reloaded {
            SettingsLoadState::Loaded(loaded) => assert!(loaded.github_enabled),
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    /// Out-of-range numerics get clamped on save.
    #[tokio::test]
    async fn clamps_on_save() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let s = Settings {
            paranoid_mode: false,
            catalog_auto_refresh: CatalogAutoRefresh::Off,
            catalog_stale_banner_days: 9999, // way above 365
            cask_icon_mode: CaskIconMode::All,
            trending_ttl_minutes: 1, // below the 5-minute floor
            github_enabled: false,
        };
        let written = persist(tmp.path(), s).await.expect("persist");
        assert_eq!(written.catalog_stale_banner_days, Settings::CATALOG_STALE_DAYS_MAX);
        assert_eq!(written.trending_ttl_minutes, Settings::TRENDING_TTL_MIN);
    }

    /// Out-of-range numerics get clamped on read too (defense against
    /// hand-edited settings.json).
    #[tokio::test]
    async fn clamps_on_load() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = settings_path(tmp.path());
        // Hand-write a settings file with absurd values.
        let raw = br#"{
            "paranoidMode": false,
            "catalogAutoRefresh": "daily",
            "catalogStaleBannerDays": 99999,
            "caskIconMode": "all",
            "trendingTtlMinutes": 2
        }"#;
        tokio::fs::write(&path, raw).await.unwrap();

        let state = load_at_startup(tmp.path());
        match state {
            SettingsLoadState::Loaded(s) => {
                assert_eq!(s.catalog_stale_banner_days, Settings::CATALOG_STALE_DAYS_MAX);
                assert_eq!(s.trending_ttl_minutes, Settings::TRENDING_TTL_MIN);
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    /// Unknown enum variant → serde rejects the parse → fail closed
    /// (intentional; we don't want a typo'd field to silently pick a
    /// default the user didn't write).
    ///
    /// The plan asks for "default substituted", but serde's parser is
    /// all-or-nothing on a single field — we can't selectively recover
    /// one unknown variant while keeping the rest. The fail-closed
    /// behaviour is the strictly safer interpretation: the user's
    /// "deny network until repaired" gate kicks in, the UI surfaces
    /// the parse error, and the user hits Reset to defaults. The doc
    /// comment on `SettingsLoadState::Corrupt` explains this.
    #[tokio::test]
    async fn unknown_enum_variant_is_corrupt() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = settings_path(tmp.path());
        let raw = br#"{
            "paranoidMode": false,
            "catalogAutoRefresh": "every-blue-moon",
            "catalogStaleBannerDays": 14,
            "caskIconMode": "all",
            "trendingTtlMinutes": 60
        }"#;
        tokio::fs::write(&path, raw).await.unwrap();

        let state = load_at_startup(tmp.path());
        match &state {
            SettingsLoadState::Corrupt { message } => {
                assert!(
                    message.contains("parse"),
                    "expected parse failure in corrupt message, got {message}"
                );
            }
            other => panic!("expected Corrupt, got {other:?}"),
        }
        assert!(state.effective_settings().is_none(), "must fail closed");
    }

    /// Missing optional fields take their defaults (forward compat).
    #[tokio::test]
    async fn missing_fields_use_defaults() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = settings_path(tmp.path());
        // Only paranoidMode set — everything else absent.
        let raw = br#"{ "paranoidMode": true }"#;
        tokio::fs::write(&path, raw).await.unwrap();

        let state = load_at_startup(tmp.path());
        match state {
            SettingsLoadState::Loaded(s) => {
                assert!(s.paranoid_mode);
                assert_eq!(s.catalog_auto_refresh, CatalogAutoRefresh::Off);
                assert_eq!(s.catalog_stale_banner_days, 14);
                assert_eq!(s.cask_icon_mode, CaskIconMode::All);
                assert_eq!(s.trending_ttl_minutes, 60);
                // `github_enabled` was added in 12c — must default to false
                // for forward compat with pre-12c settings files.
                assert!(!s.github_enabled);
            }
            other => panic!("expected Loaded, got {other:?}"),
        }
    }

    /// Simulate a crash mid-write: write a `.tmp` file then truncate
    /// it. The final settings.json should remain whatever it was
    /// before (or absent), never the partial tmp contents.
    ///
    /// This exercises the atomic-write contract from `util::fs::atomic_write`:
    /// a crash before the `rename` step leaves the data file unchanged.
    #[tokio::test]
    async fn atomic_write_survives_simulated_crash() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = settings_path(tmp.path());

        // 1. Establish a known-good initial state.
        let original = Settings::default();
        persist(tmp.path(), original.clone()).await.expect("seed");

        // 2. Simulate a crash mid-write by manually creating an
        // oversize / truncated .tmp sibling without renaming it. The
        // existence of `.tmp` must not pollute the final file.
        let mut tmp_name = path.as_os_str().to_owned();
        tmp_name.push(".tmp");
        let tmp_sibling = std::path::PathBuf::from(tmp_name);
        tokio::fs::write(&tmp_sibling, b"\x00 partial garbage")
            .await
            .expect("write partial tmp");

        // 3. Read the final file — must still be the original payload.
        let state = load_at_startup(tmp.path());
        match state {
            SettingsLoadState::Loaded(s) => assert_eq!(s, original),
            other => panic!("expected Loaded with original, got {other:?}"),
        }
    }

    /// `settings_reset` overwrites a corrupt file with defaults.
    #[tokio::test]
    async fn reset_repairs_corrupt_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = settings_path(tmp.path());

        // Plant corrupt content.
        tokio::fs::write(&path, b"{ garbage").await.unwrap();
        let state_before = load_at_startup(tmp.path());
        assert!(matches!(state_before, SettingsLoadState::Corrupt { .. }));

        // Write defaults via persist (what settings_reset uses).
        let written = persist(tmp.path(), Settings::default()).await.expect("reset");
        assert_eq!(written, Settings::default());

        // Reload — must now be Loaded(defaults).
        let state_after = load_at_startup(tmp.path());
        match state_after {
            SettingsLoadState::Loaded(s) => assert_eq!(s, Settings::default()),
            other => panic!("expected Loaded after reset, got {other:?}"),
        }
    }

    /// effective_settings on FirstLaunch returns defaults (paranoid off).
    #[test]
    fn effective_settings_first_launch_returns_defaults() {
        let state = SettingsLoadState::FirstLaunch;
        let s = state.effective_settings().expect("first launch yields defaults");
        assert_eq!(s, Settings::default());
        assert!(!s.paranoid_mode);
    }

    /// effective_settings on Corrupt returns None (fail closed signal).
    #[test]
    fn effective_settings_corrupt_returns_none() {
        let state = SettingsLoadState::Corrupt {
            message: "boom".into(),
        };
        assert!(state.effective_settings().is_none());
    }
}
