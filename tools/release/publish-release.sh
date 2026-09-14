#!/usr/bin/env bash
# publish-release.sh — push the already-built release artifacts to the web root
# that serves the auto-update feeds.
#
# This is the LAST step of a release, after the artifacts exist and after the
# GitHub release is cut. It publishes:
#
#   updater.json                  → Tauri's in-app updater feed
#   appcast.xml                   → Sparkle's feed for the native app
#   BrewBrowser-<ver>-<arch>.zip  → the native app payload the appcast points at
#   *.delta                       → Sparkle delta updates, when present
#
# It is deliberately additive and idempotent: it copies files in and fixes
# their modes, and it NEVER deletes. The same web root also serves the nightly
# trending + enrichment output from generators that write there on their own
# schedule — an `rsync --delete` would take those with it.
#
# ── Configuration (no private host names live in this file) ──────────────────
#
# Required env:
#   BREW_BROWSER_DEPLOY_HOST   ssh destination for the web host, e.g. "user@host"
#   BREW_BROWSER_WEB_ROOT      absolute path of the site root on that host
#
# Optional env:
#   BREW_BROWSER_WEB_GROUP     group that must be able to read the files
#                              (default: www-static)
#   BREW_BROWSER_SMOKE_HOST    Host: header for the on-host smoke test
#                              (default: brew-browser.zerologic.com)
#   DIST_DIR                   where updater.json lives (default: ./dist)
#   NATIVE_DIST_DIR            where appcast.xml + zips live
#                              (default: ./native/dist)
#
# Keep the real values OUT of git. Either export them from your shell profile
# or put them in a gitignored env file and source it:
#
#   # ~/.config/brew-browser/deploy.env   (chmod 0600)
#   export BREW_BROWSER_DEPLOY_HOST="user@host"
#   export BREW_BROWSER_WEB_ROOT="/srv/www/example.com"
#
#   source ~/.config/brew-browser/deploy.env
#   tools/release/publish-release.sh --tauri 0.7.3 --native 0.3.3
#
# Either version may be omitted to publish only one shell's feed.
#
# ── Why this is a script and not a handful of rsync invocations ──────────────
#
# The feeds are the one artefact where a mistake is silent and wide: a stale or
# half-published updater.json means every installed copy of the app either
# never sees the release or downloads something that doesn't match its
# signature. So this script refuses to publish a manifest whose version doesn't
# match what you said you were releasing, and it verifies the live feed on the
# host before reporting success.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

: "${BREW_BROWSER_DEPLOY_HOST:?set BREW_BROWSER_DEPLOY_HOST (ssh destination, e.g. user@host)}"
: "${BREW_BROWSER_WEB_ROOT:?set BREW_BROWSER_WEB_ROOT (absolute site root on that host)}"
WEB_GROUP="${BREW_BROWSER_WEB_GROUP:-www-static}"
SMOKE_HOST="${BREW_BROWSER_SMOKE_HOST:-brew-browser.zerologic.com}"
DIST_DIR="${DIST_DIR:-$REPO_ROOT/dist}"
NATIVE_DIST_DIR="${NATIVE_DIST_DIR:-$REPO_ROOT/native/dist}"

HOST="$BREW_BROWSER_DEPLOY_HOST"
ROOT="${BREW_BROWSER_WEB_ROOT%/}"

TAURI_VERSION=""
NATIVE_VERSION=""
DRY_RUN=0

usage() {
  cat >&2 <<EOF
usage: tools/release/publish-release.sh [--tauri X.Y.Z] [--native X.Y.Z] [--dry-run]

At least one of --tauri / --native is required.
Requires BREW_BROWSER_DEPLOY_HOST and BREW_BROWSER_WEB_ROOT in the environment.
EOF
  exit 2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tauri)   TAURI_VERSION="${2:?--tauri needs a version}"; shift 2 ;;
    --native)  NATIVE_VERSION="${2:?--native needs a version}"; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help) usage ;;
    *) echo "unknown argument: $1" >&2; usage ;;
  esac
