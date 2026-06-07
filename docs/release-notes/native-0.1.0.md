## Brew Browser (native) 0.1.0 — first release

The native macOS build of Brew Browser: **Swift 6 + SwiftUI + Liquid Glass**, for
**macOS 26 (Tahoe)**. Signed + notarized; updates via Sparkle.

Feature-complete parity with the Tauri 0.5.1 build — Library, Discover, Trending,
Snapshots, Services, Activity, Dashboard, opt-in vulnerability scanning, live
enrichment, ⌘K command palette, GitHub integration. Same `brew` integration, same
data, same privacy posture (opt-in network, no telemetry, no accounts), in a fully
native interface at roughly half the memory of the Tauri build.

Versioned independently from the Tauri build (currently 0.5.1) — see the README →
"Two builds." The Tauri build remains the cross-platform option (macOS 13+ and
Linux); this native build requires macOS 26.
