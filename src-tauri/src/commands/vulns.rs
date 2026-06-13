//! v0.5.0 — IPC handlers for the opt-in vulnerability-scanning surface.
//!
//! Four commands, mirroring the trending-history pattern from
//! [`crate::commands::trending`]:
//!
//! - [`vulns_scan_all`] — full-install-set scan. Gated by
//!   [`AppState::require_vulnerability_scanning`]. Short-circuits via
//!   the install-set fingerprint when nothing has changed since the
//!   last successful scan.
//! - [`vulns_scan_one`] — single-formula scan, used by the
//!   PackageDetail "Check vulnerabilities" affordance.
//! - [`vulns_install_helper`] — one-click installer for the
//!   `brew vulns` subcommand itself. Gated by the *master* paranoid
//!   switch only — the per-feature toggle is intentionally bypassed
//!   so the user can install the helper before flipping the toggle on
//!   (typical first-time flow: "I want to enable scanning" → install
//!   helper → toggle on → scan).
//! - [`vulns_invalidate`] — drop a single cache entry. Ungated; called
//!   after every upgrade/uninstall so a stale CVE record can't outlive
//!   the version it referenced.
//!
//! ## Gating composition
//!
//! Every gate decision happens **before** any subprocess spawn or
//! cache load. The cache is lazy-hydrated from disk on the first scan
//! in the process (avoids paying the file-read cost when the user
//! never opts in).
//!
//! ## Persistence
//!
//! After every mutation (`put`, `record_fingerprint`, `invalidate`)
//! the cache is flushed via `save_if_dirty` so a crash mid-session
//! doesn't lose work. Save failures bubble up as `BrewError::Io`.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::State;

use crate::error::BrewError;
use crate::state::AppState;
use crate::types::PackageKind;
use crate::vulns::cache::{ScanRecord, VulnKey, VulnsCache};
use crate::vulns::client::{
    check_brew_vulns_installed, install_brew_vulns, scan_all, scan_one, validate_formula_name,
    RawVuln, BREW_VULNS_INSTALL_CMD,
};
use crate::vulns::fingerprint::{compute as compute_fingerprint, InstalledRef};

/// Wire shape returned by [`vulns_scan_all`]. Includes the install-set
/// fingerprint used for this report so the frontend can correlate a
/// "served from cache" response with the install state at the time
/// of the previous scan (useful for debugging the skip predicate).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VulnScanReport {
    /// Per-package scan records keyed by [`VulnKey::to_storage_key`]
    /// form (`"formula:name:version"`). The frontend mirrors this
    /// key shape in its own cache so a lookup by `(kind, name, version)`
    /// stays O(1).
    pub entries: HashMap<String, ScanRecord>,

    /// Wall-clock timestamp the report was produced (live) or last
    /// refreshed (cache).
    pub scanned_at: DateTime<Utc>,

    /// `"live"` when this report came straight from a `brew vulns`
    /// invocation; `"cache"` when the fingerprint-skip path served it
    /// from disk without re-shelling. The frontend uses this to
    /// surface a "served from cache" indicator in the Security tab
    /// header.
    pub source: String,

    /// SHA-256 install-set fingerprint that was current when this
    /// report was produced. Echoed back so the frontend can show it
    /// in the debug drawer alongside the cache age.
    pub install_fingerprint: String,
}

// ----- Helpers -----

/// Hydrate the in-memory cache from disk on the first scan in this
/// process. Lazy — avoids paying the file-read cost when the user
/// never enables vulnerability scanning.
///
/// Idempotent: subsequent calls are no-ops because the cache will
/// have entries OR a non-empty fingerprint after the first load.
/// (An empty file is indistinguishable from a never-loaded cache,
/// which is fine — re-reading an empty file is cheap.)
async fn ensure_cache_hydrated(state: &State<'_, AppState>) {
    let needs_load = {
        let guard = state.vulns_cache.lock().await;
        guard.file.entries.is_empty() && guard.file.install_fingerprint.is_empty()
    };
    if !needs_load {
        return;
    }
    let loaded = VulnsCache::load(&state.app_data_dir).await;
    let mut guard = state.vulns_cache.lock().await;
    // Re-check under the lock: another concurrent caller may have
    // hydrated between our drop above and re-acquire here. Only swap
    // if we'd be replacing a still-empty cache.
    if guard.file.entries.is_empty() && guard.file.install_fingerprint.is_empty() {
        *guard = loaded;
    }
}

