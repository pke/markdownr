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
    /// Brightness 0 to 255 (Rec. 601 weights); below 128 counts as dark.
    pub fn luma(self) -> i32 {
        (299 * self.0[0] as i32 + 587 * self.0[1] as i32 + 114 * self.0[2] as i32) / 1000
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
    pub overlay: Option<&'static Overlay>,
    pub hr_emojis: Option<&'static [&'static str]>,
    pub santa_hat: Option<f32>,
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
            overlay: None,
            hr_emojis: None,
            santa_hat: None,
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
            overlay: None,
            hr_emojis: None,
            santa_hat: None,
        }
    }
}

/// Title bar colours of a theme (`Theme::caption`); dark: caption buttons for a dark background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caption {
    pub background: Ink,
    pub text: Ink,
    pub dark: bool,
}

impl Theme {
    /// Theme by name (front matter `theme:` or the user's choice): one of the
    /// ported iOS themes, or `light`/`dark`; anything else is the GitHub-like
    /// default following the system's dark mode.
    pub fn named(name: &str, dark: bool) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "light" | "github" => Theme::light(),
            "dark" | "night" | "github-dark" => Theme::dark(),
            n => match theme_def(n) {
                Some(def) => def.theme(dark),
                None if dark => Theme::dark(),
                None => Theme::light(),
            },
        }
    }

    /// The title bar in this theme (Windows draws its caption with it): the page background,
    /// the heading colour for the title (the text colour, or black or white, if that one is
    /// too close to the background, as front matter colours can make it), and caption buttons
    /// for a dark background if it is darker than mid grey.
    /// `bar`: how far the toolbar has faded in (0 to 1); the caption fades with it to the
    /// toolbar's colour, so title bar and toolbar read as one.
    pub fn caption(&self, bar: f32) -> Caption {
        let (from, to) = (self.background.0, self.chrome_bg.0);
        let mix = |i: usize| (from[i] as f32 + (to[i] as f32 - from[i] as f32) * bar.clamp(0.0, 1.0)).round() as u8;
        let background = Ink([mix(0), mix(1), mix(2), 255]);
        let bg = background.luma();
        let dark = bg < 128;
        let readable = |i: Ink| if dark { i.luma() - bg >= 100 } else { bg - i.luma() >= 100 };
        let plain = Ink::rgb(if dark { 0xffffff } else { 0x000000 });
        let text = [self.heading, self.text, self.chrome_text].into_iter().find(|&i| readable(i)).unwrap_or(plain);
        Caption { background, text, dark }
    }

    /// The theme of a page following its front matter (`theme:` and colours): a background of
    /// its own picks the theme's light or dark colours to fit it, not the system's mode, so a
    /// light page in dark mode gets dark text.
    pub fn for_page(name: &str, dark: bool, pairs: &[(String, String)]) -> Self {
        let background = pairs.iter().filter(|(k, _)| k == "background" || k == "background-color").find_map(|(_, v)| parse_color(v));
        let dark = background.map_or(dark, |b| b.luma() < 128);
        Theme::named(name, dark).with_overrides(pairs)
    }

    /// Apply simple overrides from front matter (`background: #fff`, `color:`, `link:`).
    pub fn with_overrides(mut self, pairs: &[(String, String)]) -> Self {
        let (mut heading, mut text) = (None, None);
        for (k, v) in pairs {
            let Some(ink) = parse_color(v) else { continue };
            match k.as_str() {
                "background" | "background-color" => self.background = ink,
                "color" | "text" | "text-color" => text = Some(ink),
                "link" | "link-color" | "accent" => self.link = ink,
                "heading" | "heading-color" => heading = Some(ink),
                _ => {}
            }
        }
        // A text colour from the front matter colours the headings too unless they get their own:
        // the theme's heading colour is made for its own background, not for the one brought here.
        if let Some(t) = text {
            self.text = t;
            self.heading = t;
        }
        if let Some(h) = heading {
            self.heading = h;
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

// ---------------------------------------------------------------------------
// Named themes (ported from the iOS app's themes.ts)
// ---------------------------------------------------------------------------

#[path = "themes_data.rs"]
mod data;
pub use data::THEMES;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub text: u32,
    pub muted: u32,
    pub heading: u32,
    pub link: u32,
    pub code: u32,
    pub code_bg: u32,
    pub quote: u32,
    pub border: u32,
    pub stripe: Option<u32>,
}

/// Falling particles (snow, petals, leaves) drawn over the text.
#[derive(Debug, Clone, Copy)]
pub struct Overlay {
    pub particles: &'static [&'static str],
    pub count: usize,
    /// Font size range in DIPs.
    pub size: (f32, f32),
    /// Fall duration range in ms.
    pub duration: (f32, f32),
    pub sway: (f32, f32),
    pub sway_divisor: f32,
    /// Rotation amplitude range in degrees.
    pub rotation: (f32, f32),
    pub rotation_divisor: f32,
    pub opacity: (f32, f32),
    /// Tint for monochrome particles (light, dark).
    pub color: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeDef {
    pub name: &'static str,
    pub icon: &'static str,
    /// Not part of the theme cycle (only reachable via front matter).
    pub hidden: bool,
    pub background: (u32, u32),
    pub light: Palette,
    pub dark: Palette,
    pub overlay: Option<Overlay>,
    pub hr_emojis: Option<&'static [&'static str]>,
    /// Santa hat on headings, size in DIPs.
    pub santa_hat: Option<f32>,
}

