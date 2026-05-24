//! Catalog commands (Phase 12a).
//!
//! Surface for the bundled-or-user-refreshed Homebrew catalog living on
//! `AppState`. All commands except `catalog_refresh` are pure reads that
//! return clones / Arc references and do no I/O beyond a single
//! `RwLock::read`.
//!
//! Outbound network: `catalog_refresh` is the ONLY command in this
//! module that talks to formulae.brew.sh. When Phase 12d lands, it
//! must call `state.require_network("catalog_refresh")` first; the
//! security review (§Cross-cutting concerns) makes this an explicit
//! retroactive gate.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::catalog::{Cask, Catalog, Formula, Manifest, MAX_CATALOG_BYTES};
use crate::commands::info::{validate_cask_token, validate_package_name};
use crate::error::BrewError;
use crate::state::AppState;

/// Endpoints the refresh command fetches from. The Python build script
/// uses the same URLs — keep them in sync if either side changes.
const FORMULA_URL: &str = "https://formulae.brew.sh/api/formula.json";
const CASK_URL: &str = "https://formulae.brew.sh/api/cask.json";
const CATALOG_API_BASE: &str = "https://formulae.brew.sh/api/";

/// HTTP timeout per fetch (whole-request). The catalog files are ~30 MB
/// and ~15 MB; on a typical home connection both arrive in well under
/// 30 seconds. 60 seconds leaves margin for slow networks without
/// stalling the UI thread indefinitely.
const REFRESH_TIMEOUT: Duration = Duration::from_secs(60);

/// User-Agent string for outbound catalog fetches.
const USER_AGENT: &str = "brew-browser/0.1 (+https://github.com/msitarzewski/brew-browser)";

// ---------- IPC payloads ----------

/// Summary surface for the Dashboard / Discover banner — small, snappy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSummary {
    pub as_of: String,
    /// "bundled" or "user-refreshed". (Matches `CatalogSource::as_wire`.)
    pub source: String,
    pub formula_count: usize,
    pub cask_count: usize,
    /// Days between `as_of` and now (UTC, server clock). Negative values
    /// are clamped to 0 (clock skew, future `as_of`).
    pub days_old: i64,
    /// True iff even the bundled catalog failed to parse. UI should
    /// show a fatal banner ("Catalog unavailable — please reinstall").
    pub corrupt: bool,
}

/// Light per-entry record for list views — narrower than the full
/// `Formula` / `Cask` so the IPC payload stays cheap.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntrySummary {
    pub name: String,
    pub desc: Option<String>,
    pub deprecated: bool,
    pub disabled: bool,
}

// ---------- Helpers ----------

fn summarize(catalog: &Catalog) -> CatalogSummary {
    let days_old = compute_days_old(&catalog.as_of);
    CatalogSummary {
        as_of: catalog.as_of.clone(),
        source: catalog.source.as_wire().to_string(),
        formula_count: catalog.formula_count,
        cask_count: catalog.cask_count,
        days_old,
        corrupt: catalog.corrupt,
    }
}

fn compute_days_old(as_of: &str) -> i64 {
    use chrono::{DateTime, Utc};
    if as_of.is_empty() {
        return 0;
    }
    let Ok(t) = as_of.parse::<DateTime<Utc>>() else {
        // Manifest produced by tools/catalog/fetch.py uses `%Y-%m-%dT%H:%M:%SZ`
        // which DateTime::from_str accepts via RFC 3339 — but be defensive
        // for hand-edited manifests.
        return 0;
    };
    let delta = Utc::now() - t;
    delta.num_days().max(0)
}

async fn read_active_catalog(state: &AppState) -> Arc<Catalog> {
    let guard = state.catalog.read().await;
    Arc::clone(&*guard)
}

// ---------- Commands ----------

#[tauri::command]
pub async fn catalog_summary(state: State<'_, AppState>) -> Result<CatalogSummary, BrewError> {
    let catalog = read_active_catalog(&state).await;
    Ok(summarize(&catalog))
}

