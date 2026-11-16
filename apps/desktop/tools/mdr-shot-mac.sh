#!/bin/sh
# Starts markdownr on macOS, optionally sends it keys (tools/mdr-keys-mac.js, posted to its pid
# only), and captures only the app's own window (screencapture -l with the window id from
# CGWindowList, owner "markdownr"); never the desktop.
#   KEYS="cmd+t pgdn" WAIT=3 SETTLE=1 tools/mdr-shot-mac.sh <binary> <out.png> [args...]
BIN=$1; OUT=$2; shift 2
DIR=$(dirname "$0")
pkill -x markdownr 2>/dev/null; sleep 0.3
"$BIN" "$@" >/tmp/mdr-run.log 2>&1 &
PID=$!
sleep "${WAIT:-3}"
if [ -n "$KEYS" ]; then
    # shellcheck disable=SC2086
    osascript -l JavaScript "$DIR/mdr-keys-mac.js" "$PID" $KEYS >/dev/null || exit 1
    sleep "${SETTLE:-1}"
fi
W=$(osascript -l JavaScript -e '
ObjC.import("CoreGraphics");
const l = ObjC.castRefToObject($.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, 0));
let id = 0, area = 0;
for (let i = 0; i < l.count; i++) { const w = ObjC.deepUnwrap(l.objectAtIndex(i));
  if (w.kCGWindowOwnerName !== "markdownr" || w.kCGWindowLayer !== 0) continue;
  const a = w.kCGWindowBounds.Width * w.kCGWindowBounds.Height; if (a > area) { area = a; id = w.kCGWindowNumber; } }
id;')
if [ -z "$W" ] || [ "$W" = 0 ]; then echo "no markdownr window"; cat /tmp/mdr-run.log; exit 1; fi
screencapture -x -o -l"$W" "$OUT" && echo "shot: $OUT (window $W)"
[ -n "$KEEP" ] || kill "$PID" 2>/dev/null
