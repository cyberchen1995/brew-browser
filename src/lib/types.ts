/**
 * TypeScript equivalents of all Rust DTOs from `memory-bank/backendApi.md`.
 *
 * Camel-case JSON shape on the wire — these types match exactly what
 * `invoke()` returns for each Tauri command.
 */

// =========================================================
// 2.1 Common enums
// =========================================================

export type PackageKind = "formula" | "cask";
export type TrendingWindow = "30d" | "90d" | "365d";

/**
 * Where a package's icon can be sourced from.
 *
 * Phase 8 — discriminated union the backend stamps on every `Package` so the
 * frontend can route to the right extractor without knowing implementation
 * details. Lets installed casks use the local `.app` bundle (fast, exact) and
 * uninstalled casks fall back to a homepage favicon (slower, best-effort).
 *
 * - `installedApp`: the cask has an `.app` in `/Applications` or `~/Applications`
 *   — use `cask_icon` to pull from the bundle.
 * - `homepage`: no installed app bundle but the cask has a homepage URL — use
 *   `cask_icon_from_homepage` to fetch a favicon for that origin.
 * - `none`: no icon source available (formulae always; casks without an app
 *   artifact AND without a homepage).
 */
export type IconSource =
  | { kind: "installedApp" }
  | { kind: "homepage"; homepage: string }
  | { kind: "none" };

// =========================================================
// 2.2 Environment probe
// =========================================================

export interface BrewEnvironment {
  installed: boolean;
  version: string | null;
  prefix: string | null;
  pathUsed: string | null;
}

// =========================================================
// 2.3 Package list (Phase 1)
// =========================================================

export interface Package {
  name: string;
  fullName: string;
  kind: PackageKind;
  installedVersion: string | null;
  stableVersion: string | null;
  description: string | null;
  homepage: string | null;
  license: string | null;
  tap: string | null;
  outdated: boolean;
  pinned: boolean;
  installedOnRequest: boolean;
  installedAsDependency: boolean;
  iconSource: IconSource;
}

export interface PackageList {
  formulae: Package[];
  casks: Package[];
  generatedAt: string;
}

// =========================================================
// 2.4 Package detail (Phase 1)
// =========================================================

export interface PackageOption {
  flag: string;
  description: string;
}

export interface PackageDetail {
  package: Package;
  caveats: string | null;
  dependencies: string[];
  buildDependencies: string[];
  optionalDependencies: string[];
  conflictsWith: string[];
  requirements: string[];
  options: PackageOption[];
  installedPaths: string[];
  analytics30dInstalls: number | null;
  rawJson: unknown;
}

// =========================================================
// 2.5 Outdated (Phase 1 / 3)
// =========================================================

export interface OutdatedPackage {
  name: string;
  kind: PackageKind;
  installedVersions: string[];
  currentVersion: string;
  pinned: boolean;
  pinnedVersion: string | null;
}

// =========================================================
// 2.6 Search results (Phase 2)
// =========================================================

export interface SearchHit {
  name: string;
  kind: PackageKind;
  installed: boolean;
  description: string | null;
}

export interface SearchResults {
  query: string;
  formulae: SearchHit[];
  casks: SearchHit[];
  generatedAt: string;
}

// =========================================================
// 2.7 Streaming events (Phase 3 & 4)
// =========================================================

export type BrewStreamEvent =
  | { kind: "started";  jobId: string; command: string; startedAt: string }
  | { kind: "stdout";   jobId: string; line: string; ts: string }
  | { kind: "stderr";   jobId: string; line: string; ts: string }
  | { kind: "progress"; jobId: string; message: string; percent: number | null }
  | { kind: "exit";     jobId: string; exitCode: number; success: boolean; durationMs: number }
  | { kind: "canceled"; jobId: string }
  | { kind: "error";    jobId: string; error: BrewErrorPayload };

export interface JobResult {
  jobId: string;
  exitCode: number;
  success: boolean;
  durationMs: number;
}

export interface StreamHandle {
  jobId: string;
}

// =========================================================
// 2.8 Brewfile (Phase 4)
// =========================================================

