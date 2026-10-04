#!/usr/bin/env bash
# Bump the latest vX.Y.Z[-rcN] git tag and apply the new tag to the current commit.
# With --rc, the new tag is a release candidate (vX.Y.Z-rcN). If the latest tag is
# an rc and no --major/--minor/--patch is given, --rc increments N, and a run
# without --rc promotes it to the final vX.Y.Z.
set -euo pipefail

bump=""
rc=false
for arg in "$@"; do
  case "$arg" in
    --major) bump=major ;;
    --minor) bump=minor ;;
    --patch) bump=patch ;;
    --rc) rc=true ;;
    *)
      echo "Usage: $0 [--major|--minor|--patch] [--rc]" >&2
      exit 1
      ;;
  esac
done

# Sort key puts a final release after its release candidates.
latest_tag=$(git tag --list 'v*.*.*' \
  | sed -nE 's/^v([0-9]+\.[0-9]+\.[0-9]+)$/\1.999999 &/p; s/^v([0-9]+\.[0-9]+\.[0-9]+)-rc([0-9]+)$/\1.\2 &/p' \
  | sort -V -k1,1 | tail -n1 | cut -d' ' -f2)
latest_tag=${latest_tag:-v0.0.0}

version=${latest_tag#v}
rc_num=""
if [[ "$version" == *-rc* ]]; then
  rc_num=${version##*-rc}
  version=${version%-rc*}
fi
IFS='.' read -r major minor patch <<< "$version"

new_rc=""
if [[ -n "$rc_num" && -z "$bump" ]]; then
  # Continue or finalize the pending release candidate series.
  if $rc; then new_rc=$((rc_num + 1)); fi
else
  case "${bump:-patch}" in
    major) major=$((major + 1)); minor=0; patch=0 ;;
    minor) minor=$((minor + 1)); patch=0 ;;
    patch) patch=$((patch + 1)) ;;
  esac
  if $rc; then new_rc=1; fi
fi

new_tag="v${major}.${minor}.${patch}${new_rc:+-rc$new_rc}"

git tag -a "$new_tag" -m "$new_tag"
echo "Tagged current commit as $new_tag (previous: $latest_tag)"
echo "Push with: git push origin $new_tag"
