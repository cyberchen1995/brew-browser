/**
 * Typed `invoke()` wrappers for every backend command in `memory-bank/backendApi.md`.
 *
 * Convention: each function resolves with the typed result, or *throws* a
 * `BrewErrorPayload`-shaped object on backend error. Callers should use
 * `try/catch` and `isBrewError(e)` to narrow.
 *
 * Streaming commands additionally take an `onEvent(BrewStreamEvent)` callback
 * — the underlying Tauri `Channel<T>` is wired up here so callers don't have
 * to import `@tauri-apps/api/core` directly.
 *
 * NOTE: backend may not be implemented yet. If `invoke()` itself throws (e.g.
 * unknown command) the error propagates — callers should handle that case
 * gracefully (show "Backend not ready" rather than crashing).
 */

import { invoke, Channel } from "@tauri-apps/api/core";

import type {
  BrewEnvironment,
  Brewfile,
  BrewfileCheckReport,
  BrewfileId,
  BrewfileSummary,
  BrewStreamEvent,
  CategoriesData,
  DeviceFlowPoll,
  DeviceFlowStart,
  DiskUsageReport,
  GithubStatus,
  JobResult,
  OutdatedPackage,
  Package,
  PackageDetail,
  PackageKind,
  PackageList,
  RepoStats,
  SearchResults,
  Service,
  Settings,
  TrendingReport,
  TrendingWindow,
} from "./types";

// ============================================================
// Phase 1 — read-only browser
// ============================================================

export function brewDoctor(): Promise<BrewEnvironment> {
  return invoke<BrewEnvironment>("brew_doctor");
}

export function brewList(): Promise<PackageList> {
  return invoke<PackageList>("brew_list");
}

export function brewInfo(name: string, kind: PackageKind): Promise<PackageDetail> {
  return invoke<PackageDetail>("brew_info", { name, kind });
}

export function brewOutdated(): Promise<OutdatedPackage[]> {
  return invoke<OutdatedPackage[]>("brew_outdated");
}

// ============================================================
// Phase 2 — search
// ============================================================

export function brewSearch(query: string): Promise<SearchResults> {
  return invoke<SearchResults>("brew_search", { query });
}

export function brewSearchDesc(query: string): Promise<SearchResults> {
  return invoke<SearchResults>("brew_search_desc", { query });
}

// ============================================================
// Phase 3 — install / uninstall / upgrade (streaming)
// ============================================================

/** Helper: wires a Tauri Channel<BrewStreamEvent> to a callback. */
function makeChannel(onEvent: (evt: BrewStreamEvent) => void): Channel<BrewStreamEvent> {
  const channel = new Channel<BrewStreamEvent>();
  channel.onmessage = onEvent;
  return channel;
}

export function brewInstall(
  name: string,
  kind: PackageKind,
  onEvent: (evt: BrewStreamEvent) => void,
): Promise<JobResult> {
  return invoke<JobResult>("brew_install", {
    name,
    kind,
    onEvent: makeChannel(onEvent),
  });
}

export function brewUninstall(
  name: string,
  kind: PackageKind,
  zap: boolean,
  onEvent: (evt: BrewStreamEvent) => void,
): Promise<JobResult> {
  return invoke<JobResult>("brew_uninstall", {
    name,
    kind,
    zap,
    onEvent: makeChannel(onEvent),
  });
}

export function brewUpgrade(
  name: string | null,
  onEvent: (evt: BrewStreamEvent) => void,
): Promise<JobResult> {
  return invoke<JobResult>("brew_upgrade", {
    name,
    onEvent: makeChannel(onEvent),
  });
}

export function brewUpdate(
  onEvent: (evt: BrewStreamEvent) => void,
): Promise<JobResult> {
  return invoke<JobResult>("brew_update", {
    onEvent: makeChannel(onEvent),
  });
}

export function cancelJob(jobId: string): Promise<void> {
  return invoke<void>("cancel_job", { jobId });
}

// ============================================================
// Phase 4 — Brewfile snapshot + restore
// ============================================================

export function brewfileDump(
  label: string,
  onEvent: (evt: BrewStreamEvent) => void,
): Promise<BrewfileSummary> {
  return invoke<BrewfileSummary>("brewfile_dump", {
    label,
    onEvent: makeChannel(onEvent),
  });
}

export function brewfileInstall(
  id: BrewfileId,
  onEvent: (evt: BrewStreamEvent) => void,
): Promise<JobResult> {
  return invoke<JobResult>("brewfile_install", {
    id,
    onEvent: makeChannel(onEvent),
  });
}

export function brewfileCheck(id: BrewfileId): Promise<BrewfileCheckReport> {
  return invoke<BrewfileCheckReport>("brewfile_check", { id });
}

