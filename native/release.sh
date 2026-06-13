#!/bin/bash
# native/release.sh — produce SIGNED, NOTARIZED, Sparkle-ready releases of the
# native app for BOTH arches (separate arm64 and x86_64 builds — deliberately
# NOT a universal binary), then generate the appcast. Run on the maintainer's
# Mac — the one holding the Apple "Developer ID Application" cert AND the
# Sparkle private key in its login Keychain (created once by `generate_keys`;
# its public half is the SUPublicEDKey baked into build-app.sh's Info.plist).
#
# x86_64 native covers ONLY the four Intel Macs that run macOS 26
# (MBP 16" 2019, MBP 13" 2020 4-port, iMac 27" 2020, Mac Pro 2019).
#
# Flow: per arch [ build(release, arch) → Developer ID sign (hardened runtime)
#       → zip BrewBrowser-$VERSION-<arch>.zip → notarize → staple → re-zip ]
#       → generate_appcast ONCE over $OUT_DIR (zips only; signs each update
#       with the Sparkle private key) → appcast.xml → per-arch dmgs.
#       Then you upload the zips + appcast.xml to the path behind the public
#       SUFeedURL (brew-browser.zerologic.com/...).
#
# Ordering is load-bearing: generate_appcast scans $OUT_DIR and treats every
# archive it finds as an update — so the dmgs (first-install download, not a
# Sparkle artifact) are built AFTER the appcast AND live in $OUT_DIR/dmg/, a
# subdir generate_appcast never scans. That also fixes the old bug where dmgs
# left in $OUT_DIR polluted the NEXT release's appcast scan.
#
# Required env:
#   DEVELOPER_ID_APP   e.g. "Developer ID Application: Your Name (TEAMID)"
#   NOTARY_PROFILE     a notarytool keychain profile name; create once with:
#                        xcrun notarytool store-credentials NOTARY_PROFILE \
#                          --apple-id you@example.com --team-id TEAMID --password <app-specific-pw>
# Optional env:
#   DOWNLOAD_URL_PREFIX  base URL the appcast enclosures point at
#                        (default: https://brew-browser.zerologic.com/native/)
#   OUT_DIR              where artifacts land (default: native/dist)
#
# NOTE: bump CFBundleShortVersionString + CFBundleVersion in build-app.sh before
# each release, or the appcast won't advertise a newer version. No private host
# names live here — only the public domain.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"

: "${DEVELOPER_ID_APP:?set DEVELOPER_ID_APP to your 'Developer ID Application: …' identity}"
# Notarization credentials: reuse the same Apple ID / app-specific password /
# team ID that the Tauri build uses (from ~/.config/brew-browser/signing.env),
# rather than a separate notarytool keychain profile. Either works; this keeps
# one credential source for both shells.
: "${APPLE_ID:?set APPLE_ID (source ~/.config/brew-browser/signing.env)}"
: "${APPLE_PASSWORD:?set APPLE_PASSWORD (app-specific pw; source signing.env)}"
: "${APPLE_TEAM_ID:?set APPLE_TEAM_ID (source signing.env)}"
DOWNLOAD_URL_PREFIX="${DOWNLOAD_URL_PREFIX:-https://brew-browser.zerologic.com/native/}"
OUT_DIR="${OUT_DIR:-$HERE/dist}"

SPARKLE_BIN="$HERE/.build/artifacts/sparkle/Sparkle/bin"
APP="$HERE/BrewBrowser.app"
[ -x "$SPARKLE_BIN/generate_appcast" ] || { echo "Sparkle tools missing — run 'swift build' first."; exit 1; }

ARCHES=(arm64 x86_64)
DMG_DIR="$OUT_DIR/dmg"
mkdir -p "$OUT_DIR" "$DMG_DIR"

# Stapled per-arch .apps are kept here until the dmgs are built — build-app.sh
# overwrites BrewBrowser.app on every run, so the arm64 app must survive the
# x86_64 build (and both must survive until after generate_appcast).
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

