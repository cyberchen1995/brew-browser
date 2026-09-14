import Testing
import Foundation
@testable import BrewBrowserKit

// Tests for the vulnerability parsing — the subsystem that produced the worst
// bug (every package flagged with the whole install set's CVEs). Mirrors the
// Rust `vulns::client` parse/keying behavior.

@Suite("VulnSeverity")
struct VulnSeverityTests {
    @Test func caseFoldsWireValues() {
        #expect(VulnSeverity(wire: "CRITICAL") == .critical)
        #expect(VulnSeverity(wire: "critical") == .critical)
        #expect(VulnSeverity(wire: "High") == .high)
        #expect(VulnSeverity(wire: "MEDIUM") == .medium)
        // GHSA "MODERATE" maps to OSV "MEDIUM" (GOTCHA #2).
        #expect(VulnSeverity(wire: "MODERATE") == .medium)
        #expect(VulnSeverity(wire: "moderate") == .medium)
        #expect(VulnSeverity(wire: "LOW") == .low)
    }

    @Test func unknownAndNilFallBackToUnknown() {
        #expect(VulnSeverity(wire: "bogus") == .unknown)
        #expect(VulnSeverity(wire: "") == .unknown)
        #expect(VulnSeverity(wire: nil) == .unknown)
    }

    @Test func ordersByRisk() {
        #expect(VulnSeverity.critical > .high)
        #expect(VulnSeverity.high > .medium)
        #expect(VulnSeverity.medium > .low)
        #expect(VulnSeverity.low > .unknown)
        // max() surfaces the worst.
        #expect([VulnSeverity.low, .critical, .medium].max() == .critical)
    }
}

@Suite("VulnsService.parseScanOutputKeyed")
struct VulnsParseTests {
    // Two formulae, each with its OWN distinct finding. The regression we guard:
    // findings must attribute to the right package, never smear across all.
    static let twoFormulae = """
    [
      { "formula": "openssl@3", "version": "3.0.0", "vulnerabilities": [
          { "id": "CVE-AAA", "severity": "HIGH", "summary": "s1", "details": "d1", "fixed_versions": ["3.0.1"], "references": [] }
        ] },
      { "formula": "curl", "version": "8.0.0", "vulnerabilities": [
          { "id": "CVE-BBB", "severity": "MODERATE", "summary": "s2", "details": "d2", "fixed_versions": [], "references": [] }
        ] }
    ]
    """

    @Test func keysFindingsByFormulaWithoutSmearing() throws {
        let out = try VulnsService.parseScanOutputKeyed(Self.twoFormulae)
        #expect(Set(out.keys) == ["openssl@3", "curl"])

        let openssl = try #require(out["openssl@3"])
        #expect(openssl.count == 1)
        #expect(openssl.first?.rawId == "CVE-AAA")
        #expect(openssl.first?.severity == .high)
        // The other formula's CVE must NOT appear here (the smearing bug).
        #expect(!openssl.contains { $0.rawId == "CVE-BBB" })

        let curl = try #require(out["curl"])
        #expect(curl.first?.rawId == "CVE-BBB")
        #expect(curl.first?.severity == .medium)  // MODERATE → medium
    }

    @Test func cleanFormulaKeptWithEmptyFindings() throws {
        // A scanned-but-clean formula is a real signal (key present, [] findings).
        let json = """
        [ { "formula": "wget", "version": "1.0", "vulnerabilities": [] } ]
        """
        let out = try VulnsService.parseScanOutputKeyed(json)
        #expect(out["wget"] != nil)
        #expect(out["wget"]?.isEmpty == true)
    }

    @Test func emptyOutputYieldsNoRecords() throws {
        #expect(try VulnsService.parseScanOutputKeyed("").isEmpty)
        #expect(try VulnsService.parseScanOutputKeyed("   \n  ").isEmpty)
    }

    @Test func pureProseYieldsNoRecords() throws {
        // Issue #62: a clean exit with non-JSON prose on stdout (e.g. an
        // unsupported-source notice) is "nothing to report", not an error.
        #expect(try VulnsService.parseScanOutputKeyed("whatcable: skipped\n").isEmpty)
        #expect(try VulnsService.parseScanOutputKeyed("this is not json").isEmpty)
    }

    @Test func salvagesJSONBehindLeadingNoise() throws {
        // Issue #62: a brew banner/notice on stdout before the --json document.
        let raw = """
        Warning: some brew notice
        [ { "formula": "curl", "version": "8.4.0", "vulnerabilities": [] } ]
        """
        let out = try VulnsService.parseScanOutputKeyed(raw)
        #expect(out["curl"] != nil)
        #expect(out["curl"]?.isEmpty == true)
    }

    @Test func salvagesJSONBetweenNoiseLines() throws {
        let raw = """
        ==> downloading advisories
        [ { "formula": "openssl@3", "version": "3.2.0", "vulnerabilities": [] } ]
        ==> done
        """
        let out = try VulnsService.parseScanOutputKeyed(raw)
        #expect(out["openssl@3"] != nil)
    }

