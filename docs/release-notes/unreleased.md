## brew-browser / Brew Browser native — next release (staging)

Staged for the next signed + notarized release. Tauri remains the macOS 13+ /
Linux build; native remains the macOS 26 SwiftUI build. Version numbers are
assigned when the release is cut.

> **Staging file.** Add notes here as changes land; rename to
> `docs/release-notes/<version>.md` when the next version is cut.

## Fixes

### Vulnerability scanning reported "no known vulnerabilities" when there were some

The security fix in this release, and a silent one — the broken state looked
exactly like a genuinely clean machine.

0.7.2 taught both shells to *find* Homebrew 6.0+'s built-in `brew vulns`, but
not to read it. The old standalone `brew-vulns` formula printed a bare JSON
array of scan records. The built-in command wraps them in an envelope instead:

```json
{ "findings": [ … ], "skipped_formulae": [ … ] }
```

Our parser tried the array, then fell back to "maybe it's a single record". That
fallback matched the envelope **vacuously** — every field of a scan record is
optional, so an object with none of them decoded into one blank record carrying
zero vulnerabilities. Parsing "succeeded", nothing errored, and the Exposure
card reported all-clear. On the machine where this was found, `brew vulns` was
reporting 20 findings across `libheif`, `boost`, `cairo`, `openjpeg`,
`tesseract` and others while the app showed a green tick.

Both shells now read the envelope, and the single-record fallback requires a
real formula name before it will trust a payload — so an unrecognised shape
surfaces as a parse error instead of a clean bill of health.

**Your cached result is cleared automatically.** A scan performed by the broken
version was written to disk under an install fingerprint that still matches, so
without intervention the stale "clean" verdict would have been served for up to
six hours after updating. The cache schema version is bumped, which discards
those files on first launch and forces one fresh scan.

If you run brew 6.0 or newer, **re-check the Exposure card after updating.**
