#!/bin/sh
# Smallest release build for Linux and macOS, as scripts/release-small.ps1 for Windows:
# nightly std rebuilt for size, panics abort immediately (no panic messages or location
# strings), no Debug formatting.
# Output: target/<target>/release/markdownr (Linux), target/<target>/release/Markdownr.app (macOS)
set -e
cd "$(dirname "$0")/.."
EXTRA=
case "$(uname -s)" in
    # macOS: constant data in __DATA instead of its own 16 KB-aligned __DATA_CONST segment, and
    # chained fixups instead of lazy-binding stubs: -16.5 KB.
    Darwin) TARGET=${TARGET:-$(uname -m | sed 's/arm64/aarch64/')-apple-darwin}
        EXTRA="-Clink-arg=-Wl,-no_data_const -Clink-arg=-Wl,-fixup_chains" ;;
    *) TARGET=${TARGET:-$(uname -m)-unknown-linux-gnu} ;;
esac
RUSTFLAGS="-Zunstable-options -Cpanic=immediate-abort -Zlocation-detail=none -Zfmt-debug=none $EXTRA" \
cargo +nightly build --release \
    -Z build-std=std,panic_abort \
    -Z build-std-features=optimize_for_size \
    --target "$TARGET"
BIN=target/$TARGET/release/markdownr
if [ "$(uname -s)" = Darwin ]; then
    sh scripts/mac-bundle.sh "$BIN" "target/$TARGET/release/Markdownr.app"
fi
ls -l "$BIN"
