#!/usr/bin/env bash
# Imports the Developer ID cert, code-signs, and notarizes the .app.
# Usage: sign-macos.sh <target-triple>
#
# Secrets, passed in via env: BUILD_CERTIFICATE_BASE64, P12_PASSWORD,
# KEYCHAIN_PASSWORD, APPLE_SIGNING_IDENTITY, APPLE_API_KEY_BASE64,
# APPLE_API_KEY_ID, APPLE_API_ISSUER_ID.
set -euo pipefail

target="$1"

# Seconds the imported keychain stays unlocked for signing.
keychain_unlock_timeout=21600

app=$(echo "target/$target/release/bundle/osx"/*.app)
certificate_path="$RUNNER_TEMP/build_certificate.p12"
keychain_path="$RUNNER_TEMP/app-signing.keychain-db"
api_key_path="$RUNNER_TEMP/AuthKey.p8"
notarize_zip="$RUNNER_TEMP/notarize.zip"

# Import signing certificate
echo -n "$BUILD_CERTIFICATE_BASE64" | base64 --decode -o "$certificate_path"

security create-keychain -p "$KEYCHAIN_PASSWORD" "$keychain_path"
security set-keychain-settings -lut "$keychain_unlock_timeout" "$keychain_path"
security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$keychain_path"

security import "$certificate_path" -P "$P12_PASSWORD" -A -t cert -f pkcs12 -k "$keychain_path"
security list-keychains -d user -s "$keychain_path" login.keychain
security set-key-partition-list -S apple-tool:,apple: -k "$KEYCHAIN_PASSWORD" "$keychain_path"

# Code sign .app
codesign --deep --force --options runtime \
  --keychain "$keychain_path" \
  --sign "$APPLE_SIGNING_IDENTITY" "$app"

# Notarize .app
echo -n "$APPLE_API_KEY_BASE64" | base64 --decode -o "$api_key_path"
ditto -c -k --keepParent "$app" "$notarize_zip"

xcrun notarytool submit "$notarize_zip" \
  --key "$api_key_path" \
  --key-id "$APPLE_API_KEY_ID" \
  --issuer "$APPLE_API_ISSUER_ID" \
  --wait

xcrun stapler staple "$app"
