# Progress

## 2026-05-24 (overnight)

### Done since last sync

- ✅ `git init` + first commit (`653e26f`) — initial release, 186 files
- ✅ `gh repo create msitarzewski/brew-browser --public --push` — repo live on GitHub
- ✅ Bulk categorize run completed against Claude Haiku 4.5 — 15,974 items, $1.50, 19 min
- ✅ Second commit (`c72e31d`) — categories.json (838 KB) + landing page in-repo
- ✅ Third commit (`2dad9be`) — Caddyfile snippet removed (user handles Caddy config manually)
- ✅ Landing page rsync'd to `michael@100.98.187.7:Sites/brew-browser/` on umbp
- ✅ Full SEO/social treatment added to landing: OG, Twitter/X cards, JSON-LD SoftwareApplication, PWA manifest, robots.txt, sitemap.xml, 1200×630 social card
- ✅ Social card iterated through multiple designs based on user feedback
- ✅ `ideas.md` captures: Recipes, optional GitHub OAuth, Liquid Glass / NSVisualEffectView discussion, Discover-UI surface ideas

### Phases

| Phase | Status |
|-------|--------|
| 0 — Scaffold | ✅ |
| 1 — Read-only Homebrew browser | ✅ |
| 2 — Search Homebrew index | ✅ (categories UI pending) |
| 3 — Install/uninstall/upgrade w/ streaming | ✅ |
| 4 — Brewfile snapshot/restore | ✅ (NB: known upstream brew bundle bug surfaced via friendly error mapping) |
| 5 — Polish + build artifact | ✅ (unsigned .dmg; signing pending cert install) |
| 6 — Trending tab | ✅ |
| 7 — Cask icons installed | ✅ |
| 8 — Cask icons homepage cascade | ✅ |
| Security — audit + fixes + tool battery + re-audit | ✅ READY-FOR-SCRUTINY |
| Reframe pass | ✅ counter-narrative dropped from all docs |
| Categorize tool + bulk run | ✅ 15,974 items via Claude Haiku 4.5 |
| Landing page + SEO/social | ✅ deployed to umbp |
| **v0.1.0 GitHub release** | ✅ SHIPPED — signed/notarized .dmg attached at <https://github.com/msitarzewski/brew-browser/releases/tag/v0.1.0> |
| **Phase 9a — Discover category tile UI** | ✅ tile grid + filtered view + Lucide icons, uncommitted |
| **Phase 9b — Category linking pass** | ✅ multi-select chip filter (Discover + Library), category pills on PackageDetail, sortable columns (Library + Trending), fixed dangling `installed` pill, uncommitted |
| **Phase 11 — Dashboard** | ✅ hero/updates/composition/donut/storage cards; brand → home; updates card → outdated library; uncommitted |
| **Phase 11b — Services** | ✅ sidebar item ⌘5, page with start/stop/restart, per-package detail card, sidebar badge for running count, uncommitted |
| **Phase 11c — Native macOS feel** | ✅ vibrancy + drag regions (data-tauri-drag-region + capability), traffic-light-aware sidebar, uncommitted |
| **Phase 11d — Activity persistence** | ✅ localStorage mirror, cap 50 jobs / 500 lines, hydrate on bootstrap, uncommitted |
| **Phase 12a — Bundled catalog + manual refresh** | ✅ |
| **Phase 12b — Settings shell** | ✅ |
| **Phase 12c — GitHub anonymous tier** | ✅ (combined with 12e in one Backend Architect pass) |
| **Phase 12d — Settings: network + paranoid + settings persistence** | ✅ |
| **Phase 12e — GitHub Device Flow OAuth + Keychain** | ✅ (combined with 12c) |
| **Phase 12f — GitHub authed actions** | next (after Wave 1+2 commit) |
| **Phase 13 — Catalog enrichment (Haiku)** | queued, parallel-OK with 12f |
| **Phase 14 — bundled cask icons** | DROPPED (trademark/redistribution risk) |
| **Phase 9c — "Wrong?" GitHub-issue link** | folds into 12f |
| **Phase 9d — `installedAt` on Package + Last-Updated sort** | small standalone, not in any phase |
| **Phase 10 — Recipes** | deferred — catalog now available so unblocked |

