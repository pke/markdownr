//! Opened folders: the Markdown files under a directory and the index page listing them
//! (as the mobile app's Open Folder).

use crate::util::path::{extension, join};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// At most this many files and directory levels: a folder opened by mistake (the home
/// directory, a drive) lists its first files quickly rather than walking the whole disk.
const MAX_FILES: usize = 2000;
const MAX_DEPTH: usize = 8;

/// A file the app shows itself rather than handing it to the system.
pub fn is_markdown(p: &Path) -> bool {
    matches!(extension(p).as_str(), "md" | "markdown" | "mdown" | "mkd" | "txt")
}

/// The Markdown files of a folder and the directories read for them.
#[derive(Debug, Default, PartialEq)]
pub struct Listing {
    /// Per directory that has Markdown files: its path below the root (`/` separated, empty
    /// for the root) and the files' names, both in reading order (see `order`).
    pub groups: Vec<(String, Vec<String>)>,
    /// The root and each directory below it that was read, for watching.
    pub dirs: Vec<PathBuf>,
}

impl Listing {
    /// The first file to show when the folder is opened, as on mobile: the only file, or the
    /// first when it is numbered (`00-intro.md`, `1 Start.md`), so a numbered series starts
    /// at its beginning. None: the index page.
    pub fn first(&self) -> Option<String> {
        let (dir, files) = self.groups.first()?;
        let only = self.groups.len() == 1 && files.len() == 1;
        (only || dir.is_empty() && files[0].starts_with(|c: char| c.is_ascii_digit())).then(|| rel(dir, &files[0]))
    }
}

fn rel(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_string() } else { [dir, "/", name].concat() }
}

/// The Markdown files under `root` (hidden entries and links left out).
pub fn scan(root: &Path) -> Listing {
    let mut l = Listing::default();
    walk(root, String::new(), 0, &mut l, &mut 0);
    l
}

fn walk(dir: &Path, below: String, depth: usize, l: &mut Listing, count: &mut usize) {
    let Some(entries) = crate::util::fs::list(dir) else { return };
    l.dirs.push(dir.to_path_buf());
    let mut entries: Vec<(String, bool)> = entries.into_iter().map(|(n, d)| (n.to_string_lossy().into_owned(), d)).collect();
    entries.sort_by(|a, b| order(&a.0, &b.0));
    let files: Vec<String> = entries.iter().filter(|(n, d)| !d && is_markdown(Path::new(n))).take(MAX_FILES - *count).map(|(n, _)| n.clone()).collect();
    *count += files.len();
    if !files.is_empty() {
        l.groups.push((below.clone(), files));
    }
    for (name, _) in entries.iter().filter(|e| e.1) {
        if depth >= MAX_DEPTH || *count >= MAX_FILES {
            break;
        }
        walk(&join(dir, Path::new(name)), rel(&below, name), depth + 1, l, count);
    }
}

/// Reading order: numbered names first, by their number (`2` before `10`), then the rest by
/// name, ignoring case.
fn order(a: &str, b: &str) -> Ordering {
    let key = |s: &str| {
        let digits = s.bytes().take_while(u8::is_ascii_digit);
        let n = digits.clone().count();
        let value = digits.fold(0u64, |v, d| v.saturating_mul(10).saturating_add((d - b'0') as u64));
        (n == 0, value)
    };
    let (la, lb) = (a.chars().flat_map(char::to_lowercase), b.chars().flat_map(char::to_lowercase));
    key(a).cmp(&key(b)).then_with(|| la.cmp(lb))
}

/// Text as it reads, with Markdown's punctuation escaped.
fn escaped(out: &mut String, text: &str) {
    for c in text.chars() {
        if c.is_ascii_punctuation() {
            out.push('\\');
        }
        out.push(c);
    }
}

/// The index page: the folder's name as heading, the root's files, then each directory's
/// under its path as a heading (so the contents list them); links relative to the root.
pub fn index(name: &str, l: &Listing) -> String {
    let mut s = String::from("# ");
    escaped(&mut s, name);
    s.push_str("\n\n");
    if l.groups.is_empty() {
        s.push_str(crate::view::ui(crate::view::Ui::NoFiles));
        s.push('\n');
    }
    for (dir, files) in &l.groups {
        if !dir.is_empty() {
            s.push_str("## ");
            escaped(&mut s, dir);
            s.push_str("\n\n");
        }
        for f in files {
            let stem = match f.rfind('.') {
                Some(i) if i > 0 => &f[..i],
                _ => f,
            };
            s.push_str("- [");
            escaped(&mut s, stem);
            s.push_str("](<");
            // App::open_url decodes %xx and cuts at '#'.
            for c in rel(dir, f).chars() {
                match c {
                    '%' => s.push_str("%25"),
                    '#' => s.push_str("%23"),
                    '<' => s.push_str("%3C"),
                    '>' => s.push_str("%3E"),
                    '\n' => s.push_str("%0A"),
                    '\r' => s.push_str("%0D"),
                    c => s.push(c),
                }
            }
            s.push_str(">)\n");
        }
        s.push('\n');
    }
    s
}