#[tauri::command]
pub async fn catalog_refresh(state: State<'_, AppState>) -> Result<CatalogSummary, BrewError> {
    // Paranoid-mode gate (Phase 12d). Replaces the prior TODO. With this
    // gate in place, the user's "block all outbound" master switch is
    // honoured by the catalog refresh just like every other network-
    // touching command.
    state.require_network("catalog_refresh").await?;

    // Single-flight enforcement. `try_lock` returns Err immediately if
    // a refresh is already in progress — the user's second click on the
    // Refresh button shouldn't queue, it should fast-fail with a typed
    // error so the UI can show "Already refreshing…".
    let _flight = match state.catalog_refresh_in_flight.try_lock() {
        Ok(guard) => guard,
        Err(_) => {
            return Err(BrewError::InvalidArgument {
                message: "catalog refresh already in progress".into(),
            });
        }
    };

    // Build a polite client.
    let client = reqwest::Client::builder()
        .timeout(REFRESH_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| BrewError::Network {
            url: CATALOG_API_BASE.to_string(),
            message: format!("client build: {e}"),
        })?;

    // Fetch both endpoints. Each goes through `fetch_capped` which
    // enforces the 64 MiB raw cap so a hostile mirror can't OOM us.
    let formula_raw = fetch_capped(&client, FORMULA_URL).await?;
    let cask_raw = fetch_capped(&client, CASK_URL).await?;

    // Quick structural sanity check before we commit anything to disk.
    let formula_count = count_top_level_array(&formula_raw, FORMULA_URL)?;
    let cask_count = count_top_level_array(&cask_raw, CASK_URL)?;

    // gzip both — this is what we'll persist + what `load_user_data`
    // expects on next launch.
    let formula_gz = gzip_compress(&formula_raw)?;
    let cask_gz = gzip_compress(&cask_raw)?;

    let manifest = Manifest {
        as_of: chrono::Utc::now().to_rfc3339(),
        formula_count,
        cask_count,
        formula_compressed_bytes: formula_gz.len() as u64,
        cask_compressed_bytes: cask_gz.len() as u64,
        fetched_from: CATALOG_API_BASE.to_string(),
    };

    Catalog::write_user_data(&state.app_data_dir, &formula_gz, &cask_gz, &manifest).await?;

    // Re-load the newly-written user-data copy through the same parser
    // the next launch will use — this catches any parse drift between
    // raw bytes and on-disk shape immediately rather than at next boot.
    let new_catalog = Catalog::load_user_data(&state.app_data_dir)
        .await?
        .ok_or_else(|| BrewError::Internal {
            message: "wrote user-data catalog but load returned None".into(),
        })?;
    let new_summary = summarize(&new_catalog);

    // Swap the AppState Arc — every subsequent reader sees the fresh
    // catalog. Existing readers holding a clone of the old Arc are fine;
    // we just drop our reference to it.
    {
        let mut guard = state.catalog.write().await;
        *guard = Arc::new(new_catalog);
    }

    Ok(new_summary)
}

#[tauri::command]
pub async fn catalog_lookup_formula(
    name: String,
    state: State<'_, AppState>,
) -> Result<Option<Formula>, BrewError> {
    // Defense in depth — even though the lookup is an in-memory HashMap
    // read with no path composition, validate so the IPC boundary stays
    // uniform with the rest of the surface (security review §12a).
    validate_package_name(&name)?;
    let catalog = read_active_catalog(&state).await;
    Ok(catalog.formulae.get(&name).cloned())
}

#[tauri::command]
pub async fn catalog_lookup_cask(
    name: String,
    state: State<'_, AppState>,
) -> Result<Option<Cask>, BrewError> {
    validate_cask_token(&name)?;
    let catalog = read_active_catalog(&state).await;
    Ok(catalog.casks.get(&name).cloned())
}

