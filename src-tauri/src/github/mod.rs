//! GitHub integration — Phase 12c (anonymous repo stats) + Phase 12e
//! (Device Flow OAuth + Keychain token storage).
//!
//! ## Layout
//!
//! - [`url`] — the strict `parse_github_url` validator. The single
//!   chokepoint that decides whether a package's `homepage` is a real
//!   `github.com/<owner>/<repo>` we're willing to talk to. The rules
//!   are pinned by ~20 unit tests; if you find yourself thinking
//!   "couldn't we just accept this one extra shape?" — don't. Re-read
//!   `memory-bank/scans/phase12-security-review.md` §12c first.
//!
//! - [`stats`] — anonymous `GET /repos/{owner}/{repo}` with a 24 h
//!   disk cache. Honours `state.require_network` and the
//!   `settings.github_enabled` opt-in toggle. If the Keychain holds a
//!   token, the request goes out with `Authorization: Bearer …` and the
//!   per-IP rate limit jumps 60 → 5000/hr.
//!
//! - [`auth`] — Device Flow polling per RFC 8628 plus Keychain
//!   storage of the resulting access token. Token never crosses the
//!   IPC boundary — only the derived `{signed_in, username, scopes}`
//!   status does. Hardcoded `client_id` (Device Flow client IDs are
//!   not secret; see RFC 8628 §3.1).
//!
//! ## Security gates (canonical list)
//!
//! Every gate below is exercised by at least one unit test in this
//! module. Loosening any of them requires updating
//! `memory-bank/scans/phase12-security-review.md` first.
//!
//! 1. URL allowlist (exact host `github.com`, strict owner+repo regex,
//!    no `..` segments) → `url::parse_github_url`.
//! 2. CSP enumeration of allowed origins → `tauri.conf.json`. Both
//!    `https://api.github.com` (stats + authed actions) and
//!    `https://github.com` (OAuth) ship together so a runtime flip of
//!    the opt-in toggle never needs a process restart.
//! 3. Runtime opt-in gate (Settings → GitHub → "Show GitHub stats")
//!    → `commands::github` consults `settings.github_enabled` before
//!    every outbound call.
//! 4. Paranoid-mode gate (Settings → Network → "Block all outbound
//!    network calls") → `state.require_network("github_*")` as the
//!    first line of every command.
//! 5. Token never crosses IPC. The DTO returned to the frontend is
//!    `GithubStatusDto { signed_in, username, scopes }` only. Verified
//!    by `auth::tests::status_dto_never_serializes_token`.
//! 6. Token never written to disk. Keychain failure surfaces
//!    `BrewError::KeychainUnavailable` with **no fallback**.
//! 7. Token never logged. `Token` is a newtype with a redacted `Debug`
//!    impl; `clippy::print_*` + `dbg_macro` are denied in `auth.rs`.
//! 8. Service identifier matches the Tauri bundle identifier. Test
//!    parses `tauri.conf.json` and asserts equality so a renamed
//!    bundle can't silently orphan stored tokens.
//! 9. OAuth scopes are exactly `read:user` + `public_repo` — no
//!    others. Pinned by `auth::tests::oauth_scopes_are_minimum`.
//! 10. Device-flow polling honours server `interval` and doubles on
//!     `slow_down` per RFC 8628 §3.5.

#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]

pub mod auth;
pub mod stats;
pub mod url;

// Re-exports used by `commands::github` and tests. The `Token`,
// `KEYCHAIN_*`, and `GITHUB_OAUTH_*` items are exported even when
// callers don't currently use them so external code (and future
// Phase 12f work) can reach them without re-importing from sub-mods.
#[allow(unused_imports)]
pub use auth::{
    DeviceFlowStart, GithubStatusDto, PollResult, PollResultDto, Token, GITHUB_OAUTH_CLIENT_ID,
    GITHUB_OAUTH_SCOPES, KEYCHAIN_ACCOUNT_TOKEN, KEYCHAIN_SERVICE,
};
pub use stats::{fetch_repo_stats, RepoStats};
#[allow(unused_imports)]
pub use url::{parse_github_url, GithubRepo};
