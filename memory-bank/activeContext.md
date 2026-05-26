# Active Context

**Date:** 2026-05-26 (v0.4.0 fully deployed and PR'd)
**State:** All 9 steps of the v0.4.0 plan complete. Branch `feat/v0.4.0-velocity-and-history` is 7+ commits ahead of `main`; deploy verified end-to-end on `brew-browser.zerologic.com` (endpoint 200, POST 405, IP-redacted logs auditable via `grep -c remote_ip → 0`); cron live, first nightly snapshot fires Wed 03:00. PR open against `main`. Once it merges, the v0.4.0 release follows the standard pipeline (sign + notarize + publish manifest + tag GH release + asset rename + manifest rsync). Workflow rule (durable): PRs into main, no direct pushes.

## Repo

- **github.com/msitarzewski/brew-browser** — public, MIT
- **Released:** v0.1.0, v0.2.0, v0.2.1, v0.3.0, v0.3.1 (live on GitHub Releases — `gh release list`)
- **Working toward:** v0.4.0 (PR'd; ship follows merge)
- **Branch:** `feat/v0.4.0-velocity-and-history`
- **Stars:** 18 (as of v0.3.1 ship)

## v0.4.0 shipped on the branch (Steps 1–9)

Full file:line detail + decisions + verification narrative in `tasks/2026-05/19-v0.4.0-backend.md`. Bullet summary:

- **Step 1** — `Settings.enhanced_trending_enabled` (default `false`, forward-compat tested), `state::require_enhanced_trending()` gate composing master paranoid with per-feature toggle, new `BrewError::FeatureDisabled` variant.
- **Step 2** — Parallel `install` + `install-on-request` fetch, `velocity_index(c30, c90, c365) → Option<f64>` pure-math helper (compares recent month vs prior 11-month average so brand-new packages return None instead of saturating the leaderboard at 12.17), server-side velocity back-fill from 3-window join via `tokio::task::JoinSet`.
- **Step 3** — New `trending::history::{mod, client, cache}` module, two IPCs (`trending_history_index`, `trending_history_fetch`), per-package LRU cache (cap 500, TTL 6h), path-traversal-safe URL builder.
- **Step 4** — `SettingsSectionTrendingHistory.svelte` opt-in subsection at the bottom of Network. 6th `pathStatuses` entry. `feature_disabled` variant in `BrewErrorPayload` with friendly toast routing.
- **Step 5** — Trending tab restructure: default sort velocity desc, new Velocity column with 🔥/❄️/dash badges, count cell becomes vertical-flex with inline `TrendingSparkline` beneath. 8-col responsive grid. New shared `TrendingSparkline.svelte` (inline + detail variants). New `trendingHistory.svelte.ts` store.
- **Step 6** — PackageDetail integration: detail-variant sparkline mounted in a new `trend-card` section. Strictly passive when toggle off (no placeholder).
- **Step 7** — `tools/trending-collector/` Node 20+ ESM cron + SQLite + JSON output. Deploys to `brew-browser.zerologic.com:/home/michael/Sites/brew-trending-collector/`. Seed-from-rolling-windows bootstrap means day-zero charts have data.
- **Step 8** — Memory bank + docs: projectbrief nine → ten paths, decisions.md ADR, security.md §16 endpoint audit, techContext.md / backendApi.md §13.14 / frontendComponents.md updates, docs/release-notes/0.4.0.md, README disclosure refresh.
- **Step 9** — Caddy block deployed (handle_path /trending-history/* + site-wide IP-redacted log via `format filter { wrap json; fields { ... delete } }`), cron live, verification curls all green, privacy claim auditable.

## Tests & lint at PR-open

- `cargo test`: **507 passed**, 0 failed, 6 ignored (473 → 507, +34 new)
- `cargo build`: clean
- `npm run check`: 0 errors, 3 pre-existing warnings (same as v0.3.1 baseline)
- `node --check` on every collector .js file: clean
- `caddy validate` on deployed config: clean

## Production verification (brew-browser.zerologic.com)

- `curl -I /trending-history/index.json` → 200, no `Set-Cookie`, no `Server` header, expected security headers all present, `cache-control: public, max-age=21600`
- `curl -IX POST` → 405
- `curl -I /trending-history/formula/wget.json` → 200
- `curl -I /trending-history/formula/nonexistent.json` → 404
- `grep -cE 'remote_ip|client_ip|X-Forwarded-For|X-Real-Ip' /var/log/caddy/brew-browser.log` → 0 (privacy claim verified)
- Cron dry-run: 43s, 12/12 endpoints succeeded, 101 new rows beyond seed, 500 index entries + 18,028 per-package files written
- Real leaderboard top: hermes-agent (v=1372), raullenchai/rapid-mlx (v=159), grafana/gcx (v=140), openssl@4 (v=129) — genuine adoption signal

## Workflow change (durable)

From this branch onward, merges to `main` go through pull requests — push branch, `gh pr create`, review/CI, merge. No more direct pushes to `main`. Persisted in `~/.claude/projects/-Users-michael-Clean/memory/feedback_pr_workflow.md`.

## What's left

- Merge the PR.
- Cut the v0.4.0 release: `tools/build/sign-and-notarize.sh` → `tools/release/publish-manifest.sh 0.4.0` → `gh release create v0.4.0 ...` → `gh api PATCH` for the asset rename → manifest rsync to `brew-browser.zerologic.com:Sites/brew-browser/updater.json`. Same flow as v0.3.1; Tauri-release gotchas in cross-session memory `tauri_release_pipeline_gotchas.md`.

## Memory bank inventory

`toc.md`, `projectbrief.md`, `techContext.md`, `decisions.md`, `activeContext.md` (this), `progress.md`, `systemPatterns.md`, `designSystem.md`, `uxArchitecture.md`, `visualStory.md`, `backendApi.md`, `frontendComponents.md`, `codeReview.md`, `apiTests.md`, `accessibility.md`, `realityCheck.md`, `security.md`, `ideas.md`, `agentLog.md` (dormant), `NEXT-SESSION.md`, `tasks/2026-05/` (19 task records + README + deferred), `phases/`, `scans/2026-05-23/`.
