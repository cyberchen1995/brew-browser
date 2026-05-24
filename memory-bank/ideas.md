# Ideas Backlog

Captured during session; not yet scoped or scheduled. Sorted by user enthusiasm, not difficulty.

## High signal — worth scoping properly

### Recipes — guided multi-package install flows

User-described: *"Like recipes: 'Want to set up local inference? Let's do a quick machine specs run to see what you can do! Here's what can do, and how.'"*

Shape:
- A small library of curated "scenario → package set" recipes (local-inference, web-dev-mac, podcast-editing, ml-research, etc.)
- Each recipe: name, narrative blurb, optional pre-flight checks (RAM ≥ N, free disk ≥ N, GPU ≥ X, macOS version), then a curated list of `formulae` + `casks` to install in order, with per-step explanations
- Pre-flight runs locally (no network) using existing host probes — `sysctl hw.memsize`, `df -h`, `system_profiler SPDisplaysDataType`, `sw_vers`
- Renders as a Library-style detail view with an "Apply recipe" button that streams through the install queue (reuses Activity drawer pattern)
- Stored as YAML/JSON in `src-tauri/data/recipes/<recipe-id>.yaml`, bundled at build time, contributor-friendly (PRs add recipes)
- Same "Wrong?" GitHub-issue link for recipe improvements

Why it lands: the user's framing is exactly what's missing from `brew install` UX today — Homebrew is a package manager, recipes are *workflows*. Maps cleanly onto already-built primitives (install streaming, snapshot/restore, categories).

### GitHub sign-in (optional)

Optional GitHub OAuth. Once signed in:
- "Wrong?" categorization link can post issues directly without opening browser
- "Star this repo" affordance for upstream visibility
- Bug-report button pre-fills issue with system info (OS version, brew version, app version)
- "What recipes have I run" history could sync as a gist (opt-in)

Posture: **strictly optional, no nag, no required.** Per project values: no accounts, no telemetry. OAuth has to feel like a power-user shortcut, not a gate.

Implementation: GitHub OAuth Device Flow (no client secret, designed for desktop apps). Tauri has community plugins; or roll a tiny implementation since the flow is ~50 lines.

## Lower-touch ideas (from the Discover-UI discussion)

- App icon thumbnails inside category tiles (top-3 popular per category, visual variety)
- "Recommended for you" tile based on your installed-cask mix (LLM at build time on a popular subset)
- Search within a single category
- Multi-select categories (intersection: "AI ∩ Developer Tools")
- Saved searches / pinned categories
- "What's new this week" surfaced via the cron diff (the categorize tool already tracks adds/removes)
- Per-cask "Similar to" suggestions (LLM at build time, pairs)

## Phase 9 candidates (close to ready)

- **Category-aware UI everywhere:** chip filters on Library + Trending, not just Discover
- **Per-package "Wrong?" link:** GitHub-issue deeplink with prefilled title + body (template documented in `decisions.md`)
- **Recipes core:** ship 5 recipes (local-inference, web-dev, podcast-edit, ml-research, system-utilities-bundle) with the apply-and-stream flow

## Phase 10+ candidates

- GitHub OAuth Device Flow + optional sign-in
- Recipe contributor docs + a few community-contributed recipes
- App icon thumbnails in tiles
- "What's new" tab driven by the cron diff
- Multi-select category intersection
- Per-cask LLM-generated "similar" suggestions

## Honest no (at least for now)

- Cloud sync of any kind — breaks the no-accounts posture
- A "BrewBucks" tier — there is no tier
- Built-in payments for paid casks (not how brew works)
- Custom URL-scheme handlers (brew-browser://...) — surface area, no clear win

## Resolved this session (2026-05-24-night)

- ✅ **Category-aware UI everywhere** — chip filters live on Discover + Library; PackageDetail shows category pills
- ✅ **Dashboard** — landed as Phase 11 with donut chart for top categories in your library
- ✅ **Tier A NSVisualEffectView (vibrancy)** — shipped this session
- ✅ **Services view** — Phase 11b, with per-package start/stop/restart
- ✅ **Storage card** — disk-usage view of Cellar / Caskroom / var/log / cache with Open-in-Finder

## Phase 12 — Catalog + Settings + GitHub (planned)

See `decisions.md` (2026-05-24-night entries) for the design decisions. The detailed task graph lives in `memory-bank/phase12-plan.md`.

Phase 12 grew out of two user prompts:
1. "What can we do with the formulae.brew.sh API?" — the catalog opens up deprecation warnings, build-error rates, reverse deps, dep-tree visualization, Brewfile validation, "what's new this week"
2. "What should live in Settings? GitHub auth for stars/follow/issues, etc." — Settings shell becomes the home for catalog refresh, paranoid mode, GitHub auth, theme overrides

## Still open

- Recipes (Phase 10) — paused; depends on catalog for validation
- Real screenshots per `visualStory.md`
- Categorize cron on Beast or umbp for daily delta
- "Wrong?" GitHub-issue link in PackageDetail (waits on Phase 12c/e)
- `installedAt` on Package + Last-Updated sort (small backend change)
- Tier B Tahoe Liquid Glass via Swift bridge (v0.2)
- App icon thumbnails inside category tiles
- "Recommended for you" tile based on installed mix (LLM at build time)
- Search within a single category
- Multi-select category intersection ("AI ∩ Developer Tools")
- Per-cask "Similar to" suggestions

## Resolved this session (Phase 12 Wave 1 + Wave 2)

- ✅ **Bundled catalog + manual refresh** — Phase 12a shipped, ~3 MB compressed, 8k formulae + 7.6k casks
- ✅ **Settings shell + 6 sections** — Phase 12b shipped, Cmd+, opens, gear icon in sidebar
- ✅ **Paranoid mode + per-feature network controls** — Phase 12d shipped, single helper gates every outbound command
- ✅ **GitHub anonymous repo stats** — Phase 12c shipped, stars/forks/last-release/archived state in PackageDetail
- ✅ **GitHub Device Flow + Keychain sign-in** — Phase 12e shipped, token never returned to frontend, never written to disk, never logged
- ❌ **Phase 14 bundled cask icons** — DROPPED per `decisions.md`; trademark/redistribution risk too real to justify the small UX win

## Still open

- Phase 12f — GitHub authed actions (star/unstar/is_starred/watch/unwatch/create_issue + "Wrong?" link + Dashboard personal-stats card)
- Phase 13 — Catalog enrichment via Haiku (Tier A friendly names + summaries, Tier B use cases + similar + tags) with AI Features master toggle. Full spec at `phase13-plan.md`
- Recipes (Phase 10) — paused, depends on catalog (now available)
- `installedAt` on Package + Last-Updated sort — small standalone backend addition
- Tier B Tahoe Liquid Glass via Swift bridge — v0.2
- Real screenshots per `visualStory.md`
- Categorize cron on Beast or umbp for daily delta
- Build-error rates / reverse deps / dep-tree viz — natural follow-ups once catalog is in regular use (Phase 12a unlocks all of these)
