import SwiftUI
import AppKit
import BrewBrowserKit

@main
struct BrewBrowserApp: App {
    /// When launched as a bare SPM binary (Xcode ⌘R uses the DerivedData
    /// executable, which has no Info.plist), macOS treats the process as a
    /// background/accessory app — the window never comes forward and there's no
    /// Dock icon or menu bar. Forcing `.regular` + activating makes it a normal
    /// foreground app. Harmless when run from the real `.app` bundle (already
    /// `.regular`); this just makes Xcode Run + the debugger usable directly.
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

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

/// Promotes a bare/unbundled launch (Xcode ⌘R) to a normal foreground app.
final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        if NSApp.activationPolicy() != .regular {
            NSApp.setActivationPolicy(.regular)
        }
        NSApp.activate(ignoringOtherApps: true)
    }
}
