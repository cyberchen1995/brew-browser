# NEXT-SESSION handoff — read this first

**Date written:** 2026-05-24 (v0.2.0 release prep, just before commit + build + tag + release)
**Session lead:** Claude Opus 4.7 (1M context) with Michael

If you're a fresh session (or future-me after `/compact`), read this first, then `activeContext.md`, then `progress.md`.

---

## Current state at compact

- **v0.1.0 released** at <https://github.com/msitarzewski/brew-browser/releases/tag/v0.1.0>
- **v0.2.0 release prep** — version bumped (Cargo.toml + tauri.conf.json), memory bank updated, landing page updated, README updated. About to commit + tag + build + release.
- **Working tree contains:** ~50 modified files (icon regen + UI restructure since `e1d6a87`) plus 2 new files (`InfoButton.svelte`, `TitlebarControls.svelte`) and 1 deleted (`TopBar.svelte`). Ready to commit as v0.2.0.
- **GitHub OAuth App live** — Device Flow client_id `Ov23liJZKbvrSBuiOPkT` is in `src-tauri/src/github/auth.rs` (RFC 8628-public; safe to commit).
- **411 tests passing**, clippy `-D warnings` clean, npm check 0 errors, build clean.

## What's queued for the post-compact / post-release session

### 1. Cut the release (if not already done)

The current session may have ended before all six steps completed. Check `git log -1` — if you see `v0.2.0` in the tag list (`git tag --list v0.2.0`), the release went out. If not, finish the ordered steps:

1. `git add -A && git commit -m "release: v0.2.0 — title bar + sidebar restructure, info popovers, intercept GitHub flow"`
2. `git tag v0.2.0`
3. `git push origin main --tags`
4. `npm run tauri build` with `~/.config/brew-browser/signing.env` sourced — produces `src-tauri/target/release/bundle/dmg/brew-browser_0.2.0_aarch64.dmg`
5. `gh release create v0.2.0 --title "brew-browser v0.2.0" --notes-file <RELEASE_NOTES.md> path/to/dmg`
6. `rsync` updated landing page to umbp (`michael@umbp:Sites/brew-browser/`)

### 2. Security audit re-run

Re-run the tool battery against the new code added since `e1d6a87`:
- `cargo audit` (no new Rust deps in this batch, but worth a check)
- `cargo deny check` (advisories + bans + licenses + sources)
- `npm audit --omit=dev`
- `semgrep` with security-audit + OWASP-top-10 + Rust + TypeScript rulesets
- `gitleaks` against the full repo (especially the new commit — confirm no PAT or client-secret leakage)
- Manual review of: `InfoButton.svelte` (popover + script execution surface — uses no `@html`, no `eval`, no `innerHTML`), `TitlebarControls.svelte` (just imports + DOM clicks), `requireGithubSignIn()` helper, search type-ahead (`onSearchInput` runs sync; no template injection)
- Update `memory-bank/security.md` §13 with the post-v0.2.0 verdict

### 3. More UI polish (open scope)

Candidates carried over and freshly relevant:

- **Sticky/frozen # + NAME columns** at narrow widths in list panes (user asked about this in a previous session; still deferred)
- **Snapshots panel-head responsive treatment** — Import + New Snapshot are primary actions; at narrow widths they need icon-only labels (currently they full-hide via @media, which is wrong for primary actions)
- **Real screenshots** per `visualStory.md` 30-min shoot — README + landing page still use placeholders
- **Tier B enrichment run** (`python tools/enrich/enrich.py --tier-b`) — use_cases + similar packages + tags. ~$10-15. Would populate the use-cases/similar/tags sections of PackageDetail that currently render nothing for most packages.
- **Categorize cron** on Beast or umbp for daily delta (catalog + categorize + enrich)
- **README "brew tap" placeholder** — update once `brew tap msitarzewski/brew-browser` exists
- **OAuth-vs-deeplink-out discussion** — we kept OAuth this release (it's intent-discovered now, no static prompts). Worth a future check-in on whether OAuth carries its weight vs simpler `open https://github.com/...` deeplinks for star/watch/issue.

## Critical context for any release

- **Apple signing env** at `~/.config/brew-browser/signing.env` (chmod 600, outside repo) is valid and live — regenerate if this transcript is ever shared publicly
- **Anthropic API key** in `tools/categorize/.env` (also used by enrich via cascade lookup) is valid and live — regenerate if transcript shared
- **GitHub OAuth App client_id** (`Ov23liJZKbvrSBuiOPkT`) is public per RFC 8628 — safe to commit, included in the binary, no secret to leak
- **Both API keys are easily regenerated** (<1 min each at console.anthropic.com / appleid.apple.com)

## Credentials / paths reference

| What | Where |
|------|-------|
| Repo on disk | `/Users/michael/Clean/brew-browser/` |
| GitHub repo | `github.com/msitarzewski/brew-browser` |
| Anthropic API key (categorize + enrich) | `tools/categorize/.env` (gitignored; enrich uses it via cascade) |
| Apple signing env | `~/.config/brew-browser/signing.env` (chmod 600, outside repo) |
| Landing page source | `landing/` in this repo |
| Landing page deploy target | `michael@umbp:Sites/brew-browser/` (Caddy on umbp, user-managed) |
| umbp Tailnet IP | `100.98.187.7` |
| Catalog data | `src-tauri/data/catalog/{formula,cask}.json.gz` (~6.1 MiB) + `manifest.json` |
| Enrichment data | `src-tauri/data/enrichment.json.gz` (15,725 entries, ~0.74 MiB) |
| Catalog refresh script | `python tools/catalog/fetch.py` |
| Enrichment script | `tools/categorize/.venv/bin/python3 tools/enrich/enrich.py --tier-a` |
| Runtime caches | `~/Library/Application Support/brew-browser/{settings.json, catalog/, github-cache/, icon-cache/, brewfiles/}` |
| Keychain | service `dev.openbrew.browser`, accounts `github_access_token` + `_scopes` |
| Icon source | `docs/icon/brew-browser.svg` (full-bleed 181×181 square — Tahoe-clean) |
| Icon regen | `npm run tauri icon docs/icon/brew-browser.svg` |

## Open items not in the post-release plan

- Recipes (Phase 10) — paused; depends on catalog (now available)
- `installedAt` on Package + Last-Updated sort — small standalone backend addition
- Tier B Tahoe Liquid Glass (Swift bridge) — v0.3+
- Phase 14 bundled cask icons — **explicitly DROPPED** (trademark/redistribution risk; see `decisions.md`)