pub fn theme_def(name: &str) -> Option<&'static ThemeDef> {
    let name = name.trim().to_ascii_lowercase();
    THEMES.iter().find(|t| t.name == name)
}

impl ThemeDef {
    pub fn label(&self) -> String {
        let mut c = self.name.chars();
        let cap: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
        [self.icon, " ", &cap].concat()
    }

    pub fn theme(&'static self, dark: bool) -> Theme {
        let base = if dark { Theme::dark() } else { Theme::light() };
        let p = if dark { &self.dark } else { &self.light };
        let bg = Ink::rgb(if dark { self.background.1 } else { self.background.0 });
        Theme {
            background: bg,
            text: Ink::rgb(p.text),
            muted: Ink::rgb(p.muted),
            heading: Ink::rgb(p.heading),
            link: Ink::rgb(p.link),
            code_text: Ink::rgb(p.code),
            code_bg: Ink::rgb(p.code_bg),
            border: Ink::rgb(p.border),
            quote_bar: Ink::rgb(p.quote),
            table_stripe: p.stripe.map_or(bg, Ink::rgb),
            chrome_bg: Ink::rgb(p.code_bg),
            chrome_text: Ink::rgb(p.text),
            chrome_hover: Ink::rgb(p.border),
            overlay: self.overlay.as_ref(),
            hr_emojis: self.hr_emojis,
            santa_hat: self.santa_hat,
            ..base
        }
    }
}

/// The user's theme choice: `None` follows the document (front matter) or the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThemeChoice(pub Option<usize>);

impl ThemeChoice {
    pub fn name(self) -> &'static str {
        self.0.map_or("auto", |i| THEMES[i].name)
    }
    pub fn from_name(s: &str) -> Self {
        ThemeChoice(THEMES.iter().position(|t| t.name == s))
    }
    pub fn label(self) -> String {
        self.0.map_or_else(|| "◐ Auto".to_string(), |i| THEMES[i].label())
    }
    pub fn next(self) -> Self {
        let start = self.0.map_or(0, |i| i + 1);
        ThemeChoice((start..THEMES.len()).find(|&i| !THEMES[i].hidden))
    }
}

/// Seasonal theme suggestion like the iOS app: Christmas in December, Easter
/// ±7 days, otherwise the season (southern hemisphere flipped).
pub fn seasonal_theme(year: i32, month: u32, day: u32, southern: bool) -> &'static str {
    if month == 12 {
        return "christmas";
    }
    let (em, ed) = easter(year);
    let doy = |m: u32, d: u32| days_from_civil(year, m, d);
    if (doy(month, day) - doy(em, ed)).abs() <= 7 {
        return "easter";
    }
    let season = match month {
        12 | 1 | 2 => 0,
        3..=5 => 1,
        6..=8 => 2,
        _ => 3,
    };
    let season = if southern { (season + 2) % 4 } else { season };
    ["winter", "spring", "summer", "autumn"][season]
}

fn easter(year: i32) -> (u32, u32) {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    (month as u32, day as u32)
}