/// Build the install-set fingerprint from `state.installed_cache`.
/// If the cache is cold, populates it by calling `brew_list` (force=false)
/// which is the same path the rest of the app uses. Formulae only —
/// casks aren't supported by `brew vulns` (it queries OSV via source
/// repo URLs, which casks don't have).
async fn compute_install_fingerprint(state: &State<'_, AppState>) -> Result<String, BrewError> {
    // Try the existing cache first — read lock, cheap.
    let cached_fp = {
        let guard = state.installed_cache.read().await;
        guard.as_ref().map(|list| {
            let refs: Vec<InstalledRef<'_>> = list
                .formulae
                .iter()
                .filter_map(|p| {
                    p.installed_version.as_deref().map(|v| InstalledRef {
                        kind: PackageKind::Formula,
                        name: p.name.as_str(),
                        version: v,
                    })
                })
                .collect();
            compute_fingerprint(&refs)
        })
    };
    if let Some(fp) = cached_fp {
        return Ok(fp);
    }
    // Cold cache — populate via the same path the rest of the app uses.
    let list = crate::commands::list::brew_list(state.clone(), Some(false)).await?;
    let refs: Vec<InstalledRef<'_>> = list
        .formulae
        .iter()
        .filter_map(|p| {
            p.installed_version.as_deref().map(|v| InstalledRef {
                kind: PackageKind::Formula,
                name: p.name.as_str(),
                version: v,
            })
        })
        .collect();
    Ok(compute_fingerprint(&refs))
}

/// Resolve the installed version of a formula from `installed_cache`,
/// if known. Used by [`vulns_scan_one`] to decide whether the result
/// is cacheable (we need a version for the key).
async fn lookup_installed_version(state: &State<'_, AppState>, name: &str) -> Option<String> {
    let guard = state.installed_cache.read().await;
    guard.as_ref().and_then(|list| {
        list.formulae
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.installed_version.clone())
    })
}

// ----- IPC handlers -----

