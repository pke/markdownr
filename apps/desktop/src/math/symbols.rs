//! LaTeX command → Unicode tables and Unicode math alphanumerics.

use super::parse::{Class, Font};

pub const FUNCS: &[&str] = &[
    "sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh", "cosh", "tanh", "coth",
    "log", "ln", "lg", "exp", "deg", "dim", "ker", "hom", "arg", "gcd", "Pr", "det", "mod", "bmod",
];
pub const LIMIT_FUNCS: &[&str] = &["lim", "liminf", "limsup", "max", "min", "sup", "inf", "argmax", "argmin"];

pub fn class_of(c: char) -> Class {
    match c {
        '+' | '-' | '*' | '/' if c != '/' => Class::Bin,
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

fn italic_greek(c: char) -> char {
    // Lowercase Greek is italic in TeX; uppercase stays upright.
    let code = c as u32;
    if (0x03B1..=0x03C9).contains(&code) {
        return char::from_u32(0x1D6FC + code - 0x03B1).unwrap_or(c);
    }
    match c {
        'ϵ' => '𝜖',
        'ϑ' => '𝜗',
        'ϕ' => '𝜙',
        'ϱ' => '𝜚',
        'ϖ' => '𝜛',
        _ => c,
    }
}

/// Looks up `\n{tag}{name}\t{char}{flag}` in the packed table (data/math.txt).
fn lookup(tag: char, name: &str) -> Option<(char, char)> {
    if name.is_empty() || name.contains(['\n', '\t']) {
        return None;
    }
    let table = crate::tables::math();
    let key = ["\n", tag.encode_utf8(&mut [0; 4]), name, "\t"].concat();
    let mut it = table[table.find(&key)? + key.len()..].chars();
    Some((it.next()?, it.next().unwrap_or('\n')))
}

pub fn command(name: &str) -> Option<(char, Class)> {
    use Class::*;
    let (c, flag) = lookup('c', name)?;
    let class = match flag {
        'b' => Bin,
        'r' => Rel,
        '(' => Open,
        ')' => Close,
        'p' => Punct,
        'i' => Inner,
        _ => Ord,
    };
    Some((italic_greek(c), class))
}

/// `\sum` → (∑, is_integral)
pub fn big_operator(name: &str) -> Option<(char, bool)> {
    lookup('b', name).map(|(c, f)| (c, f == '1'))
}

/// `\hat` → (combining-free accent char, stretchy)
pub fn accent(name: &str) -> Option<(char, bool)> {
    lookup('a', name).map(|(c, f)| (c, f == '1'))
}

pub fn delimiter(name: &str) -> Option<char> {
    lookup('d', name).map(|(c, _)| c)
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

pub fn color(name: &str) -> [u8; 3] {
    let name = name.trim();
    if let Some(hex) = name.strip_prefix('#') {
        if let Ok(v) = u32::from_str_radix(hex, 16) {
            return [(v >> 16) as u8, (v >> 8) as u8, v as u8];
        }
    }
    match name {
        "red" => [0xd1, 0x24, 0x2f],
        "green" => [0x1a, 0x7f, 0x37],
        "blue" => [0x09, 0x69, 0xda],
        "orange" => [0xd9, 0x7b, 0x00],
        "purple" | "violet" => [0x82, 0x50, 0xdf],
        "magenta" => [0xbf, 0x39, 0x89],
        "cyan" => [0x1b, 0x9a, 0xaa],
        "gray" | "grey" => [0x6e, 0x77, 0x81],
        "brown" => [0x8b, 0x4b, 0x1f],
        _ => [0x80, 0x80, 0x80],
    }
}
