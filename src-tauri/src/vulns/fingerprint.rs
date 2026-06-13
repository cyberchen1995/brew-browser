//! Install-set fingerprinting for the v0.5.0 vulnerability-scanning
//! subsystem.
//!
//! The fingerprint is a stable SHA-256 of the sorted
//! `kind:name:version` lines for every installed package. Two scans
//! taken at different times against the same install set produce the
//! same fingerprint; any install / upgrade / uninstall changes it.
//!
//! The whole-scan skip optimization compares the current fingerprint to
//! the one stored alongside the cache. When they match AND the cache is
//! within TTL, the backend skips invoking `brew vulns` entirely —
//! cache reads still produce per-package answers from disk.
//!
//! Stable across runs is the load-bearing property. Rust's
//! [`std::collections::hash_map::DefaultHasher`] randomizes its seed
//! per-process, so it would silently invalidate the cache on every
//! launch. SHA-256 avoids that.

use sha2::{Digest, Sha256};

use crate::types::PackageKind;

/// A minimal installed-package descriptor — just enough for the
/// fingerprint. Borrowed view so the caller can derive this from
/// whatever they already have (`Package`, `RawPackage`, …) without
/// cloning.
#[derive(Debug, Clone, Copy)]
pub struct InstalledRef<'a> {
    pub kind: PackageKind,
    pub name: &'a str,
    pub version: &'a str,
}

/// Compute the SHA-256 fingerprint of an install set. Order-independent
/// — the input is sorted before hashing so the caller doesn't have to
/// pre-sort.
///
/// Empty input is valid and produces the SHA-256 of the empty string.
pub fn compute(installed: &[InstalledRef<'_>]) -> String {
    let mut lines: Vec<String> = installed
        .iter()
        .map(|p| format!("{}:{}:{}", kind_token(p.kind), p.name, p.version))
        .collect();
    lines.sort();
    let joined = lines.join("\n");

    let mut hasher = Sha256::new();
    hasher.update(joined.as_bytes());
    hex::encode(hasher.finalize())
}

/// Compact one-token form of [`PackageKind`] used in the fingerprint
/// lines. `formula` / `cask` matches the serialized field name elsewhere
/// in the codebase — keep aligned if [`PackageKind`] gets a new variant.
fn kind_token(kind: PackageKind) -> &'static str {
    match kind {
        PackageKind::Formula => "formula",
        PackageKind::Cask => "cask",
    }
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn formula<'a>(name: &'a str, version: &'a str) -> InstalledRef<'a> {
        InstalledRef {
            kind: PackageKind::Formula,
            name,
            version,
        }
    }

    #[test]
    fn empty_input_is_sha256_of_empty_string() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let fp = compute(&[]);
        assert_eq!(
            fp,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn fingerprint_is_stable_across_calls() {
        let set = [formula("openssl@3", "3.2.0"), formula("curl", "8.4.0")];
        assert_eq!(compute(&set), compute(&set), "must be deterministic");
    }

    #[test]
    fn fingerprint_is_order_independent() {
        let a = [formula("openssl@3", "3.2.0"), formula("curl", "8.4.0")];
        let b = [formula("curl", "8.4.0"), formula("openssl@3", "3.2.0")];
        assert_eq!(
            compute(&a),
            compute(&b),
            "different input order must produce the same fingerprint"
        );
    }

    #[test]
    fn version_change_invalidates_fingerprint() {
        let before = [formula("openssl@3", "3.2.0")];
        let after = [formula("openssl@3", "3.2.1")];
        assert_ne!(
            compute(&before),
            compute(&after),
            "upgrading a package must change the fingerprint"
        );
    }

    #[test]
    fn install_change_invalidates_fingerprint() {
        let before = [formula("curl", "8.4.0")];
        let after = [formula("curl", "8.4.0"), formula("wget", "1.21.4")];
        assert_ne!(
            compute(&before),
            compute(&after),
            "installing a package must change the fingerprint"
        );
    }

    #[test]
    fn kind_disambiguates_same_name() {
        // A formula and a cask with the same name + version produce
        // *different* lines, so the fingerprint catches a kind-only flip.
        // (Brew doesn't actually allow this collision in practice but
        // the data model permits it, so the test pins the behaviour.)
        let f = [InstalledRef {
            kind: PackageKind::Formula,
            name: "alacritty",
            version: "0.13.0",
        }];
        let c = [InstalledRef {
            kind: PackageKind::Cask,
            name: "alacritty",
            version: "0.13.0",
        }];
        assert_ne!(compute(&f), compute(&c));
    }

    #[test]
    fn fingerprint_is_lowercase_hex_64_chars() {
        let fp = compute(&[formula("curl", "8.4.0")]);
        assert_eq!(fp.len(), 64, "SHA-256 hex length");
        assert!(
            fp.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "must be lowercase hex"
        );
    }
}
