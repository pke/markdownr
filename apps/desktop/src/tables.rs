//! Emoji, HTML entity and LaTeX symbol tables and the help texts. They ship front coded
//! and compressed and are unpacked once with the system's own decompressor, so the binary
//! carries none: LZMS with cabinet.dll on Windows (`data/tables.bin`), zlib with libz on
//! macOS and Linux (`data/tables.z`); both built by `scripts/pack-data.ps1`.

use std::sync::OnceLock;

static TABLES: OnceLock<[&'static str; 4]> = OnceLock::new();

/// Value of `name` in a table of `\n name \t value \n` lines sorted bytewise by
/// name (emoji, math), or "" if absent. Binary search; `lo` and `hi` stay line starts.
pub fn find(table: &'static str, name: &str) -> &'static str {
    let b = table.as_bytes();
    let key = name.as_bytes();
    if key.is_empty() || key.contains(&b'\n') || key.contains(&b'\t') {
        return "";
    }
    let (mut lo, mut hi) = (0, b.len());
    while lo < hi {
        let mut start = lo + (hi - lo) / 2;
        while start > lo && b[start - 1] != b'\n' {
            start -= 1;
        }
        let mut end = start;
        while end < b.len() && b[end] != b'\n' {
            end += 1;
        }
        let line = &b[start..end];
        let tab = line.iter().position(|&c| c == b'\t').unwrap_or(line.len());
        match line[..tab].cmp(key) {
            core::cmp::Ordering::Less => lo = end + 1,
            core::cmp::Ordering::Greater => hi = start,
            core::cmp::Ordering::Equal => return table.get(start + tab + 1..end).unwrap_or(""),
        }
    }
    ""
}

/// Sorted `\n name \t emoji \n` lines.
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

/// The F1 help texts in `language` (two-letter code; English when there are none), one per
/// line, in the order `view::HELP` shows them.
pub fn help(language: &str) -> &'static str {
    help_block(language).or_else(|| help_block("en")).unwrap_or("")
}

fn help_block(language: &str) -> Option<&'static str> {
    let all = tables()[3];
    let start = all.split_inclusive('\n').scan(0, |at, l| {
        let here = *at;
        *at += l.len();
        Some((here, l))
    });
    let (at, line) = start.filter(|(_, l)| l.starts_with('#')).find(|(_, l)| l[1..].trim_end() == language)?;
    let body = &all[at + line.len()..];
    Some(&body[..body.find("\n#").map_or(body.len(), |i| i + 1)])
}

/// The help's languages (FileCushion's).
#[cfg(test)]
pub const HELP_LANGUAGES: [&str; 9] = ["de", "en", "es", "fr", "ja", "ko", "pl", "ru", "zh"];

fn tables() -> &'static [&'static str; 4] {
    TABLES.get_or_init(|| {
        let all: &'static str = Box::leak(unpack().into_boxed_str());
        let mut parts = all.split('\0');
        [(); 4].map(|_| parts.next().unwrap_or(""))
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
/// tab, value, newline; tables separated by `\0`) back to `\n key \t value \n` lines. The
/// fourth table (help texts, in display order) is stored as is.
fn expand(front: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(front.len() * 5 / 4);
    for (i, table) in front.split(|&b| b == 0).enumerate() {
        if i > 0 {
            out.push(0);
        }
        if i == 3 {
            out.extend_from_slice(table);
            continue;
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

/// macOS and Linux: `data/tables.z` (same text, zlib) unpacked with the system's libz.
#[cfg(not(windows))]
fn unpack() -> String {
    #[cfg_attr(target_os = "macos", link(name = "z"))]
    unsafe extern "C" {
        fn uncompress(dst: *mut u8, dst_len: *mut core::ffi::c_ulong, src: *const u8, src_len: core::ffi::c_ulong) -> i32;
    }
    static PACKED: &[u8] = include_bytes!("../data/tables.z");
    let len = u32::from_le_bytes([PACKED[0], PACKED[1], PACKED[2], PACKED[3]]) as usize;
    let mut out = vec![0u8; len];
    let mut size = len as core::ffi::c_ulong;
    // SAFETY: zlib with valid buffers and lengths.
    let ok = unsafe { uncompress(out.as_mut_ptr(), &mut size, PACKED[4..].as_ptr(), (PACKED.len() - 4) as core::ffi::c_ulong) } == 0;
    out.truncate(if ok { size as usize } else { 0 });
    String::from_utf8(expand(&out)).unwrap_or_default()
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
        for (code, text) in super::HELP_LANGUAGES.iter().zip(super::HELP_TEXTS) {
            assert_eq!(super::help(code), text, "{code}");
        }
        assert_eq!(super::help("xx"), super::help("en"));
    }
}

#[cfg(test)]
const HELP_TEXTS: [&str; 9] = [
    include_str!("../data/help/de.txt"), include_str!("../data/help/en.txt"), include_str!("../data/help/es.txt"),
    include_str!("../data/help/fr.txt"), include_str!("../data/help/ja.txt"), include_str!("../data/help/ko.txt"),
    include_str!("../data/help/pl.txt"), include_str!("../data/help/ru.txt"), include_str!("../data/help/zh.txt"),
];
