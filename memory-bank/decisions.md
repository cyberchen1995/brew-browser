# Architectural Decisions

## 2026-05-23: MIT License

**Context:** Need an OSI-approved license for an open-source macOS GUI utility. Considered GPL (copyleft, prevents closed forks, dual-license option), AGPL (network-services clause, irrelevant for a desktop app), and source-available licenses (FSL/BSL — not OSI-approved).

**Decision:** MIT.

**Rationale:**
- Most permissive and most recognizable OSI license — clearest "use this however you want" signal
- Lowest friction for a contributor-friendly small utility: no CLA needed, no copyleft compliance overhead for downstream users
- Contributor retains copyright on own contributions, so monetization options (paid binaries, App Store, support, dual-license) remain open
- Matches the dependency stack (Tauri MIT/Apache, Svelte MIT, reqwest MIT/Apache) so there are no license-compatibility seams

**Trade-off accepted:** Anyone can fork and ship a closed derivative. For a small utility this is fine; the value is in the live project, not the license clause.

---

## 2026-05-23: Tauri 2 over Electron / Flutter / GPUI

**Context:** Need cross-platform desktop framework. Electron is the historical default but heavy. Flutter renders everything custom. GPUI (Zed's) is pre-1.0 and Zed-coupled. Tauri 2 ships a native webview, ~8 MB bundles, supports mobile.

**Decision:** Tauri 2 + SvelteKit + Svelte 5 + TypeScript.

**Rationale:**
- Smallest binary footprint, fastest cold start
- Web-developer ergonomics for the UI (Svelte 5 = minimal ceremony, fast HMR)
- Rust backend is appropriate for shelling out to `brew` safely
- Tauri 2's iOS/Android support keeps a mobile path open without re-platforming

---

## 2026-05-23: Shell out to `brew`, don't reimplement

**Context:** Could reimplement Homebrew operations directly (parse formula files, manage downloads, etc.) or shell out to the `brew` CLI.

**Decision:** Shell out exclusively. Use `--json=v2` output formats wherever available.

**Rationale:**
- `brew` is the source of truth; reimplementing duplicates state and creates drift
- `--json=v2` outputs are stable contracts
- A respectful UI on top of `brew` is the right scope for this project

---

## 2026-05-23: Trending data from `formulae.brew.sh`

**Context:** Need data source for "trending packages" tab. Options: scraping web pages, building our own analytics, using Homebrew's published analytics.

**Decision:** Use `https://formulae.brew.sh/api/analytics/install/<window>.json` — Homebrew's own published analytics, no auth required, no scraping.

**Rationale:**
- Authoritative source; no reverse-engineering or scraping
- No keys, no rate-limit-as-product
- Cache in memory ~1 hour to be a polite client
- Keeps brew-browser a respectful frontend on top of Homebrew-owned data

---

## 2026-05-23: Serialize brew invocations with a Mutex

**Context:** `brew` does not tolerate concurrent operations against its own state (lockfile collisions, partial installs). UI could trigger overlapping commands.

**Decision:** Wrap all `brew` invocations in a single `tokio::sync::Mutex<()>` held in Tauri managed state.

**Rationale:**
- Prevents data corruption with zero user-visible cost (queue and show queue state)
- Implementation is ~10 LOC
- Future: per-command-class mutex if read-only ops (`list`, `info`, `search`) should run in parallel with writes

---

## 2026-05-24-night: Dashboard is the default landing, brand area is the home button

**Context:** First-launch UX dropped users into a 325-row Library list — overwhelming. Need a friendlier "state of your setup" first impression. Open question: separate sidebar item or repurpose the brand area as the home affordance?

**Decision:** Dashboard becomes the default `ui.section` (over Library). The sidebar brand (`🍺 brew-browser`) is the home button — clicking it returns to Dashboard. No separate "Dashboard" nav item in the sidebar list; brand = logo = home, in line with how web apps work.

**Rationale:**
- First impression frames the relationship: "this is your setup" rather than "here's a list"
- Brand-as-home mirrors universal web-app convention; users already try clicking the logo
- Keeps the sidebar nav list tight (5 items: Library/Discover/Trending/Snapshots/Services/Activity)
- Cmd+0 reserves a stable keyboard shortcut for home, parallel to Cmd+1..6 for sections

**Trade-off accepted:** Discoverability of Cmd+0 is weaker than a visible nav item, but the brand's active state (background highlight when on Dashboard) provides visual feedback. Power users learn shortcuts; everyone else clicks the brand.

---

## 2026-05-24-night: Vibrancy via `window-vibrancy` (Tier A), Tahoe Liquid Glass deferred

**Context:** Requested "native feel" / Liquid Glass treatment. Tier A = NSVisualEffectView via `tauri-plugin-window-vibrancy` (works since macOS 13, ~30 min to wire). Tier B = true Tahoe Liquid Glass via Swift bridge (Tahoe-only, half day).

**Decision:** Ship Tier A this session. Defer Tier B to v0.2.

**Rationale:**
- Tier A delivers 80% of the visual win for 20% of the work
- Works across all supported macOS versions (13+) — Tier B would gate the feel on macOS 26 only
- Tier B's Swift bridge is reversible — we can layer it on top of Tier A later without breaking anything
- Surfaces a real `core:window:allow-start-dragging` capability requirement which had to be added regardless

**Implementation notes:**
- `tauri.conf.json`: `transparent: true`, `titleBarStyle: "Overlay"`, `hiddenTitle: true`
- Apply `NSVisualEffectMaterial::HudWindow` in `lib.rs` setup hook
- Body background must be transparent in CSS for the vibrancy to show through
- Drag regions use `data-tauri-drag-region` attribute on real DOM elements (NOT a fixed overlay, which intercepts scroll wheel events) — added via the new capability

---

## 2026-05-24-night: Categories donut chart (top 8 + Other), not bar list

**Context:** First implementation was a horizontal bar list per category. Two bugs surfaced: (a) bar fill color matched track in dark mode so bars looked empty, (b) when one category dwarfs the rest (Developer Tools at 256 vs next at 29), small bars are visually meaningless.

**Decision:** Donut chart with top 8 individually segmented + "Other" slice. Clickable legend on the right.

**Rationale:**
- Donut conveys *proportion at a glance* better than tiny bars
- Color encodes category (palette-rotated) — no fill/track collision possible
- Top 8 + Other is a standard pattern for long-tail distributions
- Legend rows double as nav: click → Discover with category chip selected
- 180px donut + SVG arcs = ~30 lines of CSS, no chart lib needed

**Math:** Each segment is a `<circle>` with the donut's full circumference as `stroke-dasharray` second value, with first value = `(pct/100) * C`. `stroke-dashoffset = -(startPct/100) * C` shifts each segment to start at the right angle. `transform="rotate(-90)"` puts segment 0 at 12 o'clock.

---

## 2026-05-24-night: `du -sk` over native walk for disk usage

**Context:** Need to size 4 Homebrew sub-trees (Cellar, Caskroom, var/log, cache). Could walk filesystem in Rust or shell out to `du`.

**Decision:** Shell out to `du -sk <path>` in parallel via `tokio::join!`.

**Rationale:**
- BSD `du` is highly optimised (cached inode stats, sparse-file aware)
- Single syscall per path vs O(n) recursion in Rust + serde overhead
- Parallel via `tokio::join!` keeps wall time = max(4 paths) not sum
- 60s cache on AppState means subsequent reads cost nothing
- Output parsing is trivial: `<kb><tab><path>`

**Trade-off accepted:** Shells out to an external binary. But we already shell out to `brew` everywhere; `du` is a base-system tool that's been on every Unix for 40+ years and can't reasonably be missing.

---

## 2026-05-24-night: Bundled catalog + user-initiated refresh (Phase 12a — planned)

**Context:** formulae.brew.sh exposes the entire Homebrew catalog as JSON (~10 MB raw). Caching it locally unlocks fast search, deprecation warnings, build-error stats, reverse deps, and more. But network calls without consent break the project posture; auto-refresh without explicit user action would add a quiet 5th outbound network path.

**Decision:**
- **Bundle a baseline catalog at build time** via `include_bytes!` + gzip (~3 MB compressed)
- **User-initiated refresh** only: a button writes a fresh fetch to `~/Library/Application Support/brew-browser/catalog/`
- **Resolution order at runtime:** user-data catalog (if present) → bundled fallback
- **Soft nudge** (banner) when active catalog is older than 14 days; dismissable
- **No auto-refresh** by default; deferred opt-in setting later

**Rationale:**
- Matches the project posture: every network path is disclosed in README and user-consented
- Baseline-in-binary keeps the app fully functional offline / first-launch
- Manual refresh respects user agency; freshness is visible (timestamp shown)
- Unblocks deprecation warnings, build-error rates, reverse deps, dependency tree, Brewfile validation, "what's new this week" feed — all become catalog reads instead of fresh network fetches

**Network disclosure:** Adds a 5th outbound path, explicitly labeled "user-initiated only".

---

## 2026-05-24-night: GitHub integration via Device Flow (Phase 12c–f — planned)

**Context:** Many packages have GitHub homepages — stars/forks/last-release would enrich PackageDetail. User-authenticated actions (star, file issue, watch) are bigger wins but require OAuth. Native apps can't safely embed client secrets, so PKCE/Implicit are out.

**Decision:** Two-tier GitHub integration.

**Tier 1 — anonymous (no sign-in):**
- Detect GitHub homepage URLs (`/^https?:\/\/github\.com\/(\w+)\/(\w+)$/`)
- Hit public api.github.com (60 reqs/hr anon limit)
- 24h disk cache per repo
- Show: stars, forks, last release date, archived flag, license match-check
- Graceful degradation when rate-limited

**Tier 2 — signed in (OAuth Device Flow):**
- Setting: "Sign in with GitHub"
- Device Flow (RFC 8628): no client secret needed for native apps, designed for them
- Token stored in macOS Keychain via `tauri-plugin-keyring` (or similar)
- Rate limit goes to 5000/hr
- Unlocks: star/unstar, file issue (with pre-filled context), watch, "Wrong?" reporting

**Rationale:**
- Anonymous tier ships value immediately, no consent friction beyond toggling on
- Device Flow is the correct OAuth profile for native apps — no localhost redirect, no embedded WebView, no client secret in shipped binary
- Keychain storage matches platform expectations; no plaintext tokens on disk
- Two-tier respects users who want zero-account posture (default = off entirely) and power users who want full integration

**Trade-off accepted:** Anonymous tier consumes the 60 reqs/hr per IP — heavy browsing may rate-limit. Pitch sign-in when this happens.

---

## 2026-05-24-night: Phase 14 (bundled cask icons) DROPPED

**Context:** User proposed pre-fetching cask icons at build time and bundling them in the binary to eliminate the runtime cask-homepage-probe network path. Tempting because it would reduce the documented outbound paths from 5 to 4 (or 7 to 6 after Phase 12c+e) and make Discover/Trending render instantly without probe latency.

**Decision:** NOT going to do this. Leave the runtime probe + paranoid-mode gate as the cask-icon story.

**Rationale (the redistribution problem):**
- The current probe fetches and DISPLAYS vendor icons (Slack, 1Password, Adobe, ~7,600 casks) on the user's machine — closest analog is "rendering a page that shows the vendor's logo" → generally OK as nominative fair use
- Bundling those icons into our binary is REDISTRIBUTION at scale — many vendors have explicit "no third-party redistribution of brand assets" terms; even where not explicit, the surface area for trademark complaints is real
- "Brew did it" doesn't apply — Homebrew doesn't bundle icons either, they point at homepages
- The current probe is already very respectful: 7d cache, sandboxed against SSRF (rejects RFC1918 / link-local / loopback / cloud-metadata, redirect re-check), per-cask-per-week max, concurrency-limited semaphore, paranoid mode kills it in one click

**Considered middle ground (rejected):** bundle only FOSS-licensed casks' icons via license filter from Phase 12a catalog + top-N popularity from analytics. Would gain ~90% of the network reduction without ~95% of the redistribution risk. **Rejected** as additional complexity for a non-essential win — the runtime probe is already good enough for paranoid-mode-OFF users, and paranoid-mode-ON users already get zero icon probes.

---

## 2026-05-24-night: Phase 12 Wave ordering (Foundation-first, Option A)

**Context:** Phase 12 had 6 sub-phases (a-f) with various dependency edges. Two wave orderings considered: (A) 12d first → combined 12c+12e → 12f, (B) 12c + 12d + 12e in parallel with merge resolution → 12f.

**Decision:** Option A — Foundation-first.

**Rationale:**
- 12d delivers `require_network(feature)` helper + settings persistence — 12c and 12e then consume the helper directly instead of TODO-commenting it
- 12c and 12e both touch `src-tauri/src/github/` module + commands/mod.rs + lib.rs handler list — combining them in one Backend Architect pass avoids two agents fighting over the same files
- Parallel option B saved one wave but required hand-merging mod.rs/lib.rs/tauri.conf.json conflicts — net wall time similar
- A is cleaner; each step starts from a known-good staged state

**Outcome:** delivered cleanly. 274 → 334 tests across Wave 2; zero merge conflicts.

---

## 2026-05-24-night: Combined GitHub backend (12c + 12e in one Backend Architect)

**Context:** Original plan split GitHub work into 12c (anonymous repo stats) and 12e (Device Flow + Keychain). Both touch `src-tauri/src/github/` module. Separate agents would conflict.

**Decision:** One Backend Architect pass implements the entire GitHub module end-to-end: parse_github_url validator (12c) + RepoStats fetch + cache (12c) + Token newtype + Device Flow + Keychain (12e) + 5 IPC commands.

**Rationale:** github module is a single coherent unit. Splitting it forces shared-file coordination. One agent owns it cleanly.

**Constraint observed:** the combined agent ships placeholder `GITHUB_OAUTH_CLIENT_ID` const. Real client_id must be added before any release (documented in BUILD.md, 7-step OAuth App creation guide). 12c functionality works without a client_id — anonymous tier is independent. Only 12e sign-in needs the real client_id.
