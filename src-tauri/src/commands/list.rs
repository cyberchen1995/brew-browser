//! `brew_list` and `brew_outdated` commands.

use chrono::Utc;
use tauri::State;

use crate::brew::exec::run_brew_capture;
use crate::brew::parse::{RawInfoV2, RawOutdatedV2};
use crate::error::{truncate_head, BrewError};
use crate::state::AppState;
use crate::types::{OutdatedPackage, PackageKind, PackageList};

#[tauri::command]
pub async fn brew_list(
    state: State<'_, AppState>,
    force: Option<bool>,
) -> Result<PackageList, BrewError> {
    let force = force.unwrap_or(false);
    // Cache check — bypass on explicit force=true so the Refresh
    // button (Dashboard) and the post-action reloads (Install /
    // Uninstall / Upgrade / Snapshot restore) get truly fresh data.
    //
    // Without the bypass, a user who ran `brew upgrade` in their
    // terminal would see the same stale outdated list the cache
    // returned on first launch, until they restarted the app.
    if !force {
        let cached = state.installed_cache.read().await;
        if let Some(c) = cached.as_ref() {
            return Ok(c.clone());
        }
    }

    let path = state.require_brew_path().await?;
    let display = "brew info --installed --json=v2";
    let raw = run_brew_capture(&path, &["info", "--installed", "--json=v2"], display).await?;

    let parsed: RawInfoV2 = serde_json::from_str(&raw).map_err(|e| BrewError::JsonParse {
        command: display.to_string(),
        message: e.to_string(),
        raw_excerpt: truncate_head(&raw, 2048),
    })?;

    let formulae: Vec<_> = parsed.formulae.iter().map(|f| f.to_package()).collect();
    let casks: Vec<_> = parsed.casks.iter().map(|c| c.to_package()).collect();

    let list = PackageList {
        formulae,
        casks,
        generated_at: Utc::now().to_rfc3339(),
    };

    {
        let mut cached = state.installed_cache.write().await;
        *cached = Some(list.clone());
    }

    Ok(list)
}

#[tauri::command]
pub async fn brew_outdated(state: State<'_, AppState>) -> Result<Vec<OutdatedPackage>, BrewError> {
    let path = state.require_brew_path().await?;
    let display = "brew outdated --json=v2 --greedy";
    let raw = run_brew_capture(&path, &["outdated", "--json=v2", "--greedy"], display).await?;

    let parsed: RawOutdatedV2 = serde_json::from_str(&raw).map_err(|e| BrewError::JsonParse {
        command: display.to_string(),
        message: e.to_string(),
        raw_excerpt: truncate_head(&raw, 2048),
    })?;

    let mut out = Vec::with_capacity(parsed.formulae.len() + parsed.casks.len());
    out.extend(
        parsed
            .formulae
            .iter()
            .map(|e| e.to_dto(PackageKind::Formula)),
    );
    out.extend(parsed.casks.iter().map(|e| e.to_dto(PackageKind::Cask)));

    Ok(out)
}
