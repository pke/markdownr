//! LaTeX name tables and Unicode math alphanumerics.
//!
//! All names (commands, environments, colors) live in one table, data/math.txt,
//! as lines `name \t kind payload`, sorted bytewise by name (checked by a test).
//! The kind byte says what the name is; the payload chars depend on it:
//!
//! | kind              | meaning                     | payload                                    |
//! |-------------------|-----------------------------|--------------------------------------------|
//! | `o b r ( ) p i`   | symbol of class Ord, Bin, Rel, Open, Close, Punct, Inner | char, then `d` if it is also a `\left`/`\right` delimiter |
//! | `B`               | big operator (`\sum`)       | char, `1` if integral                      |
//! | `A`               | accent (`\hat`)             | combining char, `1` if stretchy (`\widehat`) |
//! | `S`               | space (`\,`, `\quad`)       | index into `parse::SPACES`                 |
//! | `F`               | fraction                    | `0` `\frac`, `1` `\binom`                  |
//! | `R`               | `\sqrt`                     |                                            |
//! | `T`               | text (`\text`, `\textbf`)   | font digit, class (`o`; none = Op)         |
//! | `M`               | math font (`\mathbb`)       | font digit (order of `parse::Font`)        |
//! | `L`               | `\left`                     |                                            |
//! | `G`               | `\big(` etc.                | size index, class (`o ( )`)                |
//! | `H`               | over/underline, -brace      | `0` overline `1` underline `2` overbrace `3` underbrace |
//! | `E` `C` `X` `N`   | `\begin`, `\color`, `\not`, ignored (`\limits`, `\displaystyle`) | |
//! | `Z` `Y`           | function name with limits (`\lim`) / without (`\sin`) |          |
//! | `V`               | environment                 | left, right (`.` none), `1` cases / `2` array spec |
//! | `K`               | color name                  | six hex digits                             |

use super::parse::{Class, Font};

/// Payload (kind byte and following chars) of `name`, or "" if it is not in the table.
fn find(name: &str) -> &'static str {
    let t = crate::tables::math();
    let b = t.as_bytes();
    let key = name.as_bytes();
    if key.is_empty() || key.contains(&b'\n') || key.contains(&b'\t') {
        return "";
    }
    // Binary search over the lines; `lo` and `hi` are always line starts.
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
            core::cmp::Ordering::Equal => return t.get(start + tab + 1..end).unwrap_or(""),
        }
    }
    ""
}

/// Kind byte (0 if unknown) and the first three payload chars ('\0' if missing).
pub fn lookup(name: &str) -> (u8, [char; 3]) {
    let mut it = find(name).chars();
    let kind = it.next().unwrap_or('\0') as u8;
    (kind, [(); 3].map(|_| it.next().unwrap_or('\0')))
}

/// Class of a symbol kind byte (`o b r ( ) p i`).
pub fn class(kind: u8) -> Option<Class> {
    use Class::*;
    Some(match kind {
        b'o' => Ord,
        b'b' => Bin,
        b'r' => Rel,
        b'(' => Open,
        b')' => Close,
        b'p' => Punct,
        b'i' => Inner,
        _ => return None,
    })
}

pub fn class_of(c: char) -> Class {
    match c {
        '+' | '-' | '*' => Class::Bin,
        '=' | '<' | '>' | ':' => Class::Rel,
        '(' | '[' => Class::Open,
        ')' | ']' => Class::Close,
        ',' | ';' => Class::Punct,
        '!' | '?' => Class::Close,
        _ => Class::Ord,
    }
}

