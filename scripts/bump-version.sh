#!/usr/bin/env bash
# Bump the latest vX.Y.Z git tag and apply the new tag to the current commit.
set -euo pipefail

bump=patch
for arg in "$@"; do
  case "$arg" in
    --major) bump=major ;;
    --minor) bump=minor ;;
    --patch) bump=patch ;;
    *)
      echo "Usage: $0 [--major|--minor|--patch]" >&2
      exit 1
      ;;
  esac
done

latest_tag=$(git tag --list 'v*.*.*' | sort -V | tail -n1)
latest_tag=${latest_tag:-v0.0.0}

version=${latest_tag#v}
IFS='.' read -r major minor patch <<< "$version"

case "$bump" in
  major) major=$((major + 1)); minor=0; patch=0 ;;
  minor) minor=$((minor + 1)); patch=0 ;;
  patch) patch=$((patch + 1)) ;;
esac

new_tag="v${major}.${minor}.${patch}"

git tag -a "$new_tag" -m "$new_tag"
echo "Tagged current commit as $new_tag (previous: $latest_tag)"
echo "Push with: git push origin $new_tag"
