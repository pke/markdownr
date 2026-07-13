#!/usr/bin/env bash
# Setup + run for the recents E2E flow.
#
# Recents can't be created through the UI without the native document picker, so
# a DEBUG-only hook seeds them: this script drops an empty sentinel file into the
# app's documents dir, and the hook (App.tsx, __DEV__ only) resets recents to a
# known set on the next launch.
#
# Usage:  SIM=<udid> APP_ID=<bundle-id> .maestro/recents-setup.sh
#   SIM defaults to the booted simulator; APP_ID to the dev bundle id.
set -euo pipefail

SIM="${SIM:-booted}"
APP_ID="${APP_ID:-dev.dudesoft.markdownr.dev}"

CONTAINER="$(xcrun simctl get_app_container "$SIM" "$APP_ID" data)"
touch "$CONTAINER/Documents/__uitest_seed_recents__"
echo "Seeded sentinel in $CONTAINER/Documents"

maestro test "$(dirname "$0")/09-recents.yaml"
