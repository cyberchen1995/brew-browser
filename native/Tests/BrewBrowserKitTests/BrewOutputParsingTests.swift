import Testing
@testable import BrewBrowserKit

// Parity tests for the Swift port of the Rust brew-output parsing
// (`src-tauri/src/brew/error_patterns.rs`, `exec.rs`). The fixtures here are
// the SAME real-stderr shapes the Rust tests use, so the two implementations
// are pinned to identical behavior. When the Rust catalog changes, change both.

// MARK: - friendlify

@Suite("BrewErrorPatterns.friendlify")
struct FriendlifyTests {
    // Captured from a real `brew bundle dump --force` run.
    static let topo = """
    Error: key not found: "shivammathur/extensions/imap-uw"
    /opt/homebrew/Library/Homebrew/bundle/brew.rb:686:in 'Homebrew::Bundle::Brew::Topo#tsort_each_child'
    """
    static let report = """
    Error: undefined method 'foo' for nil:NilClass
    Please report this issue:
      https://docs.brew.sh/Troubleshooting
    """
    static let launchd = "Error: Could not find service \"ollama\" in domain for current user (gui/501/16).\nTry running launchctl bootstrap under the right domain, or move the plist to ~/Library/LaunchAgents.\n"
    static let locked = "Error: A `brew upgrade` process has already locked /opt/homebrew/Cellar/ca-certificates.\nPlease wait for it to finish or terminate it to continue.\n"

    @Test func topoMatchesOnBundle() {
        let msg = BrewErrorPatterns.friendlify(stderr: Self.topo, command: "brew bundle dump --file=/tmp/x --force")
        #expect(msg?.contains("upstream Homebrew bug") == true)
        #expect(msg?.contains("brew untap") == true)
    }

    @Test func topoDoesNotFireOnNonBundle() {
        #expect(BrewErrorPatterns.friendlify(stderr: Self.topo, command: "brew install foo") == nil)
    }

    @Test func pleaseReportMatches() {
        let msg = BrewErrorPatterns.friendlify(stderr: Self.report, command: "brew bundle dump")
        #expect(msg?.contains("docs.brew.sh/Troubleshooting") == true)
    }

    @Test func launchdMatchesOnServices() {
        let msg = BrewErrorPatterns.friendlify(stderr: Self.launchd, command: "brew services start ollama")
        #expect(msg?.contains("launchd") == true)
        #expect(msg?.contains("~/Library/LaunchAgents") == true)
    }

    @Test func launchdDoesNotFireOutsideServices() {
        #expect(BrewErrorPatterns.friendlify(stderr: Self.launchd, command: "brew install ollama") == nil)
    }

    @Test func lockedMatches() {
        let msg = BrewErrorPatterns.friendlify(stderr: Self.locked, command: "brew upgrade")
        #expect(msg?.contains("Another Homebrew process") == true)
    }

    @Test func genericFailureFallsThrough() {
        let stderr = "Error: No available formula with the name \"definitely-not-a-real-pkg\".\n"
        #expect(BrewErrorPatterns.friendlify(stderr: stderr, command: "brew install definitely-not-a-real-pkg") == nil)
    }

    @Test func emptyInputsReturnNil() {
        #expect(BrewErrorPatterns.friendlify(stderr: "", command: "") == nil)
        #expect(BrewErrorPatterns.friendlify(stderr: "Error: anything", command: "") == nil)
    }
}

// MARK: - upgradeWarningsOnly

@Suite("BrewErrorPatterns.upgradeWarningsOnly")
struct UpgradeWarningsTests {
    // Real stderr tails from the bogus "Upgrade-all failed" reports (#28/#53/#55).
    static let postInstall = "Warning: ffmpeg@7 was installed but not linked because ffmpeg is already linked.\nWarning: The post-install step did not complete successfully\n"
    static let skipLink = "Warning: The post-install step did not complete successfully\nWarning: It seems there is already a Binary at '/opt/homebrew/bin/codex' from formula codex; skipping link.\n"
    static let linkStep = "Error: The `brew link` step did not complete successfully\n"

    @Test func matchesPostInstallWarning() {
        #expect(BrewErrorPatterns.upgradeWarningsOnly(stderr: Self.postInstall, command: "brew upgrade"))
    }

    @Test func matchesSkipLink() {
        #expect(BrewErrorPatterns.upgradeWarningsOnly(stderr: Self.skipLink, command: "brew upgrade"))
    }

    @Test func matchesLinkStepError() {
        #expect(BrewErrorPatterns.upgradeWarningsOnly(stderr: Self.linkStep, command: "brew upgrade git"))
    }

