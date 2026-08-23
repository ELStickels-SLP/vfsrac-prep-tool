# macOS code signing + notarization plan

Status: Apple Developer Program account created, awaiting verification.
CI wiring (step 6 below) is implemented in
[.github/workflows/release.yml](../.github/workflows/release.yml), calling
[.github/scripts/sign-macos.sh](../.github/scripts/sign-macos.sh), which
checks `BUILD_CERTIFICATE_BASE64`/`P12_PASSWORD` itself and exits early,
unsigned, when they're absent - so unsigned builds keep working today.
Remaining work is entirely account/secrets setup (steps 1-5) plus the final
verification pass (step 7).

Reference: https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications

## Why this is needed

The `.app` built by `cargo bundle` in
[.github/workflows/release.yml](../.github/workflows/release.yml) is
currently unsigned. Unsigned/unnotarized apps downloaded from the internet
get quarantined by macOS Gatekeeper — users see "Apple could not verify
this app is free of malware" and have to right-click → Open (or clear the
quarantine attribute) to launch it at all.

Two separate things are needed:
1. **Code signing** — a "Developer ID Application" certificate, applied via
   `codesign`.
2. **Notarization** — submitting the signed app to Apple's notary service
   (`notarytool`), then stapling the ticket to the app so it works offline.
   Signing alone is not enough to satisfy Gatekeeper for apps distributed
   outside the App Store; notarization is required too.

## Setup steps

1. **Wait for Apple Developer Program verification** (in progress). This
   can take a few days, sometimes longer for individual enrollments.

2. **Create a "Developer ID Application" certificate**
   - Via Xcode (Settings → Accounts → Manage Certificates → +) or the
     [Apple Developer portal](https://developer.apple.com/account/resources/certificates/list).
   - This is distinct from "Apple Development" or "Mac App Store"
     certificates — must be "Developer ID Application" for
     outside-the-App-Store distribution.

3. **Export the certificate + private key as a `.p12` file**
   - From Keychain Access: select the cert, export as `.p12`, set a
     password (this becomes `P12_PASSWORD` below).
   - Or from the command line, once the cert + key are in the login
     keychain:
     ```sh
     security export -k login.keychain-db \
       -t identities \
       -f pkcs12 \
       -P "your-p12-password" \
       -o cert.p12
     ```
     `-t identities` is required (exports the cert + matching private key
     together — `-t certs` alone omits the key). If the keychain has
     multiple identities, `security export` with no filter exports all of
     them into one `.p12`; to isolate the Developer ID one, find its hash
     via `security find-identity -v -p codesigning` and export from a
     temporary keychain containing only that identity.

4. **Base64-encode the `.p12` and add GitHub secrets** (following the
   GitHub doc linked above):
   - `BUILD_CERTIFICATE_BASE64` — `base64 -i cert.p12 | pbcopy`
   - `P12_PASSWORD` — the export password from step 3
   - `APPLE_SIGNING_IDENTITY` — the certificate's full common name, e.g.
     `Developer ID Application: <name> (<team id>)` (find via
     `security find-identity -v -p codesigning` once the cert is imported
     locally, or from the Apple Developer portal)

5. **Apple ID app-specific password or API key, for notarization**
   - Simplest: an app-specific password generated at
     [appleid.apple.com](https://appleid.apple.com) (Account → App-Specific
     Passwords), used with `xcrun notarytool` via Apple ID + team ID.
   - More robust for CI: an **App Store Connect API key** (`.p8` file +
     Key ID + Issuer ID), which doesn't expire the way app-specific
     passwords tied to 2FA sessions can. Prefer this if setting up for the
     long term.
   - GitHub secrets needed (API key approach):
     - `APPLE_API_KEY_BASE64` (base64 of the `.p8`)
     - `APPLE_API_KEY_ID`
     - `APPLE_API_ISSUER_ID`

6. **CI integration** — done. The `macos-latest` job in
   [.github/workflows/release.yml](../.github/workflows/release.yml) now has,
   between the dylib-path fixup and the final `dist/` packaging step:
   - "Add microphone usage description (macos)" — always runs (needs no
     secrets); adds `NSMicrophoneUsageDescription` to `Info.plist` via
     `PlistBuddy`, since `cargo bundle` has no config surface for arbitrary
     plist keys. The app isn't sandboxed, so no entitlements plist beyond
     hardened runtime is needed.
   - "Sign and notarize .app (macos)" — runs
     [sign-macos.sh](../.github/scripts/sign-macos.sh), which exits early
     (unsigned) if `BUILD_CERTIFICATE_BASE64`/`P12_PASSWORD` aren't set, and
     fails loudly rather than silently skipping if notarization secrets are
     only partially set. When credentials are present: imports the `.p12`
     into a temporary keychain (`security create-keychain`, `security
     import`, `security list-keychains`, `security set-key-partition-list`),
     asserts a Developer ID identity actually came out of the import, signs
     with `codesign --deep --force --options runtime --sign
     "$APPLE_SIGNING_IDENTITY" "$app"`, then notarizes via `xcrun notarytool
     submit ... --wait` and `xcrun stapler staple`.
   - "Verify signing (macos)" — runs
     [verify-macos-signing.sh](../.github/scripts/verify-macos-signing.sh),
     which asserts the built app is actually signed by
     `APPLE_SIGNING_IDENTITY` (not merely that the prior step exited 0) and
     validates the staple when notarization credentials were supplied. This
     exists because a signing step silently degrading to unsigned/ad-hoc on
     some path, without failing the job, is exactly the failure mode that
     ships an artifact Gatekeeper then refuses.
   - "Package .app (macos)" — always runs (the pre-existing `ditto` step,
     now separated out so it runs after stapling regardless of whether
     signing was gated off).

   All steps after dylib fixup are ordered so signing is the last thing
   touching the bundle's contents, per the reasoning above.

7. **Verify**: download the final zip on a clean Mac (or one that hasn't
   built/run the app before), confirm `spctl -a -vvv --type execute
   voicePitchFeedback.app` reports `accepted` and `source=Notarized
   Developer ID`.

## Open questions to resolve before starting

- Whether to use app-specific-password or API-key auth for notarytool
  (leaning API key for CI stability).
- Team ID and exact certificate common name (available once the Apple
  Developer account clears verification).
- Whether `cargo bundle`'s generated `Info.plist`/entitlements need
  anything added for hardened runtime + microphone access (this app
  processes live audio input, so an entitlement/usage-description check is
  worth doing here regardless of signing).
