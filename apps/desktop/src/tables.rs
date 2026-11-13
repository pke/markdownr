//! Emoji, HTML entity and LaTeX symbol tables. On Windows they ship
//! LZMS-compressed (`data/tables.bin`, built by `scripts/pack-data.ps1`) and
//! are unpacked once with the OS's own cabinet.dll, so the exe carries no
//! decompressor.

use std::sync::OnceLock;

static TABLES: OnceLock<[&'static str; 3]> = OnceLock::new();

/// Packed `\n name \t emoji \n` lines.
pub fn emoji() -> &'static str {
    tables()[0]
}

/// Packed `\n name \t value \n` lines of HTML entities (for pulldown-cmark).
pub fn entities() -> &'static str {
    tables()[1]
}

/// Packed `\n tag name \t char flag \n` lines of LaTeX symbols (math::symbols).
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
    String::from_utf8(out).unwrap_or_default()
}

#[cfg(not(windows))]
fn unpack() -> String {
    [include_str!("../data/emoji.txt"), "\0", include_str!("../data/entities.txt"), "\0", include_str!("../data/math.txt")].concat()
}

#[cfg(test)]
mod tests {
    #[test]
    fn unpacks() {
        assert!(super::emoji().contains("\nheart_eyes\t😍\n"));
        assert!(super::entities().contains("\nnbsp\t\u{A0}\n"));
        assert!(super::math().contains("\ncalpha\tαo\n"));
    }
}
