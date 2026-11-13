/// Colour as straight RGBA bytes. Used as parley's brush type.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Ink(pub [u8; 4]);

impl Ink {
    pub const fn rgb(hex: u32) -> Self {
        Ink([(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255])
    }
    pub const fn rgba(hex: u32, a: u8) -> Self {
        Ink([(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, a])
    }
    pub fn color(self) -> vello::peniko::Color {
        let [r, g, b, a] = self.0;
        vello::peniko::Color::from_rgba8(r, g, b, a)
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
        }
    }
}
