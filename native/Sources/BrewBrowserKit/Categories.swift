import Foundation

/// Loads the bundled `categories.json` (package→category-slug map) and computes
/// the "Top categories in your library" breakdown from installed packages —
/// the real data behind the Dashboard donut, same source as the Tauri app.
struct CategoryBreakdown: Identifiable, Hashable, Sendable {
    var id: String { slug }
    let slug: String
    let label: String
    let count: Int
    let fraction: Double
}

struct CategoryCatalog: Sendable {
    /// slug → display label
    private let labels: [String: String]
    /// package name → [slug]
    private let formulae: [String: [String]]
    private let casks: [String: [String]]

    /// Decode the bundled JSON. Returns nil if the resource is missing or
    /// malformed (the Dashboard then just hides the categories card).
    static func loadBundled() -> CategoryCatalog? {
        guard let url = Bundle.module.url(forResource: "categories", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }

        var labels: [String: String] = [:]
        if let cats = root["categories"] as? [String: Any] {
            for (slug, v) in cats {
                if let obj = v as? [String: Any], let label = obj["label"] as? String {
                    labels[slug] = label
                }
            }
        }
        let formulae = (root["formulae"] as? [String: [String]]) ?? [:]
        let casks = (root["casks"] as? [String: [String]]) ?? [:]
        return CategoryCatalog(labels: labels, formulae: formulae, casks: casks)
    }

    /// Category slugs for an installed package (formula first, then cask map).
    private func slugs(for name: String, kind: InstalledPackage.Kind) -> [String] {
        let map = kind == .cask ? casks : formulae
        return map[name] ?? []
    }

    /// Display labels for a single package's categories (for the detail panel
    /// pills), excluding the "uncategorized" bucket.
    func categoryLabels(for name: String, kind: InstalledPackage.Kind) -> [String] {
        slugs(for: name, kind: kind)
            .filter { $0 != "uncategorized" }
            .map { labels[$0] ?? $0.capitalized }
    }

    /// Top-N category breakdown across the installed set. Each package
    /// contributes 1 to each of its categories (multi-membership), matching the
    /// Tauri model. "uncategorized" is folded into an "Other" bucket along with
    /// the long tail beyond `top`.
    func breakdown(installed: [InstalledPackage], top: Int = 8) -> [CategoryBreakdown] {
        var counts: [String: Int] = [:]
        for pkg in installed {
            for slug in slugs(for: pkg.name, kind: pkg.kind) {
                counts[slug, default: 0] += 1
            }
        }
        let uncategorized = counts.removeValue(forKey: "uncategorized") ?? 0
        let totalMemberships = max(1, counts.values.reduce(0, +) + uncategorized)

        let ranked = counts.sorted { $0.value > $1.value }
        var result: [CategoryBreakdown] = []
        for (slug, count) in ranked.prefix(top) {
            result.append(CategoryBreakdown(
                slug: slug,
                label: labels[slug] ?? slug.capitalized,
                count: count,
                fraction: Double(count) / Double(totalMemberships)
            ))
        }
        let tail = ranked.dropFirst(top).reduce(0) { $0 + $1.value } + uncategorized
        if tail > 0 {
            result.append(CategoryBreakdown(
                slug: "other",
                label: "Other",
                count: tail,
                fraction: Double(tail) / Double(totalMemberships)
            ))
        }
        return result
    }
}