/// Days since 1970-01-01 (proleptic Gregorian).
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// (year, month, day) from days since 1970-01-01.
#[cfg(test)]
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasons() {
        assert_eq!(easter(2026), (4, 5));
        assert_eq!(seasonal_theme(2026, 12, 3, false), "christmas");
        assert_eq!(seasonal_theme(2026, 4, 9, false), "easter");
        assert_eq!(seasonal_theme(2026, 11, 13, false), "autumn");
        assert_eq!(seasonal_theme(2026, 11, 13, true), "spring");
        assert_eq!(civil_from_days(days_from_civil(2026, 11, 13)), (2026, 11, 13));
    }

    #[test]
    fn cycle_skips_hidden() {
        let mut c = ThemeChoice::default();
        let mut seen = Vec::new();
        for _ in 0..THEMES.len() + 1 {
            c = c.next();
            seen.push(c.name());
        }
        assert!(!seen.contains(&"dude"));
        assert_eq!(seen.last(), Some(&"default")); // wrapped through auto
    }

    fn pairs(p: &[(&str, &str)]) -> Vec<(String, String)> {
        p.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// Dark title on a light caption, light on a dark one, well apart; buttons for the background.
    fn assert_readable(c: Caption, what: &str) {
        let (bg, fg) = (c.background.luma(), c.text.luma());
        assert_eq!(c.dark, bg < 128, "{what}: buttons for the background");
        assert!(if c.dark { fg - bg >= 100 } else { bg - fg >= 100 }, "{what}: title {fg} on {bg}");
    }

    #[test]
    fn caption_is_the_page_background_with_readable_title_and_buttons() {
        let names = THEMES.iter().map(|t| t.name).chain(["light", "dark"]);
        for name in names {
            for dark in [false, true] {
                let t = Theme::named(name, dark);
                let c = t.caption(0.0);
                assert_eq!(c.background, t.background, "{name} {dark}");
                assert_readable(c, &format!("{name} {dark}"));
                // A theme's own colours are kept: its heading colour.
                assert_eq!(c.text, t.heading, "{name} {dark}");
            }
        }
    }

    #[test]
    fn caption_takes_the_toolbar_colour_as_the_toolbar_fades_in() {
        let names = THEMES.iter().map(|t| t.name).chain(["light", "dark"]);
        for name in names {
            for dark in [false, true] {
                let t = Theme::named(name, dark);
                assert_eq!(t.caption(1.0).background, t.chrome_bg, "{name} {dark}: toolbar shown");
                for bar in [0.25, 0.5, 0.75, 1.0] {
                    assert_readable(t.caption(bar), &format!("{name} {dark} toolbar {bar}"));
                }
            }
        }
        // Half faded in: half way between page and toolbar.
        let t = Theme::named("default", false).with_overrides(&pairs(&[("background", "#000000")]));
        let half = t.caption(0.5).background.0;
        let [r, g, b, _] = t.chrome_bg.0;
        assert_eq!(&half[..3], &[r.div_ceil(2), g.div_ceil(2), b.div_ceil(2)]);
    }

    #[test]
    fn caption_title_stays_readable_with_front_matter_colours() {
        // A heading colour that does not fit, a text colour that does: the text colour.
        let t = Theme::named("default", false).with_overrides(&pairs(&[("background", "#202020"), ("text", "#eeeeee"), ("heading", "#303030")]));
        let c = t.caption(0.0);
        assert!(c.dark);
        assert_eq!(c.text, Ink::rgb(0xeeeeee));
        // Neither fits: the toolbar's text colour, else black on light, white on dark.
        let light = pairs(&[("background", "#ffffff"), ("text", "#eeeeee")]);
        let t = Theme::named("default", false).with_overrides(&light);
        assert_eq!(t.caption(0.0).text, t.chrome_text);
        assert_eq!(Theme::named("default", true).with_overrides(&light).caption(0.0).text, Ink::rgb(0x000000));
        let dark = pairs(&[("background", "#000000"), ("text", "#111111")]);
        assert_eq!(Theme::named("default", false).with_overrides(&dark).caption(0.0).text, Ink::rgb(0xffffff));
        // Mid grey is light enough for dark buttons.
        assert!(!Theme::named("default", false).with_overrides(&pairs(&[("background", "#808080")])).caption(0.0).dark);
    }

    #[test]
    fn a_front_matter_background_picks_the_palette_that_fits_it() {
        // A light background in dark mode: the theme's light colours, dark text on it.
        let fm = pairs(&[("background", "#fafafa")]);
        for name in ["default", "ocean", "sepia"] {
            let t = Theme::for_page(name, true, &fm);
            assert_eq!(t.background, Ink::rgb(0xfafafa));
            assert_eq!(t.text, Theme::named(name, false).text, "{name}");
            assert_eq!(t.chrome_bg, Theme::named(name, false).chrome_bg, "{name}");
        }
        // And a dark one in light mode: the dark colours.
        let t = Theme::for_page("default", false, &pairs(&[("background", "#101010")]));
        assert_eq!(t.text, Theme::named("default", true).text);
        // Without a background the system's mode decides; colours of its own stay.
        assert_eq!(Theme::for_page("ocean", true, &pairs(&[("link", "#ff0000")])).text, Theme::named("ocean", true).text);
        let t = Theme::for_page("default", true, &pairs(&[("background", "#fafafa"), ("text", "#333333")]));
        assert_eq!(t.text, Ink::rgb(0x333333));
    }
}
