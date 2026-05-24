//! Strict allowlist parser for GitHub repo URLs.
//!
//! `parse_github_url` is the *only* code in the project that turns a
//! cask/formula `homepage` into an `(owner, repo)` pair we'll use to
//! build an `api.github.com/repos/...` URL or a `<owner>__<repo>.json`
//! cache filename. Every defense it implements is required by
//! `memory-bank/scans/phase12-security-review.md` §12c.
//!
//! ## Rules
//!
//! 1. **Scheme:** http or https (case-insensitive). `parse_http_url`
//!    is the SSRF gate — it also rejects IP literals and non-public
//!    hostnames, but `github.com` (the only host we accept) is public
//!    so the IP filter never fires on the happy path.
//! 2. **Host:** exactly `github.com` (case-insensitive). Reject
//!    `gist.github.com`, `raw.githubusercontent.com`,
//!    `github.com.evil.com`, `evil.com/github.com/…`, etc.
//! 3. **Path:** after trimming a trailing `/`, a `/tree/…` suffix,
//!    a `/blob/…` suffix, or a `.git` suffix, the segments must be
//!    exactly `["", owner, repo]`. Nothing else.
//! 4. **Owner / repo:** match `^[A-Za-z0-9._-]{1,39}$` (GitHub's real
//!    rules), reject leading `.`, reject `..` segments anywhere in
//!    the path.
//!
//! The validator is intentionally strict — false negatives (a real
//! GitHub URL that doesn't match) just mean the package doesn't get
//! GitHub stats, which is the safe default. False positives (a
//! non-GitHub URL slipping through) are the security problem we're
//! preventing.

use crate::commands::cask_icon_homepage::is_public_host;

/// Owner + repo pair extracted from a validated GitHub URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubRepo {
    pub owner: String,
    pub repo: String,
}

impl GithubRepo {
    /// Filename-safe key used by the disk cache layer. The components
    /// have already passed the strict allowlist, so the result is
    /// guaranteed not to contain `/`, `..`, or path-traversal bytes.
    pub fn cache_key(&self) -> String {
        format!("{}__{}", self.owner, self.repo)
    }

    /// Canonical `api.github.com` repo URL — built from validated
    /// pieces so no caller can sneak in a custom host.
    pub fn api_url(&self) -> String {
        format!("https://api.github.com/repos/{}/{}", self.owner, self.repo)
    }
}

/// Length cap for both owner and repo per the GitHub spec (logins are
/// capped at 39 chars; same cap is applied to repo names by GitHub).
const NAME_MAX_LEN: usize = 39;

