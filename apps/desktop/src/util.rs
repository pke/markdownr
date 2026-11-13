//! Tiny replacements for `format!`/`str::parse::<f32>` so the binary does not
//! pull in core::fmt's integer, float and Debug machinery.

/// Decimal representation of an unsigned integer.
pub fn num(mut n: u64) -> String {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    buf[i..].iter().map(|&b| b as char).collect()
}

/// Parses `[-]digits[.digits]`; enough for settings and env vars.
pub fn parse_f32(s: &str) -> Option<f32> {
    let s = s.trim();
    let (neg, s) = s.strip_prefix('-').map_or((false, s), |r| (true, r));
    let (int, frac) = s.split_once('.').unwrap_or((s, ""));
    if int.is_empty() && frac.is_empty() || !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut v = int.bytes().fold(0.0f32, |a, b| a * 10.0 + (b - b'0') as f32);
    let mut scale = 0.1;
    for b in frac.bytes() {
        v += (b - b'0') as f32 * scale;
        scale *= 0.1;
    }
    Some(if neg { -v } else { v })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numbers() {
        assert_eq!(num(0), "0");
        assert_eq!(num(1234567), "1234567");
        assert_eq!(parse_f32("16.5"), Some(16.5));
        assert_eq!(parse_f32("-3"), Some(-3.0));
        assert_eq!(parse_f32("x"), None);
    }
}

/// Writes the parts plus a newline to stderr without going through core::fmt.
pub fn log(parts: &[&str]) {
    use std::io::Write;
    let mut e = std::io::stderr().lock();
    for p in parts {
        let _ = e.write_all(p.as_bytes());
    }
    let _ = e.write_all(b"\n");
}
