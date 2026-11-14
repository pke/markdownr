//! Persistent user settings as a tiny `key=value` file (platform shells choose
//! the location, e.g. %APPDATA%\Markdownr\settings.ini).

use std::path::{Path, PathBuf};

const MAX_RECENT: usize = 12;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Opt-in: start in full screen if the app was left in full screen.
    pub remember_fullscreen: bool,
    pub fullscreen: bool,
    pub font_size: f32,
    pub theme: String,
    pub recent: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { remember_fullscreen: false, fullscreen: false, font_size: crate::view::DEFAULT_FONT_SIZE, theme: "auto".into(), recent: Vec::new() }
    }
}

impl Settings {
    pub fn parse(text: &str) -> Self {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let v = v.trim();
            match k.trim() {
                "remember_fullscreen" => s.remember_fullscreen = v == "1",
                "fullscreen" => s.fullscreen = v == "1",
                "font_size" => s.font_size = crate::util::parse_f32(v).unwrap_or(s.font_size).clamp(10.0, 40.0),
                "theme" => s.theme = v.to_string(),
                "recent" if !v.is_empty() => s.recent.push(PathBuf::from(v)),
                _ => {}
            }
        }
        s
    }

    pub fn serialize(&self) -> String {
        let b = |v: bool| if v { "1" } else { "0" };
        let size = crate::util::num(self.font_size.round() as u64);
        let mut out = String::new();
        for s in ["remember_fullscreen=", b(self.remember_fullscreen), "\nfullscreen=", b(self.fullscreen), "\nfont_size=", &size, "\ntheme=", &self.theme, "\n"] {
            out.push_str(s);
        }
        for r in &self.recent {
            out.push_str("recent=");
            out.push_str(&r.to_string_lossy());
            out.push('\n');
        }
        out
    }

    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).map(|t| Self::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        let _ = std::fs::create_dir(crate::util::path::parent(path)); // %APPDATA%Markdownr
        let _ = std::fs::write(path, self.serialize());
    }

    /// Move `path` to the front of the recent list.
    pub fn add_recent(&mut self, path: &Path) {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let path = PathBuf::from(path.to_string_lossy().trim_start_matches(r"\\?\"));
        self.recent.retain(|p| p.as_os_str() != path.as_os_str());
        self.recent.insert(0, path);
        self.recent.truncate(MAX_RECENT);
    }

    /// Start page shown without a file: open button, recent files, sample.
    pub fn start_page(&self) -> String {
        let mut s = String::from("---\ntitle: Markdownr\n---\n# Markdownr\n\nEin schneller, schlanker Markdown-Reader.\n\n[Datei öffnen …](markdownr:open) · [Beispieldokument](markdownr:sample)\n\nAuch per Drag & Drop oder `Strg+O`. `F11` Vollbild, `Strg+F` Suchen, `Strg+T` Inhalt, `F1` alle Tastenkürzel.\n\n");
        let recent: Vec<_> = self.recent.iter().filter(|p| p.is_file()).collect();
        if !recent.is_empty() {
            s.push_str("## Zuletzt geöffnet\n\n");
            for p in recent {
                let name = crate::util::path::file_name(p).to_string_lossy().into_owned();
                let dir = crate::util::path::parent(p).to_string_lossy().into_owned();
                let url = p.to_string_lossy().replace('\\', "/").replace(' ', "%20");
                for part in ["- [", &name.replace(['[', ']'], ""), "](<file:///", &url, ">) — ", &dir.replace('\\', "\\\\"), "\n"] {
                    s.push_str(part);
                }
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut s = Settings { remember_fullscreen: true, fullscreen: true, font_size: 18.0, theme: "sepia".into(), recent: Vec::new() };
        s.recent.push(PathBuf::from("C:/a/b.md"));
        assert_eq!(Settings::parse(&s.serialize()), s);
    }
}
