//! `brew_info` — full detail for one package (formula or cask).

use tauri::State;

use crate::brew::exec::run_brew_capture;
use crate::brew::parse::RawInfoV2;
use crate::error::{truncate_head, BrewError};
use crate::state::AppState;
use crate::types::{PackageDetail, PackageKind};

#[tauri::command]
pub async fn brew_info(
    name: String,
    kind: PackageKind,
    state: State<'_, AppState>,
) -> Result<PackageDetail, BrewError> {
    validate_package_name(&name)?;
    let path = state.require_brew_path().await?;

    let (kind_flag, display) = match kind {
        PackageKind::Formula => (
            "--formula",
            format!("brew info --json=v2 --formula {}", name),
        ),
        PackageKind::Cask => ("--cask", format!("brew info --json=v2 --cask {}", name)),
    };

    let raw = run_brew_capture(&path, &["info", "--json=v2", kind_flag, &name], &display).await?;

    let parsed: RawInfoV2 = serde_json::from_str(&raw).map_err(|e| BrewError::JsonParse {
        command: display.clone(),
        message: e.to_string(),
        raw_excerpt: truncate_head(&raw, 2048),
    })?;

    // Capture the raw JSON for the "raw" tab in the detail panel.
    let raw_value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| BrewError::JsonParse {
            command: display.clone(),
            message: e.to_string(),
            raw_excerpt: truncate_head(&raw, 2048),
        })?;

    let mut detail = match kind {
        PackageKind::Formula => {
            let f = parsed
                .formulae
                .into_iter()
                .next()
                .ok_or_else(|| BrewError::Internal {
                    message: format!("brew returned no formula entry for {}", name),
                })?;
            f.to_detail(raw_value)
        }
        PackageKind::Cask => {
            let c = parsed
                .casks
                .into_iter()
                .next()
                .ok_or_else(|| BrewError::Internal {
                    message: format!("brew returned no cask entry for {}", name),
                })?;
            c.to_detail(raw_value)
        }
    };

    // Feature #4 — lazily size the installed keg (one `du -sk` on the
    // resolved keg dir). Only when the package is actually installed; an
    // uninstalled package skips `du` entirely and keeps `None`. Uses the
    // short `package.name` (Cellar/Caskroom dirs key on the short name,
    // never the tap-qualified full_name) and sums all installed versions.
    detail.installed_size_bytes = compute_installed_size(&path, &detail, &state).await;

    Ok(detail)
}

/// Resolve the Homebrew prefix (cached `brew_env.prefix` when present,
/// else a `brew --prefix` shell-out) and size the package's keg via the
/// shared `du -sk` helper. Returns `None` when the package isn't installed,
/// the prefix can't be resolved, the keg dir is absent (e.g. a cask on
/// Linux), or `du` errored — never a fabricated value.
async fn compute_installed_size(
    brew: &std::path::Path,
    detail: &PackageDetail,
    state: &State<'_, AppState>,
) -> Option<u64> {
    // Cheap bail-out before touching the filesystem: not installed → no size.
    let installed_version = detail.package.installed_version.as_deref()?;

    // Prefer the cached prefix; fall back to `brew --prefix`.
    let prefix = {
        let cached = state.brew_env.read().await.prefix.clone();
        match cached.filter(|s| !s.is_empty()) {
            Some(p) => p,
            None => run_brew_capture(brew, &["--prefix"], "brew --prefix")
                .await
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())?,
        }
    };

    crate::commands::disk_usage::installed_size_bytes(
        std::path::Path::new(&prefix),
        &detail.package.name,
        detail.package.kind,
        Some(installed_version),
    )
    .await
}

