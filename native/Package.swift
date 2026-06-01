// swift-tools-version:6.2
import PackageDescription

// brew-browser native — SwiftUI + Liquid Glass (macOS 26 Tahoe).
//
// Apple's intended multi-module shape: a thin `@main` executable
// (`BrewBrowser`) over a `BrewBrowserKit` library that holds all views,
// models, and services. The library layout is what makes SwiftUI `#Preview`
// (and Xcode's RenderPreview) work — previewing an *executable* target
// requires ENABLE_DEBUG_DYLIB, which is only settable in an .xcodeproj; a
// library target previews without it.
//
// Still 100% SwiftPM (no .xcodeproj). Built with `swift build`; the produced
// `BrewBrowser` executable is wrapped into a launchable .app by build-app.sh.
let package = Package(
    name: "BrewBrowser",
    platforms: [.macOS(.v26)],
    products: [
        // Exposing the library as a product makes SwiftPM/Xcode generate a
        // dedicated `BrewBrowserKit` scheme. SwiftUI previews (RenderPreview)
        // build against THAT library scheme — previewing via the executable
        // scheme fails with DebugDylibNotEnabled (.xcodeproj-only setting).
        .library(name: "BrewBrowserKit", targets: ["BrewBrowserKit"])
    ],
    targets: [
        // Thin executable: just the @main App entry, importing BrewBrowserKit.
        .executableTarget(
            name: "BrewBrowser",
            dependencies: ["BrewBrowserKit"],
            path: "Sources/BrewBrowser"
        ),
        // Library: all views, AppModel, and the brew/vulns/github/trending/
        // enrichment services. Bundled JSON resources live here alongside the
        // `Bundle.module` readers (Categories.swift, Enrichment.swift).
        .target(
            name: "BrewBrowserKit",
            path: "Sources/BrewBrowserKit",
            resources: [
                .copy("Resources/categories.json"),
                .copy("Resources/enrichment.json"),
                // App icon, loaded at runtime via Bundle.module to set the Dock
                // icon. Works even for the bare `swift build` / Xcode ⌘R binary
                // (which has no .app bundle Info.plist icon).
                .copy("Resources/AppIcon.icns"),
                // Full Homebrew catalog (gzipped) for the Discover panel —
                // 8.3k formulae + 7.7k casks. Same data the Tauri app bundles
                // (src-tauri/data/catalog); decompressed + parsed at runtime by
                // CatalogService. ~6MB gzipped / ~44MB raw, so kept compressed.
                .copy("Resources/catalog")
            ]
        )
    ]
)