export type BrewfileId = string;

export interface BrewfileCounts {
  taps: number;
  formulae: number;
  casks: number;
  masApps: number;
  vscodeExtensions: number;
}

export interface BrewfileSummary {
  id: BrewfileId;
  label: string;
  path: string;
  createdAt: string;
  sizeBytes: number;
  counts: BrewfileCounts;
}

export interface BrewfileFormula { name: string; args: string[] }
export interface BrewfileCask    { name: string; args: string[] }
export interface BrewfileMasApp  { name: string; id: number }

export interface BrewfileEntries {
  taps: string[];
  formulae: BrewfileFormula[];
  casks: BrewfileCask[];
  masApps: BrewfileMasApp[];
  vscodeExtensions: string[];
}

export interface Brewfile {
  summary: BrewfileSummary;
  entries: BrewfileEntries;
  rawText: string;
}

export interface BrewfileCheckReport {
  satisfied: boolean;
  missingTaps: string[];
  missingFormulae: string[];
  missingCasks: string[];
  missingMasApps: string[];
  missingVscodeExtensions: string[];
}

// =========================================================
// 2.9.1 Categories (Phase 9)
// =========================================================

/**
 * One entry in the `categories` map of `categories.json`. The backend bundles
 * the JSON at compile time (see `commands/categories.rs`) so this shape must
 * match the Rust `CategoryMeta` struct.
 *
 * `icon` is the PascalCase name of a Lucide icon (e.g. "Cloud", "Brain"). The
 * frontend resolves it via a static map in `lib/util/categoryIcon.ts` rather
 * than dynamic imports.
 */
export interface CategoryMeta {
  label: string;
  icon: string;
}

/**
 * Full payload of `categories_data`. `casks` and `formulae` map token → array
 * of category slugs (multiple categories per item is the norm).
 */
export interface CategoriesData {
  version: string;
  generatedAt: string;
  model: string;
  categories: Record<string, CategoryMeta>;
  casks: Record<string, string[]>;
  formulae: Record<string, string[]>;
}

// =========================================================
// 2.9.3 Services (brew services)
// =========================================================

/**
 * Raw status string from `brew services list --json`. Observed values:
 * "started", "stopped", "none", "error", "scheduled", "unknown".
 * The frontend treats unknown values as `unknown` rather than crashing.
 */
export interface Service {
  name: string;
  status: string;
  user: string | null;
  file: string | null;
  exitCode: number | null;
}

/** Normalised status for the UI's tone/icon mapping. */
export type ServiceStatus = "started" | "stopped" | "none" | "error" | "scheduled" | "unknown";

export function normalizeServiceStatus(raw: string): ServiceStatus {
  switch (raw) {
    case "started":   return "started";
    case "stopped":   return "stopped";
    case "none":      return "none";
    case "error":     return "error";
    case "scheduled": return "scheduled";
    default:          return "unknown";
  }
}

// =========================================================
// 2.9.2 Disk usage (Dashboard Storage card)
// =========================================================

export interface DiskUsageEntry {
  label: string;
  path: string;
  bytes: number;
  exists: boolean;
  error: string | null;
}

export interface DiskUsageReport {
  generatedAt: string;
  prefix: string;
  cacheDir: string;
  entries: DiskUsageEntry[];
  totalBytes: number;
  cacheAgeSeconds: number;
}

// =========================================================
// 2.9 Trending (Phase 6)
// =========================================================

export interface TrendingEntry {
  rank: number;
  name: string;
  kind: PackageKind;
  installCount: number;
  installCountFormatted: string;
  installedLocally: boolean;
}

export interface TrendingReport {
  window: TrendingWindow;
  fetchedAt: string;
  cacheAgeSeconds: number;
  totalCount: number;
  entries: TrendingEntry[];
}

// =========================================================
// 2.10 Settings (Phase 12d)
// =========================================================

/**
 * Catalog auto-refresh cadence. Wire-format mirrors the Rust enum
 * `CatalogAutoRefresh` (kebab-case).
 */
export type CatalogAutoRefresh = "off" | "weekly" | "daily";

