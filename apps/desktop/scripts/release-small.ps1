# Smallest release build: nightly std rebuilt for size, panics abort immediately
# (no panic messages or location strings), no Debug formatting.
# Output: target\x86_64-pc-windows-msvc\release\markdownr.exe
$env:RUSTFLAGS = "-Zunstable-options -Cpanic=immediate-abort -Zlocation-detail=none -Zfmt-debug=none"
cargo +nightly build --release `
    -Z build-std=std,panic_abort `
    -Z build-std-features=optimize_for_size `
    --target x86_64-pc-windows-msvc
Remove-Item Env:RUSTFLAGS
