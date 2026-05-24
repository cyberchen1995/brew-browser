# Building brew-browser

## Dev build

```sh
npm install
npm run tauri dev
```

Opens the app with HMR. No signing, no notarization — local development only.

## Release build (signed + notarized .dmg)

The release build produces a `.dmg` that's signed with the Developer ID Application certificate and notarized by Apple — no Gatekeeper warning on install.

### Prerequisites (one-time)

1. **Apple Developer ID Application certificate** installed in your login keychain. Verify:
   ```sh
   security find-identity -v -p codesigning
   ```
   You should see your `Developer ID Application: <name> (TEAMID)` identity. If not, create one at <https://developer.apple.com/account/resources/certificates/list>.

2. **App-specific password** generated at <https://appleid.apple.com> → Sign-In and Security → App-Specific Passwords. Label it `brew-browser-notarization` (or anything memorable).

3. **Apple credentials** in a local env file the build will source. **This file MUST NOT be committed.**

   Create `~/.config/brew-browser/signing.env` (or wherever you prefer — outside the repo):
   ```sh
   # NEVER commit this file. .gitignore covers any .env in the repo root,
   # but the best place is outside the repo entirely.

   export APPLE_ID="your@email.com"
   export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"   # app-specific password
   export APPLE_TEAM_ID="XXXXXXXXXX"             # 10-char team ID

   # Optional — only if you have multiple Developer ID certs and need to be explicit.
   # Tauri normally picks the right one from tauri.conf.json's bundle.macOS.signingIdentity.
   # export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
   ```

   Then `chmod 600 ~/.config/brew-browser/signing.env` so it's user-only readable.

### Build it

```sh
# Source the signing env (one-shot, current shell only)
source ~/.config/brew-browser/signing.env

# One command — runs the full flow (compile + sign + notarize-app + notarize-dmg + staple + verify)
./tools/build/sign-and-notarize.sh
```

Output: `src-tauri/target/release/bundle/dmg/brew-browser_<version>_aarch64.dmg` — signed, notarized, stapled.

### What the wrapper does (and why a wrapper exists)

Under the hood it runs:

```sh
npm run tauri build                                 # compile + sign + notarize .app
xcrun notarytool submit "$DMG" --wait …             # notarize the .dmg wrapper too
xcrun stapler staple "$DMG"                         # staple the ticket
spctl --assess --type install --verbose=4 "$DMG"    # verify
```

**Why the second `notarytool submit` is needed:** Tauri's bundler correctly notarizes the `.app` inside the `.dmg`, but does NOT notarize the `.dmg` wrapper itself. macOS Gatekeeper assesses the `.dmg` first when a user downloads it — so an un-notarized `.dmg` still triggers warnings even though the app inside is fine. Submitting + stapling the `.dmg` separately closes the gap. (Known Tauri 2.x behavior as of 2026-05.)

Full round-trip is ~5–15 min. Subsequent builds with no code changes can be faster — Apple's notary caches by binary hash.

If notarization fails, the wrapper prints the notary log URL. Read it; the failure reason is usually obvious (entitlement mismatch, unsigned helper binary, network blip). Re-run after fixing.

### Verify the signed `.dmg`

```sh
DMG=src-tauri/target/release/bundle/dmg/brew-browser_0.1.0_aarch64.dmg

# Code signature
codesign -dv --verbose=4 "$DMG"

# Gatekeeper assessment — should say "accepted" with source "Notarized Developer ID"
spctl --assess --type install --verbose=4 "$DMG"

# Notarization ticket is stapled?
xcrun stapler validate "$DMG"
```

All three should pass cleanly.

## Why the env file lives outside the repo

The signing identity (cert name + team ID) is **public** — it's embedded in every signed binary you distribute, anyone can read it with `codesign -dv`. Committing it in `tauri.conf.json` is fine.

The **app-specific password** is a credential. It can be regenerated easily, but you don't want it in git history. Tauri reads it from env vars at build time only — it never ends up in the binary.

If you ever do commit it by accident: regenerate at appleid.apple.com immediately. The old password is invalidated on regenerate.

## GitHub OAuth App (one-time setup before release)

brew-browser's GitHub integration uses the OAuth Device Flow (RFC 8628). The `client_id` is hardcoded in `src-tauri/src/github/auth.rs` and is **not a secret** — Device Flow client IDs are identifiers, not credentials (§3.1 of the RFC).

A placeholder `client_id` ships with the source tree (`Iv1.PLACEHOLDER_REPLACE_BEFORE_RELEASE`). The placeholder will fail loudly on the first sign-in attempt; you must replace it with a real GitHub App's `client_id` before publishing a release build.

### One-time steps

1. Visit <https://github.com/settings/apps> and click **New GitHub App**.
2. Fill in:
   - **Name:** `brew-browser` (or your fork's name)
   - **Homepage URL:** `https://brew-browser.zerologic.com` (or your project's URL)
   - **Callback URL:** leave blank — Device Flow doesn't need one.
   - **Webhook:** uncheck "Active" — we don't receive any.
3. In the **Device Flow** section, check **Enable Device Flow**.
4. In **Permissions**, grant the absolute minimum:
   - **Repository permissions** → **Metadata:** Read-only (required for any auth).
   - Anything else needed by Phase 12f (issues, stars) — refer to the spec when that wave lands.
5. Submit. GitHub shows the new app's `Client ID` (a string like `Iv23licABCXYZ123`).
6. Open `src-tauri/src/github/auth.rs` and replace the value of the `GITHUB_OAUTH_CLIENT_ID` constant.
7. **Do NOT** generate a client secret — Device Flow doesn't use one.

### Forking

If you fork brew-browser, repeat the steps above against your own GitHub account. Don't reuse the upstream `client_id` — it ties any sign-in attempts (and the resulting rate-limit consumption) back to the upstream maintainer's OAuth app.

## Unsigned builds (for testing only)

If you just want to test the build pipeline without notarization:

```sh
# Unset to skip signing entirely
unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID
npm run tauri build
```

Produces an unsigned `.dmg`. Gatekeeper will warn users on first launch ("developer cannot be verified"). For your own testing, fine; for distribution, never.
