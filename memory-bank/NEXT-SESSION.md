# NEXT-SESSION handoff — read this first

**Date written:** 2026-05-24 (end-of-session, Phase 12 Wave 2 complete)
**Session lead:** Claude Opus 4.7 (1M context) with Michael

If you're a fresh session (or future-me after /compact), read this first, then `activeContext.md`, `progress.md`, `phase12-plan.md`, `phase13-plan.md`, `scans/phase12-security-review.md`.

---

## Current state

- **v0.1.0 released** — signed/notarized .dmg at <https://github.com/msitarzewski/brew-browser/releases/tag/v0.1.0>
- **All of Phase 9, 11, 12a, 12b, 12c, 12d, 12e shipped** this session
- **334 tests passing**, clippy clean with `-D warnings`, npm check clean, npm build clean
- **Commit cluster `b`-then-c**: Wave 1 + Wave 2 (47 files) committed in commit `{TBD-after-push}` — single big commit so the next agent waves start from a clean baseline

## What's queued

- **Phase 12f** — GitHub authed actions (star/unstar/is_starred/watch/unwatch/create_issue + "Wrong?" issue-deeplink + Dashboard personal-stats card). Backend Architect + Frontend Developer pass. AuthRequired/ScopeRequired error variants already exist; parse_github_url validator + Token retrieval helpers already in place. Spec in `phase12-plan.md` §12f.
- **Phase 13** — Catalog enrichment via Haiku. Tier A friendly names + summaries first (~$5), then Tier B use cases + similar + tags (~$15). Single "Show AI-enriched data" master toggle in Settings → Appearance. Zero runtime LLM calls — all enrichment baked at build time. Spec in `phase13-plan.md`. Can run parallel with 12f (different domains).
- **Phase 10** — Recipes. Paused. Depends on catalog (now available so unblocked).

## What was explicitly DROPPED

- **Phase 14 — bundled cask icons.** Trademark/redistribution risk for ~7,600 vendor icons. Runtime probe + paranoid gate is sufficient. See `decisions.md` for the full reasoning.

## Critical context for any release

- **`GITHUB_OAUTH_CLIENT_ID` is a placeholder** in `src-tauri/src/github/auth.rs`. Sign-in flow will fail until swapped. Procedure documented in `BUILD.md` § "GitHub OAuth App (one-time setup before release)". 7 steps, ~10 minutes on github.com/settings/apps with "Device Flow enabled" checkbox.
- Phase 12c anonymous tier works without a client_id — only sign-in needs one. Releases can ship 12c without 12e if 12e isn't reconfigured.

## Credentials / paths reference

| What | Where |
|------|-------|
| Repo on disk | `/Users/michael/Clean/brew-browser/` |
| GitHub repo | `github.com/msitarzewski/brew-browser` |
| Anthropic API key (categorize + enrich tools) | `tools/categorize/.env` (gitignored, local only) |
| Apple signing env | `~/.config/brew-browser/signing.env` (chmod 600, outside repo) |
| Signed .dmg artifact (v0.1.0) | `src-tauri/target/release/bundle/dmg/brew-browser_0.1.0_aarch64.dmg` |
| Landing page | `brew-browser.zerologic.com` (Caddy on umbp, user-managed) |
| umbp Tailnet IP | `100.98.187.7` |
| Catalog data | `src-tauri/data/catalog/{formula,cask}.json.gz` + `manifest.json` |
| Catalog refresh script | `python tools/catalog/fetch.py` — uses stdlib only |
| GitHub disk cache (runtime) | `~/Library/Application Support/brew-browser/github-cache/<owner>__<repo>.json` |
| Settings persistence (runtime) | `~/Library/Application Support/brew-browser/settings.json` (atomic write + 1 MiB cap + corrupt-fail-closed) |
| Keychain (runtime) | service `dev.openbrew.browser`, account `github_access_token` (+ `_scopes`) |

## Phase 13 setup (when ready)

1. Verify Phase 12 is fully shipped + committed (Phase 12f done)
2. Read `phase13-plan.md` — Wave structure: parallel build script + frontend store, then PackageDetail rendering, then cron deployment + docs
3. Launch agent waves per the plan
4. The categorize tool's `.env` and pattern can be reused as the model for `tools/enrich/`

## Security posture (current verdict)

**READY-FOR-SCRUTINY** maintained throughout Phase 12. The `scans/phase12-security-review.md` mandatory-before-merge checklist was wired into every agent's prompt and verified before each completion. Updates needed in `security.md` § "Phase 12 additions" — list of recommended additions is in the review doc. Technical Writer pass can land these in a follow-up.

## Open security checkpoints for next session

- Run a fresh `cargo audit` + `cargo deny check` + `npm audit --omit=dev` after the next commit cluster — verify no new advisories from the new `keyring`, `url`, `flate2` deps
- Confirm `clippy::print_*` / `clippy::dbg_macro` deny attribute is enforced in `src-tauri/src/github/{auth,stats}.rs` (sanity grep)
- Decide whether `AppState::upgrade_catalog_from_user_data` (Phase 12a startup hook) should consult `require_network` — currently it doesn't (read-only disk access, no network), but a defense-in-depth argument exists

## Note: PHILOSOPHY.md

User added `PHILOSOPHY.md` at repo root during the session (271 lines, project manifesto in the same voice as the rest of the docs). Included in the Wave 1+2 commit cluster.