/**
 * Cask icon fetching mode. `all` matches the current Phase 8 behaviour
 * where every uninstalled cask with a homepage probes for a favicon.
 * `installed-only` skips the homepage cascade; `off` disables even
 * installed-app icon extraction.
 */
export type CaskIconMode = "off" | "installed-only" | "all";

/**
 * Persisted user settings (Phase 12d). Lives at
 * `~/Library/Application Support/brew-browser/settings.json` and is
 * round-tripped via `settingsGet` / `settingsSet`.
 *
 * Bounds (enforced server-side, also re-checked client-side for snappier
 * UX): `catalogStaleBannerDays` ∈ [1, 365]; `trendingTtlMinutes` ∈ [5, 1440].
 */
export interface Settings {
  /** Master switch — when true, every outbound command fails with
      `paranoid_mode_blocked`. */
  paranoidMode: boolean;
  catalogAutoRefresh: CatalogAutoRefresh;
  catalogStaleBannerDays: number;
  caskIconMode: CaskIconMode;
  trendingTtlMinutes: number;
  /** Phase 12c — when true, PackageDetail probes `api.github.com` for
      repo stats whenever the package's homepage is a GitHub URL. Off
      by default; the user opts in via Settings → GitHub. Independent
      of sign-in (anonymous probes still get the 60/hr public limit). */
  githubEnabled: boolean;
}

/** Defaults matching the Rust `Settings::default()`. Used when seeding
    the settings store before the first `settingsGet` resolves so the UI
    doesn't have to render an empty state. */
export const SETTINGS_DEFAULTS: Settings = {
  paranoidMode: false,
  catalogAutoRefresh: "off",
  catalogStaleBannerDays: 14,
  caskIconMode: "all",
  trendingTtlMinutes: 60,
  // Phase 12c — anonymous GitHub stats opt-in. Off by default per the
  // "zero outbound unless user consented" posture.
  githubEnabled: false,
};

// =========================================================
// 2.11 GitHub (Phase 12c + 12e)
// =========================================================

/**
 * Anonymous (or token-authenticated) repo metadata fetched from
 * `api.github.com/repos/{owner}/{repo}`. The backend caches the
 * response on disk for 24h, keyed by the validated owner/repo pair.
 *
 * `null`-able fields are absent on real-world repos: a repo with no
 * GitHub release will have `lastReleaseTag === null`, a live repo
 * will have `archivedAt === null`, etc.
 */
export interface RepoStats {
  owner: string;
  repo: string;
  stars: number;
  forks: number;
  openIssues: number;
  lastReleaseTag: string | null;
  lastReleaseDate: string | null;
  archived: boolean;
  archivedAt: string | null;
  licenseSpdx: string | null;
  defaultBranch: string;
  primaryLanguage: string | null;
}

/**
 * Sign-in status surface returned by `githubStatus`.
 *
 * **Token is never on the wire** — only the derived "what can the
 * session do?" view is. See `github::auth::GithubStatusDto` in the
 * backend for the matching Rust struct and the regression test that
 * pins the wire shape.
 */
export interface GithubStatus {
  signedIn: boolean;
  username: string | null;
  scopes: string[];
}

/**
 * Result of `githubSigninStart` — payload the frontend uses to show
 * the user code and drive the polling loop.
 */
export interface DeviceFlowStart {
  /** Short human-readable code (e.g. `WDJB-MJHT`) to type at
      `verificationUri`. */
  userCode: string;
  /** URL to open in the browser (usually `github.com/login/device`). */
  verificationUri: string;
  /** Seconds until `deviceCode` expires. After this, polling will
      return `expired`. */
  expiresIn: number;
  /** Server-recommended polling cadence in seconds. Must be honoured. */
  interval: number;
  /** Opaque code passed to `githubSigninPoll`. Never shown to the user. */
  deviceCode: string;
}

/**
 * Discriminated union returned by each `githubSigninPoll` call.
 *
 * The `slowDown` variant means GitHub asked us to back off — the
 * frontend should double its polling interval before the next call,
 * per RFC 8628 §3.5.
 */
