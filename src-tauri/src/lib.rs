//! brew-browser — Tauri 2 backend entrypoint.
//!
//! Module layout per `memory-bank/backendApi.md` §9. This file is the
//! Tauri Builder + invoke_handler registration; every command lives
//! in `commands::*`.

mod brew;
mod catalog;
mod commands;
mod error;
mod github;
mod state;
mod trending;
mod types;
mod util;

use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Best-effort tracing setup — silent if RUST_LOG is unset.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,brew_browser_lib=info")),
        )
        .try_init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            state::initialize(app)?;
            #[cfg(target_os = "macos")]
            {
                // Apply NSVisualEffectView to the main window so it picks up the
                // native macOS "frosted glass" appearance. Material::HudWindow
                // gives a slightly heavier blur that looks good behind the
                // sidebar and main panes; the WebView background must be set
                // transparent in CSS (see app.css :root) for the blur to show.
                use tauri::Manager;
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
                if let Some(window) = app.get_webview_window("main") {
                    let _ = apply_vibrancy(
                        &window,
                        NSVisualEffectMaterial::HudWindow,
                        Some(NSVisualEffectState::Active),
                        None,
                    );
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            brew_doctor,
            brew_get_analytics,
            brew_set_analytics,
            brew_list,
            brew_outdated,
            brew_info,
            brew_search,
            brew_search_desc,
            brew_install,
            brew_uninstall,
            brew_upgrade,
            brew_update,
            cancel_job,
            brewfile_dump,
            brewfile_install,
            brewfile_check,
            brewfile_list,
            brewfile_read,
            brewfile_delete,
            brewfile_export,
            brewfile_import,
            trending_fetch,
            trending_clear_cache,
            cask_icon,
            cask_icon_from_homepage,
            catalog_summary,
            catalog_refresh,
            catalog_lookup_formula,
            catalog_lookup_cask,
            catalog_formulae_summary,
            catalog_casks_summary,
            categories_data,
            disk_usage,
            disk_usage_clear_cache,
            open_in_finder,
            services_list,
            services_clear_cache,
            services_start,
            services_stop,
            services_restart,
            settings_get,
            settings_set,
            settings_reset,
            github_repo_stats,
            github_status,
            github_signin_start,
            github_signin_poll,
            github_signout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