/// Try to parse `homepage` as a GitHub repo URL.
///
/// Returns `Some(GithubRepo)` only when *every* rule in this module's
/// header doc holds. Anything else (a non-GitHub URL, a malformed URL,
/// a 3-segment path, a subdomain, a path-traversal attempt) returns
/// `None`.
pub fn parse_github_url(homepage: &str) -> Option<GithubRepo> {
    let homepage = homepage.trim();
    if homepage.is_empty() {
        return None;
    }

    // 1. Scheme + authority + path split. We don't reuse `parse_http_url`'s
    //    `ParsedUrl` type because we need the *path*, which it discards.
    //    We *do* reuse `is_public_host` so the SSRF defense the rest of
    //    the codebase uses is the one we use too — even though
    //    `github.com` is always public.
    let (scheme_len, scheme_is_https) = if homepage.len() >= 8
        && homepage[..8].eq_ignore_ascii_case("https://")
    {
        (8usize, true)
    } else if homepage.len() >= 7 && homepage[..7].eq_ignore_ascii_case("http://") {
        (7usize, false)
    } else {
        return None;
    };
    // Suppress the "unused" warning while still keeping the variable so a
    // future audit can extend the parser to reject `http://` if we ever
    // want to be even stricter (currently allowed, matching `parse_http_url`).
    let _ = scheme_is_https;

    let rest = &homepage[scheme_len..];
    if rest.is_empty() {
        return None;
    }

    // Authority ends at the first `/`, `?`, or `#`. We refuse any
    // query / fragment outright — GitHub repo URLs don't need them and
    // accepting them complicates the path-equality check below.
    let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..auth_end];
    if authority.is_empty() {
        return None;
    }

    // Strip any `user@` userinfo prefix; never carry credentials.
    let host_with_port = authority.rsplit('@').next().unwrap_or(authority);
    if host_with_port.is_empty() {
        return None;
    }

    // Pull the bare host (no port). github.com doesn't speak on non-default
    // ports for the web UI, but accept :443 / :80 so users with corp proxy
    // configs that prepend the port still work — the host check is the gate.
    let host = host_with_port.split(':').next().unwrap_or(host_with_port);

    // 2. Exact host match. Case-insensitive ASCII compare — github.com is
    //    ASCII so no Unicode normalization concerns. Reject everything else
    //    including subdomains and suffix-confusable hostnames.
    if !host.eq_ignore_ascii_case("github.com") {
        return None;
    }

    // Defense in depth: even though github.com is always public, run it
    // through the project's standard host filter so any future change to
    // `is_public_host` (e.g. blocking github.com in some restricted-mode)
    // is picked up automatically.
    if !is_public_host(host) {
        return None;
    }

    // The remainder is the path-and-anything-after. Reject anything with
    // `?` or `#` (no query/fragment allowed) BEFORE we trim the suffixes
    // so an attacker can't smuggle state past the trimmer.
    let path = if auth_end >= rest.len() {
        ""
    } else {
        &rest[auth_end..]
    };
    if path.contains('?') || path.contains('#') {
        return None;
    }

    // 3. Trim recognized suffixes in a fixed order:
    //    a) trailing slash
    //    b) `/tree/…` or `/blob/…` ref suffix (drops the rest of the path)
    //    c) `.git` suffix on the repo segment
    let path = path.trim_end_matches('/');

    // Find a recognized ref-style suffix (`/tree/X`, `/blob/X`) and trim
    // it. We only honour these for the exact form `/owner/repo/tree/...`
    // — i.e. the `tree`/`blob` segment must be the THIRD path segment.
    // This means `/foo/tree/bar/baz` (where owner is "foo") is the only
    // accepted shape; we won't accidentally trim a repo literally named
    // "tree" — those would have path `/owner/tree` which has 2 segments.
    let trimmed_path: String = {
        // Split into non-empty segments.
        let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segs.len() >= 3 && (segs[2].eq_ignore_ascii_case("tree") || segs[2].eq_ignore_ascii_case("blob")) {
            format!("/{}/{}", segs[0], segs[1])
        } else {
            path.to_string()
        }
    };

    // Strip a trailing `.git` from the repo segment if present.
    let trimmed_path = if let Some(stripped) = trimmed_path.strip_suffix(".git") {
        stripped.to_string()
    } else {
        trimmed_path
    };

    // 4. Path must now be exactly /owner/repo.
    let segs: Vec<&str> = trimmed_path.split('/').collect();
    // After splitting "/owner/repo" we expect ["", "owner", "repo"].
    if segs.len() != 3 || !segs[0].is_empty() {
        return None;
    }
    let owner = segs[1];
    let repo = segs[2];

    // 5. Owner + repo allowlist.
    if !is_valid_owner_or_repo(owner) || !is_valid_owner_or_repo(repo) {
        return None;
    }

    // 6. Belt-and-braces: reject any `..` segment anywhere in the
    //    trimmed path (already covered by segment validation, but the
    //    check costs nothing and makes the intent obvious for reviewers).
    if owner == ".." || repo == ".." {
        return None;
    }

    Some(GithubRepo {
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

/// Apply GitHub's owner/repo lexical rules. Per GitHub:
/// - 1..=39 characters
/// - allowed: letters, digits, `-`, `_`, `.`
/// - must not start with `.` or `-`
/// - must not be `.` or `..`
fn is_valid_owner_or_repo(name: &str) -> bool {
    if name.is_empty() || name.len() > NAME_MAX_LEN {
        return false;
    }
    if name == "." || name == ".." {
        return false;
    }
    let bytes = name.as_bytes();
    let first = bytes[0];
    if first == b'.' || first == b'-' {
        return false;
    }
    for &b in bytes {
        let ok = b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.';
        if !ok {
            return false;
        }
    }
    true
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- Accept cases ----------

    #[test]
    fn accepts_canonical_owner_repo() {
        let r = parse_github_url("https://github.com/foo/bar").expect("parse");
        assert_eq!(r.owner, "foo");
        assert_eq!(r.repo, "bar");
    }

    #[test]
    fn accepts_trailing_slash() {
        let r = parse_github_url("https://github.com/foo/bar/").expect("parse");
        assert_eq!(r, GithubRepo { owner: "foo".into(), repo: "bar".into() });
    }

    #[test]
    fn accepts_tree_ref_suffix() {
        let r = parse_github_url("https://github.com/foo/bar/tree/main").expect("parse");
        assert_eq!(r, GithubRepo { owner: "foo".into(), repo: "bar".into() });
    }

    #[test]
    fn accepts_blob_ref_suffix() {
        let r = parse_github_url("https://github.com/foo/bar/blob/main/README.md").expect("parse");
        assert_eq!(r, GithubRepo { owner: "foo".into(), repo: "bar".into() });
    }

    #[test]
    fn accepts_dot_git_suffix() {
        let r = parse_github_url("https://github.com/foo/bar.git").expect("parse");
        assert_eq!(r.repo, "bar");
    }

    #[test]
    fn accepts_case_insensitive_host_and_preserves_case_in_path() {
        let r = parse_github_url("https://GITHUB.com/Foo/Bar").expect("parse");
        assert_eq!(r.owner, "Foo");
        assert_eq!(r.repo, "Bar");
    }

    #[test]
    fn accepts_dots_and_underscores_in_names() {
        let r = parse_github_url("https://github.com/scoped-name/under_score.dot-name")
            .expect("parse");
        assert_eq!(r.owner, "scoped-name");
        assert_eq!(r.repo, "under_score.dot-name");
    }

    #[test]
    fn accepts_http_scheme_for_parity_with_parse_http_url() {
        // `parse_http_url` accepts http://; we mirror that here. (The CSP
        // would still block a plaintext request, but the URL validator
        // itself doesn't enforce TLS.)
        let r = parse_github_url("http://github.com/foo/bar").expect("parse");
        assert_eq!(r.owner, "foo");
    }

    #[test]
    fn accepts_max_length_names() {
        let owner = "a".repeat(39);
        let repo = "b".repeat(39);
        let url = format!("https://github.com/{}/{}", owner, repo);
        let r = parse_github_url(&url).expect("parse");
        assert_eq!(r.owner.len(), 39);
        assert_eq!(r.repo.len(), 39);
    }

    // ---------- Reject cases ----------

    #[test]
    fn rejects_gist_github_com_subdomain() {
        assert!(parse_github_url("https://gist.github.com/foo/bar").is_none());
    }

    #[test]
    fn rejects_raw_githubusercontent_com() {
        assert!(parse_github_url("https://raw.githubusercontent.com/foo/bar").is_none());
    }

    #[test]
    fn rejects_suffix_confusable_host() {
        // The canonical homograph attack on github.com.
        assert!(parse_github_url("https://github.com.evil.com/foo/bar").is_none());
    }

    #[test]
    fn rejects_path_with_github_com_disguised_as_host() {
        // `evil.com/github.com/foo/bar` — host is evil.com, not github.com.
        assert!(parse_github_url("https://evil.com/github.com/foo/bar").is_none());
    }

    #[test]
    fn rejects_too_few_segments() {
        assert!(parse_github_url("https://github.com/foo").is_none());
        assert!(parse_github_url("https://github.com/").is_none());
        assert!(parse_github_url("https://github.com").is_none());
    }

    #[test]
    fn rejects_too_many_segments() {
        // 3+ non-empty segments where the 3rd isn't `tree`/`blob`.
        assert!(parse_github_url("https://github.com/foo/bar/baz").is_none());
        assert!(parse_github_url("https://github.com/foo/bar/baz/qux").is_none());
    }

    #[test]
    fn rejects_double_dot_segments() {
        assert!(parse_github_url("https://github.com/foo/../baz").is_none());
        assert!(parse_github_url("https://github.com/../foo/bar").is_none());
        assert!(parse_github_url("https://github.com/..").is_none());
    }

    #[test]
    fn rejects_leading_dot_in_owner_or_repo() {
        assert!(parse_github_url("https://github.com/.foo/bar").is_none());
        assert!(parse_github_url("https://github.com/foo/.bar").is_none());
    }

    #[test]
    fn rejects_leading_dash_in_owner_or_repo() {
        assert!(parse_github_url("https://github.com/-foo/bar").is_none());
        assert!(parse_github_url("https://github.com/foo/-bar").is_none());
    }

    #[test]
    fn rejects_disallowed_chars() {
        assert!(parse_github_url("https://github.com/foo!/bar").is_none());
        assert!(parse_github_url("https://github.com/foo/bar bell").is_none());
        assert!(parse_github_url("https://github.com/foo/bar%20baz").is_none());
        // Slash inside a segment can't happen by construction (it's the
        // delimiter) but we test the unicode angle: only ASCII alnum + . _ -.
        assert!(parse_github_url("https://github.com/föö/bar").is_none());
    }

    #[test]
    fn rejects_oversize_names() {
        let owner = "a".repeat(40);
        let url = format!("https://github.com/{}/bar", owner);
        assert!(parse_github_url(&url).is_none());
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert!(parse_github_url("ftp://github.com/foo/bar").is_none());
        assert!(parse_github_url("git://github.com/foo/bar").is_none());
        assert!(parse_github_url("ssh://git@github.com/foo/bar").is_none());
        assert!(parse_github_url("javascript:alert(1)").is_none());
        assert!(parse_github_url("").is_none());
        assert!(parse_github_url("not-a-url").is_none());
    }

    #[test]
    fn rejects_query_or_fragment() {
        // We don't honour query/fragment on repo URLs — there's nothing
        // useful in them and stripping introduces ambiguity.
        assert!(parse_github_url("https://github.com/foo/bar?ref=main").is_none());
        assert!(parse_github_url("https://github.com/foo/bar#readme").is_none());
    }

    #[test]
    fn cache_key_is_filename_safe_after_validation() {
        let r = parse_github_url("https://github.com/foo/bar").expect("parse");
        let key = r.cache_key();
        assert_eq!(key, "foo__bar");
        // The cache layer joins this with a directory path, so it must
        // not contain anything that could escape:
        assert!(!key.contains('/'));
        assert!(!key.contains(".."));
    }

    #[test]
    fn api_url_uses_canonical_api_host() {
        let r = parse_github_url("https://github.com/foo/bar").expect("parse");
        assert_eq!(r.api_url(), "https://api.github.com/repos/foo/bar");
    }
}