done

[[ -n "$TAURI_VERSION" || -n "$NATIVE_VERSION" ]] || usage

say()  { printf '==> %s\n' "$*"; }
warn() { printf 'WARN: %s\n' "$*" >&2; }
die()  { printf 'FATAL: %s\n' "$*" >&2; exit 1; }

run() {
  if [[ $DRY_RUN -eq 1 ]]; then
    printf '   [dry-run] %s\n' "$*"
  else
    "$@"
  fi
}

# ── 1. Preflight: the artifacts exist and say what we think they say ─────────
#
# Checking the manifest's own version against the release being published is
# the guard that matters. Publishing last release's updater.json is invisible
# at the rsync layer and looks exactly like "no update available" to every
# installed app.

UPLOAD_ROOT=()   # files that land in the site root
UPLOAD_NATIVE=() # files that land in <root>/native/

if [[ -n "$TAURI_VERSION" ]]; then
  MANIFEST="$DIST_DIR/updater.json"
  [[ -f "$MANIFEST" ]] || die "no updater.json at $MANIFEST — run tools/release/publish-manifest.sh $TAURI_VERSION first"

  MANIFEST_VERSION="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$MANIFEST" | head -1)"
  [[ -n "$MANIFEST_VERSION" ]] || die "could not read a version out of $MANIFEST"
  [[ "$MANIFEST_VERSION" == "$TAURI_VERSION" ]] \
    || die "updater.json says version $MANIFEST_VERSION but --tauri says $TAURI_VERSION — regenerate the manifest"

  grep -q '"signature"' "$MANIFEST" \
    || die "updater.json carries no signature field — the app will reject the update"

  say "updater.json OK (version $MANIFEST_VERSION)"
  UPLOAD_ROOT+=("$MANIFEST")
fi