    @Test func malformedJSONPayloadThrows() {
        // A bracketed-but-broken payload is a genuine problem, still surfaced.
        #expect(throws: (any Error).self) {
            _ = try VulnsService.parseScanOutputKeyed("notice\n[ {broken json ]\n")
        }
    }

    @Test func extractJSONDocumentSkipsSurroundingLines() {
        #expect(VulnsService.extractJSONDocument("lead\n  [1,2,3]  \ntrail") == "  [1,2,3]  ")
        #expect(VulnsService.extractJSONDocument("no json here") == nil)
    }
}

@Suite("VulnsService.validateFormulaName")
struct VulnsValidateNameTests {
    @Test func acceptsTypicalAndTapQualifiedNames() throws {
        for name in [
            "curl", "openssl@3", "python@3.12", "git-lfs", "lib_foo",
            // Issue #92: third-party tap formulae are `user/repo/name` or shorthand `user/name`.
            "anomalyco/tap/opencode", "homebrew/core/wget", "user-name/repo_2/formula@1.2",
            "anomalyco/opencode", "homebrew/wget",
        ] {
            try VulnsService.validateFormulaName(name)  // throws on reject
        }
    }

    @Test func rejectsMalformedSlashesAndShellMeta() {
        for bad in [
            "/leading", "trailing/", "a//b", "a/b/c/d",
            "a/../b", "../etc/passwd", "./relative",
            "curl; rm -rf /", "curl|nc", "curl`whoami`", "curl $(id)", "",
        ] {
            #expect(throws: (any Error).self, "should reject: \(bad)") {
                try VulnsService.validateFormulaName(bad)
            }
        }
    }
}

@Suite("VulnsService brew resolution")
struct VulnsResolutionTests {
    // The regression we guard: a failed resolution used to be papered over
    // with a hardcoded "/opt/homebrew/bin/brew". The default init must carry
    // exactly what the shared resolver returns — including nil.
    @Test func defaultInitNeverFabricatesAPath() {
        #expect(VulnsService().brewPath == BrewService.resolveBrewPath())
    }

    @Test func nilBrewPathSurfacesBrewNotFound() async {
        let service = VulnsService(brewPath: nil)
        do {
            _ = try await service.scanOne(name: "wget", isCask: false)
            Issue.record("scanOne should throw .brewNotFound when no brew path resolved")
        } catch let error as VulnsServiceError {
            guard case .brewNotFound = error else {
                Issue.record("expected .brewNotFound, got \(error)")
                return
            }
        } catch {
            Issue.record("expected VulnsServiceError.brewNotFound, got \(error)")
        }
    }

    @Test func nilBrewPathReportsHelperNotInstalled() async {
        // The install probe is best-effort (`try?`) — with no brew it must
        // report not-installed, never crash or pretend a probe ran.
        let service = VulnsService(brewPath: nil)
        #expect(await service.isBrewVulnsInstalled() == false)
    }
}

@Suite("Homebrew 6 findings envelope")
struct VulnsFindingsEnvelopeTests {
    /// Homebrew 6.0+ absorbed `brew vulns` as a built-in command and wraps the
    /// records in an envelope rather than emitting a bare array. Before the
    /// fix, the envelope decoded vacuously into one blank record and the
    /// Exposure card reported "no known vulnerabilities" on a machine with 20
    /// real findings.
    @Test func envelopeIsUnwrapped() throws {
        let raw = """
        {
          "findings": [
            {"formula": "libheif", "version": "1.23.4",
             "vulnerabilities": [
               {"id": "OSV-2020-2308", "severity": "MEDIUM",
                "summary": "Heap-buffer-overflow", "fixed_versions": []}
             ]}
          ],
          "skipped_formulae": ["some-formula"]
        }
        """
        let keyed = try VulnsService.parseScanOutputKeyed(raw)
        #expect(keyed.count == 1, "findings must be unwrapped, not swallowed")
        #expect(keyed["libheif"]?.count == 1)
        #expect(keyed["libheif"]?.first?.id == "OSV-2020-2308")
    }

    /// Regression guard: an object that carries no formula name is not a scan
    /// record and must never decode as one — that vacuous match is exactly what
    /// hid the envelope's findings.
    @Test func unknownObjectShapeDoesNotDecodeAsCleanScan() {
        let raw = #"{"something_else": [1, 2, 3]}"#
        #expect(throws: (any Error).self) {
            _ = try VulnsService.parseScanOutputKeyed(raw)
        }
    }

    /// The legacy bare-array shape from the external brew-vulns formula must
    /// keep working — both forms are in the wild.
    @Test func legacyBareArrayStillParses() throws {
        let raw = """
        [{"formula": "augeas", "version": "1.14.1",
          "vulnerabilities": [{"id": "CVE-2024-1234", "severity": "HIGH",
                               "summary": "x", "fixed_versions": []}]}]
        """
        let keyed = try VulnsService.parseScanOutputKeyed(raw)
        #expect(keyed["augeas"]?.count == 1)
    }
}