VERSION=""
for arch in "${ARCHES[@]}"; do
  echo "==> [$arch] build (release) + assemble"
  ./build-app.sh release "$arch"

  V="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
  if [ -z "$VERSION" ]; then
    VERSION="$V"
    echo "==> version $VERSION"
  elif [ "$V" != "$VERSION" ]; then
    echo "version mismatch across arches ($VERSION vs $V)"; exit 1
  fi

  # Developer ID sign, inside-out, with the hardened runtime (notarization needs
  # it). Nested Sparkle code (XPC services, Updater.app, Autoupdate) + the bundled
  # SwiftPM resource bundles must be signed before the framework / app that contain
  # them. The ad-hoc signature build-app.sh applied is replaced here.
  echo "==> [$arch] Developer ID sign"
  SIGN=(codesign --force --options runtime --timestamp --sign "$DEVELOPER_ID_APP")
  FW="$APP/Contents/Frameworks/Sparkle.framework"
  if [ -d "$FW" ]; then
    find "$FW" -type d \( -name "*.xpc" -o -name "*.app" \) -print0 \
      | xargs -0 -I{} "${SIGN[@]}" "{}"
    [ -f "$FW/Versions/Current/Autoupdate" ] && "${SIGN[@]}" "$FW/Versions/Current/Autoupdate"
    "${SIGN[@]}" "$FW"
  fi
  for b in "$APP"/Contents/Resources/*.bundle; do [ -e "$b" ] && "${SIGN[@]}" "$b"; done
  "${SIGN[@]}" "$APP"
  codesign --verify --deep --strict --verbose=2 "$APP"

  ZIP="$OUT_DIR/BrewBrowser-$VERSION-$arch.zip"
  echo "==> [$arch] zip $ZIP"
  rm -f "$ZIP"
  ditto -c -k --keepParent "$APP" "$ZIP"

  echo "==> [$arch] notarize (waits for Apple)"
  xcrun notarytool submit "$ZIP" --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" --team-id "$APPLE_TEAM_ID" --wait
  echo "==> [$arch] staple"
  xcrun stapler staple "$APP"
  rm -f "$ZIP"                     # re-zip so the download carries the ticket
  ditto -c -k --keepParent "$APP" "$ZIP"

  # Park the stapled .app for the dmg pass (after generate_appcast).
  mkdir -p "$WORK/$arch"
  cp -R "$APP" "$WORK/$arch/BrewBrowser.app"
done

# Defensive: relocate any stray .dmg sitting in $OUT_DIR's root into $DMG_DIR
# before scanning. generate_appcast treats a .dmg and a .zip of the SAME bundle
# version as duplicate updates and aborts (SUSparkleErrorDomain 1002). Legacy
# releases (e.g. 0.1.0) left their .dmg in $OUT_DIR root, which collides with
# that version's .zip on the next run. The current convention keeps dmgs in
# $DMG_DIR (never scanned); sweep any old ones there too so the scan is clean.
shopt -s nullglob
for stray in "$OUT_DIR"/*.dmg; do
  echo "==> moving stray dmg out of appcast scan dir: $(basename "$stray") -> dmg/"
  mv "$stray" "$DMG_DIR/"
done

# Sparkle's generate_appcast (current tooling) treats two archives that share a
# CFBundleVersion as DUPLICATE updates and aborts (SUSparkleErrorDomain 1002) —
# it does NOT coexist same-version arches in one feed via hardwareRequirements.
# Apple Silicon is the primary platform and Intel macOS is sunsetting (macOS 28
# drops Rosetta), so the Sparkle AUTO-UPDATE feed is arm64-only: sideline any
# non-arm64 (x86_64) zip out of the scan dir before generating. Both arches'
# .dmgs are still built below for FIRST-INSTALL download — only the auto-update
# feed is arm64. (If Intel auto-update is ever needed: dual per-arch feeds with
# a per-arch SUFeedURL baked into Info.plist — see git history of this script.)
SIDELINE_DIR="$OUT_DIR/no-feed"
mkdir -p "$SIDELINE_DIR"
for nonarm in "$OUT_DIR"/*-x86_64.zip; do
  echo "==> sidelining non-arm64 zip from arm64 feed: $(basename "$nonarm") -> no-feed/"
  mv "$nonarm" "$SIDELINE_DIR/"
done
shopt -u nullglob

# generate_appcast runs ONCE over $OUT_DIR, which at this point holds ONLY the
# arm64 .zips (this release + prior arm64 history) — non-arm64 zips were
# sidelined above, and the dmgs don't exist yet (they land in $DMG_DIR, which
# generate_appcast never scans).
echo "==> generate appcast (arm64 feed; signs with the Sparkle private key in your Keychain)"
"$SPARKLE_BIN/generate_appcast" --download-url-prefix "$DOWNLOAD_URL_PREFIX" "$OUT_DIR"

# Disk images for FIRST-INSTALL download (humans prefer a .dmg; Sparkle uses the
# .zips above for auto-updates). Built from the stapled per-arch .apps, staged
# with an /Applications symlink for drag-to-install, then Developer-ID signed +
# notarized + stapled — same treatment as the Tauri build's .dmg.
for arch in "${ARCHES[@]}"; do
  DMG="$DMG_DIR/BrewBrowser-$VERSION-$arch.dmg"
  echo "==> [$arch] dmg $DMG"
  STAGE="$WORK/dmg-$arch"
  mkdir -p "$STAGE"
  cp -R "$WORK/$arch/BrewBrowser.app" "$STAGE/"
  ln -s /Applications "$STAGE/Applications"
  rm -f "$DMG"
  hdiutil create -volname "Brew Browser" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null
  codesign --force --timestamp --sign "$DEVELOPER_ID_APP" "$DMG"
  echo "==> [$arch] notarize dmg (waits for Apple)"
  xcrun notarytool submit "$DMG" --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" --team-id "$APPLE_TEAM_ID" --wait
  xcrun stapler staple "$DMG"
done

# Post-run sanity: the auto-update feed is arm64-only (Intel macOS is sunsetting
# and Sparkle won't coexist same-version arches in one feed), so the appcast
# must carry exactly the arm64 enclosure for this version. Counted here so a
# bad/empty feed never ships silently.
APPCAST="$OUT_DIR/appcast.xml"
ITEM_COUNT="$(grep -c "BrewBrowser-$VERSION-arm64.zip" "$APPCAST" 2>/dev/null || true)"

echo
echo "==> done."
echo "   Post-run checklist:"
echo "   [ ] appcast.xml advertises the arm64 build of $VERSION (found $ITEM_COUNT arm64 enclosure(s))"
echo "   [ ] spot-check: grep -A3 \"BrewBrowser-$VERSION-arm64.zip\" \"$APPCAST\""
if [ "${ITEM_COUNT:-0}" -lt 1 ]; then
  echo
  echo "   ############################################################################"
  echo "   ## WARNING: appcast.xml does NOT advertise the arm64 build of $VERSION.    "
  echo "   ## generate_appcast may have failed or the arm64 zip was missing/sidelined.##"
  echo "   ## Do NOT ship this appcast as-is — investigate before publishing.        ##"
  echo "   ############################################################################"
fi
echo
echo "   Upload to the host (path behind $DOWNLOAD_URL_PREFIX) — Sparkle auto-update feed:"
ls -1 "$OUT_DIR"/BrewBrowser-"$VERSION"-*.zip "$APPCAST"
echo "   (appcast.xml must be served at the SUFeedURL in build-app.sh's Info.plist)"
echo "   Attach to the GitHub release (first-install downloads, per arch):"
ls -1 "$DMG_DIR"/BrewBrowser-"$VERSION"-*.dmg
