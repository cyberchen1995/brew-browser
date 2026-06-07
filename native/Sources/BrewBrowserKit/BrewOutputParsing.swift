import Foundation

/// Parsing of brew's textual output — friendly error mapping + non-fatal
/// upgrade classification + live progress. This is a Swift port of the Tauri
/// `src-tauri/src/brew/error_patterns.rs` and the `ProgressParser` in
/// `src-tauri/src/brew/exec.rs`. The two builds parse the SAME brew output, so
/// the rules here must stay in lockstep with the Rust side (parity charter —
/// `memory-bank/decisions.md` 2026-06-01). Canonical tests live on the Rust
/// side (`error_patterns.rs`, `exec.rs`); there is no native test target.
enum BrewErrorPatterns {
    /// Friendly one-sentence message for a known upstream-brew pattern, or nil.
    /// Mirrors `error_patterns::friendlify`.
    static func friendlify(stderr: String, command: String) -> String? {
        let isBundle = command.contains("bundle dump") || command.contains("bundle install")

        // Pattern 1 — `brew bundle` topo-sort key-not-found (upstream Ruby bug).
        if isBundle,
           stderr.contains("key not found:"),
           stderr.contains("Homebrew::Bundle::Brew::Topo") {
            return "Homebrew's `brew bundle` hit an internal topo-sort error on one "
                + "of your installed formulae. This is an upstream Homebrew bug, not "
                + "a brew-browser issue. Try `brew untap` on a recently-added tap, or "
                + "see the full output in Activity."
        }

        // Pattern 2 — Homebrew explicitly asks the user to report upstream.
        if stderr.contains("Please report this issue:"),
           stderr.contains("docs.brew.sh/Troubleshooting") {
            return "Homebrew reported an internal error and asked you to report it "
                + "upstream. See the full output in Activity, and visit "
                + "https://docs.brew.sh/Troubleshooting for next steps."
        }

        // Pattern 3 — `brew services` failed: plist in a launchd domain the
        // current session doesn't own (ports PR #51).
        if command.contains("services"),
           stderr.contains("Could not find service") || stderr.contains("not found in domain") {
            return "brew could not find this service in the current launchd domain. "
                + "The plist is likely registered for a different user or session. "
                + "Try `brew services restart`, or move the plist to "
                + "~/Library/LaunchAgents and run `launchctl bootstrap gui/$UID`."
        }

        // Pattern 4 — another brew process holds the lock (environmental).
        if stderr.contains("has already locked")
            || stderr.contains("Please wait for it to finish or terminate it") {
            return "Another Homebrew process is already running and holds the lock. "
                + "Wait for it to finish (or quit the other process) and try again — "
                + "this isn't a brew-browser problem."
        }

        return nil
    }

    /// True when a non-zero `brew upgrade`/`install` exit carries ONLY non-fatal
    /// warnings (post-install warnings, link conflicts, already-linked kegs,
    /// already-present binaries). brew escalates these to exit code 1 even
    /// though the work completed — the dominant source of bogus "Upgrade-all
    /// failed" reports. Mirrors `error_patterns::upgrade_warnings_only`.
    static func upgradeWarningsOnly(stderr: String, command: String) -> Bool {
        guard command.contains("upgrade") || command.contains("install") else {
            return false
        }

        // Hard-fatal signatures — a real failure regardless of warnings.
        let fatal = [
            "No available formula",
            "No such file or directory",
            "Permission denied",
            "Failed to download",
            "Download failed",
            "Could not resolve host",
            "has already locked",          // concurrent lock — env. failure
            "Please report this issue:",
            "Homebrew::Bundle::Brew::Topo",
            "checksum does not match",
            "SHA256 mismatch",
        ]
        if fatal.contains(where: { stderr.contains($0) }) { return false }

        // Known non-fatal warning markers (the `brew link` "Error:" line is a
        // single-keg link conflict, not a failed upgrade).
        let nonFatal = [
            "post-install step did not complete successfully",
            "not linked because",
            "already linked",
            "skipping link",
            "already a Binary at",
            "`brew link` step did not complete successfully",
        ]
        return nonFatal.contains(where: { stderr.contains($0) })
    }
}

/// Best-effort progress tracker over brew's stdout `==>` markers. Stateful:
/// learns the total from "Upgrading N outdated packages:" /
/// "Installing dependencies for X: a, b, c", and advances a per-package counter
/// as work markers (Pouring / Installing / Upgrading) name new packages.
/// Heuristic — unrecognized output yields nil, so the stream is never affected.
/// Port of the Rust `ProgressParser` (`exec.rs`).
struct BrewProgressParser {
    private var total: Int?
    private var current: Int = 0
    private var lastPackage: String?

    /// `Pouring foo--1.2.arm64.bottle.tar.gz` → `foo`; `Installing foo` → `foo`.
    private static func pkgName(_ rest: String) -> String {
        let first = rest.split(whereSeparator: { $0 == " " || $0 == "\t" }).first.map(String.init) ?? ""
        if let range = first.range(of: "--") {
            return String(first[first.startIndex..<range.lowerBound])
        }
        return first
    }

    mutating func observe(_ line: String) -> JobProgress? {
        let t = line.drop(while: { $0 == " " || $0 == "\t" })
        guard t.hasPrefix("==> ") else { return nil }
        let rest = String(t.dropFirst(4))

        // Total from the upgrade header.
        if rest.hasPrefix("Upgrading ") {
            let r = String(rest.dropFirst("Upgrading ".count))
            if r.contains("outdated package") {
                if let first = r.split(separator: " ").first, let n = Int(first) {
                    total = n
                }
                return nil
            }
            // else "Upgrading <pkg>" — fall through to work markers.
        }

        // Total from a dependency list.
        if rest.hasPrefix("Installing dependencies for ") {
            if let colon = rest.firstIndex(of: ":") {
                let list = rest[rest.index(after: colon)...]
                let n = list.split(separator: ",").filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }.count
                if n > 0 {
                    total = max(total ?? 0, n + 1)  // +1 for the target itself
                }
            }
            return nil
        }

        // Per-package work markers — advance the counter on a new package.
        for kw in ["Pouring ", "Installing ", "Upgrading "] {
            if rest.hasPrefix(kw) {
                let pkg = Self.pkgName(String(rest.dropFirst(kw.count)))
                if pkg.isEmpty { continue }
                if lastPackage != pkg {
                    current += 1
                    lastPackage = pkg
                }
                return JobProgress(phase: String(kw.dropLast()), package: pkg,
                                   current: current, total: total)
            }
        }

        // Phase-only markers — update the phase without advancing the counter.
        for kw in ["Downloading ", "Fetching "] {
            if rest.hasPrefix(kw) {
                let pkg = kw.hasPrefix("Fetching")
                    ? Self.pkgName(String(rest.dropFirst(kw.count)))
                    : (lastPackage ?? "")
                return JobProgress(phase: String(kw.dropLast()), package: pkg,
                                   current: current, total: total)
            }
        }

        return nil
    }
}
