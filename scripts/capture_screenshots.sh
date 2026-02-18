#!/bin/bash
# capture_screenshots.sh — Capture App Store screenshots using Xcode Test Plans
#
# Generic tool for any iOS project. Screenshots are captured via XCTAttachment
# in UI tests and extracted from the xcresult bundle.
#
# Prerequisites:
#   - A UI test scheme with screenshot tests using XCTAttachment (.keepAlways)
#   - A test plan with configurations for each language/appearance combo
#
# Usage:
#   ./scripts/capture_screenshots.sh --scheme MyAppUITests --testplan Screenshots \
#       [--devices "iPhone 15 Pro Max,iPad Pro (12.9-inch) (6th generation)"] \
#       [--project path/to/App.xcodeproj | --workspace path/to/App.xcworkspace] \
#       [--configuration Release] \
#       [--swift-flags "DEMO_MODE"] \
#       [--output path/to/screenshots]

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# ──────────────────────────────────────────────
# Defaults (all overridable via CLI)
# ──────────────────────────────────────────────
SCHEME=""
TESTPLAN=""
CONFIGURATION="Release"
OUTPUT_DIR="$PROJECT_DIR/fastlane/screenshots"
PROJECT_PATH=""
WORKSPACE_PATH=""
SWIFT_FLAGS=""
DERIVED_DATA=$(mktemp -d "${TMPDIR}snapshot_derived.XXXXXX")
XCRESULT_PATH=$(mktemp -d "${TMPDIR}ss_xcresult.XXXXXX")/result.xcresult
ATTACHMENTS_DIR=$(mktemp -d "${TMPDIR}ss_attachments.XXXXXX")

# Default devices
DEVICES=("iPhone 15 Pro Max" "iPad Pro (12.9-inch) (6th generation)")

# ──────────────────────────────────────────────
# Parse arguments
# ──────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case $1 in
        --scheme)
            SCHEME="$2"
            shift 2
            ;;
        --testplan)
            TESTPLAN="$2"
            shift 2
            ;;
        --configuration)
            CONFIGURATION="$2"
            shift 2
            ;;
        --project)
            PROJECT_PATH="$2"
            shift 2
            ;;
        --workspace)
            WORKSPACE_PATH="$2"
            shift 2
            ;;
        --devices)
            IFS=',' read -ra DEVICES <<< "$2"
            shift 2
            ;;
        --output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --swift-flags)
            SWIFT_FLAGS="$2"
            shift 2
            ;;
        --keep-xcresult)
            KEEP_XCRESULT=1
            shift
            ;;
        --help|-h)
            sed -n '2,/^$/s/^# //p' "$0"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Run with --help for usage."
            exit 1
            ;;
    esac
done

# ──────────────────────────────────────────────
# Auto-detect project/workspace if not specified
# ──────────────────────────────────────────────
if [[ -z "$PROJECT_PATH" && -z "$WORKSPACE_PATH" ]]; then
    # Prefer workspace over project (CocoaPods, SPM workspace)
    WORKSPACE_PATH=$(find "$PROJECT_DIR" -maxdepth 1 -name "*.xcworkspace" ! -path "*/xcodeproj/*" | head -1)
    if [[ -z "$WORKSPACE_PATH" ]]; then
        PROJECT_PATH=$(find "$PROJECT_DIR" -maxdepth 1 -name "*.xcodeproj" | head -1)
    fi
fi

if [[ -z "$PROJECT_PATH" && -z "$WORKSPACE_PATH" ]]; then
    echo "ERROR: No .xcodeproj or .xcworkspace found in $PROJECT_DIR"
    echo "Specify with --project or --workspace"
    exit 1
fi

# Build the project/workspace flag for xcodebuild
if [[ -n "$WORKSPACE_PATH" ]]; then
    BUILD_TARGET_FLAG=(-workspace "$WORKSPACE_PATH")
    echo "Workspace: $WORKSPACE_PATH"
else
    BUILD_TARGET_FLAG=(-project "$PROJECT_PATH")
    echo "Project:   $PROJECT_PATH"
fi

# ──────────────────────────────────────────────
# Validate required arguments
# ──────────────────────────────────────────────
if [[ -z "$SCHEME" ]]; then
    echo "ERROR: --scheme is required"
    echo "Run with --help for usage."
    exit 1