/// Stricter validator for cask tokens that reach the filesystem.
///
/// `validate_package_name` exists to prevent argv flag injection and shell
/// metacharacter injection — it accepts `/` and `.` because legitimate
/// tap-qualified formula names contain them (`homebrew/core/wget`,
/// `python@3.14`). But anywhere a token is composed directly into a
/// filesystem path (`<cache_dir>/icons/<token>.png`) those characters
/// can compose into a directory traversal (`../../etc/passwd`) that
/// escapes the cache root.
///
/// Cask tokens in practice are `[a-z0-9._-]` — they never contain `/`,
/// never start with `.`, and the empty/parent-dir components `""` and
/// `..` are never valid. This validator enforces those tighter rules on
/// top of `validate_package_name`.
///
/// Apply this validator **before** any code path that builds a path from
/// the token — including before constructing cache file paths, not just
/// before shelling out to `brew`. See `cask_icon` and
/// `cask_icon_from_homepage` for the wiring.
pub fn validate_cask_token(token: &str) -> Result<(), BrewError> {
    validate_package_name(token)?;
    // Cask tokens never contain path separators.
    if token.contains('/') {
        return Err(BrewError::InvalidArgument {
            message: format!("cask token may not contain '/': {:?}", token),
        });
    }
    // No leading `.` (would hide cache files / collide with `.` and `..`).
    if token.starts_with('.') {
        return Err(BrewError::InvalidArgument {
            message: format!("cask token may not start with '.': {:?}", token),
        });
    }
    // Reject `.` and `..` outright (filesystem parent / current dir).
    if token == "." || token == ".." {
        return Err(BrewError::InvalidArgument {
            message: format!("cask token may not be a directory component: {:?}", token),
        });
    }
    // Reject empty or `.` segments between dots — `foo..bar` collapses
    // to two non-empty segments with an empty middle, which is the same
    // wire shape as a path-component `..`. Defense in depth in case a
    // future caller composes `<token>` into a larger path.
    for seg in token.split('.') {
        if seg.is_empty() || seg == "." || seg == ".." {
            return Err(BrewError::InvalidArgument {
                message: format!(
                    "cask token contains an empty or dot-only segment: {:?}",
                    token
                ),
            });
        }
    }
    Ok(())
}

