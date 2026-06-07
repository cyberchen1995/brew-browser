import Testing
import Foundation
@testable import BrewBrowserKit

// Forward-compatibility tests for the settings on-disk contract. A settings
// file written by an OLDER app (missing the newest keys) must decode into the
// current defaults — never wipe a user's config on upgrade. Mirrors the Rust
// `#[serde(default)]` per-field defaults in `commands/settings.rs` and the
// state-machine forward-compat tests.

@Suite("SettingsDTO forward-compat")
struct SettingsContractTests {
    private func decode(_ json: String) throws -> SettingsDTO {
        try JSONDecoder().decode(SettingsDTO.self, from: Data(json.utf8))
    }

    @Test func emptyObjectDecodesToDefaults() throws {
        let d = SettingsDTO.defaults()
        let got = try decode("{}")
        // Security-relevant defaults must hold when keys are absent.
        #expect(got.paranoidMode == d.paranoidMode)
        #expect(got.githubEnabled == d.githubEnabled)
        #expect(got.aiFeaturesEnabled == d.aiFeaturesEnabled)
        #expect(got.enhancedTrendingEnabled == d.enhancedTrendingEnabled)
        #expect(got.vulnerabilityScanningEnabled == d.vulnerabilityScanningEnabled)
        #expect(got.liveEnrichmentEnabled == d.liveEnrichmentEnabled)
        #expect(got.updateAutoCheck == d.updateAutoCheck)
        #expect(got.skippedUpdateVersions == d.skippedUpdateVersions)
    }

    @Test func newToggleDefaultsOffWhenAbsent() throws {
        // An older config that predates these opt-in network features must NOT
        // silently enable them.
        let oldConfig = """
        { "paranoidMode": false, "githubEnabled": true }
        """
        let got = try decode(oldConfig)
        #expect(got.githubEnabled == true)                       // preserved
        #expect(got.vulnerabilityScanningEnabled == false)       // absent → off
        #expect(got.liveEnrichmentEnabled == false)              // absent → off
        #expect(got.enhancedTrendingEnabled == false)            // absent → off
    }

    @Test func presentKeyOverridesDefault() throws {
        let got = try decode("""
        { "vulnerabilityScanningEnabled": true, "liveEnrichmentEnabled": true }
        """)
        #expect(got.vulnerabilityScanningEnabled == true)
        #expect(got.liveEnrichmentEnabled == true)
    }

    @Test func unknownKeysAreIgnored() throws {
        // A NEWER app's key we don't know about must not break decoding.
        let got = try decode("""
        { "paranoidMode": true, "someFutureKey": { "nested": [1, 2, 3] } }
        """)
        #expect(got.paranoidMode == true)
    }
}
