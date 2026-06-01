import SwiftUI
import BrewBrowserKit

@main
struct BrewBrowserApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .windowStyle(.automatic)
        // Native macOS toolbar style — the unified title bar that hosts the
        // Liquid Glass toolbar buttons.
        .windowToolbarStyle(.unified)
        .defaultSize(width: 1100, height: 720)
        // Keep a coherent minimum window size (sidebar + main-pane min +
        // inspector min) while staying freely resizable. Without this, dragging
        // the inspector near the window edge can get grabbed as a window resize.
        .windowResizability(.contentMinSize)

        // Native Settings scene — opened by ⌘, the app menu, or the toolbar
        // gear (SettingsLink in ContentView).
        Settings {
            SettingsView()
        }
    }
}
