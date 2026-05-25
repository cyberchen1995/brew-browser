# 2026-05-25 — Phase 15 fix-up pass: 5 CRITICAL findings resolved

**Phase:** 15 fix-up (post-review NEEDS-WORK)
**Status:** ✅ Complete (uncommitted, ships in v0.3.0)
**Date:** 2026-05-25 (post-compact, second session)

## Scope

Resolve the 5 CRITICAL findings from the Code Reviewer + Security Engineer wave-2 review (see task #13). Plus the IMPORTANT finding "`cached_available` never cleared after successful install" because it compounded CRITICAL-2.

## Findings + fixes

### CRITICAL #1 — IPC wire-shape mismatch on `UpdateCheckOutcome::Available`

**Problem:** backend serializes the `Available` variant flattened next to the `kind` tag (`#[serde(tag = "kind", rename_all_fields = "camelCase")]` on the enum) → `{ kind, version, currentVersion, notes, pubDate, skipped }`. Frontend declared `{ kind: "available"; info: UpdateInfo }` with a nested object and read `outcome.info.{version, notesUrl, sha256}`. Every field came back `undefined`. The indicator pill rendered "Update available: brew-browser undefined".

**Fix:** flatten the frontend type to match the backend's serde shape exactly. Drop the `notesUrl` + `sha256` invented sub-fields — they were never on the wire. `UpdateInfo` repurposed as the store-internal type (lifted from the flat outcome by the store), with fields `{ version, currentVersion, notes, pubDate, skipped }`. The `blocked` variant on the union dropped — Offline Mode surfaces as `BrewError::ParanoidModeBlocked`, not as a third outcome variant.

**Files:**
- `src/lib/types.ts` — `UpdateCheckOutcome` flattened, `UpdateInfo` repurposed, `blocked` variant removed.
- `src/lib/stores/updater.svelte.ts` — switch arm reads flat fields, lifts into `UpdateInfo`; dead `blocked` case removed.

### CRITICAL #2 — "Relaunch now" re-runs `update_install`

**Problem:** the post-install Relaunch button in `SettingsSectionUpdates.svelte` was wired to `onInstall` (which calls `updater.install(version)`). With no `app.restart()` IPC anywhere in the codebase, clicking the button re-ran the entire download + verify + install pipeline. Infinite re-install loop.

**Fix:** new `update_relaunch` IPC command. The command spawns a 50ms-delayed async task that calls `tauri::AppHandle::restart()` — the delay lets the IPC JSON response make it back to the renderer before the process dies. Frontend `updater.relaunch()` method wraps the IPC; treats any error as benign (the socket tears down mid-call when the restart actually fires). Button wired to `onRelaunch()`.

**Files:**
- `src-tauri/src/commands/updater.rs` — new `update_relaunch` command (cfg-gated for tests).
- `src-tauri/src/lib.rs` — registered in `generate_handler![]`.
- `src/lib/api.ts` — `updateRelaunch()` wrapper.
- `src/lib/stores/updater.svelte.ts` — `relaunch()` method.
- `src/lib/components/SettingsSectionUpdates.svelte` — Relaunch button onclick → `onRelaunch`.

**Bundled bonus (IMPORTANT finding):** `run_install` now clears `cached_available` + sets `last_outcome = UpToDate` after a successful install, so the indicator + Settings card don't re-offer the same install. +1 backend test pinning this behavior.

### CRITICAL #3 — Manifest artifact format `.dmg` → `.app.tar.gz`

**Problem:** the Tauri updater plugin's macOS install path expects a gzipped tar of the `.app` bundle (`.app.tar.gz`). Our `publish-manifest.sh` operated on `brew-browser_<version>_aarch64.dmg`, hashed and signed the `.dmg`, and emitted a URL pointing at the `.dmg` GH release asset. Every install attempt fails with `"invalid gzip"`. The plugin doesn't accept `.dmg` files; the `.dmg` is for fresh installs only.

**Fix:** script now operates on `src-tauri/target/release/bundle/macos/brew-browser.app.tar.gz`, signs and hashes that, and emits a URL pointing at the GH release asset named `brew-browser_<version>_aarch64.app.tar.gz`. `BUILD.md` rewritten to:
- Explain the two-artifact release (`.dmg` for fresh installs + `.app.tar.gz` for auto-updater)
- Show `gh release create` with the `#newname` syntax to upload `brew-browser.app.tar.gz` under the versioned name
- Manifest format example updated to point at `.app.tar.gz`
- "Two separate signing concerns" table clarified — Apple signs the `.dmg`, minisign signs the `.app.tar.gz`; they protect different install paths

**Files:**
- `tools/release/publish-manifest.sh` — `DMG_PATH` → `ARTIFACT_PATH`, manifest URL → `.app.tar.gz`, header docs updated, error messages updated.
- `BUILD.md` — per-release flow rewritten to cover both artifacts; manifest-format JSON updated; signing-concerns table clarified.

### CRITICAL #4 — Missing error variants in frontend `BrewErrorPayload` union

**Problem:** backend's `BrewError::HashMismatch`, `SignatureVerificationFailed`, and `DowngradeRejected` had serde tests pinning the wire shape, but the frontend `BrewErrorPayload` union didn't declare them. `isBrewError(e)` returned true (the `code` discriminator exists), but `brewErrorMessage(e)` fell through the closed switch and returned `undefined`. Security-relevant errors silently suppressed.

**Fix:** added all three variants to `BrewErrorPayload` and corresponding cases to `brewErrorMessage`. Hash mismatch shows the first 12 hex chars of each digest for compactness. Signature failure surfaces the raw plugin message. Downgrade rejected reads as "Update refused: X is not newer than the installed version (Y)" — friendly for a defense-in-depth check that ordinary users should never see.

**Files:**
- `src/lib/types.ts` — `BrewErrorPayload` extended (3 new variants), `brewErrorMessage` extended (3 new arms).

### CRITICAL #5 — `update_skip` silently revokes paranoid mode on Corrupt settings

**Problem:** the bridging `update_skip` command (added by Lead during Phase 15 integration) wrote `Settings::default()` (paranoid_mode = false) to disk when settings were Corrupt. Dismissing an update indicator with × would silently disable the network kill switch. Security Engineer caught this.

**Fix:** `update_skip` now differentiates the three `SettingsLoadState` variants:
- `Loaded(s)` → clone, push skip, persist (unchanged).
- `FirstLaunch` → write defaults with the skip recorded (no settings file existed, so defaults match the user's lack of preference).
- `Corrupt { message }` → **refuse**, return `BrewError::Internal { message: "cannot record update skip while settings file is unreadable; reset settings from Settings → Network first" }`. The frontend's optimistic `available = null` still hides the indicator for the session; the user must hit "Reset to defaults" before a persisted skip is possible.

Refactored into `run_skip(state, version)` inner so the Corrupt-refusal branch is unit-testable without a Tauri State wrapper. The IPC command delegates.

**Files:**
- `src-tauri/src/commands/updater.rs` — `run_skip` extracted, Corrupt branch refuses with typed error; `update_skip` IPC delegates. +2 backend tests (`skip_refuses_on_corrupt_settings`, `skip_rejects_empty_version`).

## Tests / verification

- `cargo test`: **450 passed**, 0 failed, 6 ignored (447 → 450, +3 new: `skip_refuses_on_corrupt_settings`, `skip_rejects_empty_version`, `install_clears_cached_available_on_success`)
- `cargo clippy --all-targets -- -D warnings`: clean
- `cargo check`: clean
- `npm run check`: 0 errors, 3 pre-existing warnings (unchanged)
- `npm run build`: clean
- `bash -n tools/release/publish-manifest.sh`: clean

## Outstanding (deferred to v0.3.0 release work, NOT v0.3.0+ follow-up)

These were flagged IMPORTANT but are NOT in the CRITICAL set and were not in-scope for this pass:

- Generic `brewErrorMessage` for `paranoid_mode_blocked` still says "Paranoid mode is on" — rename sweep didn't touch the central default. Trivial wording-only change for v0.3.0.
- Manifest URL allowlist documented in `security.md` §15 but NOT implemented at the plugin layer. `tauri-plugin-updater 2.10.1` doesn't expose pre-fetch hooks; documenting the gap rather than promising defense we can't deliver.
- 8 KiB manifest cap / 200 MB artifact cap / per-hop redirect re-validation called for in the plan but not enforceable through the current plugin version.
- Placeholder pubkey duplicated in `lib.rs` + `tauri.conf.json` with no startup guard. Adding a runtime check (`UPDATER_PUBKEY.contains("PLACEHOLDER")` → panic in release) is a 5-line v0.3.0 cleanup.
- `update_skip` snapshot-then-write race window — only matters under simultaneous skip clicks, which the UI debounces anyway.
- Auto-check scheduler timestamp is in-memory only. For typical "open in morning, close at night" usage, the auto-check effectively never fires across launches. Persisting `last_checked_at` to disk is a v0.3.x ergonomics improvement, not a v0.3.0 blocker.

## Files (full inventory)

**Backend:**
- `src-tauri/src/commands/updater.rs` — `run_skip` extracted; Corrupt branch refuses; `update_relaunch` IPC added; `run_install` clears cache on success; +3 tests.
- `src-tauri/src/lib.rs` — `update_relaunch` registered in `generate_handler![]`.

**Frontend:**
- `src/lib/types.ts` — `UpdateInfo` + `UpdateCheckOutcome` flattened; 3 BrewError variants + messages added.
- `src/lib/api.ts` — `updateRelaunch()` wrapper added.
- `src/lib/stores/updater.svelte.ts` — switch arm rewritten for flat outcome; `relaunch()` method added.
- `src/lib/components/SettingsSectionUpdates.svelte` — Relaunch button onclick → `onRelaunch`; `notesUrl` removed (release-notes URL now derived from version); `sha256` block removed (not on the wire); notes body rendered inline.

**Release pipeline:**
- `tools/release/publish-manifest.sh` — `.dmg` → `.app.tar.gz` throughout.
- `BUILD.md` — per-release flow rewritten; manifest format updated; signing-concerns table clarified.

## Notes

- The CRITICAL fixes are 100% integration-seam work — the Phase 15 architecture was sound, the bugs were all wire-shape / button-wiring / format-mismatch / Corrupt-edge-case oversights. Six fixes (5 CRITICAL + 1 IMPORTANT-as-bonus) in ~2 hours.
- The biggest cleanup-cost surprise was BUILD.md — the manifest documentation referenced `.dmg` everywhere as if that were the auto-updater artifact. Now it explicitly says: `.dmg` is fresh-install, `.app.tar.gz` is auto-updater, they're both built by one `npm run tauri build`, both go up to GH Releases.
- `update_relaunch` uses a 50ms delay before `app.restart()`. The Tauri community pattern for this is to fire-and-forget the restart from a spawn so the IPC response makes it back; 50ms is empirically reliable. If a slower machine ever sees the renderer die before the response arrives, the UX is "button click did nothing visible, app restarted" — acceptable.
- The Corrupt-refusal toast surface relies on the frontend already catching the typed error. Verified: `updater.svelte.ts skip()` catch block stores the message on `this.error`; the indicator's optimistic clear still hides the pill. The user sees the error only if they then open Settings → Updates, which is the right surface for "settings are corrupt, reset them" guidance anyway.
- v0.3.0 is now blocker-clean. Remaining release work: generate real minisign keypair, replace placeholder pubkey in `lib.rs` + `tauri.conf.json`, version-bump to 0.3.0, build + sign + notarize, `gh release create` with both `.dmg` + `.app.tar.gz`, run `publish-manifest.sh 0.3.0` + rsync, comment on issue #1 + close.
