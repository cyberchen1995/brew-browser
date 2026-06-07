import Testing
import Foundation
@testable import BrewBrowserKit

// Tests for CategoryCatalog — the membership/breakdown logic that powers the
// Dashboard "Top categories" card and the #58 Library category filter.

@Suite("CategoryCatalog")
struct CategoryCatalogTests {
    static let fixtureJSON = """
    {
      "categories": {
        "dev": { "label": "Developer Tools", "iconSF": "hammer" },
        "media": { "label": "Media", "iconSF": "play" },
        "uncategorized": { "label": "Uncategorized" }
      },
      "formulae": { "git": ["dev"], "ffmpeg": ["media", "dev"], "foo": ["uncategorized"] },
      "casks": { "iterm2": ["dev"] }
    }
    """

    private func catalog() throws -> CategoryCatalog {
        let data = Data(Self.fixtureJSON.utf8)
        return try #require(CategoryCatalog.parse(data: data))
    }

    @Test func parseRejectsGarbage() {
        #expect(CategoryCatalog.parse(data: Data("not json".utf8)) == nil)
    }

    @Test func isMemberMatchesFormulaeAndCasks() throws {
        let c = try catalog()
        #expect(c.isMember(token: "git", kind: .formula, slug: "dev"))
        #expect(c.isMember(token: "ffmpeg", kind: .formula, slug: "media"))
        #expect(c.isMember(token: "iterm2", kind: .cask, slug: "dev"))
        // Negatives: wrong slug, wrong kind, unknown token.
        #expect(!c.isMember(token: "git", kind: .formula, slug: "media"))
        #expect(!c.isMember(token: "git", kind: .cask, slug: "dev"))
        #expect(!c.isMember(token: "nope", kind: .formula, slug: "dev"))
    }

    @Test func categoryLabelsExcludeUncategorized() throws {
        let c = try catalog()
        #expect(c.categoryLabels(for: "git", kind: .formula) == ["Developer Tools"])
        // "foo" is only uncategorized → no labels surfaced.
        #expect(c.categoryLabels(for: "foo", kind: .formula).isEmpty)
    }

    @Test func allCategoriesExcludesUncategorizedAndSorts() throws {
        let c = try catalog()
        let all = c.allCategories()
        #expect(all.map(\.slug) == ["dev", "media"])  // alphabetised by label
        #expect(!all.contains { $0.slug == "uncategorized" })
    }

    @Test func breakdownCountsMembershipsAndFoldsOther() throws {
        let c = try catalog()
        let installed = [
            InstalledPackage(name: "git", version: "1", kind: .formula),     // dev
            InstalledPackage(name: "ffmpeg", version: "1", kind: .formula),  // media, dev
            InstalledPackage(name: "iterm2", version: "1", kind: .cask),     // dev
            InstalledPackage(name: "foo", version: "1", kind: .formula),     // uncategorized
        ]
        let bd = c.breakdown(installed: installed)
        let byslug = Dictionary(uniqueKeysWithValues: bd.map { ($0.slug, $0.count) })
        #expect(byslug["dev"] == 3)     // git + ffmpeg + iterm2
        #expect(byslug["media"] == 1)   // ffmpeg
        #expect(byslug["other"] == 1)   // uncategorized folded into Other
    }
}