/// Reject obviously-malicious package names. brew's own validation
/// will catch the rest, but this prevents flag injection at the
/// command-construction layer.
pub fn validate_package_name(name: &str) -> Result<(), BrewError> {
    if name.is_empty() {
        return Err(BrewError::InvalidArgument {
            message: "package name is empty".into(),
        });
    }
    if name.starts_with('-') {
        return Err(BrewError::InvalidArgument {
            message: format!("package name '{}' may not start with '-'", name),
        });
    }
    if name.len() > 200 {
        return Err(BrewError::InvalidArgument {
            message: "package name is too long".into(),
        });
    }
    // brew formulae are [a-z0-9._+-] plus tap prefix `user/repo/`.
    for c in name.chars() {
        if !(c.is_ascii_alphanumeric()
            || c == '-'
            || c == '_'
            || c == '.'
            || c == '+'
            || c == '/'
            || c == '@')
        {
            return Err(BrewError::InvalidArgument {
                message: format!("package name contains invalid char: {:?}", c),
            });
        }
    }
    Ok(())
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::validate_package_name;
    use crate::error::BrewError;

    fn err_message(r: Result<(), BrewError>) -> String {
        match r {
            Err(BrewError::InvalidArgument { message }) => message,
            other => panic!("expected InvalidArgument, got {:?}", other),
        }
    }

    // ---------- Happy path ----------

    #[test]
    fn accepts_simple_name() {
        validate_package_name("wget").expect("plain ascii name");
    }

    #[test]
    fn accepts_versioned_formula() {
        validate_package_name("python@3.14").expect("@version is allowed");
        validate_package_name("openssl@3").expect("@major is allowed");
    }

    #[test]
    fn accepts_tap_qualified_name() {
        validate_package_name("homebrew/core/wget").expect("tap-qualified");
        validate_package_name("user/tap/pkg").expect("third-party tap");
    }

    #[test]
    fn accepts_punctuated_names() {
        validate_package_name("gcc-13.2.0").expect("dotted/dashed");
        validate_package_name("foo_bar").expect("underscore");
        validate_package_name("c++").expect("plus");
        validate_package_name("node18").expect("digit suffix");
    }

    // ---------- Empty / oversize ----------

    #[test]
    fn rejects_empty_name() {
        let msg = err_message(validate_package_name(""));
        assert!(msg.contains("empty"), "msg was {:?}", msg);
    }

    #[test]
    fn rejects_oversize_name() {
        let huge = "a".repeat(201);
        let msg = err_message(validate_package_name(&huge));
        assert!(msg.contains("too long"), "msg was {:?}", msg);
    }

    #[test]
    fn accepts_max_size_name() {
        let big = "a".repeat(200);
        validate_package_name(&big).expect("200 chars should be allowed");
    }

    // ---------- Flag injection (leading dash) ----------

    #[test]
    fn rejects_leading_dash_injection() {
        for s in &["-rm", "--force", "-version", "-"] {
            let msg = err_message(validate_package_name(s));
            assert!(
                msg.contains("may not start with '-'"),
                "input {:?} -> msg {:?}",
                s,
                msg
            );
        }
    }

    // ---------- Shell metacharacters / path traversal / null bytes ----------

    #[test]
    fn rejects_shell_metacharacters() {
        // None of these would actually reach a shell (we use exec, not sh -c)
        // but defense in depth at the IPC boundary catches them anyway.
        for s in &[
            "foo;bar",
            "foo&&bar",
            "foo|bar",
            "$(whoami)",
            "`whoami`",
            "foo bar", // space
            "foo>out",
            "foo<in",
            "foo*",
            "foo?bar",
            "foo'bar",
            "foo\"bar",
            "foo\\bar",
        ] {
            let r = validate_package_name(s);
            assert!(
                matches!(r, Err(BrewError::InvalidArgument { .. })),
                "input {:?} should be rejected, got {:?}",
                s,
                r
            );
        }
    }

    #[test]
    fn rejects_path_traversal_attempts() {
        // `..` itself parses (since `.` and `/` are both allowed) but the
        // intent of these tests is to flag that traversal could compose
        // from valid characters. We document this in apiTests.md as a
        // potential downstream concern — `brew` itself would reject them.
        // The empty-segment, control-char forms ARE rejected:
        for s in &[
            "../etc/passwd\0", // null byte
            "foo\nbar",        // newline
            "foo\rbar",        // CR
            "foo\tbar",        // tab
        ] {
            let r = validate_package_name(s);
            assert!(
                matches!(r, Err(BrewError::InvalidArgument { .. })),
                "input {:?} should be rejected, got {:?}",
                s,
                r
            );
        }
    }

    #[test]
    fn rejects_unicode_lookalikes() {
        // Non-ASCII chars are not in the allowed set.
        let r = validate_package_name("wgét");
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
    }

    #[test]
    fn rejects_brackets_and_braces() {
        for s in &["foo[", "foo]", "foo{", "foo}", "foo(", "foo)"] {
            let r = validate_package_name(s);
            assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
        }
    }

    // ---------- Boundary: 199 vs 200 vs 201 chars ----------

    #[test]
    fn boundary_199_chars_accepted() {
        validate_package_name(&"a".repeat(199)).expect("199 chars");
    }

    #[test]
    fn boundary_200_chars_accepted() {
        validate_package_name(&"a".repeat(200)).expect("200 chars");
    }

    #[test]
    fn boundary_201_chars_rejected() {
        let r = validate_package_name(&"a".repeat(201));
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
    }

    // ---------- validate_cask_token (L1 — filesystem-safe tokens) ----------

    use super::validate_cask_token;

    #[test]
    fn cask_token_accepts_realistic_tokens() {
        validate_cask_token("firefox").expect("firefox");
        validate_cask_token("visual-studio-code").expect("dashed");
        validate_cask_token("1password").expect("digit-first");
        validate_cask_token("font-fira-code").expect("font cask");
        validate_cask_token("docker").expect("plain");
        validate_cask_token("c++").expect("plus chars ok");
    }

    #[test]
    fn cask_token_rejects_slash() {
        // tap-qualified names are fine for formulae but never for cask
        // filesystem cache paths.
        let r = validate_cask_token("homebrew/cask/firefox");
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
        let r = validate_cask_token("../etc/passwd");
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
        let r = validate_cask_token("/");
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
    }

    #[test]
    fn cask_token_rejects_dotdot() {
        // `..` would walk out of the cache directory.
        let r = validate_cask_token("..");
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
        // `..foo` would not be a traversal but starts with `.`; also reject.
        let r = validate_cask_token("..foo");
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
        // Segments after dots — `foo..bar` must be rejected because its
        // middle segment is `..`.
        let r = validate_cask_token("foo..bar");
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
    }

    #[test]
    fn cask_token_rejects_leading_dot() {
        // Hidden-file shape.
        let r = validate_cask_token(".hidden");
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
        let r = validate_cask_token(".");
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
    }

    #[test]
    fn cask_token_rejects_empty() {
        let r = validate_cask_token("");
        assert!(matches!(r, Err(BrewError::InvalidArgument { .. })));
    }

    #[test]
    fn cask_token_inherits_package_name_rejections() {
        // Anything validate_package_name rejects must also be rejected here.
        for s in &["-rf", "foo bar", "foo;bar", "$(whoami)", "foo\0bar", "wgét"] {
            let r = validate_cask_token(s);
            assert!(
                matches!(r, Err(BrewError::InvalidArgument { .. })),
                "input {:?} should be rejected by cask token validator, got {:?}",
                s,
                r
            );
        }
    }
}