export function brewfileList(): Promise<BrewfileSummary[]> {
  return invoke<BrewfileSummary[]>("brewfile_list");
}

export function brewfileRead(id: BrewfileId): Promise<Brewfile> {
  return invoke<Brewfile>("brewfile_read", { id });
}

export function brewfileDelete(id: BrewfileId): Promise<void> {
  return invoke<void>("brewfile_delete", { id });
}

export function brewfileExport(id: BrewfileId, targetPath: string): Promise<void> {
  return invoke<void>("brewfile_export", { id, targetPath });
}

export function brewfileImport(sourcePath: string, label: string): Promise<BrewfileSummary> {
  return invoke<BrewfileSummary>("brewfile_import", { sourcePath, label });
}

// ============================================================
// Phase 6 — trending
// ============================================================

export function trendingFetch(window: TrendingWindow): Promise<TrendingReport> {
  return invoke<TrendingReport>("trending_fetch", { window });
}

export function trendingClearCache(): Promise<void> {
  return invoke<void>("trending_clear_cache");
}

// ============================================================
// Phase 7 — cask icons
// ============================================================

/**
 * Fetch a cask icon as a base64 data URL (e.g. `data:image/png;base64,…`).
 *
 * Returns `null` when the cask has no resolvable icon (no .app bundle,
 * extraction failed, or network unavailable). Backend (`cask_icon`) handles
 * its own disk caching; the frontend keeps an in-memory layer via the
 * `iconCache` store to avoid re-invoking on every PackageRow render.
 *
 * Only meaningful for `kind === "cask"` — formulae are CLI tools and have
 * no icon. Callers should gate on kind before invoking.
 */
export function caskIcon(token: string): Promise<string | null> {
  return invoke<string | null>("cask_icon", { token });
}

/**
 * Fetch a homepage-derived icon (favicon) for a cask that has no installed
 * `.app` bundle. Returns a base64 data URL on success, `null` on miss/error.
 *
 * Same return semantics as `caskIcon` — the iconCache store treats `null` as
 * sticky so a known-missing cask won't keep retrying within the session. The
 * backend (`cask_icon_from_homepage`) handles its own disk cache (7-day TTL)
 * keyed by token, so calling twice for the same cask = cache hit on the
 * backend.
 *
 * Routing happens in `iconCache.getIcon(pkg)` via `pkg.iconSource.kind`; call
 * sites typically don't invoke this directly.
 */
export function caskIconFromHomepage(token: string, homepage: string): Promise<string | null> {
  return invoke<string | null>("cask_icon_from_homepage", { token, homepage });
}

// ============================================================
// Phase 9 — categories
// ============================================================

/**
 * Fetch the bundled `categories.json` payload (19 categories + 15,974
 * categorized tokens). The backend embeds the JSON at compile time via
 * `include_str!` and memoises the parsed result, so subsequent invocations
 * within the same process are effectively free.
 *
 * Frontend callers should hit this via the `categoriesStore` rather than
 * invoking directly — the store caches across components and exposes the
 * derived helpers used by Discover / Library / Trending.
 */
export function categoriesData(): Promise<CategoriesData> {
  return invoke<CategoriesData>("categories_data");
}

// ============================================================
// Dashboard — disk usage + Finder reveal
// ============================================================

/**
 * Probe disk usage for the four canonical Homebrew sub-trees (Cellar,
 * Caskroom, var/log, download cache). Backend caches the result for ~60 s
 * to keep Dashboard renders cheap.
 */
export function diskUsage(): Promise<DiskUsageReport> {
  return invoke<DiskUsageReport>("disk_usage");
}

/** Force the next `diskUsage()` call to re-run `du` instead of using cache. */
export function diskUsageClearCache(): Promise<void> {
  return invoke<void>("disk_usage_clear_cache");
}

/**
 * Reveal a path in macOS Finder. Backend gates against paths outside the
 * Homebrew prefix and cache, so the frontend can only request paths the
 * disk-usage report itself surfaced.
 */
export function openInFinder(path: string): Promise<void> {
  return invoke<void>("open_in_finder", { path });
}

// ============================================================
// Services (brew services)
// ============================================================

export function servicesList(): Promise<Service[]> {
  return invoke<Service[]>("services_list");
}

export function servicesClearCache(): Promise<void> {
  return invoke<void>("services_clear_cache");
}

export function servicesStart(name: string): Promise<void> {
  return invoke<void>("services_start", { name });
}

export function servicesStop(name: string): Promise<void> {
  return invoke<void>("services_stop", { name });
}

export function servicesRestart(name: string): Promise<void> {
  return invoke<void>("services_restart", { name });
}

