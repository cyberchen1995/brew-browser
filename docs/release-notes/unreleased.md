## brew-browser (unreleased) — Window state persistence

Signed + notarized. macOS 13+, Apple Silicon. Auto-updates via the in-app updater.

> **Staging file.** Rename to `docs/release-notes/<version>.md` when the next version is cut.

### What's new

**The window remembers its size and position.** Resize or move the window, quit, and relaunch — it reopens exactly where and how you left it. Powered by `tauri-plugin-window-state`, which saves geometry on move/resize and on exit, then restores it on the next launch. The previous `1100×720` default is now used only on a true first launch. (#17, #19)

### Acknowledgments

- @bytepl (Maciej Chojnacki) for reporting the window-state issue (#17) with a clean, reproducible repro.
