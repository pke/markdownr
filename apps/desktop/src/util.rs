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

/// A map on a `Vec` with linear lookups and the few `HashMap` methods the app uses: its
/// maps hold a handful of keys (brushes, text formats, glyph sprites, image URLs,
/// heading slugs), and dropping std's `HashMap` (hashing, random seed, table code) from
/// the whole program saves about 12 KB of exe.
pub struct VecMap<K, V>(Vec<(K, V)>);

impl<K, V> Default for VecMap<K, V> {
    fn default() -> Self {
        VecMap(Vec::new())
    }
}

impl<K, V> VecMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    fn position<Q: PartialEq + ?Sized>(&self, key: &Q) -> Option<usize>
    where
        K: core::borrow::Borrow<Q>,
    {
        self.0.iter().position(|(k, _)| k.borrow() == key)
    }

    pub fn get<Q: PartialEq + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: core::borrow::Borrow<Q>,
    {
        self.position(key).map(|i| &self.0[i].1)
    }

    pub fn get_mut<Q: PartialEq + ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: core::borrow::Borrow<Q>,
    {
        self.position(key).map(|i| &mut self.0[i].1)
    }

    pub fn contains_key<Q: PartialEq + ?Sized>(&self, key: &Q) -> bool
    where
        K: core::borrow::Borrow<Q>,
    {
        self.position(key).is_some()
    }

    pub fn remove<Q: PartialEq + ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: core::borrow::Borrow<Q>,
    {
        self.position(key).map(|i| self.0.swap_remove(i).1)
    }

    pub fn insert(&mut self, key: K, value: V)
    where
        K: PartialEq,
    {
        match self.position(&key) {
            Some(i) => self.0[i].1 = value,
            None => self.0.push((key, value)),
        }
    }

    /// The value at `key`, inserted with `f()` if there is none.
    pub fn or_insert_with(&mut self, key: K, f: impl FnOnce() -> V) -> &mut V
    where
        K: PartialEq,
    {
        let i = self.position(&key).unwrap_or_else(|| {
            self.0.push((key, f()));
            self.0.len() - 1
        });
        &mut self.0[i].1
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.0.iter_mut().map(|(_, v)| v)
    }
}

/// Windows path helpers on the raw bytes (`\` or `/` separate, `X:` or `\` starts an
/// absolute path), in place of `Path::parent`/`join`/`file_name`/`extension`/
/// `is_absolute`, whose `Components` parser costs about 5 KB of exe. Splitting an
/// `OsStr` at an ASCII byte keeps it valid, as `from_encoded_bytes_unchecked` requires.
pub mod path {
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    fn bytes(p: &Path) -> &[u8] {
        p.as_os_str().as_encoded_bytes()
    }

    /// Without trailing separators (`C:\a\` → `C:\a`), as std's parent/file_name see it.
    fn trimmed(p: &Path) -> &[u8] {
        let mut b = bytes(p);
        while b.len() > 1 && matches!(b[b.len() - 1], b'\\' | b'/') && !(b.len() == 3 && b[1] == b':') {
            b = &b[..b.len() - 1];
        }
        b
    }

    fn os(b: &[u8]) -> &Path {
        // SAFETY: `b` is a prefix or suffix of a path's encoded bytes, split at an ASCII byte.
        Path::new(unsafe { OsStr::from_encoded_bytes_unchecked(b) })
    }

    fn last_sep(b: &[u8]) -> Option<usize> {
        b.iter().rposition(|&c| c == b'\\' || c == b'/')
    }

    /// `C:\x`, `C:/x`, `\server\x` or `\x` (rooted).
    pub fn is_absolute(p: &Path) -> bool {
        let b = bytes(p);
        (b.len() >= 3 && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')) || b.first().is_some_and(|&c| c == b'\\' || c == b'/')
    }

    /// The directory part: `C:\a\b.md` → `C:\a`, `C:\b.md` → `C:\`, `b.md` → `` (as std).
    pub fn parent(p: &Path) -> &Path {
        let b = trimmed(p);
        match last_sep(b) {
            None => os(&[]),
            Some(i) if i == 2 && b[1] == b':' => os(&b[..3]),
            Some(0) => os(&b[..1]),
            Some(i) => os(&b[..i]),
        }
    }

    /// The last part: `C:\a\b.md` → `b.md`.
    pub fn file_name(p: &Path) -> &Path {
        let b = trimmed(p);
        os(&b[last_sep(b).map_or(0, |i| i + 1)..])
    }

    /// The file name's extension, lowercased (`b.MD` → `md`; none for `.bashrc` or `b`).
    pub fn extension(p: &Path) -> String {
        let name = bytes(file_name(p));
        match name.iter().rposition(|&c| c == b'.') {
            Some(i) if i > 0 => String::from_utf8_lossy(&name[i + 1..]).to_ascii_lowercase(),
            _ => String::new(),
        }
    }

    /// `rel` against `dir` (`rel` itself if absolute or `dir` empty).
    pub fn join(dir: &Path, rel: &Path) -> PathBuf {
        let d = bytes(dir);
        if is_absolute(rel) || d.is_empty() {
            return rel.to_path_buf();
        }
        let mut s = dir.as_os_str().to_os_string();
        if !matches!(d.last(), Some(b'\\' | b'/')) {
            s.push("\\");
        }
        s.push(rel.as_os_str());
        PathBuf::from(s)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn like_std_for_our_paths() {
            for s in [r"C:\a\b.md", r"C:\b.MD", "b.md", r"\srv\share\x.png", r"C:/a/b.tar.gz", ".bashrc", r"C:\a\", "dir/sub/f"] {
                let p = Path::new(s);
                assert_eq!(parent(p), p.parent().unwrap_or(Path::new("")), "{s}");
                assert_eq!(file_name(p), Path::new(p.file_name().unwrap_or_default()), "{s}");
                assert_eq!(extension(p), p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default(), "{s}");
                assert_eq!(is_absolute(p), p.is_absolute() || s.starts_with('\\') || s.starts_with('/'), "{s}");
            }
            assert_eq!(join(Path::new(r"C:\a"), Path::new("img/x.png")), PathBuf::from(r"C:\a\img/x.png"));
            assert_eq!(join(Path::new(r"C:\"), Path::new("x.png")), PathBuf::from(r"C:\x.png"));
            assert_eq!(join(Path::new(""), Path::new("x.png")), PathBuf::from("x.png"));
            assert_eq!(join(Path::new(r"C:\a"), Path::new(r"D:\y.png")), PathBuf::from(r"D:\y.png"));
        }
    }
}
