#!/usr/bin/env bash
# Imports the Developer ID cert, code-signs, and notarizes the .app. Builds
# unsigned when the signing secrets aren't set (see
# .claude/macos-code-signing-plan.md) - that's expected today, not an error.
# Usage: sign-macos.sh <target-triple>
#
# Secrets, passed in via env: BUILD_CERTIFICATE_BASE64, P12_PASSWORD,
# APPLE_SIGNING_IDENTITY, APPLE_API_KEY_BASE64, APPLE_API_KEY_ID,
# APPLE_API_ISSUER_ID.
set -euo pipefail

target="$1"
app=$(echo "target/$target/release/bundle/osx"/*.app)

if [ -z "${BUILD_CERTIFICATE_BASE64:-}" ] || [ -z "${P12_PASSWORD:-}" ]; then
  echo "No macOS signing credentials; building unsigned."
  exit 0
fi

# Fail here rather than let verify-macos-signing.sh report a missing staple
# after the whole packaging run: notarytool needs all three of these and
# there's no way to ask it to just skip notarization, so a partial set is a
# configuration error, not a "notarize later" situation.
notarize_set=0
notarize_missing=""
for var in APPLE_API_KEY_BASE64 APPLE_API_KEY_ID APPLE_API_ISSUER_ID; do
  if [ -n "${!var:-}" ]; then
    notarize_set=$((notarize_set + 1))
  else
    notarize_missing="$notarize_missing $var"
  fi
done
if [ "$notarize_set" -gt 0 ] && [ "$notarize_set" -lt 3 ]; then
  echo "Notarization needs all three of APPLE_API_KEY_BASE64, APPLE_API_KEY_ID and"
  echo "APPLE_API_ISSUER_ID. These are unset:$notarize_missing"
  exit 1
fi

# Seconds the imported keychain stays unlocked for signing.
keychain_unlock_timeout=21600
# Only needs to protect the keychain for this job's lifetime, so generate it
# rather than thread a secret through for a value nothing needs to remember.
keychain_password=$(openssl rand -base64 24)

certificate_path="$RUNNER_TEMP/build_certificate.p12"
keychain_path="$RUNNER_TEMP/app-signing.keychain-db"
api_key_path="$RUNNER_TEMP/AuthKey.p8"
notarize_zip="$RUNNER_TEMP/notarize.zip"

# Import signing certificate
echo -n "$BUILD_CERTIFICATE_BASE64" | base64 --decode -o "$certificate_path"

security create-keychain -p "$keychain_password" "$keychain_path"
security set-keychain-settings -lut "$keychain_unlock_timeout" "$keychain_path"
security unlock-keychain -p "$keychain_password" "$keychain_path"

security import "$certificate_path" -P "$P12_PASSWORD" -A -t cert -f pkcs12 -k "$keychain_path"
# list-keychains replaces the search list wholesale rather than appending, so
# hardcoding "login.keychain" here would drop whatever's actually on the
# list - codesign only resolves the cert's chain to the Apple root through
# keychains on this list.
security list-keychains -d user -s "$keychain_path" \
  $(security list-keychains -d user | sed 's/[" ]//g')
security set-key-partition-list -S apple-tool:,apple:,codesign: -k "$keychain_password" "$keychain_path"

# find-identity exits 0 even when it finds nothing, so assert on the output:
# otherwise a bad cert/password surfaces only later as a confusing "no
# identity found" from codesign.
identities=$(security find-identity -v -p codesigning "$keychain_path")
echo "$identities"
case "$identities" in
  *"Developer ID Application"*) ;;
  *)
    echo "Imported the certificate but no Developer ID Application identity is usable."
    exit 1
    ;;
esac

# Code sign .app
#
# --entitlements grants the hardened runtime microphone access
# (com.apple.security.device.audio-input); without it, TCC silently denies
# mic input in the signed build even though Info.plist's
# NSMicrophoneUsageDescription is set.
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
codesign --deep --force --options runtime \
  --entitlements "$script_dir/voice-pitch-feedback.entitlements" \
  --keychain "$keychain_path" \
  --sign "$APPLE_SIGNING_IDENTITY" "$app"

if [ "$notarize_set" -eq 0 ]; then
  echo "No notarization credentials: signed without notarization."
  exit 0
fi

# Notarize .app
echo -n "$APPLE_API_KEY_BASE64" | base64 --decode -o "$api_key_path"
ditto -c -k --keepParent "$app" "$notarize_zip"

xcrun notarytool submit "$notarize_zip" \
  --key "$api_key_path" \
  --key-id "$APPLE_API_KEY_ID" \
  --issuer "$APPLE_API_ISSUER_ID" \
  --wait

xcrun stapler staple "$app"
