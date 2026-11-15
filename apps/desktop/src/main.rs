//! Markdownr: a fast, lean Markdown reader.
//!
//! Shared core: `ast` (pulldown-cmark → own tree), `transforms`, `layout`
//! (platform-neutral block layout behind a `Typesetter` trait) and `view`
//! (scrolling, selection, keys). Platform shells render with the native text
//! stack: Direct2D + DirectWrite on Windows (Core Text / Pango to follow).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// No std runtime start-up (lang_start: thread name, stack guard, its abort messages): the C
// runtime calls `main` below directly.
#![cfg_attr(all(windows, not(test)), no_main)]
// Shell-specific helpers (touch gestures, the compositor overlay) are used by the Windows shell only.
#![cfg_attr(not(windows), allow(dead_code))]

mod app;
mod ast;
mod highlight;
mod layout;
mod math;
mod paint;
mod particles;
mod settings;
mod tables;
mod theme;
mod transforms;
mod util;
mod view;

#[cfg(windows)]
mod win;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod mac;

const SAMPLE: &str = include_str!("../assets/sample.md");

#[cfg(all(windows, not(test)))]
#[unsafe(no_mangle)]
extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    start();
    0
}

#[cfg(any(not(windows), test))]
fn main() {
    start();
}

fn start() {
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    #[cfg(windows)]
    win::run(path, SAMPLE);
    #[cfg(target_os = "linux")]
    linux::run(path, SAMPLE);
    #[cfg(target_os = "macos")]
    mac::run(path, SAMPLE);
}
