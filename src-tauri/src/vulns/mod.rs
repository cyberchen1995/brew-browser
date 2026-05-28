//! v0.5.0 — opt-in vulnerability scanning for installed Homebrew
//! formulae.
//!
//! Three submodules collaborate:
//!
//! - [`client`] — subprocess wrapper around the official `brew vulns`
//!   subcommand from `Homebrew/homebrew-brew-vulns`. Queries OSV.dev
//!   via the GIT ecosystem (source repo URL + version tag).
//! - [`cache`] — per-package LRU+TTL cache persisted to
//!   `<app_data_dir>/vulns_cache.json`. Crash-safe via the established
//!   `util::fs::atomic_write` helper. Empty-vec entries mean
//!   "scanned, clean" — distinct from "no record".
//! - [`fingerprint`] — SHA-256 over the sorted install set. Powers
//!   the whole-scan skip optimization: if the install set hasn't
//!   changed since the last full scan AND that scan is within TTL,
//!   skip the subprocess invocation entirely.
//!
//! Distinct trust boundary from the always-on Homebrew endpoints:
//! `brew vulns` is an official Homebrew subcommand but it talks to
//! `api.osv.dev` (Google) and indirectly to the source forges
//! (github.com, gitlab.com, codeberg.org) for version-tag resolution.
//! Gated end-to-end by [`crate::state::AppState::require_vulnerability_scanning`]
//! which composes the master Offline Mode switch with the per-feature
//! `vulnerability_scanning_enabled` toggle.

pub mod cache;
pub mod client;
pub mod enrich;
pub mod fingerprint;