    @Test func falseOnRealFailure() {
        let mixed = "Warning: The post-install step did not complete successfully\nError: Failed to download resource \"foo\"\n"
        #expect(!BrewErrorPatterns.upgradeWarningsOnly(stderr: mixed, command: "brew upgrade"))
    }

    @Test func falseOnLock() {
        let locked = "Error: A `brew upgrade` process has already locked /opt/homebrew/Cellar/x.\n"
        #expect(!BrewErrorPatterns.upgradeWarningsOnly(stderr: locked, command: "brew upgrade"))
    }

    @Test func gatedToUpgradeInstall() {
        #expect(!BrewErrorPatterns.upgradeWarningsOnly(stderr: Self.postInstall, command: "brew services list"))
    }

    @Test func falseWhenNoMarker() {
        #expect(!BrewErrorPatterns.upgradeWarningsOnly(stderr: "✔︎ Bottle foo (1.0)\n", command: "brew upgrade"))
    }
}

// MARK: - BrewProgressParser

@Suite("BrewProgressParser")
struct ProgressParserTests {
    @Test func parsesUpgradeSequence() {
        var p = BrewProgressParser()
        // Header sets total, not itself a tick.
        #expect(p.observe("==> Upgrading 3 outdated packages:") == nil)
        // Non-marker lines ignored.
        #expect(p.observe("foo 1.0 -> 1.1") == nil)

        let t1 = p.observe("==> Pouring foo--1.1.arm64.bottle.tar.gz")
        #expect(t1?.phase == "Pouring")
        #expect(t1?.package == "foo")
        #expect(t1?.current == 1)
        #expect(t1?.total == 3)

        // Downloading updates phase without advancing the counter.
        let t2 = p.observe("==> Downloading https://example.com/bar.bottle")
        #expect(t2?.phase == "Downloading")
        #expect(t2?.current == 1)

        let t3 = p.observe("==> Pouring bar--2.0.arm64.bottle.tar.gz")
        #expect(t3?.package == "bar")
        #expect(t3?.current == 2)

        // Same package's later phase does not double-count.
        let t4 = p.observe("==> Installing bar")
        #expect(t4?.current == 2)
    }

    @Test func totalFromDependencyList() {
        var p = BrewProgressParser()
        #expect(p.observe("==> Installing dependencies for wget: openssl@3, ca-certificates") == nil)
        let t = p.observe("==> Installing openssl@3")
        #expect(t?.total == 3) // 2 deps + the target
    }

    @Test func nonMarkerLinesYieldNil() {
        var p = BrewProgressParser()
        #expect(p.observe("") == nil)
        #expect(p.observe("just some output") == nil)
        #expect(p.observe("Warning: something") == nil)
    }

    // Robustness: adversarial lines must never crash, and the counter must be
    // monotonic non-decreasing (parity with the Rust fuzz test in exec.rs).
    @Test func robustAgainstAdversarialLines() {
        var p = BrewProgressParser()
        let lines = [
            "", "==>", "==> ", "==> Pouring", "==> Pouring ", "==> Pouring --",
            "==> Upgrading 4294967296 outdated packages:",
            "==> Upgrading 999999999999999999999 outdated packages:",
            "==> Upgrading -1 outdated packages:",
            "==> Installing dependencies for x:",
            "==> Installing dependencies for x: ,, , ,",
            "==> Fetching ", "==> Downloading ",
            "==> Pouring x--" + String(repeating: "y", count: 50_000),
            "日本語==> Pouring 日本--1.0",
        ]
        var last = 0
        for _ in 0..<200 {
            for l in lines {
                if let t = p.observe(l) {
                    #expect(t.current >= last)
                    last = t.current
                }
            }
        }
    }
}

@Suite("Classifier robustness")
struct ClassifierFuzzTests {
    @Test func neverCrashOnAdversarialInput() {
        var inputs = ["", " ", "\n\0\t", "Error:", "Warning:", "has already locked"]
        inputs.append(String(repeating: "A", count: 200_000))
        inputs.append(String(repeating: "日本語", count: 20_000))
        inputs.append("\u{0}\u{1}\u{2}\u{7f}control")
        let commands = ["", "brew upgrade", "brew install x", "brew services start y", "brew bundle dump"]
        for inp in inputs {
            for cmd in commands {
                _ = BrewErrorPatterns.friendlify(stderr: inp, command: cmd)
                _ = BrewErrorPatterns.upgradeWarningsOnly(stderr: inp, command: cmd)
            }
        }
    }
}