export type DeviceFlowPoll =
  | { kind: "pending" }
  | { kind: "slowDown" }
  | { kind: "approved"; username: string | null; scopes: string[] }
  | { kind: "denied" }
  | { kind: "expired" };

// =========================================================
// 3.3 Error model
// =========================================================

export type BrewErrorPayload =
  | { code: "brew_not_found" }
  | { code: "brew_exit_non_zero"; command: string; exitCode: number; stderrExcerpt: string; friendlyMessage?: string }
  | { code: "json_parse";         command: string; message: string; rawExcerpt: string }
  | { code: "io";                 message: string }
  | { code: "network";            url: string; message: string }
  | { code: "http_status";        url: string; status: number }
  | { code: "invalid_argument";   message: string }
  | { code: "job_not_found";      jobId: string }
  | { code: "canceled" }
  | { code: "brewfile_not_found"; id: string }
  | { code: "internal";           message: string }
  | { code: "paranoid_mode_blocked"; feature: string }
  | { code: "github_rate_limited"; resetAt: number }
  | { code: "keychain_unavailable"; message: string }
  | { code: "auth_required" }
  | { code: "scope_required"; scope: string };

/** Type-narrowing helper: is the thrown value a BrewErrorPayload? */
export function isBrewError(e: unknown): e is BrewErrorPayload {
  return (
    typeof e === "object" &&
    e !== null &&
    "code" in e &&
    typeof (e as { code: unknown }).code === "string"
  );
}

/** Human-readable message for a BrewError. */
export function brewErrorMessage(e: BrewErrorPayload): string {
  switch (e.code) {
    case "brew_not_found":      return "Homebrew not found on PATH.";
    case "brew_exit_non_zero":  return e.friendlyMessage ?? `brew exited ${e.exitCode}: ${e.stderrExcerpt}`;
    case "json_parse":          return `Failed to parse brew output: ${e.message}`;
    case "io":                  return `I/O error: ${e.message}`;
    case "network":             return `Network error: ${e.message}`;
    case "http_status":         return `HTTP ${e.status} from ${e.url}`;
    case "invalid_argument":    return `Invalid argument: ${e.message}`;
    case "job_not_found":       return `Job ${e.jobId} not found.`;
    case "canceled":            return "Operation canceled.";
    case "brewfile_not_found":  return `Brewfile "${e.id}" not found.`;
    case "internal":            return `Internal error: ${e.message}`;
    case "paranoid_mode_blocked":
      return `Paranoid mode is on — ${e.feature} is blocked. Disable it in Settings → Network.`;
    case "github_rate_limited": {
      const reset = e.resetAt > 0 ? new Date(e.resetAt * 1000).toLocaleTimeString() : "soon";
      return `GitHub API rate limit reached. Resets at ${reset}. Sign in to lift the limit.`;
    }
    case "keychain_unavailable":
      return `macOS Keychain unavailable: ${e.message}`;
    case "auth_required":
      return "Sign in to GitHub to use this feature.";
    case "scope_required":
      return `GitHub permission "${e.scope}" required. Sign in again to grant it.`;
  }
}

// =========================================================
// UI-only types (frontend stores, command palette, etc.)
// =========================================================

export type SidebarSection =
  | "dashboard"
  | "library"
  | "discover"
  | "trending"
  | "snapshots"
  | "services"
  | "activity";

export type ThemePreference = "light" | "dark" | "system";

/** A job tracked locally on the frontend (status + accumulated lines). */
export interface ActivityJob {
  jobId: string;
  label: string;             // human-friendly: "Installing wget"
  command: string;
  startedAt: string;
  status: "running" | "succeeded" | "failed" | "canceled";
  lines: ActivityLine[];
  exitCode?: number;
  durationMs?: number;
}

export interface ActivityLine {
  stream: "stdout" | "stderr";
  text: string;
  ts: string;
}

/** Command-palette item — either a verb (action) or a package. */
export type PaletteItem =
  | { kind: "command"; id: string; label: string; shortcut?: string; section?: string; run: () => void | Promise<void> }
  | { kind: "package"; name: string; pkgKind: PackageKind; installed: boolean; description?: string | null };
