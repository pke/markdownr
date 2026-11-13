/// Colour as straight RGBA bytes. Used as parley's brush type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Ink(pub [u8; 4]);

impl Ink {
    pub const fn rgb(hex: u32) -> Self {
        Ink([(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255])
    }
    pub const fn rgba(hex: u32, a: u8) -> Self {
        Ink([(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, a])
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub background: Ink,
    pub text: Ink,
    pub muted: Ink,
    pub heading: Ink,
    pub link: Ink,
    pub code_text: Ink,
    pub code_bg: Ink,
    pub border: Ink,
    pub quote_bar: Ink,
    pub mark_bg: Ink,
    pub ins: Ink,
    pub selection: Ink,
    pub table_stripe: Ink,
    pub alert_note: Ink,
    pub alert_tip: Ink,
    pub alert_important: Ink,
    pub alert_warning: Ink,
    pub alert_caution: Ink,
    pub syn_keyword: Ink,
    pub syn_string: Ink,
    pub syn_comment: Ink,
    pub syn_number: Ink,
    pub syn_type: Ink,
    pub syn_function: Ink,
    pub syn_tag: Ink,
    pub syn_attr: Ink,
    pub find: Ink,
    pub find_current: Ink,
    pub chrome_bg: Ink,
    pub chrome_text: Ink,
    pub chrome_hover: Ink,
}

impl Theme {
    pub fn light() -> Self {
        Theme {
            background: Ink::rgb(0xffffff),
            text: Ink::rgb(0x1f2328),
            muted: Ink::rgb(0x59636e),
            heading: Ink::rgb(0x1f2328),
            link: Ink::rgb(0x0969da),
            code_text: Ink::rgb(0x1f2328),
            code_bg: Ink::rgb(0xf3f4f6),
            border: Ink::rgb(0xd1d9e0),
            quote_bar: Ink::rgb(0xd1d9e0),
            mark_bg: Ink::rgb(0xfff8c5),
            ins: Ink::rgb(0x1a7f37),
            selection: Ink::rgba(0x0969da, 70),
            table_stripe: Ink::rgb(0xf6f8fa),
            alert_note: Ink::rgb(0x0969da),
            alert_tip: Ink::rgb(0x1a7f37),
            alert_important: Ink::rgb(0x8250df),
            alert_warning: Ink::rgb(0x9a6700),
            alert_caution: Ink::rgb(0xd1242f),
            syn_keyword: Ink::rgb(0xcf222e),
            syn_string: Ink::rgb(0x0a3069),
            syn_comment: Ink::rgb(0x6e7781),
            syn_number: Ink::rgb(0x0550ae),
            syn_type: Ink::rgb(0x953800),
            syn_function: Ink::rgb(0x8250df),
            syn_tag: Ink::rgb(0x116329),
            syn_attr: Ink::rgb(0x0550ae),
            find: Ink::rgba(0xd4a72c, 110),
            find_current: Ink::rgba(0xfb8f44, 200),
            chrome_bg: Ink::rgb(0xf6f8fa),
            chrome_text: Ink::rgb(0x1f2328),
            chrome_hover: Ink::rgb(0xdde4ec),
        }
    }
    pub fn dark() -> Self {
        Theme {
            background: Ink::rgb(0x0d1117),
            text: Ink::rgb(0xe6edf3),
            muted: Ink::rgb(0x9198a1),
            heading: Ink::rgb(0xf0f6fc),
            link: Ink::rgb(0x4493f8),
            code_text: Ink::rgb(0xe6edf3),
            code_bg: Ink::rgb(0x1f2428),
            border: Ink::rgb(0x3d444d),
            quote_bar: Ink::rgb(0x3d444d),
            mark_bg: Ink::rgba(0xbb8009, 110),
            ins: Ink::rgb(0x3fb950),
            selection: Ink::rgba(0x4493f8, 90),
            table_stripe: Ink::rgb(0x151b23),
            alert_note: Ink::rgb(0x4493f8),
            alert_tip: Ink::rgb(0x3fb950),
            alert_important: Ink::rgb(0xab7df8),
            alert_warning: Ink::rgb(0xd29922),
            alert_caution: Ink::rgb(0xf85149),
            syn_keyword: Ink::rgb(0xff7b72),
            syn_string: Ink::rgb(0xa5d6ff),
            syn_comment: Ink::rgb(0x8b949e),
            syn_number: Ink::rgb(0x79c0ff),
            syn_type: Ink::rgb(0xffa657),
            syn_function: Ink::rgb(0xd2a8ff),
            syn_tag: Ink::rgb(0x7ee787),
            syn_attr: Ink::rgb(0x79c0ff),
            find: Ink::rgba(0xbb8009, 120),
            find_current: Ink::rgba(0xf0883e, 200),
            chrome_bg: Ink::rgb(0x161b22),
            chrome_text: Ink::rgb(0xe6edf3),
            chrome_hover: Ink::rgb(0x2a313c),
        }
    }
}

impl Theme {
    /// Warm paper-like reading theme.
    pub fn sepia() -> Self {
        Theme {
            background: Ink::rgb(0xf4ecd8),
            text: Ink::rgb(0x433422),
            muted: Ink::rgb(0x7a6a55),
            heading: Ink::rgb(0x3b2c1a),
            link: Ink::rgb(0x9c4a1a),
            code_text: Ink::rgb(0x433422),
            code_bg: Ink::rgb(0xeadfc6),
            border: Ink::rgb(0xd8c9a8),
            quote_bar: Ink::rgb(0xd0bf98),
            table_stripe: Ink::rgb(0xefe5cd),
            chrome_bg: Ink::rgb(0xece2c9),
            chrome_text: Ink::rgb(0x433422),
            chrome_hover: Ink::rgb(0xddd0b0),
            ..Theme::light()
        }
    }

    /// Theme by name (front matter `theme:` or the user's choice). `dark` is the
    /// system preference used for `auto`/unknown names.
    pub fn named(name: &str, dark: bool) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "light" | "default-light" | "github" => Theme::light(),
            "dark" | "night" | "github-dark" => Theme::dark(),
            "sepia" | "paper" => Theme::sepia(),
            _ if dark => Theme::dark(),
            _ => Theme::light(),
        }
    }

    /// Apply simple overrides from front matter (`background: #fff`, `color:`, `link:`).
    pub fn with_overrides(mut self, pairs: &[(String, String)]) -> Self {
        for (k, v) in pairs {
            let Some(ink) = parse_color(v) else { continue };
            match k.as_str() {
                "background" | "background-color" => self.background = ink,
                "color" | "text" | "text-color" => self.text = ink,
                "link" | "link-color" | "accent" => self.link = ink,
                "heading" | "heading-color" => self.heading = ink,
                _ => {}
            }
        }
        self
    }
}

pub fn parse_color(v: &str) -> Option<Ink> {
    let h = v.trim().trim_matches(['"', '\'']).strip_prefix('#')?;
    let n = u32::from_str_radix(h, 16).ok()?;
    match h.len() {
        6 => Some(Ink::rgb(n)),
        3 => {
            let (r, g, b) = ((n >> 8) & 0xf, (n >> 4) & 0xf, n & 0xf);
            Some(Ink::rgb((r * 17) << 16 | (g * 17) << 8 | b * 17))
        }
        _ => None,
    }
}

/// Parse flat `key: value` lines of YAML front matter.
pub fn front_matter_pairs(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            if k.starts_with([' ', '\t', '-', '#']) {
                return None;
            }
            Some((k.trim().to_ascii_lowercase(), v.trim().trim_matches(['"', '\'']).to_string()))
        })
        .collect()
}
