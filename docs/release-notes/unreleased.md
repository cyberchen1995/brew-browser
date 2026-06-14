## brew-browser / Brew Browser native — next release (staging)

Staged for the next signed + notarized release. Tauri remains the macOS 13+ /
Linux build; native remains the macOS 26 SwiftUI build. Version numbers are
assigned when the release is cut.

> **Staging file.** Add notes here as changes land; rename to
> `docs/release-notes/<version>.md` when the next version is cut.

### What's new

- **Cache maintenance on the Dashboard.** The Storage card gains two actions,
  both streamed into the Activity drawer:
  - **Run brew doctor** — runs `brew doctor` and shows the diagnostics live
    (advisories are surfaced as info, not a failure).
  - **Clean up cache…** — `brew cleanup --prune=all` to reclaim cached
    downloads, with a "frees ~X" estimate and a confirm step. `--scrub` (also
    clears the current versions' downloads) is an opt-in toggle, off by default.
  - Both shells, in parity (#80).

### Acknowledgments

- **@modeezie** — the Brew Doctor / Cleanup cache-maintenance request, with a
  nicely scoped "show me what it's doing" ask and the safer non-scrub default
  suggestion (#80).
