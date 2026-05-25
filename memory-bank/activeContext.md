# Active Context

**Date:** 2026-05-25 (v0.3.0 prep, post-compact follow-up session)
**State:** Phase 15 fix-up complete — all 5 CRITICAL findings resolved + 1 IMPORTANT (cached_available clear). v0.3.0 is now blocker-clean. Remaining v0.3.0 work is release plumbing (minisign keypair, version bump, build, ship).

## Repo

- **github.com/msitarzewski/brew-browser** — public, MIT
- **Released:** v0.1.0, v0.2.0, v0.2.1 (live on GitHub Releases — `gh release list`)
- **Working toward:** v0.3.0 (single coherent release, NOT splitting a v0.2.2)
- **Open issue:** [#1 — Post-GitHub-Auth Issues](https://github.com/msitarzewski/brew-browser/issues/1) (reported by @heyjawrsh, recurring; ROOT CAUSED + FIXED this session, ship in v0.3.0)
- **Stars:** 9 (as of last check)
- **HN post:** [item 48260242](https://news.ycombinator.com/item?id=48260242) (buried at 1 point)
- **LinkedIn announcement:** alive — 1,496 impressions / 914 reached / 28 reactions / 10 comments / 5 followers in first 10 hours

## What landed this session (uncommitted, ~30 files)

### Phase 15 — In-app updater + Offline Mode UI rename (FIX-UP DONE)

4 parallel agents (Backend Architect + Frontend Developer #1 + Frontend Developer #2 for rename sweep + Technical Writer) plus a Lead-written bridging `update_skip` IPC. **+34 backend tests** (411 → 445). All `npm run check` + `cargo check` + `cargo clippy -D warnings` clean.

2 review agents (Code Reviewer + Security Engineer) returned **NEEDS-WORK with 5 CRITICAL findings**; all 5 resolved in this session's fix-up pass (task #16, **+3 backend tests**, 447 → 450). Full detail in `tasks/2026-05/16-phase-15-fixup-pass.md`:

1. ✅ IPC wire-shape mismatch — frontend `UpdateCheckOutcome` flattened to match backend's flat `{kind, version, currentVersion, notes, pubDate, skipped}`; `blocked` variant removed (Offline Mode surfaces as `ParanoidModeBlocked` error). `UpdateInfo` repurposed as the store's internal shape (no more invented `notesUrl`/`sha256`).
2. ✅ Relaunch button — new `update_relaunch` IPC that spawns a 50ms-delayed `app.restart()`; frontend `updater.relaunch()` wires the button. `run_install` now clears `cached_available` post-success.
3. ✅ Manifest artifact format — `publish-manifest.sh` operates on `bundle/macos/brew-browser.app.tar.gz`, signs and hashes that, emits a URL pointing at the GH release asset under `brew-browser_<v>_aarch64.app.tar.gz`. `BUILD.md` rewritten to cover the two-artifact release (`.dmg` for fresh installs + `.app.tar.gz` for auto-updater).
4. ✅ Missing error variants — `hash_mismatch`, `signature_verification_failed`, `downgrade_rejected` added to `BrewErrorPayload` and `brewErrorMessage`.
5. ✅ `update_skip` Corrupt-settings safety — refactored into `run_skip` inner; Corrupt branch refuses with typed error; `FirstLaunch` materializes defaults with the skip (correct first-write behavior). +2 backend tests pin the new behavior.

**Outstanding IMPORTANT findings** (not blockers for v0.3.0; tracked in task #16's "Outstanding" section): the central `paranoid_mode_blocked` toast still reads "Paranoid mode is on" (wording-only sweep), manifest size caps + allowlist not enforceable through plugin 2.10.1, placeholder pubkey has no startup guard, scheduler `last_checked_at` is in-memory only.

### Issue #1 fixes (4 cascading bugs)

Spent ~6 hours debugging the toast cascade from issue #1. The chase:

1. **Cache loop fix** (task #14 + `tasks/2026-05/14-issue-1-hunt-cache-loop.md`) — `PackageDetail`'s `isStarred` effect overloaded `"unknown"` as both the cache-miss sentinel AND the fetch-failure value, causing infinite IPC storms when failures happened. Distinct `"error"` variant ended the loop.
2. **Toast architectural fix** (task #15 + `tasks/2026-05/15-github-integration-completion.md`) — even after the cache loop was gone, users could hit `effect_update_depth_exceeded` from the toast `$effect` itself. **Moved `toast.success` out of `$effect` and into the imperative call site in `signIn()` poll loop.** This is the officially-recommended Svelte 5 pattern: `$effect` is "an escape hatch" not "a side-effect channel."
3. **Scope parser fix** — GitHub returns the OAuth `scope` field comma-separated (`"public_repo,read:user"`), not space-separated per RFC 6749. Our `split_whitespace()` parser produced a one-element array; the action gate's exact-match check failed. Now splits on both commas AND whitespace.
4. **Watch scope** — GitHub's `PUT /repos/{o}/{r}/subscription` requires the `notifications` scope, NOT `public_repo`. Returns 404 (privacy-mask for "you don't have it") when missing. Added `notifications` to `GITHUB_OAUTH_SCOPES`.

### GitHub integration polish (task #15)

- **Per-action scope gate** — `authed_gate(required_scope)` parameterized. Each command passes its scope (`star/issue → public_repo`, `watch/unwatch → notifications`). Pre-empts the GitHub-returns-404 dance. **+2 new tests** (445 → 447) pin the per-action behavior.
- **Actionable re-auth toast** — `Toast.action: { label, onClick }`. When an authed action fails with `ScopeRequired`, the toast offers a "Re-authorize" button that calls `signIn()`. GitHub's consent screen shows only the missing scope. New token replaces old in Keychain transparently. **No sign-out needed.**
- **Octocat status chip** in title-bar — real Octocat from Primer/Octicons (MIT-licensed; Lucide strips brand icons). Green = signed-in with required scope, amber = signed-in but scope-incomplete, hidden = signed-out. Click → opens Settings → GitHub.
- **Eager `loadStatus()` in `TitlebarControls.onMount`** — necessary for the chip to know its state on first paint. ⚠️ Re-introduces a Keychain ACL prompt on every dev-binary rebuild. **v0.3.0+ follow-up:** gate on a `localStorage["brew-browser:has-signed-in"]` flag so users who never sign in see zero Keychain prompts.

## Tests & lint (current)

- `cargo test`: **450 passed**, 0 failed, 6 ignored (447 → 450, +3 new for Phase 15 fix-up: `skip_refuses_on_corrupt_settings`, `skip_rejects_empty_version`, `install_clears_cached_available_on_success`)
- `cargo clippy --all-targets -- -D warnings`: clean
- `cargo check`: clean
- `npm run check`: 0 errors, 3 pre-existing warnings (SettingsSectionGitHub unused-CSS, tsconfig-node-types)
- `npm run build`: clean
- `bash -n tools/release/publish-manifest.sh`: clean
- All diagnostic instrumentation reverted (no `[diag]` / `console.trace` left in code)

## Working tree (~30 files)

**New (this session):**
- `src/lib/components/GithubMarkIcon.svelte` (Octocat from Primer)

**Modified (Phase 15):**
- `src-tauri/{Cargo.toml,Cargo.lock,tauri.conf.json,capabilities/default.json,src/lib.rs,src/error.rs,src/state.rs,src/commands/mod.rs,src/commands/updater.rs,src/commands/settings.rs}`
- `src/lib/{api.ts,types.ts,stores/updater.svelte.ts (new)}`
- `src/lib/components/{UpdateIndicator.svelte (new),SettingsSectionUpdates.svelte (new),SettingsSectionNetwork.svelte,SettingsSectionGitHub.svelte}`
- `src/routes/+page.svelte`
- `BUILD.md`, `memory-bank/security.md`
- New: `.gitleaks.toml`, `tools/release/publish-manifest.sh`, `memory-bank/phase15-plan.md`

**Modified (GitHub-integration session):**
- `src-tauri/src/github/auth.rs` (scope parser fix + `notifications` scope)
- `src-tauri/src/commands/github.rs` (per-action gate)
- `src/lib/stores/{github.svelte.ts,toast.svelte.ts}`
- `src/lib/components/{DeviceFlowModal.svelte,Toast.svelte,PackageDetail.svelte,TitlebarControls.svelte}`
- `src/lib/components/PackageDetail.svelte` (cache-loop fix + showActionFailureToast)

**Memory bank:**
- New task records: `tasks/2026-05/{14-issue-1-hunt-cache-loop.md, 15-github-integration-completion.md}`
- Updated: `tasks/2026-05/README.md`, `activeContext.md` (this file), `progress.md` (next), `NEXT-SESSION.md` (next), `toc.md` already updated last session
- Moved (rename sweep last session, also in this commit): `memory-bank/{phase12-plan.md,phase13-plan.md} → memory-bank/phases/`; `memory-bank/scans/* → memory-bank/scans/2026-05-23/`

**Untracked:**
- `AGENTS.md`, `CLAUDE.md` (symlink) — intentional, your AI-workflow guide
- `landing/screenshots/` (Phase 15 deploy artifacts)

## Memory bank inventory

`toc.md`, `projectbrief.md`, `techContext.md`, `decisions.md`, `activeContext.md` (this), `progress.md`, `systemPatterns.md`, `designSystem.md`, `uxArchitecture.md`, `visualStory.md`, `backendApi.md`, `frontendComponents.md`, `codeReview.md`, `apiTests.md`, `accessibility.md`, `realityCheck.md`, `security.md`, `ideas.md`, `phase15-plan.md` (in-flight at top; 12+13 moved to `phases/`), `agentLog.md` (dormant), `NEXT-SESSION.md`, `tasks/2026-05/` (15 task records + README + deferred), `phases/`, `scans/2026-05-23/`.