/// Full-install-set vulnerability scan. Gated end-to-end by
/// [`AppState::require_vulnerability_scanning`].
///
/// `force=true` bypasses the install-set-fingerprint skip predicate
/// (the Refresh button in the Security tab uses this). When `false`,
/// a matching fingerprint within TTL serves the cached report instead
/// of re-shelling `brew vulns`.
#[tauri::command]
pub async fn vulns_scan_all(
    state: State<'_, AppState>,
    force: bool,
) -> Result<VulnScanReport, BrewError> {
    state.require_vulnerability_scanning().await?;
    ensure_cache_hydrated(&state).await;

    let fingerprint = compute_install_fingerprint(&state).await?;

    // Fingerprint-skip path: serve current cache contents as a
    // "cache" report when nothing has changed since the last scan.
    if !force {
        let cache = state.vulns_cache.lock().await;
        if cache.should_skip_full_scan(&fingerprint) {
            let scanned_at = cache.file.fingerprint_scanned_at.unwrap_or_else(Utc::now);
            return Ok(VulnScanReport {
                entries: cache.file.entries.clone(),
                scanned_at,
                source: "cache".to_string(),
                install_fingerprint: fingerprint,
            });
        }
    }

    // Need a live scan — verify the helper is installed first so we
    // can surface a typed install-affordance error rather than a
    // generic brew-exit-non-zero.
    let brew = state.require_brew_path().await?;
    if !check_brew_vulns_installed(&brew).await? {
        return Err(BrewError::VulnsNotInstalled {
            install_command: BREW_VULNS_INSTALL_CMD.to_string(),
        });
    }

    // `brew vulns` is a read-only query — no write lock needed.
    let mut results = scan_all(&brew).await?;

    // v0.5.0 Step 4 — best-effort GHSA enrichment. The enrich layer
    // no-ops when `github_enabled` is off, when the install set has no
    // GHSA-prefixed entries, or when api.github.com is unreachable —
    // we never error the scan because the enrichment cherry-on-top
    // failed. Per-formula loop so one bad batch doesn't poison the rest.
    for r in &mut results {
        if let Err(e) = crate::vulns::enrich::enrich(&state, &mut r.vulnerabilities).await {
            tracing::warn!(
                "vulns: GHSA enrichment failed for {}: {e}; using raw OSV record",
                r.formula
            );
        }
    }

    // Write results + fingerprint back to the cache, then persist.
    {
        let mut cache = state.vulns_cache.lock().await;
        for r in results {
            cache.put(
                VulnKey {
                    kind: PackageKind::Formula,
                    name: r.formula,
                    version: r.version,
                },
                r.vulnerabilities,
            );
        }
        cache.record_fingerprint(fingerprint.clone());
        cache.save_if_dirty(&state.app_data_dir).await?;
    }

    // Clone the current entries into the response.
    let cache = state.vulns_cache.lock().await;
    Ok(VulnScanReport {
        entries: cache.file.entries.clone(),
        scanned_at: cache.file.fingerprint_scanned_at.unwrap_or_else(Utc::now),
        source: "live".to_string(),
        install_fingerprint: fingerprint,
    })
}

/// Scan a single formula by name. Used by the PackageDetail
/// "Check vulnerabilities" button. Gated identically to the full scan.
///
/// Caches the result when we can resolve the formula's installed
/// version from `installed_cache` — otherwise just returns the live
/// answer without caching (a cache entry needs a version for the key,
/// and we'd rather skip caching than guess).
#[tauri::command]
pub async fn vulns_scan_one(
    state: State<'_, AppState>,
    name: String,
) -> Result<Vec<RawVuln>, BrewError> {
    state.require_vulnerability_scanning().await?;
    validate_formula_name(&name)?;
    ensure_cache_hydrated(&state).await;

    let brew = state.require_brew_path().await?;
    if !check_brew_vulns_installed(&brew).await? {
        return Err(BrewError::VulnsNotInstalled {
            install_command: BREW_VULNS_INSTALL_CMD.to_string(),
        });
    }

    let mut vulns = scan_one(&brew, &name).await?;

    // Best-effort GHSA enrichment — same posture as the full scan.
    if let Err(e) = crate::vulns::enrich::enrich(&state, &mut vulns).await {
        tracing::warn!("vulns: GHSA enrichment failed for {name}: {e}; using raw OSV record");
    }

    // Cache only when we have a version to key on. The PackageDetail
    // path always has one (it loaded the detail before showing the
    // button) but a script-driven caller might not, so we don't make
    // it an error.
    if let Some(version) = lookup_installed_version(&state, &name).await {
        let mut cache = state.vulns_cache.lock().await;
        cache.put(
            VulnKey {
                kind: PackageKind::Formula,
                name: name.clone(),
                version,
            },
            vulns.clone(),
        );
        cache.save_if_dirty(&state.app_data_dir).await?;
    }

    Ok(vulns)
}