// ============================================================
// Phase 12b — Settings (brew analytics + app version)
// ============================================================

/**
 * Read the user's current Homebrew analytics posture.
 *
 * Shells `brew analytics state` and parses the first line of stdout.
 * Throws `BrewErrorPayload` with `code === "internal"` if brew prints
 * anything unrecognised (a defensive behaviour per the Phase 12 security
 * review — we'd rather surface "unexpected output" than guess).
 */
export function brewGetAnalytics(): Promise<boolean> {
  return invoke<boolean>("brew_get_analytics");
}

/**
 * Set the user's Homebrew analytics posture. Takes the brew write lock
 * because `brew analytics on|off` mutates global brew state.
 */
export function brewSetAnalytics(enabled: boolean): Promise<void> {
  return invoke<void>("brew_set_analytics", { enabled });
}

/**
 * App version string from `tauri::App::package_info()` — the source of
 * truth is `Cargo.toml` (mirrored by `tauri.conf.json`). Cheaper and
 * more honest than reading `package.json` from the renderer.
 */
export function appVersion(): Promise<string> {
  return invoke<string>("app_version");
}

// ============================================================
// Phase 12d — Settings persistence
// ============================================================

/**
 * Read the currently-loaded settings.
 *
 * Throws a `BrewErrorPayload` with `code === "internal"` when the
 * settings file on disk is unparseable — in that case the backend is
 * already failing closed (`require_network` denies all outbound calls
 * until the user resets). The Settings UI should catch the throw and
 * show a "Settings file unreadable — Reset to defaults?" affordance
 * that calls `settingsReset()`.
 */
export function settingsGet(): Promise<Settings> {
  return invoke<Settings>("settings_get");
}

/**
 * Persist a complete settings object. Returns the canonicalized
 * settings (numerics clamped, etc.) so the caller can re-broadcast the
 * authoritative values to the store.
 */
export function settingsSet(settings: Settings): Promise<Settings> {
  return invoke<Settings>("settings_set", { settings });
}

/**
 * Overwrite `settings.json` with defaults. Used by the "Reset to
 * defaults" button in Settings → Network when the file is corrupt or
 * the user wants to start fresh.
 */
export function settingsReset(): Promise<Settings> {
  return invoke<Settings>("settings_reset");
}

// ============================================================
// Phase 12c + 12e — GitHub integration
// ============================================================

/**
 * Fetch repo stats for `homepage`. Returns `null` when:
 * - The user hasn't enabled GitHub stats in Settings (the toggle
 *   defaults off).
 * - `homepage` doesn't parse as a `github.com/<owner>/<repo>` URL.
 * - The repo returns 404.
 *
 * Throws `BrewErrorPayload` with `code === "paranoid_mode_blocked"`
 * when paranoid mode is on (regardless of the GitHub toggle), or
 * `"github_rate_limited"` when the anonymous 60/hr per-IP cap is hit.
 *
 * Backend handles its own 24h disk cache, so calling twice for the
 * same homepage = cache hit on the backend.
 */
export function githubRepoStats(homepage: string): Promise<RepoStats | null> {
  return invoke<RepoStats | null>("github_repo_stats", { homepage });
}

/**
 * Read the current sign-in status. Reads from the macOS Keychain only —
 * no network call. The DTO contains `{ signedIn, username, scopes }`,
 * never the token. Callers should `loadStatus()` on mount + after each
 * sign-in / sign-out.
 */
export function githubStatus(): Promise<GithubStatus> {
  return invoke<GithubStatus>("github_status");
}

/**
 * Begin a GitHub Device Flow sign-in. POSTs to
 * `github.com/login/device/code` and returns the user code +
 * verification URI to show in the DeviceFlowModal.
 *
 * Subject to the paranoid-mode gate — the sign-in handshake itself is
 * outbound and gets blocked when "Block all outbound" is on.
 */
export function githubSigninStart(): Promise<DeviceFlowStart> {
  return invoke<DeviceFlowStart>("github_signin_start");
}

/**
 * Poll the token endpoint once with the opaque `deviceCode` returned
 * by `githubSigninStart`. Returns a tagged union — caller drives the
 * polling loop using the `interval` from the start response and
 * doubles it on `slowDown` per RFC 8628 §3.5.
 */
export function githubSigninPoll(deviceCode: string): Promise<DeviceFlowPoll> {
  return invoke<DeviceFlowPoll>("github_signin_poll", { deviceCode });
}

/**
 * Delete the stored OAuth token (and cached username/scopes) from the
 * macOS Keychain. Idempotent.
 */
export function githubSignout(): Promise<void> {
  return invoke<void>("github_signout");
}

// ============================================================
// Re-exports for convenience
// ============================================================

export type { Package };