### Phase 9b notes

- New store: `src/lib/stores/discover.svelte.ts` — multi-select `selectedCategories: Set<string>`, shared by Discover + Library + PackageDetail. `selectOnly(slug)` for tile-click semantics, `toggle(slug)` for chip add/remove.
- Discover.svelte: replaces local single-`activeCategory` with the shared store; tile click → adds single chip; chip bar above results with per-chip X + Clear button; search results filter to OR-match selected chips; chip-only browse mode (no query, chips set) lists union sorted alphabetically.
- Fixed UX bug from the user's screenshot: `installed` pill no longer floats. Two row layouts: `.row--with-desc` (1fr 80px 2fr auto) for search; `.row--no-desc` (1fr 80px auto) for chip-filtered browse.
- PackageDetail: new "Categories" meta row with clickable pills. Click jumps to Discover with that single category selected (closes detail panel so user lands on the filtered list, not an obscured view).
- New component: `src/lib/components/SortableHeader.svelte` — small reusable header button with up/down arrow indicator, click toggles direction or switches column. Uses `aria-label` (not `aria-sort`, since that requires `role="columnheader"` and our list-grids aren't true tables).
- Library: sortable Name / Version / Type / Outdated; shares the Discover category chips so the user can keep context across tabs; updated empty-state messaging to reflect chip vs. text filters separately.
- Trending: sortable # / Name / Type / Installs. Installs defaults to descending on first click.
- Lint/test: `npm run check` 0 errors / 1 pre-existing warning. `npm run build` clean in 1.64s. Backend untouched this pass — no Rust regression risk.
- Status: code is in working tree, NOT committed. Awaiting user UX confirmation.

### Phase 11 notes (Dashboard + Services + native feel + persistence)

Single big session 2026-05-24-night. Highlights:

- **Dashboard.svelte** is the new default landing. Hero row (installed / outdated / brew version), Updates panel with one-click upgrade-all (and the title is a clickable link → Library outdated filter), Composition split bar with on-request/dep/pinned meta, Top-Categories donut (180px SVG, 9-color palette, top 8 + Other, click legend → Discover with chip pre-selected), Storage card with 4 paths and Open-in-Finder per row.
- **Donut math:** `stroke-dasharray="(pct/100)*C C"` + `stroke-dashoffset="-(startPct/100)*C"` + `rotate(-90)` for top start. Center text shows total installed.
- **Services backend** (`commands/services.rs`): 5 commands (list, clear-cache, start, stop, restart), 5s list cache, write-lock around state mutations, alphanumeric+symbol name validation.
- **Services frontend:** sidebar item ⌘5, sortable Name/Status/User columns, per-row action buttons (smart-disabled by current state), badge = count of running services. PackageDetail shows a Service card with pill + 3 buttons when the formula has a brew services entry.
- **Disk usage backend** (`commands/disk_usage.rs`): `disk_usage` + `open_in_finder`, 4 paths surveyed in parallel via `tokio::join!`, 60s cache, security gate on Finder reveal (must be inside Homebrew prefix/cache).
- **Native macOS feel:** vibrancy via `window-vibrancy = "0.6"` + `apply_vibrancy(NSVisualEffectMaterial::HudWindow, …)`; tauri.conf.json `transparent: true` + `titleBarStyle: "Overlay"` + `hiddenTitle: true`; sidebar brand padded to clear traffic lights; `data-tauri-drag-region` on brand-wrap + every panel-head with the new `core:window:allow-start-dragging` capability.
- **Activity persistence:** localStorage mirror `brew-browser:activity:v1`, cap 50 jobs / 500 lines per job, debounced 400ms writes + immediate flush on terminal events, hydrate from +layout mount.
- **Sortable lists hardening:** `1fr` → `minmax(0, 1fr)` everywhere a flex column had text (Discover, Library header + PackageRow, Trending). Fixed cross-row pill alignment that depended on name length. Also `auto` → `90px` for the installed column so it doesn't collapse-and-shift the kind cell.
- **Trending Refresh fix:** the force flag now busts the backend cache before calling `trending_fetch` — was silently ignored before.
- **Dashboard scroll + drag bug fix:** removed the fixed-position drag-overlay (was eating scroll wheel + not actually triggering drag); fixed flex children getting shrunken to fit by adding `.body > * { flex-shrink: 0 }`.
- **Test count:** 207 → 210 (3 new for `services` name validation, 2 for `disk_usage` du_bytes, 1 for `categories` already counted last session = pre-session 204 + 6 = 210).

### Phase 9a notes

- Backend: `commands/categories.rs` — `categories_data` Tauri command, embeds JSON via `include_str!` (zero runtime file dep), parsed once + memoised on `AppState.categories_cache`. 1 new unit test (205 total, was 204).
- Frontend types: `CategoryMeta`, `CategoriesData` in `types.ts`.
- API wrapper: `categoriesData()` in `api.ts`.
- Store: `src/lib/stores/categories.svelte.ts` — lazy-load, derived `tiles` (sorted by count, uncategorized last), `tokensInCategory(slug)` for the filtered view, `categoriesOf(name, kind)` for future chip rendering.
- Icon resolver: `src/lib/util/categoryIcon.ts` — static map of 19 Lucide icons, falls back to `HelpCircle`.
- Discover.svelte: new tile grid (`auto-fill, minmax(180px, 1fr)`), clicking a tile drills into a filtered list, back button returns to grid. Search still wins when there's a query.
- Lint/test: cargo clippy `-D warnings` clean, cargo test 205 pass, `npm run check` 0 errors, `npm run build` clean.
- Status: code is in working tree, NOT committed. Awaiting user sign-off on UX before commit.

### Test + build status (current)

- `cargo test --manifest-path src-tauri/Cargo.toml`: **210 passed / 0 failed / 6 ignored** (up from 204)
- `cargo check`: clean
- `cargo clippy --all-targets -- -D warnings`: clean
- `npm run build`: clean
- `npm run check`: 0 errors (1 pre-existing tsconfig-node warning)
- `cargo deny check`: advisories ok, bans ok, licenses ok, sources ok (pre-session)
- `cargo tauri build`: produces signed/notarized 5.7 MB `.dmg` (v0.1.0 already shipped)

### Security posture

| Tool | Result |
|------|--------|
| Wave 1 audit findings | **16/16 verified fixed** (0C / 0H / 0M / 0L / 0N open) |
| `cargo audit` | 0 vulns |
| `cargo deny check` | advisories+bans+licenses+sources ok |
| `npm audit --omit=dev` | 0 vulns |
| `osv-scanner` | 19 advisories (all Linux-only or acknowledged) |
| `gitleaks` | 0 leaks in source |
| `semgrep` (security-audit + OWASP-10 + rust + typescript) | 0 findings |
| `unsafe` Rust in brew-browser | 0 |
| `@html` / `innerHTML` / `eval` in frontend | 0 |
| Tauri shell plugin | not used (IPC is the security boundary) |

### Open items

| Item | Blocker |
|------|---------|
| Apple Developer ID Application cert | User must install via developer.apple.com |
| Signed + notarized `.dmg` | Above |
| v0.1.0 GitHub release with `.dmg` attached | Above |
| Updated social card PNG saved to persistent path | User must drop file somewhere I can grab |
| Master icon swap to beer-mug variant (optional) | Decision pending |
| Phase 9 — Discover category UI build | Ready to start when user signals |

### Repo state

```
/Users/michael/Clean/brew-browser/  (15.9k+ packages categorized, 2 production commits + this sync pending)
├── LICENSE                           MIT
├── README.md                         polished, security section, 4-path network disclosure
├── CONTRIBUTING.md                   141 lines
├── SECURITY.md                       responsible disclosure
├── PLAN.md                           phase tracker
├── .gitignore                        comprehensive (target/, node_modules/, .env, etc.)
├── package.json                      brew-browser, MIT
├── src/                              36+ files
├── src-tauri/
│   ├── src/                          22 Rust files (modular)
│   ├── Cargo.toml                    8 deps
│   ├── deny.toml                     permissive-license allowlist
│   ├── data/categories.json          838 KB — 7,607 casks + 8,367 formulae from Haiku 4.5
│   ├── icons/                        38 minted platform icons
│   ├── tests/                        integration + 10 real-brew fixtures
│   └── target/release/bundle/dmg/    brew-browser_0.1.0_aarch64.dmg (6.1 MB, unsigned)
├── tools/categorize/                 offline LLM-driven category tool
│   ├── categorize.py                 main script
│   ├── prompts/system.txt            calibration prompt
│   ├── .env.example                  template (real .env gitignored)
│   ├── state/last-tokens.json        diff state (15,974 tokens recorded)
│   └── README.md                     setup + cron docs
├── landing/                          static landing page
│   ├── index.html                    full OG/Twitter/JSON-LD/PWA treatment
│   ├── style.css                     OKLCH tokens, dark-first
│   ├── brew-browser.svg              icon copy
│   ├── manifest.json                 PWA
│   ├── robots.txt + sitemap.xml      SEO basics
│   ├── social-card.png / .svg        1200×630
│   └── README.md                     deploy via rsync to umbp
├── docs/icon/                        master SVG + size previews
└── memory-bank/                      20 files (this dir)
```

## 2026-05-24 (late session — Phase 12 Wave 1 + Wave 2)

### Done

- ✅ Commit `84ad010` pushed (Phase 9 + 11 + memory bank refresh)
- ✅ Phase 12 plan written: `memory-bank/phase12-plan.md`
- ✅ Pre-implementation Security Engineer review: `memory-bank/scans/phase12-security-review.md` — APPROVED with explicit gates
- ✅ **Phase 12a** (Backend Architect agent) — bundled catalog + manual refresh, +38 tests
- ✅ **Phase 12b** (Frontend Developer agent) — Settings shell + 6 sections + brew analytics, +8 tests
- ✅ **Phase 12d** (Backend Architect agent) — paranoid mode + settings persistence + Network section, +18 tests
- ✅ **Phase 12c + 12e** combined (Backend Architect agent) — GitHub anonymous tier + Device Flow + Keychain, +60 tests
- ✅ Phase 13 plan written: `memory-bank/phase13-plan.md` — Tier A friendly names+summaries, Tier B use cases+similar+tags, AI Features master toggle, ~$20 cost, zero runtime LLM calls
- ✅ Phase 14 (bundled cask icons) **explicitly DROPPED** — trademark/redistribution risk, runtime probe + paranoid gate is enough

### Phases (updated)

| Phase | Status |
|-------|--------|
| **Phase 12a — Bundled catalog + manual refresh** | ✅ |
| **Phase 12b — Settings shell + brew analytics** | ✅ |
| **Phase 12c — GitHub anonymous repo stats** | ✅ (combined with 12e) |
| **Phase 12d — Paranoid + network settings + settings persistence** | ✅ |
| **Phase 12e — Device Flow OAuth + Keychain** | ✅ (combined with 12c) |
| **Phase 12f — GitHub authed actions** | next |
| **Phase 13 — Catalog enrichment** | queued, can run parallel with 12f |
| **Phase 14 — bundled cask icons** | DROPPED (trademark risk) |
| **Phase 10 — Recipes** | deferred — depends on catalog (now available) |
| **Phase 9d — installedAt + Last-Updated sort** | small standalone, not blocking |

### Test + lint status (current)

- `cargo test`: **334 passed / 0 failed / 6 ignored** (was 274 at start of Wave 2; 210 at start of session)
- `cargo clippy --all-targets -- -D warnings`: clean
- `npm run check`: 0 errors, 1 pre-existing tsconfig-node warning
- `npm run build`: clean

### Phase 12 Wave 2 notes

**Wave 2 ordering (Option A — Foundation-first):** 12d → combined 12c+12e → 12f. Locked in by user. Reasoning: 12d delivers `require_network` helper + settings persistence; 12c+12e then consume the helper directly (no TODOs); combined as one Backend Architect pass because 12c and 12e both touch the same `src-tauri/src/github/` module.

**Phase 12a key deviations from spec (accepted):**
- `CatalogRefreshInProgress` returned as generic `InvalidArgument` until 12d added the proper variant — agent left a TODO grep-marker
- Bundled gzipped catalog is 6.1 MiB not "~3 MiB" estimate (catalog grew upstream)
- `fetch.py` doesn't strip unused JSON fields (deferred — would shrink to ~1 MiB at cost of build-time coupling to Rust struct shape)

**Phase 12b key deviations (accepted):**
- `commands/mod.rs` alphabetical position is true-alphabetical (brew_env < brewfile), not literal-spec position
- Lucide has no `github` icon (trademark) → `git-fork` substituted for the GitHub section
- Settings modal sized `220px nav + 1fr content` not `350px + 600px` (looked awkward at macOS density)
- `brew_get_analytics` parser accepts both trailing-period and non-period forms (empirically brew has shipped both)
- Activity caps wired to Settings but not yet consumed by activity store (deferred — value persists, no retroactive trim)

**Phase 12d key deviations (accepted, more conservative than spec):**
- Unknown enum variant → file treated as Corrupt → fail closed (instead of "log + substitute default"). Aligned with §12d "fail closed when corrupt" rule
- `require_network` gates Trending even on cache hits (UX consistency over micro-savings)
- Catalog stale banner threshold + cask icon mode NOT retroactively wired to consume the new settings — store is ready, consumers swap as a 1-line change later

**Phase 12c+12e key deviations (accepted):**
- Cache TTL backdating test rewritten as constant-pin + fresh-read positive test (filetime isn't a dep)
- CSP comment moved to Rust module docs (tauri-build rejects unknown JSON fields like `_comment_csp`)
- Custom `KeychainSlot` trait + in-memory mock instead of `keyring` crate's mock feature (same coverage, no runtime context switch)
- Username resolution failure is non-fatal during sign-in (token still stored; username shows "github user" until next sign-in)
- `AuthRequired` + `ScopeRequired` error variants land but `#[allow(dead_code)]` until 12f consumes them

### Files staged (47 changes, ready to commit)

Backend: `Cargo.toml`, `Cargo.lock`, `tauri.conf.json`, `capabilities/default.json`, `src/{catalog,github,util}/`, `src/commands/{catalog,brew_env,disk_usage,github,services,settings}.rs`, `src/commands/{mod,trending,cask_icon_homepage}.rs` (paranoid gate wiring), `src/{error,lib,state}.rs`

Frontend: `src/{app.css, +layout, +page}.svelte`, `src/lib/{api,types}.ts`, `src/lib/components/{Dashboard, Discover, Library, PackageDetail, PackageRow, Services, Settings, SettingsSection*, DeviceFlowModal, Sidebar, Snapshots, Trending, ActivityHistory, SortableHeader}.svelte`, `src/lib/stores/{activity, categories, discover, github, library, services, settings, trending, ui}.svelte.ts`, `src/lib/util/categoryIcon.ts`

Docs: README.md (Open by default → 7 paths + Paranoid Mode), BUILD.md (GitHub OAuth section), memory bank updates

Data: `src-tauri/data/catalog/{formula,cask}.json.gz` + `manifest.json`

Tools: `tools/catalog/{fetch.py,README.md}`

Misc untracked: `PHILOSOPHY.md` (user-authored, 271 lines)