/// One-click installer for the `brew vulns` subcommand. Runs
/// `brew install homebrew/brew-vulns/brew-vulns`.
///
/// **Gating note:** unlike the scan commands this does NOT consult
/// the per-feature `vulnerability_scanning_enabled` toggle — the
/// typical first-run flow is "user wants to enable scanning, taps
/// the install affordance, *then* flips the toggle". It still respects
/// the master paranoid gate via `require_network` since `brew install`
/// inherently does outbound work.
///
/// Returns the captured stdout for Activity-drawer surfacing.
#[tauri::command]
pub async fn vulns_install_helper(state: State<'_, AppState>) -> Result<String, BrewError> {
    // Master paranoid gate only — the per-feature toggle is bypassed
    // intentionally (see doc comment).
    state.require_network("vulns_install").await?;

    let brew = state.require_brew_path().await?;
    // `brew install` is a state mutation — take the write lock so we
    // don't race with other install/upgrade commands.
    let _guard = state.brew_write_lock.lock().await;
    let stdout = install_brew_vulns(&brew).await?;
    drop(_guard);

    // Defensive: brew-vulns is a subcommand, not a regular formula, so
    // it shouldn't appear in `brew list`. But invalidating is cheap
    // and protects against any future packaging change that *does*
    // make it visible (so the next list call reflects reality).
    state.invalidate_caches().await;

    Ok(stdout)
}