#[tauri::command]
pub async fn catalog_formulae_summary(
    state: State<'_, AppState>,
) -> Result<Vec<CatalogEntrySummary>, BrewError> {
    let catalog = read_active_catalog(&state).await;
    let mut out: Vec<CatalogEntrySummary> = catalog
        .formulae
        .values()
        .map(|f| CatalogEntrySummary {
            name: f.name.clone(),
            desc: f.desc.clone(),
            deprecated: f.deprecated,
            disabled: f.disabled,
        })
        .collect();
    // Stable order so the frontend can rely on it for paging / virtualization.
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[tauri::command]
pub async fn catalog_casks_summary(
    state: State<'_, AppState>,
) -> Result<Vec<CatalogEntrySummary>, BrewError> {
    let catalog = read_active_catalog(&state).await;
    let mut out: Vec<CatalogEntrySummary> = catalog
        .casks
        .values()
        .map(|c| CatalogEntrySummary {
            name: c.token.clone(),
            desc: c.desc.clone(),
            deprecated: c.deprecated,
            disabled: c.disabled,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

// ---------- Refresh internals ----------

/// Stream `url` into a `Vec<u8>`, capping at `MAX_CATALOG_BYTES`. The
/// per-chunk loop lets us reject oversize responses before allocating
/// the full body — a hostile mirror that promises 30 MB and streams
/// 30 GB gets cut off at 64 MiB.
async fn fetch_capped(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, BrewError> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(BrewError::HttpStatus {
            url: url.to_string(),
            status: resp.status().as_u16(),
        });
    }
    let mut bytes: Vec<u8> = Vec::with_capacity(8 * 1024 * 1024);
    let mut stream = resp;
    loop {
        let chunk = stream.chunk().await?;
        let Some(chunk) = chunk else { break };
        if bytes.len() as u64 + chunk.len() as u64 > MAX_CATALOG_BYTES {
            return Err(BrewError::Network {
                url: url.to_string(),
                message: format!(
                    "response exceeded {} byte cap",
                    MAX_CATALOG_BYTES
                ),
            });
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn gzip_compress(bytes: &[u8]) -> Result<Vec<u8>, BrewError> {
    use std::io::Write;
    let mut encoder =
        flate2::write::GzEncoder::new(Vec::with_capacity(bytes.len() / 4), flate2::Compression::best());
    encoder.write_all(bytes).map_err(|e| BrewError::Io {
        message: format!("gzip write: {e}"),
    })?;
    encoder.finish().map_err(|e| BrewError::Io {
        message: format!("gzip finish: {e}"),
    })
}

/// Count the elements in the top-level JSON array without fully
/// re-deserializing the records into typed structs. Used pre-write to
/// (a) catch totally non-JSON responses and (b) seed `Manifest.*_count`.
fn count_top_level_array(bytes: &[u8], url: &str) -> Result<usize, BrewError> {
    let v: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|e| BrewError::JsonParse {
            command: url.to_string(),
            message: e.to_string(),
            raw_excerpt: String::new(),
        })?;
    let arr = v.as_array().ok_or_else(|| BrewError::JsonParse {
        command: url.to_string(),
        message: "expected top-level JSON array".into(),
        raw_excerpt: String::new(),
    })?;
    Ok(arr.len())
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::CatalogSource;

    // Most tests don't have an AppState; they go through Catalog
    // directly. Command-level tests that need AppState live in the
    // integration tests folder; the unit tests here cover everything
    // testable in isolation.

    #[test]
    fn summary_carries_source_string() {
        // Build a minimal Catalog by hand (skipping the heavy load).
        let cat = Catalog {
            formulae: Default::default(),
            casks: Default::default(),
            as_of: "2026-05-24T00:00:00Z".to_string(),
            source: CatalogSource::Bundled,
            formula_count: 5,
            cask_count: 3,
            corrupt: false,
        };
        let s = summarize(&cat);
        assert_eq!(s.source, "bundled");
        assert_eq!(s.formula_count, 5);
        assert_eq!(s.cask_count, 3);
        assert!(!s.corrupt);
    }

    #[test]
    fn summary_user_refreshed_source_string() {
        let cat = Catalog {
            formulae: Default::default(),
            casks: Default::default(),
            as_of: "2026-05-24T00:00:00Z".to_string(),
            source: CatalogSource::UserRefreshed,
            formula_count: 0,
            cask_count: 0,
            corrupt: false,
        };
        assert_eq!(summarize(&cat).source, "user-refreshed");
    }

    #[test]
    fn compute_days_old_handles_empty() {
        assert_eq!(compute_days_old(""), 0);
    }

    #[test]
    fn compute_days_old_handles_bad_input() {
        assert_eq!(compute_days_old("not a date"), 0);
    }

    #[test]
    fn compute_days_old_clamps_future_dates_to_zero() {
        // Year 9999 should always be in the future for any practical run.
        let days = compute_days_old("9999-01-01T00:00:00Z");
        assert_eq!(days, 0);
    }

    #[test]
    fn compute_days_old_returns_positive_for_old_date() {
        // Year 2000 is far enough in the past that the result is huge
        // and positive.
        let days = compute_days_old("2000-01-01T00:00:00Z");
        assert!(days > 365 * 20);
    }

    #[test]
    fn gzip_round_trips() {
        let payload = b"hello world, this is a small test payload \xff\x00\x01";
        let gz = gzip_compress(payload).expect("compress");
        use std::io::Read;
        let mut d = flate2::read::GzDecoder::new(&gz[..]);
        let mut out = Vec::new();
        d.read_to_end(&mut out).unwrap();
        assert_eq!(out, payload);
    }

    #[test]
    fn count_top_level_array_counts_elements() {
        let bytes = br#"[{"a":1},{"a":2},{"a":3}]"#;
        let n = count_top_level_array(bytes, "test").expect("count");
        assert_eq!(n, 3);
    }

    #[test]
    fn count_top_level_array_rejects_non_array() {
        let bytes = br#"{"a":1}"#;
        let r = count_top_level_array(bytes, "test");
        assert!(matches!(r, Err(BrewError::JsonParse { .. })));
    }

    // ---------- Catalog-level tests ----------
    //
    // These use the global Catalog directly (no AppState) — they cover
    // the wiring catalog_lookup_* and *_summary commands rely on.

    #[tokio::test]
    async fn bundled_catalog_parses() {
        let cat = Catalog::load_bundled().expect("load bundled");
        assert!(cat.formulae.len() > 1000, "expected >1k formulae");
        assert!(cat.casks.len() > 1000, "expected >1k casks");
    }

    #[tokio::test]
    async fn lookup_known_formula() {
        let cat = Catalog::load_bundled().expect("load bundled");
        let f = cat.formulae.get("wget").cloned().expect("wget present");
        assert_eq!(f.name, "wget");
    }

    #[tokio::test]
    async fn lookup_unknown_returns_none() {
        let cat = Catalog::load_bundled().expect("load bundled");
        let f = cat.formulae.get("this-is-not-a-real-formula-xyzzy").cloned();
        assert!(f.is_none());
    }

    #[tokio::test]
    async fn deprecation_flag_surfaces() {
        // Any deprecated entry in the bundled snapshot proves the flag
        // round-trips through serde.
        let cat = Catalog::load_bundled().expect("load bundled");
        let any_dep = cat.formulae.values().any(|f| f.deprecated);
        assert!(any_dep, "expected at least one deprecated formula");
    }

    #[test]
    fn validate_blocks_invalid_name_for_formula_lookup() {
        // Mirrors what catalog_lookup_formula does first. The formula
        // validator accepts `/` and `.` (tap-qualified names like
        // `homebrew/core/wget` need them), so a path-traversal shape
        // like `../../etc/passwd` is silently treated as a non-match
        // (HashMap miss → Ok(None)). The validator IS still the IPC
        // boundary chokepoint; it must reject anything that would let
        // a flag injection or control-char attack through:
        for bad in &["", "-flag", "foo bar", "foo\0", "foo;bar"] {
            let r = validate_package_name(bad);
            assert!(
                matches!(r, Err(BrewError::InvalidArgument { .. })),
                "expected invalid_argument for {:?}, got {:?}",
                bad,
                r
            );
        }
    }

    #[test]
    fn validate_blocks_invalid_name_for_cask_lookup() {
        // The cask-token validator is stricter — it rejects `/` and
        // leading `.` outright, so traversal-shaped tokens never even
        // reach a HashMap lookup.
        let r = validate_cask_token("../../../etc/passwd");
        assert!(
            matches!(r, Err(BrewError::InvalidArgument { .. })),
            "expected invalid_argument for traversal-shaped token"
        );
        // Plus the same flag/control-char rejections.
        for bad in &["", "-flag", "foo bar", "foo\0", ".hidden"] {
            let r = validate_cask_token(bad);
            assert!(
                matches!(r, Err(BrewError::InvalidArgument { .. })),
                "expected invalid_argument for {:?}, got {:?}",
                bad,
                r
            );
        }
    }
}
