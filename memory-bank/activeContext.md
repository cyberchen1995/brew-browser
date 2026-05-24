# Active Context

**Date:** 2026-05-24 (v0.2.0 release prep)
**State:** All UI/UX overhaul work complete and ready to ship. Tier A enrichment baked. Native-square app icon (Tahoe-clean). 0 errors on `npm run check`, `cargo check`, and `cargo clippy -D warnings`. Working tree has ~50 modified files (icon regen + UI restructure) ready to commit for v0.2.0.

## Repo

- **github.com/msitarzewski/brew-browser** — public, MIT, `main` branch
- **Last release:** v0.1.0 (2026-05-23)
- **Next release:** v0.2.0 (about to cut)
- Commits to date (commit 9 = v0.2.0 in progress):
  - `653e26f` initial release v0.1.0 (186 files)
  - `c72e31d` LLM-generated categories + landing page
  - `2dad9be` drop Caddyfile snippet
  - `cb60e4a` signed + notarized release pipeline
  - `c2ab41f` NEXT-SESSION handoff doc
  - `84ad010` Phase 9 + 11 — Dashboard, Services, donut, vibrancy
  - `99a1f2c` Phase 12 Wave 1+2 — bundled catalog + Settings + paranoid mode
  - `8b89c40` Phase 12f + Phase 13 — GitHub authed actions + enrichment infra
  - `e1d6a87` Phase 12g/13b cleanup + extensive UI polish + Tier A enrichment baked
  - **v0.2.0 (next)** — structural overhaul (sidebar/title-bar/search/info popovers/intercept GitHub flow)

## What landed since `e1d6a87`

### Native macOS title bar
- New `<header class="titlebar">` in `+page.svelte` spans the window above the main split — the macOS unified-toolbar pattern. 36 px tall, same `--color-surface-raised` chrome as the sidebar so the two form one continuous L-shaped frame around the main content.
- `tauri.conf.json` gets `trafficLightPosition: { x: 14, y: 20 }` so the macOS-rendered traffic lights vertically center on the same axis as the toggle and the title.
- Page title (`ui.pageTitle`, derived from `ui.section` via a small `SECTION_TITLES` map) lives in the title bar with absolute positioning + CSS-variable left-offsets so it always sits just past where the sidebar divider lands.
- Each pane's `<h1>` was removed from its `panel-head`; where the head had no remaining content (Dashboard, Discover) the entire `<header>` was dropped. Where actions remained (Library / Trending / Snapshots / Services / Activity) the head became a thin secondary toolbar that right-aligns its content.
- Right-side cluster (new `TitlebarControls.svelte`): theme dropdown (single icon → 3-item popover Light / Dark / System) + Settings gear + pink-filled Donate heart, grouped as a pill-shaped button group with hair-line dividers. Background uses `--color-surface` (panel-body gray) — softer than the previous dark sunken bg. Right edge aligns with the main panel's right padding (`var(--space-4)`). Cluster nudged 1 px below center to optically match the macOS traffic lights.

