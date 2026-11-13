//! Markdownr: a fast, lean Markdown reader.
//!
//! Shared core: `ast` (pulldown-cmark → own tree), `transforms`, `layout`
//! (platform-neutral block layout behind a `Typesetter` trait) and `view`
//! (scrolling, selection, keys). Platform shells render with the native text
//! stack: Direct2D + DirectWrite on Windows (Core Text / Pango to follow).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ast;
mod highlight;
mod layout;
mod math;
mod settings;
mod theme;
mod transforms;
mod view;

#[cfg(windows)]
mod win;

const SAMPLE: &str = include_str!("../assets/sample.md");

fn main() {
    let path = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    #[cfg(windows)]
    win::run(path, SAMPLE);
    #[cfg(not(windows))]
    {
        let _ = (path, SAMPLE);
        eprintln!("Markdownr: the macOS and Linux shells are not implemented yet.");
    }
}
