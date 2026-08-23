#!/usr/bin/env bash
# Asserts the .app is signed by the expected identity (and stapled, when
# notarization credentials were supplied) before the workflow packages it
# for release.
#
# This exists because a signing step can silently degrade to an unsigned or
# ad-hoc-signed artifact without failing the job - a code path that changes
# to no longer call codesign, a mismatched/expired identity string, or a
# notarization submission that succeeds without actually stapling. Gatekeeper
# then refuses the published build, but only after it's already shipped.
# Usage: verify-macos-signing.sh <target-triple> <expected-identity>
set -euo pipefail

target="$1"
expected_identity="$2"
app=$(echo "target/$target/release/bundle/osx"/*.app)

if [ -z "${BUILD_CERTIFICATE_BASE64:-}" ]; then
  echo "No signing credentials were supplied; $app ships unsigned."
  exit 0
fi

codesign --verify --deep --strict "$app"

# Substring-matched rather than piped to grep -q: grep exits on first match
# and the SIGPIPE that sends codesign becomes the pipeline's status under
# pipefail, failing this step on a correctly signed app.
signature=$(codesign -dv --verbose=2 "$app" 2>&1)
case "$signature" in
  *"Authority=$expected_identity"*) ;;
  *)
    echo "$app is signed, but not by the expected identity ($expected_identity):"
    echo "$signature"
    exit 1
    ;;
esac

if [ -n "${APPLE_API_KEY_BASE64:-}" ]; then
  xcrun stapler validate "$app"
fi

echo "Verified $app is signed by $expected_identity."
