//! Emoji, HTML entity and LaTeX symbol tables. On Windows they ship
//! LZMS-compressed and front coded (`data/tables.bin`, built by `scripts/pack-data.ps1`)
//! and are unpacked once with the OS's own cabinet.dll, so the exe carries no
//! decompressor.

use std::sync::OnceLock;

static TABLES: OnceLock<[&'static str; 3]> = OnceLock::new();

/// `\n name \t emoji \n` lines.
pub fn emoji() -> &'static str {
    tables()[1]
}

/// `\n name \t value \n` lines of HTML entities (for pulldown-cmark).
pub fn entities() -> &'static str {
    tables()[0]
}

/// Sorted `\n name \t kind payload \n` lines of LaTeX names (math::symbols).
pub fn math() -> &'static str {
    tables()[2]
}

fn tables() -> &'static [&'static str; 3] {
    TABLES.get_or_init(|| {
        let all: &'static str = Box::leak(unpack().into_boxed_str());
        let mut parts = all.split('\0');
        [(); 3].map(|_| parts.next().unwrap_or(""))
    })
}

#[cfg(windows)]
fn unpack() -> String {
    use core::ffi::c_void;
    #[link(name = "cabinet")]
    unsafe extern "system" {
        fn CreateDecompressor(alg: u32, routines: *const c_void, handle: *mut isize) -> i32;
        fn Decompress(h: isize, src: *const u8, n: usize, dst: *mut u8, cap: usize, size: *mut usize) -> i32;
        fn CloseDecompressor(h: isize) -> i32;
    }
    const COMPRESS_ALGORITHM_LZMS: u32 = 5;
    static PACKED: &[u8] = include_bytes!("../data/tables.bin");
    let len = u32::from_le_bytes([PACKED[0], PACKED[1], PACKED[2], PACKED[3]]) as usize;
    let mut out = vec![0u8; len];
    let mut size = 0usize;
    // SAFETY: plain Win32 calls with valid buffers and lengths.
    let ok = unsafe {
        let mut h = 0isize;
        CreateDecompressor(COMPRESS_ALGORITHM_LZMS, core::ptr::null(), &mut h) != 0
            && {
                let r = Decompress(h, PACKED[4..].as_ptr(), PACKED.len() - 4, out.as_mut_ptr(), len, &mut size) != 0;
                CloseDecompressor(h);
                r
            }
    };
    out.truncate(if ok { size } else { 0 });
    String::from_utf8(expand(&out)).unwrap_or_default()
}

/// Front-coded lines (one byte 0x20 + bytes shared with the previous key, rest of the key,
/// tab, value, newline; tables separated by `\0`) back to `\n key \t value \n` lines.
#[cfg(windows)]
fn expand(front: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(front.len() * 5 / 4);
    for (i, table) in front.split(|&b| b == 0).enumerate() {
        if i > 0 {
            out.push(0);
        }
        out.push(b'\n');
        let mut prev = out.len();
        for line in table.split_inclusive(|&b| b == b'\n') {
            let Some((&shared, rest)) = line.split_first() else { continue };
            let start = out.len();
            out.extend_from_within(prev..prev + (shared.saturating_sub(0x20) as usize).min(start - prev));
            out.extend_from_slice(rest);
            prev = start;
        }
    }
    out
}

#[cfg(not(windows))]
fn unpack() -> String {
    [include_str!("../data/entities.txt"), "\0", include_str!("../data/emoji.txt"), "\0", include_str!("../data/math.txt")].concat()
}

#[cfg(test)]
mod tests {
    #[test]
    fn unpacks() {
        assert!(super::emoji().contains("\nheart_eyes\t😍\n"));
        assert!(super::entities().contains("\nnbsp\t\u{A0}\n"));
        assert!(super::math().contains("\nalpha\to𝛼\n"));
    }

    /// The packed tables hold exactly the lines of the data files.
    #[test]
    fn same_lines_as_sources() {
        let lines = |t: &str| {
            let mut v: Vec<String> = t.split('\n').filter(|l| !l.is_empty()).map(str::to_string).collect();
            v.sort();
            v
        };
        assert_eq!(lines(super::entities()), lines(include_str!("../data/entities.txt")));
        assert_eq!(lines(super::emoji()), lines(include_str!("../data/emoji.txt")));
        assert_eq!(super::math(), include_str!("../data/math.txt"));
    }
}