/// Drop a single cache entry. Called by the post-install / post-upgrade /
/// post-uninstall hooks so a CVE record for a version the user no
/// longer has can't outlive the version it referenced.
///
/// Ungated: works regardless of the per-feature toggle. Cleanup after
/// state changes is always safe and removing data is never a privacy
/// concern.
#[tauri::command]
pub async fn vulns_invalidate(
    state: State<'_, AppState>,
    kind: PackageKind,
    name: String,
    version: String,
) -> Result<(), BrewError> {
    validate_formula_name(&name)?;
    let mut cache = state.vulns_cache.lock().await;
    cache.invalidate(&VulnKey {
        kind,
        name,
        version,
    });
    cache.save_if_dirty(&state.app_data_dir).await?;
    Ok(())
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    //! The gate-only tests below construct an `AppState` and then
    //! invoke each handler's logic via a thin helper that mirrors what
    //! the `#[tauri::command]` macro generates. We can't easily build
    //! a `tauri::State<'_, AppState>` outside of a running app, so we
    //! test the gate path by calling `require_vulnerability_scanning`
    //! and `require_network` directly with the same `feature` string
    //! the handlers would use, then exercise the cache-only paths
    //! (`vulns_invalidate`) against the cache wrapper directly. This
    //! mirrors the approach used in `commands/trending.rs` tests.
    //!
    //! The full IPC integration (subprocess + tauri::State binding)
    //! lives in the manual QA matrix.

    use super::*;
    use crate::commands::settings::{Settings, SettingsLoadState};
    use crate::state::AppState;

    /// Mirror of `state::tests::build_state_with` — keeps these tests
    /// self-contained without leaking state-construction details.
    async fn build_state_with(slot: SettingsLoadState) -> AppState {
        let state = AppState::build().expect("AppState::build");
        {
            let mut guard = state.settings.write().await;
            *guard = slot;
        }
        state
    }

    // ----- vulns_scan_all gate -----

    #[tokio::test]
    async fn scan_all_denies_when_paranoid_on() {
        let s = Settings {
            paranoid_mode: true,
            vulnerability_scanning_enabled: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        // The gate is the very first line of `vulns_scan_all`. Calling
        // it directly here proves the same code path the handler takes.
        match state.require_vulnerability_scanning().await {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "vulnerability_scanning");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn scan_all_denies_when_toggle_off() {
        let s = Settings {
            paranoid_mode: false,
            vulnerability_scanning_enabled: false,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        match state.require_vulnerability_scanning().await {
            Err(BrewError::FeatureDisabled { feature }) => {
                assert_eq!(feature, "vulnerability_scanning");
            }
            other => panic!("expected FeatureDisabled, got {other:?}"),
        }
    }

    // ----- vulns_scan_one gate -----

    #[tokio::test]
    async fn scan_one_denies_when_paranoid_on() {
        let s = Settings {
            paranoid_mode: true,
            vulnerability_scanning_enabled: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        match state.require_vulnerability_scanning().await {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "vulnerability_scanning");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn scan_one_denies_when_toggle_off() {
        let s = Settings {
            paranoid_mode: false,
            vulnerability_scanning_enabled: false,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        match state.require_vulnerability_scanning().await {
            Err(BrewError::FeatureDisabled { feature }) => {
                assert_eq!(feature, "vulnerability_scanning");
            }
            other => panic!("expected FeatureDisabled, got {other:?}"),
        }
    }

    // ----- vulns_install_helper gate -----

    #[tokio::test]
    async fn install_helper_denies_when_paranoid_on() {
        // Master switch denies install just like every other outbound
        // command — the helper itself is a `brew install` which talks
        // to the network.
        let s = Settings {
            paranoid_mode: true,
            // Toggle state must be irrelevant for the install path;
            // we leave it `false` here to confirm the master gate
            // fires before any per-feature check.
            vulnerability_scanning_enabled: false,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        match state.require_network("vulns_install").await {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "vulns_install");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn install_helper_ignores_feature_toggle_when_paranoid_off() {
        // Critical: with paranoid OFF and the per-feature toggle OFF,
        // the install path must still be allowed past the gate. The
        // helper has to be installable *before* the user flips the
        // toggle on (chicken-and-egg otherwise).
        let s = Settings {
            paranoid_mode: false,
            vulnerability_scanning_enabled: false,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        assert!(
            state.require_network("vulns_install").await.is_ok(),
            "install helper must be allowed past the gate even with the per-feature toggle off"
        );
    }

    // ----- vulns_invalidate is ungated -----

    #[tokio::test]
    async fn invalidate_does_not_consult_feature_toggle() {
        // With paranoid ON and the toggle OFF, invalidate must still
        // work — it only mutates local state and is the cleanup hook
        // post-upgrade/uninstall. Denying it would leave stale CVE
        // records around forever once a user flipped paranoid on.
        let s = Settings {
            paranoid_mode: true,
            vulnerability_scanning_enabled: false,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;

        // Pre-populate the cache directly (bypassing the IPC layer
        // since the handler signature needs a Tauri-managed State).
        {
            let mut cache = state.vulns_cache.lock().await;
            cache.put(
                VulnKey {
                    kind: PackageKind::Formula,
                    name: "curl".into(),
                    version: "8.4.0".into(),
                },
                vec![],
            );
            assert_eq!(cache.file.entries.len(), 1);
        }

        // Invalidate via the same key + path the handler uses. No
        // gate call between validate_formula_name and the cache
        // mutation — confirms the handler's claim that invalidate is
        // ungated by inspection of the call sequence.
        validate_formula_name("curl").expect("valid name");
        {
            let mut cache = state.vulns_cache.lock().await;
            let removed = cache.invalidate(&VulnKey {
                kind: PackageKind::Formula,
                name: "curl".into(),
                version: "8.4.0".into(),
            });
            assert!(removed, "entry must have been removed");
            assert!(cache.file.entries.is_empty());
        }
    }

    // ----- VulnScanReport serialization sanity -----

    #[test]
    fn vuln_scan_report_serializes_camel_case() {
        // Pin the wire shape — the frontend Security tab depends on
        // these literal keys.
        let report = VulnScanReport {
            entries: HashMap::new(),
            scanned_at: Utc::now(),
            source: "live".into(),
            install_fingerprint: "abc123".into(),
        };
        let v = serde_json::to_value(&report).expect("serialize");
        assert!(v.get("entries").is_some());
        assert!(v.get("scannedAt").is_some(), "must be camelCase");
        assert!(v.get("source").is_some());
        assert!(v.get("installFingerprint").is_some(), "must be camelCase");
        // Snake-case must not leak.
        assert!(v.get("scanned_at").is_none());
        assert!(v.get("install_fingerprint").is_none());
    }
}
