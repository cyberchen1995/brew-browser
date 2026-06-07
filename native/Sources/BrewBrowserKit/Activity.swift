import Foundation

/// A streaming brew job (install / upgrade / uninstall) — mirrors the Tauri
/// `ActivityJob` (`src/lib/types.ts`, `stores/activity.svelte.ts`). Lives in the
/// Activity drawer (live) and the Activity panel (history). Codable so the
/// history persists to UserDefaults across launches.
struct ActivityJob: Identifiable, Hashable, Codable, Sendable {
    let id: UUID
    /// Human label, e.g. "Installing wget".
    let label: String
    /// The brew argv, joined for display ("brew install --cask iterm2").
    let command: String
    /// Epoch seconds when the job started (Codable-friendly; no Date() in
    /// preview/build contexts that forbid it).
    let startedAt: Double
    var status: JobStatus
    var lines: [ActivityLine]
    var exitCode: Int32?
    var durationMs: Int?
    /// Best-effort live progress from brew's `==>` markers (running jobs).
    /// Mirrors the Tauri `ActivityJob.progress`. See #57.
    var progress: JobProgress?

    enum JobStatus: String, Codable, Sendable {
        case running, succeeded, failed, canceled
    }
}

/// Live progress for a running job, parsed from brew's `==>` output markers.
/// Mirrors the Tauri `JobProgress` (`src/lib/types.ts`).
struct JobProgress: Hashable, Codable, Sendable {
    var phase: String          // "Pouring" | "Downloading" | "Installing" | …
    var package: String        // current package (may be empty)
    var current: Int           // 1-based index of the current unit
    var total: Int?            // total units when known, else nil
}

/// One line of streamed output.
struct ActivityLine: Hashable, Codable, Sendable {
    enum Stream: String, Codable, Sendable { case stdout, stderr }
    let stream: Stream
    let text: String
}