/// `path` lies in `root` (or is it).
pub fn within(root: &Path, path: &Path) -> bool {
    let (r, p) = (root.as_os_str().as_encoded_bytes(), path.as_os_str().as_encoded_bytes());
    let r = r.strip_suffix(b"/").or_else(|| r.strip_suffix(b"\\")).unwrap_or(r);
    p.starts_with(r) && (p.len() == r.len() || matches!(p[r.len()], b'/' | b'\\'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_names_first_by_number() {
        let mut v = vec!["b.md", "10-z.md", "A.md", "2-y.md", "01-x.md", "c.md"];
        v.sort_by(|a, b| order(a, b));
        assert_eq!(v, ["01-x.md", "2-y.md", "10-z.md", "A.md", "b.md", "c.md"]);
    }

    #[test]
    fn within_the_root() {
        assert!(within(Path::new("/a/docs"), Path::new("/a/docs/x.md")));
        assert!(within(Path::new("/a/docs/"), Path::new("/a/docs/sub/x.md")));
        assert!(within(Path::new("/a/docs"), Path::new("/a/docs")));
        assert!(within(Path::new(r"C:\docs"), Path::new(r"C:\docs\sub/x.md")));
        assert!(!within(Path::new("/a/docs"), Path::new("/a/docs2/x.md")));
        assert!(!within(Path::new("/a/docs"), Path::new("/a/x.md")));
    }

    #[test]
    fn index_lists_the_files_by_directory() {
        let l = Listing {
            groups: vec![("".into(), vec!["01 Start.md".into(), "a_b.md".into()]), ("sub dir".into(), vec!["100% #1.markdown".into()])],
            dirs: Vec::new(),
        };
        assert_eq!(index("Docs", &l), "# Docs\n\n- [01 Start](<01 Start.md>)\n- [a\\_b](<a_b.md>)\n\n## sub dir\n\n- [100\\% \\#1](<sub dir/100%25 %231.markdown>)\n\n");
        assert_eq!(l.first().as_deref(), Some("01 Start.md"));
    }

    #[test]
    fn first_file_only_when_numbered_or_alone() {
        let groups = |g: &[(&str, &[&str])]| Listing { groups: g.iter().map(|(d, f)| (d.to_string(), f.iter().map(|s| s.to_string()).collect())).collect(), dirs: Vec::new() };
        assert_eq!(groups(&[("", &["a.md", "b.md"])]).first(), None);
        assert_eq!(groups(&[("sub", &["only.md"])]).first().as_deref(), Some("sub/only.md"));
        assert_eq!(groups(&[("sub", &["1.md"]), ("t", &["2.md"])]).first(), None);
        assert_eq!(groups(&[]).first(), None);
    }

    #[test]
    fn scan_skips_hidden_and_other_files() {
        let root = std::env::temp_dir().join(["markdownr-scan-", &std::process::id().to_string()].concat());
        let _ = std::fs::remove_dir_all(&root);
        for d in ["sub/deeper", ".git", "empty"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        for f in ["b.md", "a.txt", "pic.png", ".hidden.md", "sub/10.md", "sub/9.md", "sub/deeper/x.mkd", ".git/HEAD.md"] {
            std::fs::write(root.join(f), "x").unwrap();
        }
        let l = scan(&root);
        let groups: Vec<(&str, Vec<&str>)> = l.groups.iter().map(|(d, f)| (d.as_str(), f.iter().map(String::as_str).collect())).collect();
        assert_eq!(groups, [("", vec!["a.txt", "b.md"]), ("sub", vec!["9.md", "10.md"]), ("sub/deeper", vec!["x.mkd"])]);
        assert_eq!(l.dirs.len(), 4, "root, empty, sub, sub/deeper: {:?}", l.dirs);
        let _ = std::fs::remove_dir_all(&root);
    }
}
