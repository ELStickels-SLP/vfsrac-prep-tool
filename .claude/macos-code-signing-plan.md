# macOS code signing + notarization plan

Status: Apple Developer Program account created, awaiting verification.
Notes for once the account is active.

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

4. **Base64-encode the `.p12` and add GitHub secrets** (following the
   GitHub doc linked above):
   - `BUILD_CERTIFICATE_BASE64` — `base64 -i cert.p12 | pbcopy`
   - `P12_PASSWORD` — the export password from step 3
   - `KEYCHAIN_PASSWORD` — any throwaway password, used only to protect the
     temporary CI keychain for the duration of the job

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

6. **CI integration** — insert steps into the `macos-latest` job in
   [.github/workflows/release.yml](../.github/workflows/release.yml),
   between "Fix up dylib paths and package .app (macos)" and "Upload
   artifact" (i.e. sign after `dylibbundler` has finished rewriting the
   bundle's dylib paths — signing must be the last thing that touches the
   bundle's contents, since re-signing is needed if anything changes the
   binary/frameworks afterward):
   - Import the certificate into a temporary keychain (per the GitHub doc's
     recipe: `security create-keychain`, `security import`, add to the
     search list, unlock it).
   - `codesign --deep --force --options runtime --sign "Developer ID Application: <name> (<team id>)" "$app"`
     - `--options runtime` (hardened runtime) is required for notarization
       to succeed.
     - May need an entitlements plist if the app needs specific
       capabilities (audio input via `com.apple.security.device.audio-input`
       if sandboxed — check whether `cargo bundle`'s output is sandboxed;
       if not sandboxed, entitlements are likely unnecessary beyond the
       hardened runtime).
   - Zip the `.app` (notarization submission wants a zip, not a raw bundle)
     and submit: `xcrun notarytool submit app.zip --key <path-to-p8> --key-id <id> --issuer <issuer-id> --wait`
   - On success: `xcrun stapler staple "$app"` to attach the notarization
     ticket, so Gatekeeper can verify offline.
   - Continue with the existing `ditto` step to produce the final
     distributable zip (staple before this, so the stapled ticket is
     included in what ships).

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
