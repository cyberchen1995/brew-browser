//! Trending tab commands: `trending_fetch` and `trending_clear_cache`.

use std::collections::HashSet;
use std::time::Instant;

use tauri::State;

use crate::error::BrewError;
use crate::state::AppState;
use crate::trending::cache::{CachedTrending, TRENDING_TTL};
use crate::trending::client;
use crate::types::{TrendingReport, TrendingWindow};

#[tauri::command]
pub async fn trending_fetch(
    window: TrendingWindow,
    state: State<'_, AppState>,
) -> Result<TrendingReport, BrewError> {
    // Paranoid-mode gate (Phase 12d). Even a stale cache hit is
    // disallowed in paranoid mode — the user's expectation is "no
    // outbound calls would happen here", and even though the cached
    // path doesn't hit the network, returning fresh-looking data
    // contradicts the toggle. The cost is zero (the gate is a single
    // RwLock read), the policy is unambiguous.
    state.require_network("trending_fetch").await?;

    // 1. Short critical section: check cache freshness.
    {
        let cache = state.trending_cache.lock().await;
        if let Some(cached) = cache.get(window) {
            let age = cached.fetched_at.elapsed();
            if age < TRENDING_TTL {
                let mut report = cached.report.clone();
                report.cache_age_seconds = age.as_secs();
                return Ok(report);
            }
        }
    }

    // 2. Fetch.
    let installed_set = build_installed_set(&state).await;
    let fetched = client::fetch(window, &installed_set).await;

    match fetched {
        Ok(report) => {
            // 3. Insert into cache.
            let mut cache = state.trending_cache.lock().await;
            cache.put(
                window,
                CachedTrending {
                    fetched_at: Instant::now(),
                    report: report.clone(),
                },
            );
            Ok(report)
        }
        Err(e) => {
            // 4. Fall back to stale cache if available.
            let cache = state.trending_cache.lock().await;
            if let Some(cached) = cache.get(window) {
                let mut report = cached.report.clone();
                report.cache_age_seconds = cached.fetched_at.elapsed().as_secs();
                return Ok(report);
            }
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn trending_clear_cache(state: State<'_, AppState>) -> Result<(), BrewError> {
    let mut cache = state.trending_cache.lock().await;
    cache.clear();
    Ok(())
}

async fn build_installed_set(state: &AppState) -> HashSet<String> {
    let cache = state.installed_cache.read().await;
    let mut set = HashSet::new();
    if let Some(list) = cache.as_ref() {
        for p in list.formulae.iter().chain(list.casks.iter()) {
            set.insert(p.name.clone());
        }
    }
    set
}
