#!/usr/bin/env bash
# Build the optimized Android release APK and publish it to the GitHub release
# for the current version tag. The website links the stable asset name
# (releases/latest/download/markdownr.apk), so no site changes are needed.
#
# Prereqs: android/ prebuilt (npx expo prebuild -p android), gh CLI
# authenticated as the account that owns the repo (pke for this project:
# `gh auth switch --user pke`), upload keystore configured in
# ~/.gradle/gradle.properties (UPLOAD_*).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep VERSION_MAJOR version.properties | cut -d= -f2).$(grep VERSION_MINOR version.properties | cut -d= -f2).$(grep VERSION_PATCH version.properties | cut -d= -f2)"
TAG="v$VERSION"

ACCOUNT="$(gh api user --jq .login 2>/dev/null || echo unknown)"
if [ "$ACCOUNT" != "pke" ]; then
  echo "gh is authenticated as '$ACCOUNT' — run: gh auth switch --user pke" >&2
  exit 1
fi

echo "== build =="
(cd android && ./gradlew :app:assembleRelease)

APK="$(ls android/app/build/outputs/apk/release/*.apk | head -1)"
[ -n "$APK" ] || { echo "no APK built" >&2; exit 1; }
echo "built: $APK"

echo "== verify signature is the upload key (not androiddebugkey) =="
BT="$(ls -d "$HOME"/Library/Android/sdk/build-tools/* | sort -V | tail -1)"
"$BT/apksigner" verify --print-certs "$APK" | head -2
if "$BT/apksigner" verify --print-certs "$APK" | grep -q "Android Debug"; then
  echo "APK is debug-signed — configure UPLOAD_* in ~/.gradle/gradle.properties" >&2
  exit 1
fi

echo "== publish to GitHub release $TAG =="
STABLE="$(mktemp -d)/markdownr.apk"
cp "$APK" "$STABLE"
if gh release view "$TAG" > /dev/null 2>&1; then
  gh release upload "$TAG" "$APK" "$STABLE" --clobber
else
  gh release create "$TAG" "$APK" "$STABLE" \
    --title "Markdownr $VERSION" \
    --notes-file fastlane/metadata/en-GB/release_notes.txt
fi
echo "https://github.com/pke/markdownr/releases/tag/$TAG"
