#!/usr/bin/env bash
set -euo pipefail

expected_runtime_build="${1-}"
expected_version="${2:?Pass the release app version}"
expected_build="${3:?Pass the release app build number}"
app_id=dev.dudesoft.markdownr
project_root="$(cd "$(dirname "$0")/../.." && pwd -P)"
cd "$project_root"

maestro="$(command -v maestro || true)"
maestro="${maestro:-$HOME/.maestro/bin/maestro}"
if [[ ! -x "$maestro" ]]; then
  echo "Install Maestro to run the release E2E suite." >&2
  exit 1
fi

runtime="$(python3 modules/ui-tests/release_runtime.py "$expected_runtime_build")"
IFS=$'\t' read -r runtime_build runtime_id runtime_version device_type device_name <<< "$runtime"

artifacts="$(mktemp -d "${TMPDIR:-/tmp}/markdownr-release-tests.XXXXXX")"
artifacts="$(cd "$artifacts" && pwd -P)"
sim="$(xcrun simctl create 'Markdownr Release Tests' "$device_type" "$runtime_id")"
cleanup() {
  xcrun simctl shutdown "$sim" >/dev/null 2>&1 || true
  xcrun simctl delete "$sim" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "Testing Markdownr Release on $device_name, iOS $runtime_version ($runtime_build)"
echo "Test logs and results are in $artifacts"
xcodebuild -version
xcrun simctl bootstatus "$sim" -b
xcrun simctl spawn "$sim" defaults write NSGlobalDomain AppleLanguages -array en
xcrun simctl spawn "$sim" defaults write NSGlobalDomain AppleLocale en_US

export APP_VARIANT=production WITH_UITESTS=1 CI=1
npx expo prebuild --clean --platform ios

# Xcode can pass a symlinked path to Metro, which resolves the physical root.
ENTRY_FILE="$(node -e "require('expo/scripts/resolveAppEntry')" "$project_root" ios absolute | tail -n 1)"
export ENTRY_FILE
xcode_options=(
  -workspace "$project_root/ios/Markdownr.xcworkspace"
  -scheme Markdownr -configuration Release
  -destination "platform=iOS Simulator,id=$sim"
  -derivedDataPath "$artifacts/DerivedData"
  -parallel-testing-enabled NO
)
if ! xcodebuild build-for-testing "${xcode_options[@]}" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO > "$artifacts/build.log" 2>&1; then
  tail -n 80 "$artifacts/build.log" >&2
  exit 1
fi

app="$artifacts/DerivedData/Build/Products/Release-iphonesimulator/Markdownr.app"
version="$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$app/Info.plist")"
build="$(/usr/libexec/PlistBuddy -c 'Print CFBundleVersion' "$app/Info.plist")"
echo "Markdownr $version ($build)"
if [[ "$version" != "$expected_version" || "$build" != "$expected_build" || ! -s "$app/main.jsbundle" ]]; then
  echo "Expected Markdownr $expected_version ($expected_build) with an embedded JS bundle." >&2
  exit 1
fi
xcrun simctl install "$sim" "$app"

run_maestro() {
  local name="$1"
  shift
  "$maestro" --device "$sim" test -e "MARKDOWNR_APP_ID=$app_id" \
    --test-output-dir "$artifacts/$name" --format junit --output "$artifacts/$name.xml" "$@"
}

run_native() {
  local name="$1"
  local test="$2"
  local result="$artifacts/$name.xcresult"
  echo "Running $test"
  if ! xcodebuild test-without-building "${xcode_options[@]}" \
    -testLanguage en -testRegion US -resultBundlePath "$result" \
    -collect-test-diagnostics never \
    -only-testing:"MarkdownrUITests/$test" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO \
    > "$artifacts/$name.log" 2>&1; then
    tail -n 80 "$artifacts/$name.log" >&2
    return 1
  fi
  xcrun xcresulttool get test-results summary --path "$result" | node -e '
    const summary = JSON.parse(require("node:fs").readFileSync(0, "utf8"));
    if (summary.totalTestCount !== 1 || summary.passedTests !== 1) {
      console.error("Expected one passing test, got %d of %d.", summary.passedTests, summary.totalTestCount);
      process.exit(1);
    }
  '
}

flows=()
for flow in .maestro/[0-9][0-9]-*.yaml; do
  if [[ "$flow" != .maestro/09-recents.yaml ]]; then
    flows+=("$flow")
  fi
done
failed=0
run_maestro maestro-main "${flows[@]}" || failed=1
run_native launch 'DeepLinkUITests/testAppLaunchesAndRendersUI' || failed=1

container="$(xcrun simctl get_app_container "$sim" "$app_id" data)"
documents="$container/Documents"
recent_cache="$container/Library/Caches/recent-files"
mkdir -p "$documents"

open_document() {
  local file="$1"
  local url
  url="$(node -e 'console.log(require("node:url").pathToFileURL(process.argv[1]).href)' "$file")"
  xcrun simctl openurl "$sim" "$url"
}

app_pid() {
  xcrun simctl spawn "$sim" launchctl list | awk -v label="UIKitApplication:${app_id}[" '
    index($3, label) == 1 && $1 ~ /^[0-9]+$/ { print $1 }
  '
}

cp modules/ui-tests/fixtures/deeplink-fixture.md "$documents/Cold.md"
printf '# Warm Deep Link Heading\n' > "$documents/Warm.md"
if [[ -n "$(app_pid)" ]]; then
  xcrun simctl terminate "$sim" "$app_id"
fi
open_document "$documents/Cold.md"
run_native cold-document 'DeepLinkUITests/testDeepLinkRendersOpenedFile' || failed=1
xcrun simctl launch "$sim" "$app_id"
pid_before_open="$(app_pid)"
open_document "$documents/Warm.md"
if [[ -z "$pid_before_open" || "$pid_before_open" != "$(app_pid)" ]]; then
  echo "The warm document handoff changed the Markdownr process." >&2
  exit 1
fi
run_native warm-document 'DeepLinkUITests/testWarmDeepLinkRendersOpenedFile' || failed=1

seed_recents() {
  local title file deadline cached saved
  for title in Alpha Beta Gamma; do
    file="$documents/$title.md"
    printf '# %s\n\nRelease test fixture.\n' "$title" > "$file"
    open_document "$file"
    deadline=$((SECONDS + 20))
    saved=false
    while ((SECONDS < deadline)); do
      for cached in "$recent_cache/"*.md; do
        if cmp -s "$file" "$cached"; then
          saved=true
          break
        fi
      done
      [[ "$saved" == true ]] && break
      sleep 1
    done
    if [[ "$saved" != true ]]; then
      echo "Opening $title did not create a recent file." >&2
      return 1
    fi
  done
}

seed_recents
run_maestro maestro-recents .maestro/09-recents.yaml || failed=1
seed_recents
run_native swipe-to-delete 'RecentsUITests/testSwipeToDeleteRecentFile' || failed=1
if ((failed)); then
  echo "Release E2E suite failed. Results: $artifacts" >&2
  exit 1
fi
echo "Release E2E suite passed. Results: $artifacts"