### Collapsible sidebar + Dashboard-as-nav + type-ahead search
- New `ui.sidebarCollapsed` state + `toggleSidebarCollapsed()` + `loadSidebarCollapsedFromStorage()`; persists to `brew-browser:sidebar-collapsed` localStorage. Hot-reload-safe; survives launches.
- Sidebar collapsed mode: 200 px → 56 px. Nav items go icon-only with a 14 px badge overlay; theme group hidden (only the status dot remains in the foot).
- Sidebar toggle position transitions via CSS variables on `.app`:
  - expanded: `left: 168px` (just inside the 200 px sidebar's right edge)
  - collapsed: `left: 84px` (next to traffic lights; the 56 px sidebar can't fit a button)
- Brand area entirely removed. **Dashboard is now the first nav item** (`LayoutDashboard` icon, `⌘0` shortcut tooltip) — the menu bar already identifies the app on macOS, so the brand-text was redundant.
- New persistent **type-ahead search input** at the top of the sidebar (above the nav, no border separator). Uses the existing shared `search` store (debounced 300 ms → `brew_search`). Dropdown shows top 7 results with `[name] [kind pill] [installed badge if installed]`. Keyboard: ArrowDown/Up navigate, Enter opens (or routes to Discover if nothing selected), Esc clears then unfocuses on second press. Mouse: hover highlights, click opens detail. "See all results in Discover →" link at the bottom for the user to drill deeper. Hidden in collapsed mode (no room for an input in the 56 px rail).
- Theme + Settings + Donate **removed** from the sidebar footer (they live in the title-bar cluster now). Sidebar foot now contains only the brew-status row.

### Info popovers replacing "Wrong?" + AI labels
- New reusable `InfoButton.svelte`:
  - Hover-activated (mouse) with 120 ms open / 180 ms close delays so the popover doesn't vanish when the user moves into it.
  - Focus-activated for keyboard a11y; Esc returns focus to trigger.
  - Click toggles for touch.
  - `position: fixed` with viewport-aware top/left so the popover escapes any ancestor `overflow: hidden` (the detail panel's `.body` was clipping it before).
  - Auto-closes on scroll/resize.
  - No `title` attribute on the trigger (the popover IS the tooltip; native title bubble would collide); `aria-label` carries the accessible name; `aria-expanded` + `aria-haspopup="dialog"` signal state.
- Removed from `PackageDetail.svelte`:
  - Every "Wrong?" link (Categories, Tags, Summary, Why install this?, Similar packages)
  - Every "AI-enriched" sparkle badge
  - The `Sparkles` import + `.wrong-link*` and `.ai-badge*` CSS
- Each cluster replaced with one `<InfoButton title body onReport />`. The body text says: *"Generated offline at build time by Claude Haiku 4.5 — no network or LLM calls happen while you use brew-browser. Open an issue if X looks off and we'll fix it in the next release."* Hover any (i) to discover both the AI provenance and the privacy posture.
- Summary blockquote's (i) sits **inline at the end of the summary text** (with `&nbsp;` so it doesn't wrap onto its own line). Other instances (Categories, Tags, h3 sections) sit after their respective labels.
- Per-tag tooltips ("AI-enriched tag") also removed — the (i) covers it.

### GitHub: intercept-on-action instead of static sign-in hint
- Removed the "Sign in via Settings → GitHub to star, watch, or file issues." paragraph from `PackageDetail.svelte`.
- Star / Watch / File issue buttons now paint **whenever the GitHub stats card is visible**, regardless of sign-in state.
- New `requireGithubSignIn(actionLabel)` helper inside `PackageDetail.svelte`: signed-in → proceeds; signed-out → deep-links to Settings → GitHub via `ui.openSettings("github")` + toasts `"Sign in to GitHub to {action}"`. Each action button's tooltip adapts ("Sign in to GitHub to star this repository" when signed-out).
- `ui.openSettings(section?: SettingsSection)` plumbed through:
  - `SettingsSection` type promoted to `src/lib/types.ts` so the store + component share one source.
  - New `ui.settingsInitialSection` state + cleared on `closeSettings()`.
  - `Settings.svelte` honors `ui.settingsInitialSection ?? "appearance"` on open.
- GitHub OAuth App credential **set live**: `GITHUB_OAUTH_CLIENT_ID = "Ov23liJZKbvrSBuiOPkT"` in `src-tauri/src/github/auth.rs`. (Device Flow client_ids are public per RFC 8628 — no client secret needed, safe to commit.)

### Other UI fixes
- **License-mismatch row** wraps as prose now: whole sentence (with inline `<code>` license tags) lives inside one `<span>`, with the `AlertCircle` as the only sibling flex child. Fixes the awkward 2-column wrap from `inline-flex` + multiple inline children.
- **EmptyState vertically centered**: added `min-height: 100%; box-sizing: border-box` to `.empty`. Every empty state across the app (Library "No packages installed", Discover "Search failed", Snapshots "No snapshots yet.", Trending "Quiet for now", etc.) now sits in the visual middle of its pane instead of stacking at the top.
- **Snapshots inline CTAs removed** — the `New Snapshot` / `Import Brewfile…` buttons inside the empty state were duplicating the panel-head's primary actions. Empty state is now purely informational.
- **Selected-row state persists across panes** — Library, Trending, Discover (both row variants), Services, and Dashboard's Outdated preview all bind `selected={ui.selectedPackage?.name === x.name && ui.selectedPackage?.kind === x.kind}` so the source row stays highlighted while the detail panel is open. Library already had this; the other four panes had been missing it.
- **Chip-filter clears on section change** — `ui.setSection(s)` now calls `discover.clear()` when the section actually changes. Deeplink callers (Dashboard's category donut, PackageDetail's category pills) reordered to call `setSection` first, then `selectOnly` — order documented in `setSection`'s comment.
- **Brew analytics parser widened** (already in `e1d6a87`) to accept InfluxDB and arbitrary backend prefixes — no regression here.

### Native-square app icon (Tahoe-clean)
- Source: `docs/icon/brew-browser.svg` updated to a full-bleed 181×181 square (no pre-rounded outer squircle). Regenerated all icon sizes via `npm run tauri icon docs/icon/brew-browser.svg`.
- Fix: macOS Tahoe auto-applies its own squircle mask. The previous pre-rounded source produced double-rounded artifacts ("white corners" where Tahoe's mask exposed transparent pixels in the original PNG). The new square source lets Tahoe mask cleanly; older macOS gets the same result via its (slightly different) mask.

## Tests & lint (current)

- `cargo test`: **411 passed**, 0 failed, 6 ignored (unchanged since `e1d6a87`)
- `cargo clippy --all-targets -- -D warnings`: clean
- `cargo check`: clean
- `npm run check`: 0 errors, 3 warnings (1 tsconfig-node pre-existing, 2 SettingsSectionGitHub unused-CSS pre-existing)
- `npm run build`: clean

## Memory bank inventory

`toc.md`, `projectbrief.md`, `techContext.md`, `decisions.md`, `activeContext.md` (this), `progress.md`, `systemPatterns.md`, `designSystem.md`, `uxArchitecture.md`, `backendApi.md`, `frontendComponents.md`, `codeReview.md`, `apiTests.md`, `accessibility.md`, `visualStory.md`, `security.md`, `ideas.md`, `phase12-plan.md`, `phase13-plan.md`, `agentLog.md`, `NEXT-SESSION.md`, `scans/{phase12-security-review.md, ...}`, `tasks/2026-05/`.