if [[ -n "$NATIVE_VERSION" ]]; then
  APPCAST="$NATIVE_DIST_DIR/appcast.xml"
  [[ -f "$APPCAST" ]] || die "no appcast.xml at $APPCAST — run native/release.sh first"

  grep -q "BrewBrowser-$NATIVE_VERSION-" "$APPCAST" \
    || die "appcast.xml does not mention BrewBrowser-$NATIVE_VERSION-* — it is stale for this release"
  grep -q 'sparkle:edSignature' "$APPCAST" \
    || die "appcast.xml has no edSignature — Sparkle will refuse the update"

  say "appcast.xml OK (advertises $NATIVE_VERSION)"
  UPLOAD_ROOT+=("$APPCAST")

  shopt -s nullglob
  ZIPS=("$NATIVE_DIST_DIR/BrewBrowser-$NATIVE_VERSION-"*.zip)
  DELTAS=("$NATIVE_DIST_DIR"/*.delta)
  shopt -u nullglob

  [[ ${#ZIPS[@]} -gt 0 ]] || die "no BrewBrowser-$NATIVE_VERSION-*.zip in $NATIVE_DIST_DIR"
  for z in "${ZIPS[@]}"; do say "payload $(basename "$z") ($(du -h "$z" | cut -f1))"; done
  UPLOAD_NATIVE+=("${ZIPS[@]}")

  if [[ ${#DELTAS[@]} -gt 0 ]]; then
    say "${#DELTAS[@]} delta file(s) to publish"
    UPLOAD_NATIVE+=("${DELTAS[@]}")
  fi
fi

# ── 2. Copy in. Payloads before feeds. ──────────────────────────────────────
#
# Ordering is load-bearing: publish the zips BEFORE the appcast that points at
# them, and the manifest last. A feed that advertises a payload which is still
# uploading hands users a 404 or a truncated download for the length of the
# copy. The reverse order is always safe.

say "publishing to $HOST:$ROOT"

if [[ ${#UPLOAD_NATIVE[@]} -gt 0 ]]; then
  run ssh "$HOST" "mkdir -p '$ROOT/native'"
  say "[1/3] native payloads → $ROOT/native/"
  run rsync -av --no-perms --no-owner --no-group "${UPLOAD_NATIVE[@]}" "$HOST:$ROOT/native/"
fi

if [[ ${#UPLOAD_ROOT[@]} -gt 0 ]]; then
  say "[2/3] feeds → $ROOT/"
  run rsync -av --no-perms --no-owner --no-group "${UPLOAD_ROOT[@]}" "$HOST:$ROOT/"
fi

# Serving modes. The web root is group-owned by the web group with setgid dirs,
# so new files inherit the group; the chmod is here for idempotence and for the
# case where a file arrived by some other route. Scoped to exactly the files
# this run published — never a recursive chmod of the whole root, which also
# holds nightly generator output.
say "[3/3] modes (0644, group $WEB_GROUP)"
PUBLISHED=()
for f in "${UPLOAD_ROOT[@]}";   do PUBLISHED+=("$ROOT/$(basename "$f")"); done
for f in "${UPLOAD_NATIVE[@]}"; do PUBLISHED+=("$ROOT/native/$(basename "$f")"); done
if [[ ${#PUBLISHED[@]} -gt 0 ]]; then
  run ssh "$HOST" "chgrp '$WEB_GROUP' ${PUBLISHED[*]@Q} && chmod 0644 ${PUBLISHED[*]@Q}"
fi

# ── 3. Verify on the host, through the real server ──────────────────────────
#
# Internal check with an explicit Host: header — no DNS, no edge, no CDN cache
# in the way. If this says the wrong version, the publish did not take, and
# that is worth failing on rather than discovering from a bug report.

if [[ $DRY_RUN -eq 1 ]]; then
  say "dry run — skipping verification"
  exit 0
fi

say "verifying on host (Host: $SMOKE_HOST via 127.0.0.1)"
FAILED=0

if [[ -n "$TAURI_VERSION" ]]; then
  LIVE="$(ssh "$HOST" "curl -sS --max-time 20 -H 'Host: $SMOKE_HOST' http://127.0.0.1/updater.json" \
          | sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)"
  if [[ "$LIVE" == "$TAURI_VERSION" ]]; then
    say "    updater.json → $LIVE ✓"
  else
    warn "updater.json serves '${LIVE:-<empty>}', expected $TAURI_VERSION"
    FAILED=1
  fi
fi

if [[ -n "$NATIVE_VERSION" ]]; then
  if ssh "$HOST" "curl -sS --max-time 20 -H 'Host: $SMOKE_HOST' http://127.0.0.1/appcast.xml" \
       | grep -q "BrewBrowser-$NATIVE_VERSION-"; then
    say "    appcast.xml → advertises $NATIVE_VERSION ✓"
  else
    warn "appcast.xml does not advertise $NATIVE_VERSION"
    FAILED=1
  fi

  for z in "${ZIPS[@]}"; do
    name="$(basename "$z")"
    code="$(ssh "$HOST" "curl -sS -o /dev/null -w '%{http_code}' --max-time 30 -H 'Host: $SMOKE_HOST' http://127.0.0.1/native/$name")"
    if [[ "$code" == "200" ]]; then
      say "    native/$name → HTTP 200 ✓"
    else
      warn "native/$name → HTTP $code"
      FAILED=1
    fi
  done
fi

[[ $FAILED -eq 0 ]] || die "verification failed — the feeds may be inconsistent; fix before announcing"

cat <<EOF

Published. Verify publicly from anywhere:

  curl -s https://$SMOKE_HOST/updater.json | head -3
  curl -s https://$SMOKE_HOST/appcast.xml | grep -m1 shortVersionString

Feeds are served with Cache-Control: no-cache, so installed apps see this
release on their next check without waiting out a cache.
EOF
