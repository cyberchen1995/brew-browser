# 15 — Brew Doctor / Cleanup + cache maintenance (PLAN)

**Status:** PLAN (scoped, not yet built) · **Issue:** [#80](https://github.com/msitarzewski/brew-browser/issues/80) (`modeezie`)
**Target:** 0.x.1 batch · **Shells:** Tauri + native, in parity (per parity charter, `decisions.md` 2026-06-01)

## Objective

From #80, three asks:
1. A **visual of the cache footprint** (download cache + other Homebrew caches).
2. A **`brew doctor`** button — run diagnostics, stream output.
3. A **`brew cleanup --prune=all --scrub`** button — reclaim cache space, stream output. `--verbose` optional.

Reporter explicitly values the integrated terminal view (our Activity drawer) showing "what all is being done." Two **separate** buttons preferred.

## Analyzed (what already exists — heavy reuse)

- **Streaming brew actions** — `src-tauri/src/commands/actions.rs:165` (`brew_update`, a no-package streaming command) is the exact template. All actions funnel through `run_brew_streaming(&path, args, display, on_event, jobs)` at `src-tauri/src/brew/exec.rs:100`, which drives the `BrewStreamEvent` lifecycle (stdout/stderr/progress/exit) to a `Channel`. Commands registered in `src-tauri/src/lib.rs:130` (`generate_handler!`).
- **Cache is ALREADY measured + labeled** — `src-tauri/src/commands/disk_usage.rs:177` shells `brew --cache`, sizes it via `du_bytes` (`disk_usage.rs:81`), and emits a `DiskUsageEntry { label: "Download cache", … }` (`disk_usage.rs:~218`) inside `DiskUsageReport.entries` → already rendered in the **Storage card** (`src/lib/components/Dashboard.svelte:852`). So "see the cache" is largely shipped; the gap is *reclaimable* space + the action buttons.
- **Non-zero-but-informational classification** — `friendlify` (`error_patterns.rs:31`) + `upgrade_warnings_only` (`error_patterns.rs:202`) already model "brew exited non-zero but it's not a real failure." `brew doctor` is the same shape (exits 1 when it finds advisories) and must reuse this pattern, or it throws a scary error toast for a normal run.
- **Activity surfacing** — the `Exit` event already carries `friendlyMessage` (Stream A, this month); the Activity drawer (`ActivityDrawer.svelte`) and store (`activity.svelte.ts`) render streamed jobs. No new plumbing for the terminal view.
- **Native equivalents** — `BrewService.runStreaming(jobId:_:)` (`native/Sources/BrewBrowserKit/BrewService.swift:215`); `StorageCard` (`native/Sources/BrewBrowserKit/DashboardView.swift:502`); `AppModel` action triggers + Activity.

## Reuse strategy

- **Extend** `actions.rs` — add `brew_doctor` and `brew_cleanup` commands, near-copies of `brew_update:165` (no package arg). Register both in `lib.rs:130`.
- **Extend** `disk_usage.rs` — add a `reclaimable_bytes: Option<u64>` to `DiskUsageReport` (or a sibling command `brew_cleanup_preview`) sourced from `brew cleanup -n --prune=all` output, so the UI can show "frees ~X". Do NOT re-measure; reuse the existing report.
- **Extend** the Storage card (both shells) — it already shows the Download cache size and is the disk surface. Add a **Doctor** button and a **Cleanup** button there. No new top-level view.
- **Reuse** `friendlify`/`upgrade_warnings_only` for doctor's non-zero exit (treat as informational; no error toast).
- Cannot reuse a single command for both: doctor and cleanup are distinct actions with different exit semantics and different confirm requirements — two commands, as the reporter requested.

## Proposed design

- **Home:** the Dashboard **Storage card**. It already lists Cellar / Caskroom / Logs / Download cache. Add a footer action row: `[Run brew doctor]` `[Clean up cache…]`, plus a "~X reclaimable" hint next to the Download cache row.
- **Doctor:** one click → streams `brew doctor` in the Activity drawer. Non-zero exit = "Doctor found N advisories" (informational, not a failure). Read-only; no confirm needed.
- **Cleanup:** click → a confirm step that shows what it does and the reclaimable estimate ("Frees ~1.2 GB. Removes cached downloads including current versions (`--scrub`). Installed packages are not affected."), then streams `brew cleanup --prune=all --scrub` (append `--verbose` from a toggle, default on per the reporter). On success, invalidate the disk-usage cache so the Storage card refreshes the now-smaller cache size.
- **Verbose toggle:** small checkbox in the confirm dialog, default on (reporter likes the detail).

## Steps

1. **Tauri backend** — `brew_doctor` + `brew_cleanup` in `actions.rs` (mirror `brew_update:165`); register in `lib.rs:130`. Cleanup arg vector: `["cleanup", "--prune=all", "--scrub"]` (+ `"--verbose"` when requested). Doctor: `["doctor"]`.
2. **Doctor exit semantics** — route doctor's non-zero through the `upgrade_warnings_only`-style path so the `Exit` event reports `success: true`-with-notice rather than `BrewExitNonZero`. Add a `doctor`-specific branch/predicate in `error_patterns.rs` (keep the catalog small per its own guidance).
3. **Reclaimable preview** — `brew_cleanup_preview` command running `brew cleanup -n --prune=all`, parse the trailing "This operation would free approximately X" line → `u64` bytes. Surface on the Storage card.
4. **Tauri frontend** — `api.ts` wrappers (`brewDoctor`, `brewCleanup`, `brewCleanupPreview`); Storage-card action row + confirm dialog (reuse existing modal/confirm pattern); invalidate disk cache on cleanup success (`diskUsageClearCache` already exists, `api.ts`).
5. **Native parity** — `BrewService` doctor/cleanup via `runStreaming:215`; `AppModel` triggers + Activity; `StorageCard:502` buttons + confirm sheet; same reclaimable hint. Mirror the doctor non-zero handling in the Swift classifier.
6. **Tests** — Rust: arg-vector construction, cleanup-preview byte parsing (incl. "nothing to clean" → None), doctor-exit classification (non-zero → informational). Native: same predicates (`BrewOutputParsing`/classifier tests). Frontend: confirm-gate + verbose-flag wiring.

## Data contract (new)

- `brew_doctor(on_event) -> JobResult` ; `brew_cleanup(verbose: bool, on_event) -> JobResult` — streaming, mirror `brew_update`.
- `brew_cleanup_preview() -> { reclaimable_bytes: u64 | null }` (or fold into `DiskUsageReport`).
- No change to the `BrewStreamEvent` shape (reuses `friendlyMessage`).

## Safety considerations

- **`cleanup --prune=all --scrub` is destructive-ish** — deletes cached downloads incl. current versions. It does NOT touch installed packages, but must be **confirm-gated** with a plain-English description + the reclaimable figure. Never one-click.
- **`brew doctor` is read-only** — safe, no confirm. Its non-zero exit must NOT surface as a failure.
- No new subprocess shapes beyond `brew` itself; arg vectors are fixed constants (no user interpolation) → no injection surface.

## Risks & mitigations

- *Doctor noise misread as failure* → reuse the warnings-only classification (step 2); explicit test.
- *Cleanup preview format drift across brew versions* → parse defensively, fall back to `reclaimable: None` (hide the hint) rather than error.
- *Long cleanup blocks UI* → it streams like any other job; cancellable via existing `cancel_job` (`actions.rs:186`).

## Out of scope / open questions

- Per-cache-type breakdown beyond what `brew --cache` covers (e.g. separating download cache from API cache) — defer unless requested.
- A standalone "Maintenance" view — not needed; the Storage card is the right home. Revisit only if the action set grows.
- Q for reporter (optional): default `--scrub` on, or offer a lighter `cleanup --prune=all` without scrub as the default and scrub as an opt-in? (Scrub is more aggressive.)