fi

if [[ -z "$TESTPLAN" ]]; then
    echo "ERROR: --testplan is required"
    echo "Run with --help for usage."
    exit 1
fi

# Build extra xcodebuild args for swift flags
EXTRA_BUILD_ARGS=()
if [[ -n "$SWIFT_FLAGS" ]]; then
    EXTRA_BUILD_ARGS+=("SWIFT_ACTIVE_COMPILATION_CONDITIONS=$SWIFT_FLAGS \$(inherited)")
fi

cleanup() {
    echo "Cleaning up..."
    rm -rf "$DERIVED_DATA"
    rm -rf "$ATTACHMENTS_DIR"
    if [[ -z "${KEEP_XCRESULT:-}" ]]; then
        rm -rf "$(dirname "$XCRESULT_PATH")"
    else
        echo "xcresult preserved at: $XCRESULT_PATH"
    fi
}
trap cleanup EXIT

# Resolve simulator UDID + OS version for each device, build destination flags
echo "Resolving simulators..."
DESTINATION_ARGS=()
DEVICE_UDIDS=()
for i in "${!DEVICES[@]}"; do
    device=$(echo "${DEVICES[$i]}" | xargs) # trim whitespace
    DEVICES[$i]="$device"
    # Find the newest available simulator for this device name
    read -r UDID OS_VER < <(xcrun simctl list devices available -j | ruby -rjson -e '
data = JSON.parse($stdin.read)
best = nil
data["devices"].each do |runtime, devs|
  devs.each do |d|
    next unless d["name"] == ARGV[0] && d["isAvailable"]
    m = runtime.match(/(\d+[-\.]\d+(?:[-\.]\d+)?)/)
    next unless m
    ver = m[1].tr("-", ".")
    ver_tuple = ver.split(".").map(&:to_i)
    best = [d["udid"], ver, ver_tuple] if best.nil? || (ver_tuple <=> best[2]) > 0
  end
end
if best
  puts "#{best[0]} #{best[1]}"
else
  exit 1
end
' "$device" 2>/dev/null)

    if [[ -n "${UDID:-}" ]]; then
        DESTINATION_ARGS+=(-destination "platform=iOS Simulator,name=$device,OS=$OS_VER")
        DEVICE_UDIDS+=("$UDID")
        echo "  $device -> iOS $OS_VER ($UDID)"
    else
        echo "  ERROR: Could not find simulator for '$device'"
        exit 1
    fi
done

echo "=== Screenshot Capture ==="
echo "Scheme:  $SCHEME"
echo "Plan:    $TESTPLAN"
echo "Config:  $CONFIGURATION${SWIFT_FLAGS:+ (+$SWIFT_FLAGS)}"
echo "Devices: ${DEVICES[*]}"
echo "Output:  $OUTPUT_DIR"
echo ""

# ──────────────────────────────────────────────
# Step 1: Override status bars on all simulators
# ──────────────────────────────────────────────
echo "→ Step 1: Overriding simulator status bars..."
for i in "${!DEVICES[@]}"; do
    device="${DEVICES[$i]}"
    UDID="${DEVICE_UDIDS[$i]}"
    # Boot if not already booted
    xcrun simctl boot "$UDID" 2>/dev/null || true
    xcrun simctl status_bar "$UDID" override \
        --time "9:41" \
        --batteryState charged \
        --batteryLevel 100 \
        --wifiBars 3 \
        --cellularBars 4 \
        --operatorName "" 2>/dev/null || true
    echo "  Status bar set for $device ($UDID)"
done

# ──────────────────────────────────────────────
# Step 2: Build for testing (once)
# ──────────────────────────────────────────────
echo ""
echo "→ Step 2: Building for testing..."
xcodebuild build-for-testing \
    "${BUILD_TARGET_FLAG[@]}" \
    -scheme "$SCHEME" \
    -configuration "$CONFIGURATION" \
    -testPlan "$TESTPLAN" \
    -derivedDataPath "$DERIVED_DATA" \
    "${DESTINATION_ARGS[@]}" \
    ${EXTRA_BUILD_ARGS[@]+"${EXTRA_BUILD_ARGS[@]}"} \
    -quiet > /dev/null 2>&1

echo "  Build succeeded."

# ──────────────────────────────────────────────
# Step 3: Run tests (all configs × all devices)
# ──────────────────────────────────────────────
echo ""
echo "→ Step 3: Running screenshot tests..."
set +e
xcodebuild test-without-building \
    "${BUILD_TARGET_FLAG[@]}" \
    -scheme "$SCHEME" \
    -configuration "$CONFIGURATION" \
    -testPlan "$TESTPLAN" \
    -derivedDataPath "$DERIVED_DATA" \
    -resultBundlePath "$XCRESULT_PATH" \
    "${DESTINATION_ARGS[@]}" \
    -quiet > /dev/null 2>&1
TEST_EXIT=$?
set -e

if [[ $TEST_EXIT -ne 0 ]]; then
    echo "  WARNING: xcodebuild exited with code $TEST_EXIT (checking xcresult for results...)"
fi

# ──────────────────────────────────────────────
# Step 4: Export attachments from xcresult
# ──────────────────────────────────────────────
echo ""
echo "→ Step 4: Extracting screenshots from xcresult..."
xcrun xcresulttool export attachments \
    --path "$XCRESULT_PATH" \
    --output-path "$ATTACHMENTS_DIR" \
    > /dev/null 2>&1

MANIFEST="$ATTACHMENTS_DIR/manifest.json"
if [[ ! -f "$MANIFEST" ]]; then
    echo "ERROR: No manifest.json found. Test run may have failed."
    exit 1
fi

# ──────────────────────────────────────────────
# Step 5: Organize screenshots into output dir
# ──────────────────────────────────────────────
echo ""
echo "→ Step 5: Organizing screenshots..."

# Clear previous screenshots (PNGs only)
find "$OUTPUT_DIR" -name "*.png" -delete 2>/dev/null || true

# Parse manifest and copy files to correct locations
# Configuration names map to language dirs (e.g. "de-DE_dark" -> "de-DE")
# Target format: {language}/{deviceName}-{screenshotName}.png
export ATTACHMENTS_DIR OUTPUT_DIR
ruby -rjson -rfileutils << 'RUBY_SCRIPT'
attachments_dir = ENV["ATTACHMENTS_DIR"]
output_dir = ENV["OUTPUT_DIR"]
manifest = JSON.parse(File.read(File.join(attachments_dir, "manifest.json")))

count = 0
manifest.each do |test_entry|
  (test_entry["attachments"] || []).each do |att|
    config_name = att["configurationName"]       # e.g. "de-DE_dark"
    device_name = att["deviceName"]              # e.g. "iPhone 15 Pro Max"
    exported_file = att["exportedFileName"]       # UUID.png
    suggested_name = att["suggestedHumanReadableName"]  # "01_Home_dark_0_UUID.png"

    # Extract screenshot name: "{name}_{index}_{uuid}.png" — we want just {name}
    parts = suggested_name.rpartition("_").first.rpartition("_").first
    screenshot_name = parts.empty? ? suggested_name.sub(".png", "") : parts

    # "de-DE_dark" -> "de-DE", "en-US" -> "en-US"
    language = config_name.sub(/_dark$/, "")

    # Final filename: {deviceName}-{screenshotName}.png
    dest_dir = File.join(output_dir, language)
    FileUtils.mkdir_p(dest_dir)

    src = File.join(attachments_dir, exported_file)
    dst = File.join(dest_dir, "#{device_name}-#{screenshot_name}.png")
    FileUtils.cp(src, dst)
    count += 1
  end
end

puts "  Organized #{count} screenshots."
RUBY_SCRIPT

# ──────────────────────────────────────────────
# Step 6: Summary
# ──────────────────────────────────────────────
echo ""
echo "=== Screenshot Capture Complete ==="
echo ""
for lang_dir in "$OUTPUT_DIR"/*/; do
    lang=$(basename "$lang_dir")
    file_count=$(ls -1 "$lang_dir"*.png 2>/dev/null | wc -l | xargs)
    echo "  $lang: $file_count screenshots"
done
echo ""
echo "Screenshots saved to: $OUTPUT_DIR"