/// Map an ASCII letter/digit to a Unicode mathematical alphanumeric symbol.
pub fn styled(c: char, font: Font) -> char {
    let (upper, lower, digit): (u32, u32, Option<u32>) = match font {
        Font::Upright => return c,
        Font::Italic => (0x1D434, 0x1D44E, None),
        Font::Bold => (0x1D400, 0x1D41A, Some(0x1D7CE)),
        Font::BoldItalic => (0x1D468, 0x1D482, Some(0x1D7CE)),
        Font::Script => (0x1D49C, 0x1D4B6, None),
        Font::Fraktur => (0x1D504, 0x1D51E, None),
        Font::DoubleStruck => (0x1D538, 0x1D552, Some(0x1D7D8)),
        Font::SansSerif => (0x1D5A0, 0x1D5BA, Some(0x1D7E2)),
        Font::Mono => (0x1D670, 0x1D68A, Some(0x1D7F6)),
    };
    // Letters that live in the Letterlike Symbols block instead.
    let hole = match (font, c) {
        (Font::Italic, 'h') => Some('ℎ'),
        (Font::Script, 'B') => Some('ℬ'),
        (Font::Script, 'E') => Some('ℰ'),
        (Font::Script, 'F') => Some('ℱ'),
        (Font::Script, 'H') => Some('ℋ'),
        (Font::Script, 'I') => Some('ℐ'),
        (Font::Script, 'L') => Some('ℒ'),
        (Font::Script, 'M') => Some('ℳ'),
        (Font::Script, 'R') => Some('ℛ'),
        (Font::Script, 'e') => Some('ℯ'),
        (Font::Script, 'g') => Some('ℊ'),
        (Font::Script, 'o') => Some('ℴ'),
        (Font::Fraktur, 'C') => Some('ℭ'),
        (Font::Fraktur, 'H') => Some('ℌ'),
        (Font::Fraktur, 'I') => Some('ℑ'),
        (Font::Fraktur, 'R') => Some('ℜ'),
        (Font::Fraktur, 'Z') => Some('ℨ'),
        (Font::DoubleStruck, 'C') => Some('ℂ'),
        (Font::DoubleStruck, 'H') => Some('ℍ'),
        (Font::DoubleStruck, 'N') => Some('ℕ'),
        (Font::DoubleStruck, 'P') => Some('ℙ'),
        (Font::DoubleStruck, 'Q') => Some('ℚ'),
        (Font::DoubleStruck, 'R') => Some('ℝ'),
        (Font::DoubleStruck, 'Z') => Some('ℤ'),
        _ => None,
    };
    if let Some(h) = hole {
        return h;
    }
    let code = match c {
        'A'..='Z' => upper + (c as u32 - 'A' as u32),
        'a'..='z' => lower + (c as u32 - 'a' as u32),
        '0'..='9' => match digit {
            Some(d) => d + (c as u32 - '0' as u32),
            None => return c,
        },
        _ => return c,
    };
    char::from_u32(code).unwrap_or(c)
}

pub fn negate(c: char) -> char {
    match c {
        '=' => '≠',
        '∈' => '∉',
        '⊂' => '⊄',
        '⊆' => '⊈',
        '≤' => '≰',
        '≥' => '≱',
        '∼' => '≁',
        '≡' => '≢',
        '<' => '≮',
        '>' => '≯',
        c => c,
    }
}

/// `\color` argument: `#rrggbb` or a name from the table (`K` entries); gray if unknown.
pub fn color(name: &str) -> [u8; 3] {
    let name = name.trim();
    let hex = match name.strip_prefix('#') {
        Some(hex) => hex,
        None => find(name).strip_prefix('K').unwrap_or(""),
    };
    match u32::from_str_radix(hex, 16) {
        Ok(v) => [(v >> 16) as u8, (v >> 8) as u8, v as u8],
        Err(_) => [0x80; 3],
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn table_sorted_and_found() {
        let t = crate::tables::math();
        let names: Vec<&str> = t.lines().filter(|l| !l.is_empty()).map(|l| l.split('\t').next().unwrap()).collect();
        assert!(names.len() > 300);
        for w in names.windows(2) {
            assert!(w[0].as_bytes() < w[1].as_bytes(), "math.txt not sorted/unique at {:?}", w);
        }
        for n in &names {
            assert_ne!(super::lookup(n).0, 0, "{n}");
        }
        assert_eq!(super::lookup("alphaa").0, 0);
        assert_eq!(super::lookup("").0, 0);
        assert_eq!(super::lookup("\t").0, 0);
        assert_eq!(super::lookup("pmatrix"), (b'V', ['(', ')', '0']));
        assert_eq!(super::color("violet"), [0x82, 0x50, 0xdf]);
        assert_eq!(super::color(" orange "), [0xd9, 0x7b, 0x00]);
        assert_eq!(super::color("#123456"), [0x12, 0x34, 0x56]);
        assert_eq!(super::color("pmatrix"), [0x80; 3]);
    }
}
