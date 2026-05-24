# Active Context

**Date:** 2026-05-24 (end-of-session 2026-05-24-night, post-Phase-12 Wave 2)
**State:** Phases 9, 11, 12a, 12b, 12c, 12d, 12e all landed in working tree. 334 tests passing. Only Phase 12f (GitHub authed actions) + Phase 13 (catalog enrichment) remain in the current plan.

## Repo

- **github.com/msitarzewski/brew-browser** — public, MIT, `main` branch
- **Release:** v0.1.0 at <https://github.com/msitarzewski/brew-browser/releases/tag/v0.1.0> — signed/notarized `brew-browser_0.1.0_aarch64.dmg`
- 6 commits to date (next commit cluster pending — Wave 1 + Wave 2):
  - `653e26f` feat: initial release — brew-browser v0.1.0
  - `c72e31d` data: initial LLM-generated package categories + landing page
  - `2dad9be` landing: drop Caddyfile snippet
  - `cb60e4a` build: signed + notarized release pipeline
  - `c2ab41f` memory-bank: NEXT-SESSION handoff doc
  - `84ad010` feat: Phase 9 + 11 — Dashboard, Services, donut, category linking, native vibrancy

## Working tree this session (post-Phase-11 commit `84ad010`)

### Phase 12a — Bundled catalog + manual refresh ✅
- `tools/catalog/fetch.py` + README
- `src-tauri/data/catalog/{formula,cask}.json.gz` + `manifest.json` (6.1 MiB bundled, 8,369 formulae + 7,659 casks as of 2026-05-24T07:59:56Z)
- `src-tauri/src/catalog/mod.rs` — `Catalog`, `Formula`, `Cask`, `Manifest`, `CatalogSource`; size caps (64 MiB raw / 128 MiB decompressed / 4 KiB per field / 200 char names); `load_bundled`/`load_user_data`/`resolve_active`/`write_user_data`; custom deserializers for `license` and `versions.stable`; corrupt-recovery cleanup
- `src-tauri/src/commands/catalog.rs` — 6 commands: `catalog_summary`/`catalog_refresh`/`catalog_lookup_formula`/`catalog_lookup_cask`/`catalog_formulae_summary`/`catalog_casks_summary`. Single-flight refresh via `try_lock`; 60s reqwest timeout; streaming fetch_capped with 64 MiB cap; gzip then atomic write; post-write reload before swap
- `src-tauri/src/util/{mod,fs}.rs` — `atomic_write` (temp + fsync + rename + fsync parent) and `read_capped` (errors on oversize, doesn't truncate). Shared chokepoints used by 12c + 12d
- AppState additions: `catalog: RwLock<Arc<Catalog>>`, `catalog_refresh_in_flight: Arc<Mutex<()>>`. Sync bundled load in `build()`, async upgrade from user-data spawned at startup
- `flate2 = "1"` added
- +38 tests (210 → 248)

### Phase 12b — Settings shell + brew analytics ✅
- `src-tauri/src/commands/brew_env.rs` — `brew_get_analytics` (strict first-line parser), `brew_set_analytics`, `app_version`. Parser extracted as private fn + 8 tests
- `src/lib/components/Settings.svelte` — modal with left-nav, focus trap, Esc + click-outside-to-close, gear-icon trigger, Cmd+, shortcut
- 6 section components: `SettingsSectionAppearance`, `SettingsSectionNetwork`, `SettingsSectionGitHub`, `SettingsSectionBrew`, `SettingsSectionActivity`, `SettingsSectionAbout`
- `src/lib/stores/ui.svelte.ts` — added `settingsOpen`, `defaultSection`, `vibrancyMaterial`, `confirmDestructive`, `activityMaxJobs`, `activityMaxLines` with persistence + clamp validators
- `src/lib/api.ts` — `brewGetAnalytics`, `brewSetAnalytics`, `appVersion` wrappers
- Sidebar gear icon next to theme controls
- +8 tests (248 → 256)

### Phase 12d — Paranoid mode + network settings + settings persistence ✅
- `src-tauri/src/commands/settings.rs` — `Settings` struct, `CatalogAutoRefresh`/`CaskIconMode` enums, `SettingsLoadState { FirstLaunch | Loaded(Settings) | Corrupt(...) }`, `settings_get`/`settings_set`/`settings_reset` commands, `load_at_startup` sync loader, `persist` writer using `atomic_write`
- `state.rs` — `settings: Arc<RwLock<SettingsLoadState>>` field, `require_network(feature)` method (fail-closed on Corrupt, ok on FirstLaunch + Loaded-and-paranoid-off)
- `error.rs` — `ParanoidModeBlocked { feature: String }` variant
- Wired `require_network` into `trending_fetch`, `cask_icon_from_homepage`, `catalog_refresh` as first line of each command
- `src/lib/stores/settings.svelte.ts` — Settings store with `data`/`loading`/`error`/`corruptOnDisk` + `load`/`save`/`reset`
- `SettingsSectionNetwork.svelte` rewritten with Paranoid Mode toggle + warning callout, Catalog auto-refresh radios, stale-banner threshold, Cask icon mode radios, Trending TTL, dynamic disclosure list with allowed/blocked indicators, corrupt-file recovery UI with [Reset to defaults]
- `paranoid_mode_blocked` added to `BrewErrorPayload`
- README "Open by default" updated for 5 paths + Paranoid Mode + corrupt-file fail-closed
- +18 tests (256 → 274)

### Phase 12c + 12e — GitHub anonymous + Device Flow + Keychain ✅ (combined Backend Architect pass)
- `src-tauri/src/github/{mod,url,auth,stats}.rs`
  - `url.rs` — strict `parse_github_url`: exact host match `github.com` (rejects subdomains + suffix attacks), owner+repo `^[A-Za-z0-9._-]{1,39}$`, no `..` segments, strips `.git`/`/tree/...`. 20 validator tests
  - `auth.rs` — Device Flow + Keychain. `Token` newtype with redacted `Debug`. Service ID `dev.openbrew.browser` matches bundle ID (test parses tauri.conf.json). OAuth scopes `["read:user", "public_repo"]` minimum (pinned by test). Polling honors server `interval` + doubles on `slow_down` per RFC 8628 §3.5. No disk fallback on Keychain failure
  - `stats.rs` — `fetch_repo_stats` with 24h disk cache at `app_data_dir/github-cache/<owner>__<repo>.json`. 1 MiB body cap. Rate-limit handling (403 + `X-RateLimit-Remaining: 0` → typed `GithubRateLimited { reset_at }`, no retry, no backoff). Auth header sent when token present
  - `mod.rs` — `#![deny(clippy::print_*, dbg_macro)]` enforced; token never logged
- `src-tauri/src/commands/github.rs` — 5 commands: `github_repo_stats`/`github_status`/`github_signin_start`/`github_signin_poll`/`github_signout`. Every one consults `require_network` AND `settings.github_enabled` before any network attempt
- `settings.rs` — added `github_enabled: bool` field (default false)
- `error.rs` — `GithubRateLimited`/`KeychainUnavailable`/`AuthRequired`/`ScopeRequired` variants (last two prepped for 12f)
- `tauri.conf.json` CSP: `connect-src` adds `https://api.github.com` + `https://github.com` (both in one shot)
- `keyring = "3"` + `url = "2"` added
- `src/lib/stores/github.svelte.ts` — github store with status + repoStatsCache + signIn/signOut/getRepoStats
- `src/lib/components/DeviceFlowModal.svelte` — user code display, "Open in browser" button, poll loop
- `SettingsSectionGitHub.svelte` — toggle + sign-in flow + privacy text
- `PackageDetail.svelte` — GitHub stats card below homepage when settings allow
- BUILD.md addendum: "GitHub OAuth App (one-time setup before release)" 7-step guide. Placeholder `GITHUB_OAUTH_CLIENT_ID` MUST be swapped before any release
- +60 tests (274 → 334)

## Tests & lint (current)

- `cargo test`: **334 passed**, 0 failed, 6 ignored
- `cargo clippy --all-targets -- -D warnings`: clean
- `cargo check`: clean
- `npm run check`: 0 errors, 1 pre-existing tsconfig-node warning
- `npm run build`: clean

## What's left

| Sub-phase | Status |
|-----------|--------|
| 12f — GitHub authed actions (star/unstar/is_starred/watch/unwatch/create_issue + "Wrong?" link + Dashboard personal-stats card) | next |
| Phase 13 — Catalog enrichment (Haiku Tier A friendly names + summaries, then Tier B use cases + similar + tags, AI Features master toggle, full plan in `phase13-plan.md`) | queued, can run parallel with 12f |
| Recipes (Phase 10) | deferred — depends on catalog (now available), pairs naturally with enrichment |
| `installedAt` on Package + Last-Updated sort | small standalone backend addition, not in any phase |
| Tier B Tahoe Liquid Glass (Swift bridge) | v0.2 |
| Phase 14 — bundled cask icons | **explicitly DROPPED** — trademark/redistribution concern raised; runtime probe with paranoid gate is sufficient |

## Memory bank inventory

`toc.md`, `projectbrief.md`, `techContext.md`, `decisions.md`, `activeContext.md` (this), `progress.md`, `systemPatterns.md`, `designSystem.md`, `uxArchitecture.md`, `backendApi.md`, `frontendComponents.md`, `codeReview.md`, `apiTests.md`, `accessibility.md`, `visualStory.md`, `security.md`, `ideas.md`, `phase12-plan.md`, `phase13-plan.md`, `agentLog.md`, `NEXT-SESSION.md`, `scans/{phase12-security-review.md, ...other scans}`, `tasks/2026-05/`.

Also new and uncommitted: `PHILOSOPHY.md` (271 lines, root) — project manifesto added by user during session.
