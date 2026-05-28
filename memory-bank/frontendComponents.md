# Frontend Components — brew-browser

**Owner:** Frontend Developer
**Wave:** 2 — Implementation (extended through Phases 9, 11, 12, 13, 15, 17)
**Last updated:** 2026-05-25 (v0.3.0 ship — adds the Phase 15 updater UI + Phase 12+ Octocat chip + today's UpgradeModal)
**Status:** inventory current as of `npm run build` / `npm run check` clean (0 errors, 3 pre-existing warnings)

This file is the canonical inventory of Svelte 5 components in `src/lib/components/`. Format: name, file, props summary, state owned, notes.

---

## Icon library

**Lucide via `@lucide/svelte` (v1.16.0, ISC license).** Imported per-icon (e.g. `import X from "@lucide/svelte/icons/x"`) so Vite tree-shakes unused glyphs. Spec recommended Lucide; the package `lucide-svelte` is deprecated in favor of `@lucide/svelte` (same project, same icons).

## npm deps added in Wave 2

| Package | Why |
|---|---|
| `@lucide/svelte` | Icon set — referenced throughout components |
| `@tauri-apps/plugin-dialog` | Native file pickers for Brewfile export / import (per designSystem §8) |

No other runtime additions. **No Tailwind / CSS-in-JS / preprocessor** — UI Designer's decision, honored.

---

## Style layer

| File | Purpose |
|---|---|
| `src/app.css` | Entry — imports tokens / reset / typography, sets body bg & `prefers-reduced-motion` |
| `src/lib/styles/tokens.css` | OKLCH color tokens (light + dark via `[data-theme="dark"]`), spacing, radius, shadows, motion, typography vars |
| `src/lib/styles/reset.css` | Modern reset + `:focus-visible` ring per macOS convention |
| `src/lib/styles/typography.css` | Base type rules, helpers (`.mono`, `.truncate`, `.uppercase-label`) |

Theme is applied to `document.documentElement.dataset.theme`. `ui.theme` of `"system"` listens to `prefers-color-scheme` via `watchSystemTheme()`.

---

## Types & API surface

| File | Purpose |
|---|---|
| `src/lib/types.ts` | TS-equivalents of every Rust DTO from `backendApi.md` (`Package`, `PackageList`, `PackageDetail`, `SearchResults`, `BrewStreamEvent`, `BrewfileSummary`, `TrendingReport`, `BrewErrorPayload`, etc.) + UI-only types (`ActivityJob`, `PaletteItem`, `ThemePreference`). Includes `isBrewError()` guard and `brewErrorMessage()` formatter. |
| `src/lib/api.ts` | Typed `invoke()` wrappers for all 20 backend commands. Streaming commands take an `onEvent(BrewStreamEvent)` callback; the underlying `Channel<T>` is wired inside `makeChannel()` so callers don't import Tauri directly. Functions resolve with typed result or throw `BrewErrorPayload`-shaped objects. |
| `src/lib/util/url.ts` | URL safety helpers. `safeOpenUrl(url: string): Promise<void>` is the **only sanctioned way to hand a URL to `tauri-plugin-opener`** from the renderer. Scheme allowlist: `http:` / `https:` only — anything else (`file:`, `mailto:`, `vscode://`, `slack://`, `javascript:`, etc.) is rejected with `toast.error()` and never reaches `open(1)`. Exported `classifyUrl(url)` returns `{ ok: true, url } \| { ok: false, reason }` for callers that need the verdict without the side-effect. Security audit §H1 (`memory-bank/security.md`) — keep all opener calls funneling through here so the allowlist is enforced in one place. |

---

## Stores (Svelte 5 runes)

All stores are module-level singleton class instances using `$state` / `$derived` fields. Files end in `.svelte.ts` so SvelteKit's Vite plugin compiles runes correctly.

| Store | File | Owns | Notes |
|---|---|---|---|
| `ui` | `src/lib/stores/ui.svelte.ts` | current `section`, `drawerOpen`/`drawerMinimized`, `paletteOpen`, `theme`, `selectedPackage`, `detailPaneWidth` | Theme persisted to `localStorage["brew-browser.theme"]`. `watchSystemTheme()` reflects OS changes when `"system"`. **Detail-pane width** (`detailPaneWidth`, default 420 px) persisted to `localStorage["brew-browser:detail-pane-width"]` via `setDetailPaneWidth(w)`; `loadDetailPaneWidthFromStorage()` (called from `+page.svelte` onMount) clamps to `[DETAIL_PANE_MIN_WIDTH=320, floor(window.innerWidth * 0.6)]` via the exported `clampDetailPaneWidth(w)` helper. `resetDetailPaneWidth()` snaps back to `DETAIL_PANE_DEFAULT_WIDTH`. |
| `packages` | `src/lib/stores/packages.svelte.ts` | `list: PackageList`, `loading`, `error`, derived `formulae` / `casks` / `all` / `outdated`. | `load(force?)` micro-caches (5 s) to avoid double-fetch on quick re-mount. |
| `search` | `src/lib/stores/search.svelte.ts` | `query`, `results`, `loading`, `error`, `recent` | 300 ms debounce; ≥ 2-char minimum; recent ring buffer (8). |
| `activity` | `src/lib/stores/activity.svelte.ts` | `jobs: ActivityJob[]`, `activeJobId`, derived `runningCount` | `handleEvent(BrewStreamEvent)` mutates the matching job in place. `cancel(id)` calls backend `cancel_job`. |
| `brewfiles` | `src/lib/stores/brewfiles.svelte.ts` | `list: BrewfileSummary[]`, `loading`, `error` | |
| `trending` | `src/lib/stores/trending.svelte.ts` | `window`, `report`, `loading`, `error`, `fetchedAtMs` | Backend handles the 1 h cache; frontend reads `cacheAgeSeconds` for the "Updated N min ago" label. |
| `toast` | `src/lib/stores/toast.svelte.ts` | `items: Toast[]` | Auto-dismiss: 4 s success/info, 7 s warning, persistent error. |
| `iconCache` | `src/lib/stores/iconCache.svelte.ts` | `cache: Map<token, string \| null>`, `inFlight: Set<token>`, `pending: Map<token, Promise>` | Phase 7 + Phase 8. Per-session memoization of cask icon results. **`getIcon(pkg: Package)`** (was `getIcon(token: string)`) now routes via `pkg.iconSource.kind`: `installedApp` → `cask_icon(pkg.name)`, `homepage` → `cask_icon_from_homepage(pkg.name, homepage)`, `none` → cached as `null` synchronously without an IPC hop. Coalesces concurrent calls per token; null is a sticky "tried, no icon" marker so we don't keep retrying. Both backend commands share token-keyed disk caches. Map is re-assigned (not mutated) to trip Svelte 5 reactivity. **Defense-in-depth (§M3):** results from the backend are run through `isSafeIconDataUrl()` and coerced to `null` unless they start with `data:image/png;base64,` or `data:image/jpeg;base64,` — a future backend bug (or compromise) returning `javascript:`, `https://...`, or `data:image/svg+xml,...` can never reach `<img src>`. Contract for backend: both `cask_icon` and `cask_icon_from_homepage` MUST return either `null` or a `data:image/{png,jpeg};base64,...` string; anything else is treated as a miss. |

---

## Components

### Primitives (`src/lib/components/`)

| Component | File | Props | State owned | Notes |
|---|---|---|---|---|
| `Button` | `Button.svelte` | `variant` (primary / secondary / danger / ghost / link), `size` (sm / md / lg), `type`, `disabled`, `loading`, `title`, `ariaLabel`, `onclick`, `icon` snippet, `children` snippet | — | macOS conventions: primary on right in actions row, danger destructive only. Loading spinner replaces leading icon. |
| `Input` | `Input.svelte` | `value` (`$bindable`), `placeholder`, `type`, `size` (sm / md), `variant` (default / search), `disabled`, `invalid`, `ariaLabel`, `onInput`, `onKeydown`, `leading` / `trailing` snippets | — | `variant="search"` adds magnifying glass + clear-X. Focus ring via `:focus-within`. |
| `Pill` | `Pill.svelte` | `tone` (neutral / formula / cask / success / warning / danger / info / brand), `children` snippet | — | Formula = info blue, Cask = brand amber. |
| `EmptyState` | `EmptyState.svelte` | `title`, `body?`, `icon?` snippet, `cta?` snippet | — | Vertical-center, max-width 360px, per designSystem §7.8. |
| `LoadingState` | `LoadingState.svelte` | `rows` (default 6), `label` (default "Loading…") | — | Skeleton row shimmer, disabled on `prefers-reduced-motion`. |
| `Toast` | `Toast.svelte` | (none — reads `toast` store) | — | Stack bottom-right; auto-dismiss; spring slide-in. |
| `Modal` | `Modal.svelte` | `open`, `title`, `dismissible?`, `onClose?`, `children` snippet, `actions?` snippet | dialog ref | Focus on first focusable on open, primitive focus trap, Esc to close, scrim click to close (when dismissible). |
| `DestructiveConfirm` | `DestructiveConfirm.svelte` | `open`, `title`, `confirmLabel?`, `cancelLabel?`, `confirmVariant?` (danger / primary), `onConfirm`, `onCancel`, `children?` snippet | — | Wraps `Modal`; default focus = Cancel; "primary" variant for additive ops like Restore. |
| `ResizeHandle` | `ResizeHandle.svelte` | `width`, `min`, `max?`, `defaultWidth`, `direction?` ("left"/"right", default "left"), `onChange(next)`, `onCommit(next)`, `label?` | local `dragging` (`$state`), imperative `startX`/`startWidth`/`pendingWidth`/`pointerId`; `dirSign` derived | Vertical splitter for resizing a sibling pane. 6 px hit zone with a 1 px hairline (`--color-border`, brightens to `--color-border-strong` on hover / focus / active drag; `--color-border-focus` on `:focus-visible`). Pointer Events API with `setPointerCapture` so drags survive cursor leaving the handle. `role="separator"` + `aria-orientation="vertical"` + `aria-valuemin`/`aria-valuemax`/`aria-valuenow` + `tabindex="0"`; keyboard: ←/→ ±8 px, Shift+←/→ ±32 px, Home → min, End → max. Double-click resets to `defaultWidth`. `touch-action: none` to suppress native gestures. `onChange` fires live during drag (parent applies but should not persist); `onCommit` fires on pointerup/keyup (parent persists). The two `noninteractive` lints suppressed inline because role=separator is the correct ARIA pattern. |

### Structure / sections

| Component | File | Props | State owned | Notes |
|---|---|---|---|---|
| `Sidebar` | `Sidebar.svelte` | (none — reads `ui`, `packages`, `activity`, `brewfiles` stores) | — | 5 sections (Library/Discover/Trending/Snapshots/Activity), badges (outdated count, snapshots count, running ops), theme tri-toggle (Light/Dark/System), brew status dot. |
| `CommandPalette` | `CommandPalette.svelte` | (none — reads `ui`, `packages`, `search`, `brewfiles`, `trending` stores) | local `query`, `selectedIdx`, input ref | Cmd+K. Groups: Installed → Index → Commands. Arrow keys navigate, Enter activates, Esc closes. Index group debounces `search.run` at 300 ms. |
| `Library` | `Library.svelte` | (none) | local `filter`, `query`, derived `filtered` / `sorted` | Filter chips (All / Formulae / Casks / Outdated), inline search, sticky column header, row click → detail. |
| `PackageRow` | `PackageRow.svelte` | `pkg: Package`, `selected?`, `onSelect?` | local `iconDataUrl`, `iconLoaded` | Grid: **24px icon-slot** / name / version / kind pill / outdated chevron. macOS selection style (filled blue when window-focused). Phase 7 + 8: all rows lazy-load via `iconCache.getIcon(pkg)`; the store routes by `pkg.iconSource` (installed-app extractor → homepage favicon → none). When the resolved data URL is present, render `<img>`; when `iconSource.kind !== "none"` but resolved-null, render a low-opacity 18px Lucide `Package` fallback; when `iconSource.kind === "none"` (formulae + casks with no app artifact AND no homepage), leave the slot empty so the name column stays aligned. `<img>` element (backend returns base64 data URL), `alt=""` (decorative — name is adjacent), no fade-in (instant per designSystem §6). The `Library` `list-header` grid mirrors the column template. |
| `PackageDetail` | `PackageDetail.svelte` | (none — reads `ui.selectedPackage`, `ui.detailPaneWidth`) | local `detail`, `loading`, `error`, `depsOpen`, `dependentsOpen`, `confirmUninstall` | Slide-over right panel; width driven by `ui.detailPaneWidth` (default 420 px) via inline `--detail-pane-width` CSS var on `.detail`, resized by sibling `ResizeHandle` mounted in `+page.svelte`. Long-content blocks (caveats `<pre>`, homepage URL, dependency names, description) use `overflow-wrap: anywhere` + `word-break: break-word`; caveats `<pre>` keeps `white-space: pre-wrap` for newlines plus `overflow-x: auto` as a last-resort scroll for unbreakable tokens. Install / Uninstall / Reinstall / Upgrade actions, Activity drawer auto-opens, toast on success. Homepage link opens via `@tauri-apps/plugin-opener`. Phase 8: meta row "Icon source: installed app / homepage / none" surfaces the resolved `pkg.iconSource.kind` (homepage URL goes in the title tooltip), keeping the transparency narrative loud without claiming a whole section. |
| `Discover` | `Discover.svelte` | (none) | — | Search input bound to `search.query`, recent-search chips, results list grouped (formulae + casks together), installed badge cross-referenced from `packages` store. |
| `Trending` | `Trending.svelte` | (none) | — | Window segmented control (30d / 90d / 365d), refresh button + "Updated N min ago", top-100 list with rank + install count + installed badge. |
| `Snapshots` | `Snapshots.svelte` | (none) | local `newLabel`, `creating`, `showNewModal`, `toDelete`, `toRestore` | Card grid of saved Brewfiles, New Snapshot modal, Restore (additive confirm), Delete (destructive confirm), Export → native save dialog, Import → native open dialog. |
| `ActivityDrawer` | `ActivityDrawer.svelte` | (none — reads `activity`, `ui`) | local `consoleEl`, `autoScroll` | Bottom drawer (Xcode-debug pattern). Header strip with current op + elapsed time + cancel/copy/close. Per-job tabs when >1. Streaming console: monospace, ANSI-stripped, per-line classifier (==> info, Error: red, Downloading green, etc.). Smart autoscroll pause when user scrolls up. |
| `ActivityHistory` | `ActivityHistory.svelte` | (none — reads `activity`) | — | Sidebar "Activity" section view: chronological list of all jobs with status icon + duration; click opens that job in the drawer. "Clear completed" action. |

---

## Routes

| File | Purpose |
|---|---|
| `src/routes/+layout.ts` | `ssr=false` (unchanged from scaffold) |
| `src/routes/+layout.svelte` | Imports `app.css`, calls `ui.loadThemeFromStorage()` and starts `watchSystemTheme()`. Renders children. |
| `src/routes/+page.svelte` | The whole app shell — sidebar + content area + detail panel + bottom drawer + palette + toast layer. Conditional render of one of `<Library / Discover / Trending / Snapshots / ActivityHistory />` based on `ui.section`. Owns global keymap (Cmd+1..5, Cmd+K, Cmd+L, Cmd+R, Cmd+Shift+L, "/", Esc). |

Single-route SPA. No nested SvelteKit routes — sidebar navigation is state, not URL.

---

## Keyboard shortcuts (wired in `+page.svelte`)

| Shortcut | Action |
|---|---|
| `⌘K` | Open Command Palette |
| `⌘1` … `⌘5` | Library / Discover / Trending / Snapshots / Activity |
| `⌘L` | Toggle Activity drawer (expand → minimize → close cycle) |
| `⌘⇧L` | Cycle theme (light → dark → system) |
| `⌘R` | Refresh current view (Library / Trending / Snapshots / Discover-rerun) |
| `/` | Focus the first text input in view (filter / search), unless already in an input |
| `Esc` | Priority: palette → detail (modals handle their own Esc) |

Cmd+W and Cmd+Q are macOS defaults — not intercepted; the OS quits the single-window app, matching the activeContext decision.

---

## Patterns introduced (also recorded in `systemPatterns.md`)

- **Store-class pattern.** Module-singleton class instances with `$state` fields. Imported as `{ packages }` etc.; methods mutate fields directly. Avoids passing context.
- **Typed-`invoke` wrapper.** Every Tauri command wrapped in a one-liner in `api.ts` so call sites import a named function, not a string. Streaming commands take `(evt) => void` callback; channel construction is hidden.
- **Activity-job pattern.** Frontend creates a job *before* invoking (with a tmp UUID), then patches `jobId` on the first `Started` event. This means the drawer is interactive immediately, even before the backend's first event lands.
- **Modal + DestructiveConfirm pattern.** `Modal` is the generic dialog; `DestructiveConfirm` is the opinionated wrapper for "are you sure?". `confirmVariant="primary"` reuses the same UI for additive ops (Restore).
- **In-page slide-over** (PackageDetail) — not a modal, not a separate route. Library remains visible and interactive on its left.
- **Theme application** — `data-theme="dark|light"` on `<html>`, `localStorage` for user pref, matchMedia subscription for `system`. CSS does the rest via OKLCH tokens.
- **`safeOpenUrl` scheme allowlist** — `src/lib/util/url.ts` centralizes the http/https-only check before any `tauri-plugin-opener` call. Every present and future "open this URL in the user's browser" call site goes through `safeOpenUrl(url)` so attacker-influenced metadata (cask `homepage`, future search-result URLs, etc.) can't smuggle `file://` / `vscode://` / `mailto:` past the opener. The rejection path toasts `toast.error(...)` so the user sees why. See systemPatterns §15.
- **Adaptive `aria-live` for streaming console** — `ActivityDrawer` rate-detects line arrivals; >3/sec for >5s flips the log region to `aria-live="off"` to avoid flooding screen readers during noisy installs. A separate sr-only polite live region announces only the completion summary ("Done", "Failed", "Canceled") so SR users still get an exit signal. Reverts to polite after 1.5s of calm. See systemPatterns §16.

---

## Drift from spec

- **Icon library:** switched from `lucide-svelte` (deprecated by upstream) to `@lucide/svelte` (1.16.0). Same project, same imports — purely a rename. Documented in `decisions.md` candidate.
- **No `<DesignSystemPreview>` route** at `/_design`. designSystem.md §10 listed this as a checklist item; deferred to a Wave 4 polish task because every primitive is exercised by the live UI already and we'd rather ship.
- **No vibrancy / no scrollbar styling** — both explicitly Wave-4 polish in designSystem.md §8.
- **Streaming `Channel<T>` race-condition note:** the frontend optimistically creates a local job with a tmp UUID, then patches `jobId` on the first `Started` event. If the backend's first event isn't `Started` (spec says it always is), the tmp UUID remains. Worth a Code Reviewer eyeball.
- **`progress` events** are currently rendered as plain console lines prefixed `[progress]`. Phase 3 polish could use the `percent` field for a real progress bar; left as TODO.

## TODOs for Wave 3 / 4

- `<DesignSystemPreview>` route at `/_design` (Wave 4 polish per designSystem §10).
- Per-row inline `[+ Install]` button in Trending (Phase 6 polish per uxArchitecture §7).
- Description-search variant (`brew_search_desc`) — backend wrapper is in `api.ts` but no UI surfaces it yet.
- Real progress bar from `BrewStreamEvent.Progress.percent`.
- Right-click context menus (out of scope MVP, noted in designSystem §8).
- axe DevTools / Polypane pass to verify the OKLCH contrast claims in real WKWebView.
- Cmd+, preferences popover stub (binding reserved per uxArchitecture §8).

---

*End of inventory. Updated alongside any new component or significant prop change.*

---

## Phase 9 + 11 + 12 + 13 additions

**Author:** Technical Writer (post-implementation pass, 2026-05-24 evening)
**Scope:** Components, stores, and utilities shipped after the original Wave 2 inventory (Phase 9 categories UI, Phase 11 Dashboard/Services/native feel, Phase 12 Settings/GitHub, Phase 13 enrichment).
**Note:** Files listed here are appended without modifying the original tables above. Where a component or store later changed behavior in a subsequent phase, the row below reflects the current shipped state.

### Components

| Component | File | Props | State owned | Notes |
|---|---|---|---|---|
| `Dashboard` | `Dashboard.svelte` | (none — reads `packages` / `env` / `categories` / `activity` / `github` / `settings` stores) | local `disk: DiskUsageReport \| null`, `diskLoading`, `diskError` | Default landing per the "brand = home" decision. Hero row (installed count / outdated count / brew version), Updates panel with one-click upgrade-all (title is a link → Library outdated filter), Composition split bar (on-request / dep / pinned), top-categories donut (180px SVG, 9-color palette, top 8 + Other, legend click → Discover with chip pre-selected), Storage card (4 paths from `disk_usage`, Reveal-in-Finder per row), optional "Personal stats" card when signed in to GitHub. The hero brand area is a `data-tauri-drag-region`. |
| `Services` | `Services.svelte` | (none — reads `services`, `ui`, `packages`) | local `sortKey: "name"\|"status"\|"user"`, `sortDir: "asc"\|"desc"` | Sidebar item ⌘5. Sortable list of `brew services` entries with per-row start/stop/restart buttons (smart-disabled by current state). Status sort order: started → scheduled → error → stopped → none → unknown. Refresh button bypasses the 5s backend cache. |
| `SortableHeader` | `SortableHeader.svelte` | `label: string`, `sortKey: string`, `active: boolean`, `dir: "asc"\|"desc"`, `onSort(key)`, `align?: "left"\|"right"` | — | Reusable column header for list grids. Click toggles direction when already active, switches key otherwise. Uses `aria-label` (not `aria-sort`, since that requires `role="columnheader"` and our list grids aren't true tables). Used by Library, Trending, Services. |
| `DeviceFlowModal` | `DeviceFlowModal.svelte` | (none — reads `github.signinState`) | local `remainingSeconds: number \| null`, `tickHandle` for the 1s countdown timer | Renders the user-facing half of the GitHub OAuth Device Flow. Shows the user code with a copy button, an "Open in browser" affordance for `verification_uri`, a 15-min countdown, and a Cancel button. Polling lives in the `github` store; this component is pure presentation. Mounts when `github.signinState.kind !== "idle"`. |
| `IssueModal` | `IssueModal.svelte` | `open`, `title`, `body`, `labels: string[]`, `repo: { owner, repo }`, `homepage`, `onClose()` | local form state (title / body / submitting / error) | Modal form for filing a GitHub issue. Used by the PackageDetail "File issue" button and by the "Wrong?" categorization affordance. Mirrors the backend caps (title ≤ 256, body ≤ 64 KiB) and surfaces error states inline. On success, opens the created issue's `html_url` via `safeOpenUrl`. |
| `Settings` | `Settings.svelte` | (none — reads `ui.settingsOpen`, `ui.defaultSection`) | local `activeSection: "appearance"\|"network"\|"github"\|"brew"\|"activity"\|"about"` | The Settings modal container. Opens via ⌘, or the sidebar gear icon. 220px left nav + 1fr right pane (deviated from spec's 350+600 — looked awkward at macOS density). Focus trap, Esc to close, click-outside to close. Z-index 81 (sits above palette at 80). Each section is a sibling Svelte file rendered inline by `activeSection`. |
| `SettingsSectionAppearance` | `SettingsSectionAppearance.svelte` | (none) | — | Theme radio (Light/Dark/System bound to `ui.setTheme`), default landing dropdown (writes to `ui.setDefaultSection` → localStorage), vibrancy material dropdown (writes to `ui.setVibrancyMaterial` with "restart required" note), AI Features master toggle (Phase 13 — reads/writes `settings.data.aiFeaturesEnabled`). |
| `SettingsSectionNetwork` | `SettingsSectionNetwork.svelte` | (none — reads `settings.svelte.ts`) | — | Real Network controls populated in Phase 12d. Paranoid Mode toggle with warning callout, catalog auto-refresh radios (Off/Weekly/Daily), stale-banner threshold input, cask icon mode radios (Off/Installed only/All), trending TTL input, disclosure list with allowed/blocked indicators per path. Corrupt-file recovery panel with `[Reset to defaults]` when `settings.corruptOnDisk`. |
| `SettingsSectionGitHub` | `SettingsSectionGitHub.svelte` | (none — reads `github` + `settings` stores) | — | Two independent controls: "Show GitHub stats on package pages" toggle (defaults off), and the Sign-in card that drives the Device Flow modal. Signed-in state shows username + scopes + Sign Out button. Decoupled by design — the user can sign in without enabling stats. |
| `SettingsSectionBrew` | `SettingsSectionBrew.svelte` | (none) | local `analyticsEnabled: boolean \| null`, `inFlight: boolean` | Analytics toggle (reads via `brewGetAnalytics()` on mount, writes via `brewSetAnalytics`), Confirm-before-destructive toggle (persisted via `ui.setConfirmDestructive` → localStorage). |
| `SettingsSectionActivity` | `SettingsSectionActivity.svelte` | (none) | — | Two clamped number inputs for the localStorage-backed activity retention caps (Keep last N jobs / Lines per job). Clamping happens in `ui.setActivityMax*` so a hostile localStorage entry can't drive the activity store into pathological retention. |
| `SettingsSectionAbout` | `SettingsSectionAbout.svelte` | (none) | local `version: string \| null`, `versionError: string \| null` | App version (from `app_version` Tauri command → `tauri::App::package_info`), brew version (from `env` store), MIT license, repo link via `safeOpenUrl`, "zero telemetry, zero accounts" affirmation paragraph. |

### Stores (Svelte 5 runes)

| Store | File | Owns | Notes |
|---|---|---|---|
| `categories` | `src/lib/stores/categories.svelte.ts` | `data: CategoriesData \| null`, `loading`, `error`, derived `tiles` (sorted by count, uncategorized last) | Lazy-loads via `categoriesData()` Tauri command on first access. Helpers: `tokensInCategory(slug)` for the filtered Discover view, `categoriesOf(name, kind)` for PackageDetail category pills. Singleton — fetches once per process. |
| `discover` | `src/lib/stores/discover.svelte.ts` | `selectedCategories: Set<string>` | Shared selection state for Discover / Library / PackageDetail. `selectOnly(slug)` for tile-click semantics (single-element set), `toggle(slug)` for chip add/remove. `hasFilter` derived. Empty set = browse mode; non-empty set + search query = filtered results; non-empty set + no query = union of category memberships. |
| `library` | `src/lib/stores/library.svelte.ts` | `filter: "all"\|"formulae"\|"casks"\|"outdated"` | Lifted out of the Library component so the Dashboard's "Updates available" card and (future) command palette can preset the filter before navigating. Session-only — does not persist to localStorage by design. |
| `services` | `src/lib/stores/services.svelte.ts` | `list: Service[]`, `loading`, `error`, `pending: Set<string>` (service names with an in-flight action) | Wraps `services_list` / `services_start` / `services_stop` / `services_restart`. `byName(name)` helper used by PackageDetail to render service controls per-formula. Re-fetches after every mutation (backend already memoises 5s). |
| `settings` | `src/lib/stores/settings.svelte.ts` | `data: Settings \| null`, `loading`, `error`, `corruptOnDisk: boolean`, derived `effective` | Mirrors backend `settings.json`. Three-state load result (loading → loaded \| corrupt). `corruptOnDisk` triggers the recovery UI in Settings → Network. `effective` returns defaults when `data` is null so consumers don't have to null-check. Single source of truth — frontend reads from here, not from `localStorage`, for any value persisted by Phase 12d+. |
| `github` | `src/lib/stores/github.svelte.ts` | `status: GithubStatusDto \| null`, `repoStatsCache: Map<homepage, RepoStats \| "miss" \| "rate-limited">`, `signinState: { kind: "idle" \| "waiting" \| ... }`, `pollHandle` for the active Device Flow | Wraps every `github_*` IPC command. `signIn()` runs the full Device Flow loop (start → poll until approved/denied/expired) honouring server `interval` and doubling on `slowDown` per RFC 8628 §3.5. `cancelSignin()` aborts the poll loop. `getRepoStats(homepage)` per-session memo on top of the backend's 24h disk cache. `createIssue(...)` opens the returned `html_url` via `safeOpenUrl` after success. **No token state ever lives here** — everything is derived from `status.signedIn`. |
| `enrichment` | `src/lib/stores/enrichment.svelte.ts` | `data: EnrichmentData \| null`, `loading`, `error` | Lazy-loads the bundled enrichment payload via `enrichmentData()`. Every public lookup checks `settings.effective.aiFeaturesEnabled` and short-circuits to `null` when the user has the AI Features toggle off — so UI components don't have to re-implement the gate at every call site. |

### Utilities

| Module | File | Exports | Notes |
|---|---|---|---|
| `categoryIcon` | `src/lib/util/categoryIcon.ts` | `resolveCategoryIcon(name: string): Component` | Static map of 19 Lucide icon-name → Svelte component for the categories declared in `src-tauri/data/categories.json`. Static map keeps the bundler happy (no dynamic imports) and the supported icon set explicit. Falls back to `HelpCircle` for unknown names so a missing entry won't crash — but it WILL look out of place, so `tools/categorize/categorize.py` introducing a new category requires adding the mapping here too. |

### Mount points (where these get rendered)

- **Dashboard** is the default landing per the `ui.section = "dashboard"` initial state; the sidebar brand is the home button. Rendered in `+page.svelte` when `ui.section === "dashboard"`.
- **Services** is wired to sidebar item ⌘5 (Library/Discover/Trending/Snapshots/Services/Activity is the current order).
- **Settings** mounts at z-index 81 (overlay + dialog) inside `+page.svelte` when `ui.settingsOpen === true`. Triggered by ⌘, keyboard shortcut, the sidebar gear icon, or any "Open Settings" deep link from an error state.
- **DeviceFlowModal** mounts inside `SettingsSectionGitHub` when `github.signinState.kind !== "idle"`.
- **IssueModal** mounts inside `PackageDetail` and is also reachable from the Categories row's "Wrong?" affordance when the user is signed in.
- **All 7 SettingsSection\*** components are rendered inline by `Settings.svelte` based on `activeSection`; only one is mounted at a time. (Network now contains the Phase 15 Updates subsection — see SettingsSectionUpdates below.)
- **SortableHeader** is consumed by Library, Trending, and Services list grids.
- **UpdateIndicator** mounts in `TitlebarControls.svelte` — only rendered when `updater.available !== null` AND Offline Mode is off.
- **GithubMarkIcon (Octocat chip)** mounts in `TitlebarControls.svelte` — hidden when signed out, green when scope-complete, amber when scope-incomplete.
- **UpgradeModal** mounts inside `Dashboard.svelte` and opens when the user clicks "Choose…" next to "Upgrade all" on the Updates card.

### Phase 15 + 17 additions (v0.3.0)

Net new components since the last inventory refresh (2026-05-23):

| Component | File | Props | State owned | Notes |
|---|---|---|---|---|
| `UpdateIndicator` | `UpdateIndicator.svelte` | (none — reads `updater` + `settings` stores) | — | Title-bar pill that surfaces a "newer brew-browser version is available" notice in chrome. Click opens Settings → Network → Updates. Inline × dismisses-as-skip (`updater.skip(version)`). Spinner replaces × while `updater.installing === true`. ARIA role="button" with explicit Space/Enter handlers; nested real `<button>` for the dismiss. |
| `SettingsSectionUpdates` | `SettingsSectionUpdates.svelte` | (none — reads `updater` + `settings`) | local: `info` derived from `updater.available`, `releaseNotesUrl` derived from version | Mounted at the bottom of `SettingsSectionNetwork`. Three always-visible rows: Check-for-updates-now button, Auto-check-daily toggle (bound to `settings.updateAutoCheck`), Update channel (read-only "Stable" for now). One conditional card when an update is available: v-tag heading + release notes link (canonical GH tag URL derived from version) + Install/Relaunch/Try-again button (per `updater.installComplete` / `installing` / `error` state). |
| `GithubMarkIcon` | `GithubMarkIcon.svelte` | `size?: number`, `class?: string` | — | Real Octocat SVG, path lifted from Primer/Octicons (MIT). Used because Lucide strips brand icons (trademark policy). API matches Lucide-icon shape so it slots into icon-anywhere call sites. Rendered as the Octocat chip in TitlebarControls. |
| `UpgradeModal` | `UpgradeModal.svelte` | `open: boolean`, `onClose: () => void` | `selected: Map<string, boolean>`, `upgrading: boolean` | Curated multi-select Upgrade flow. Lists every outdated package with checkbox + name + current→target + pinned badge (pinned packages start unchecked + disabled — brew refuses to upgrade them anyway). Top toolbar: live "N of M selected" + Select all / Deselect all. On submit: single batched `brew_upgrade_many(names)` IPC streams into the Activity drawer. Errors flow through `reportableToastError`. Triggered from the Dashboard Updates card's "Choose…" button. |

### v0.4.0 additions

| Component | File | Props | State owned | Notes |
|---|---|---|---|---|
| `TrendingSparkline` | `TrendingSparkline.svelte` | `data: number[]`, `variant?: "inline" \| "detail"`, `title?: string` | local: `dims`, `pathD`, `lastPoint` derived | Shared SVG line sparkline. `inline` variant is 60×16 with stroke-only (used in trending list rows); `detail` is 360×80 with a "current" dot at the right edge (used in PackageDetail). Auto-fits min/max scaling so trajectory is the message regardless of magnitude. Renders nothing gracefully when data is empty, all-zero, or single-point — the empty branch renders an em-dash placeholder so column heights stay consistent across packages. |
| `SettingsSectionTrendingHistory` | `SettingsSectionTrendingHistory.svelte` | (none — reads `settings` store) | — | Opt-in subsection for the `brew-browser.zerologic.com/trending-history/*` endpoint. Single toggle bound to `settings.enhancedTrendingEnabled`; disabled and "locked off" message when Offline Mode is on. Hint copy spells out the data practice in plain language (only the package name sent, no IP logging, no cookies, no fingerprinting). Modeled on `SettingsSectionUpdates.svelte` — same nested-section pattern, mounted at the bottom of `SettingsSectionNetwork.svelte` alongside the existing Updates subsection. |

**Trending tab restructure** (`Trending.svelte`):
- Default sort key changed from `rank` (asc) to `velocity` (desc) — the headline v0.4.0 change. The whole point of the work is to surface what's accelerating, not the dep-chain leaderboard.
- New `Velocity` column with `Flame` / `Snowflake` / dash badge + numeric value. Tier-coded: surge (`>= 1.5`) accent-warm, cool (`<= 0.5`) accent-cool, neutral plain. Sort with None-last regardless of direction so missing-velocity entries don't win the leaderboard.
- New `velocityOf(name, kind, fallback)` helper prefers the index blob's server-precomputed value (freshest, nightly) and falls back to the `velocityIndex` carried on each `TrendingEntry` from the backend.
- Count cell becomes vertical-flex: formatted number on top, inline `TrendingSparkline` beneath (when `enhancedReady` derived is true). One HTTP GET to `/trending-history/index.json` on tab mount populates every row's sparkline — no per-row fetches.
- Responsive grid reworked from 7 cols to 8 (added Velocity); breakpoints drop columns in priority order (`<=1200px` drops Trail, `<=1000px` also drops Description, `<=800px` also drops Version, `<=640px` also drops Type).

**PackageDetail integration** (`PackageDetail.svelte`):
- New `trend-card` section between the description/homepage area and the AI-enriched blocks. Renders the `detail`-variant `TrendingSparkline` from a full `TrendingHistorySeries`.
- Strictly **passive** per D4: NO placeholder when the toggle is off. The section simply doesn't exist. No "Enable in Settings" CTA, no banner.
- `loadDetail` fires `trendingHistory.ensureSeriesLoaded(name, kind)` after `brewInfo`. The store's internal `enabled` getter no-ops when the toggle is off, so the call is always safe.
- Sub-header label distinguishes seed-only history ("Bootstrap + daily snapshots — granularity grows over time") from real daily history ("Daily install snapshots").

**New store** (`trendingHistory.svelte.ts`):
- `index: TrendingHistoryIndex | null` — the summary blob.
- `seriesByKey: Map<string, TrendingHistorySeries>` — per-package full series cache (key = `"{kind}:{name}"`).
- `loadingIndex` / `loadingSeriesKeys: Set<string>` — in-flight markers prevent redundant concurrent fetches.
- `enabled` getter — single source of truth for "toggle on AND paranoid off."
- `ensureIndexLoaded()`, `ensureSeriesLoaded(name, kind)` — idempotent, silent-on-failure (feature is enrichment, not load-bearing).
- `entryFor`, `sparklineFor`, `velocityFor` — sync lookups over the cached index. Lazy lookup table rebuilt only when the index changes.
- `seriesFor(name, kind)` — sync lookup over the cached per-package series.

### v0.5.0 additions — Opt-in vulnerability scanning

| Component | File | Props | State owned | Notes |
|---|---|---|---|---|
| `SettingsSectionVulnerabilities` | `SettingsSectionVulnerabilities.svelte` | (none — reads `settings` + `vulnerabilities` stores) | local: `installInFlight`, last-scan summary derived | Opt-in subsection for the `vulnerability_scanning_enabled` toggle. Mounted at the bottom of `SettingsSectionNetwork.svelte` alongside the existing Updates + Enhanced Trending History subsections. Single toggle disabled with "locked off" message when Offline Mode is on. When `brew vulns` isn't installed, surfaces an inline install affordance (button → `vulns_install_helper` IPC, captures stdout into the Activity drawer). Credit line + link to `https://github.com/Homebrew/homebrew-brew-vulns` makes the provenance loud ("Powered by brew vulns by Andrew Nesbitt"). Hint copy spells out the data practice in plain language: what's sent to OSV, what's sent to GitHub Advisories (only when GitHub auth is on), what isn't (no IP correlation since the subprocess speaks for itself; no telemetry to brew-browser infra). |

**PackageRow integration** (`PackageRow.svelte`):
- New severity dot rendered inline next to the installed pill. Color encodes max-severity from `vulnerabilities.byPackage` lookup (critical → red, high → orange, medium → amber, low → blue, unknown → grey). Hidden when the store has no entry for the package OR when the feature is off (the store's `enabled` getter gates the lookup).
- Tooltip on hover: "N CVE(s) — click for details" routes the user to the PackageDetail Security card.
- Zero-impact when the feature is off: `vulnerabilities.maxSeverityFor(kind, name)` returns `null` synchronously without any IPC hop, so the dot's `{#if}` branch elides cleanly.

**PackageDetail integration** (`PackageDetail.svelte`):
- New `Security` card section between the description and the AI-enriched blocks. Renders one row per CVE/GHSA with severity pill (Critical / High / Medium / Low / Unknown), advisory ID (linkified to GHSA or OSV when a `references[0]` URL exists), summary text, and "fixed in" range when present.
- "Upgrade to fix" button wired to the existing `brew_upgrade` pipeline (single-package upgrade with live Activity-drawer stream). Visible only when the package is outdated AND at least one CVE has a `fixed_in` range — there's no point offering the action if the upgrade can't help.
- "Check vulnerabilities" button triggers `vulnerabilities.scanOne(name)` for the current package; useful for cask-formula-pair edge cases and when the user wants to bypass the install-set cache for one package without the full Refresh.
- For casks: the section renders the same shell but reads "Cask coverage isn't supported — `brew vulns` is formula-only" instead of a CVE list. Honest UX about the coverage gap; no fake clean state.
- Strictly passive when the feature is off: the section simply doesn't exist (no "Enable in Settings" CTA, no banner). Matches the `trendingHistory` D4 pattern.

**Dashboard integration** (`Dashboard.svelte`):
- New `Exposure` card alongside the existing Updates / Composition / Categories cards. Renders severity counts as a horizontal bar (critical / high / medium / low / unknown) with a numeric total and a "Scan now" button (calls `vulnerabilities.scanAll(force=true)`).
- Clean-state framing: when the scan completed and `vulnerablePackages === 0`, the card shows a ✓ checkmark with "No known vulnerabilities" rather than collapsing — the clean state IS the message.
- Hidden when the feature is off (consistent with Personal Stats hiding when GitHub auth is off).
- Reads from `vulnerabilities.severityCounts` derived getter (`$derived` rollup across the store's full Map); no per-render scan.

**Sidebar integration** (`Sidebar.svelte`):
- Count badge on the Library nav item: shows the number of vulnerable installed packages (`vulnerabilities.severityCounts.vulnerablePackages`). Max-severity tone (red for critical, orange for high, amber for medium, blue for low, grey for unknown).
- Hidden when count is 0 OR feature is off — same "absence of badge is the clean state" pattern as the existing outdated / running-ops badges.

**Refresh-feed integration** (cross-cutting):
- The post-`brew update` refresh fan-out (Dashboard Refresh button, Library Refresh button) fires `vulnerabilities.scanAll(force=false)` after the catalog reload. The `force=false` parameter means the install-set fingerprint skip predicate still applies — a refresh that only ran `brew update` (without any install changes) won't re-shell `brew vulns`.
- Post-mutation hooks (install / upgrade / uninstall in `packages.svelte.ts` action wrappers) call `vulnerabilities.invalidate(kind, name, version)` AND then `vulnerabilities.scanOne(name)` so the affected package's CVE row reflects the new state immediately.

**New store** (`vulnerabilities.svelte.ts`, ~350 lines):
- `byPackage: Map<string, VulnRecord>` — keyed by `"{kind}:{name}"` (no version — the store is the "what's vulnerable RIGHT NOW" view; versioning is the backend cache's concern).
- `severityCounts: SeverityCounts` derived — `{ critical, high, medium, low, unknown, total, vulnerablePackages }` rolled up across the full Map. Bound directly by the Dashboard Exposure card and Sidebar badge.
- `lastScanAt: Date | null`, `lastScanSource: "live" | "cache" | null`, `lastScanError: BrewErrorPayload | null` — surfaces for the Settings card and the optional debug strip.
- `enabled` getter — single source of truth for "toggle on AND paranoid off." Same pattern as `trendingHistory.enabled`.
- `scanAll(force?: boolean)`, `scanOne(name: string)`, `installHelper()`, `invalidate(kind, name, version)` — wraps the four IPCs.
- `byPackage` / `maxSeverityFor(kind, name)` / `vulnsFor(kind, name)` — sync lookups for inline UI consumers (PackageRow, Sidebar, PackageDetail).
- Error routing: `vulns_not_installed` → captured into `lastScanError` for the Settings card to render the install affordance (not a toast — the install button is the user-facing remediation). Everything else → `reportableToastError` so the user gets the "Report to brew-browser" action.
- Reactivity: writes reassign `byPackage` (Svelte 5 doesn't track `Map.set()` mutations) — mirrors the `trendingHistory.seriesByKey` and `services.setPending` patterns already used elsewhere.

