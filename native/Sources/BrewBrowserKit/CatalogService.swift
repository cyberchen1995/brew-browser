import Foundation
import Compression

/// One package from the full Homebrew catalog (available, not necessarily
/// installed) — the data behind the Discover panel. Sourced from the bundled
/// gzipped `catalog/{formula,cask}.json.gz` (same data the Tauri app ships at
/// `src-tauri/data/catalog/`). Only the fields Discover renders are kept.
struct CatalogPackage: Identifiable, Hashable, Sendable {
    var id: String { "\(kind.rawValue):\(token)" }
    /// Homebrew token (formula name / cask token) — the install identifier.
    let token: String
    /// Human display name (cask `name[0]`; formula uses the token).
    let displayName: String
    let desc: String
    let homepage: String
    let version: String
    let kind: InstalledPackage.Kind
}

/// Loads + decompresses the bundled catalog once, off the main actor, and
/// exposes the parsed package list. ~16k entries: parse a single time, hold in
/// memory. Mirrors the Tauri catalog loader (`src-tauri/src/catalog/`) at the
/// data level (parity charter: same bundled JSON, same shapes).
actor CatalogService {
    private var cache: [CatalogPackage]?

    init() {}

    /// The full catalog (formulae + casks), name-sorted. Loads + decompresses on
    /// first call, then serves from memory. Returns [] if resources are missing
    /// or malformed (Discover then shows an empty state rather than crashing).
    func all() async -> [CatalogPackage] {
        if let cache { return cache }
        var out: [CatalogPackage] = []
        out.append(contentsOf: Self.loadFormulae())
        out.append(contentsOf: Self.loadCasks())
        out.sort { $0.displayName.localizedCaseInsensitiveCompare($1.displayName) == .orderedAscending }
        cache = out
        return out
    }

    // MARK: - Decode

    private static func loadFormulae() -> [CatalogPackage] {
        guard let arr = loadGzippedJSONArray("formula") else { return [] }
        return arr.compactMap { obj in
            guard let name = obj["name"] as? String else { return nil }
            let version = ((obj["versions"] as? [String: Any])?["stable"] as? String) ?? "—"
            return CatalogPackage(
                token: name,
                displayName: name,                       // formulae have no separate display name
                desc: obj["desc"] as? String ?? "",
                homepage: obj["homepage"] as? String ?? "",
                version: version,
                kind: .formula
            )
        }
    }

    private static func loadCasks() -> [CatalogPackage] {
        guard let arr = loadGzippedJSONArray("cask") else { return [] }
        return arr.compactMap { obj in
            guard let token = obj["token"] as? String else { return nil }
            // cask `name` is an array of human names; first is the primary.
            let display = (obj["name"] as? [String])?.first ?? token
            return CatalogPackage(
                token: token,
                displayName: display,
                desc: obj["desc"] as? String ?? "",
                homepage: obj["homepage"] as? String ?? "",
                version: obj["version"] as? String ?? "—",
                kind: .cask
            )
        }
    }

    /// Read `catalog/<name>.json.gz` from the module bundle, gunzip, parse as a
    /// JSON array of objects. Returns nil on any failure.
    private static func loadGzippedJSONArray(_ name: String) -> [[String: Any]]? {
        guard let url = Bundle.module.url(forResource: name, withExtension: "json.gz", subdirectory: "catalog")
                ?? Bundle.module.url(forResource: name, withExtension: "json.gz"),
              let gz = try? Data(contentsOf: url),
              let raw = gunzip(gz),
              let arr = try? JSONSerialization.jsonObject(with: raw) as? [[String: Any]]
        else { return nil }
        return arr
    }

    /// Decompress a gzip blob via Apple's Compression framework. `.gz` is a
    /// 10-byte header + raw DEFLATE + 8-byte trailer; COMPRESSION_ZLIB wants the
    /// raw DEFLATE body, so strip the fixed header and trailer. Our catalog
    /// files have no extra-field/name flags, so the header is exactly 10 bytes.
    /// (Verified against the real bundled files before adopting.)
    private static func gunzip(_ data: Data) -> Data? {
        guard data.count > 18, data[data.startIndex] == 0x1f, data[data.startIndex + 1] == 0x8b else { return nil }
        let body = data.subdata(in: (data.startIndex + 10)..<(data.endIndex - 8))
        let dstCapacity = 64 * 1024 * 1024  // 64 MiB ceiling (raw catalog ~44 MiB)
        let dst = UnsafeMutablePointer<UInt8>.allocate(capacity: dstCapacity)
        defer { dst.deallocate() }
        let written = body.withUnsafeBytes { src -> Int in
            guard let base = src.bindMemory(to: UInt8.self).baseAddress else { return 0 }
            return compression_decode_buffer(dst, dstCapacity, base, body.count, nil, COMPRESSION_ZLIB)
        }
        guard written > 0 else { return nil }
        return Data(bytes: dst, count: written)
    }
}
