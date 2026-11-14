//! LaTeX math (the common subset used in Markdown documents) → math tree.
//!
//! Works on bytes; all names and texts are slices of the source. Every
//! command is resolved by one lookup in the sorted table `data/math.txt`
//! (see [`symbols::lookup`]), whose kind byte selects what to parse next.

use super::symbols;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Inner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Font {
    Italic,
    Upright,
    Bold,
    BoldItalic,
    DoubleStruck,
    Script,
    Fraktur,
    SansSerif,
    Mono,
}

/// Fonts by table digit (`M` and `T` entries).
const FONTS: [Font; 9] = [
    Font::Italic,
    Font::Upright,
    Font::Bold,
    Font::BoldItalic,
    Font::DoubleStruck,
    Font::Script,
    Font::Fraktur,
    Font::SansSerif,
    Font::Mono,
];
/// `S` entries: `\,` `\:` `\;` `\!` `\ ` `\quad` `\qquad` `\\` (em).
const SPACES: [f32; 8] = [3.0 / 18.0, 4.0 / 18.0, 5.0 / 18.0, -3.0 / 18.0, 0.25, 1.0, 2.0, 0.0];
/// `G` entries: `\big` `\Big` `\bigg` `\Bigg` (× font size).
const BIG: [f32; 4] = [1.2, 1.8, 2.4, 3.0];

#[derive(Debug, Clone, PartialEq)]
pub enum M<'a> {
    Row(Vec<M<'a>>),
    /// A single symbol (already mapped to its final code point).
    Sym(char, Class),
    /// Upright text run (`\text{}`, numbers, function names like `sin`); letters
    /// and digits are drawn in the given font (`\textbf`, `\textit`).
    Text(&'a str, Class, Font),
    Frac { num: Box<M<'a>>, den: Box<M<'a>>, rule: bool, delims: Option<(char, char)> },
    Sqrt { degree: Option<Box<M<'a>>>, body: Box<M<'a>> },
    Scripts { base: Box<M<'a>>, sup: Option<Box<M<'a>>>, sub: Option<Box<M<'a>>> },
    /// Large operator; `limits`: Some(true) = \limits, Some(false) = \nolimits, None = default.
    BigOp { sym: char, limits: Option<bool>, integral: bool },
    /// Function name with display-style limits (`\lim`, `\max`).
    LimitFunc(&'a str),
    Delimited { left: Option<char>, right: Option<char>, body: Box<M<'a>> },
    /// `\big(` etc.: a delimiter at a fixed size (multiple of the font size).
    BigDelim(char, f32, Class),
    Accent { accent: char, body: Box<M<'a>>, stretchy: bool },
    OverLine(Box<M<'a>>),
    UnderLine(Box<M<'a>>),
    /// `\overbrace`, `\underbrace` (stretched horizontal glyph).
    Brace { over: bool, body: Box<M<'a>> },
    Matrix { rows: Vec<Vec<M<'a>>>, left: Option<char>, right: Option<char>, align_left: bool, cases: bool },
    /// Horizontal space in em.
    Space(f32),
    /// Text color (`\color{red}{...}`)
    Color([u8; 3], Box<M<'a>>),
    /// Unknown command, including its backslash.
    Error(&'a str),
}

/// `row` end marker for "no closing token" (never a byte of UTF-8 text).
const NONE: u8 = 0xFF;

struct Parser<'a> {
    s: &'a str,
    pos: usize,
    font: Font,
}

pub fn parse(src: &str) -> M<'_> {
    let mut p = Parser { s: src, pos: 0, font: Font::Italic };
    M::Row(p.row(NONE))
}

/// One shared, out-of-line boxing site instead of an inlined allocation per use.
#[inline(never)]
fn bx(m: M<'_>) -> Box<M<'_>> {
    Box::new(m)
}

fn empty<'a>() -> M<'a> {
    M::Row(Vec::new())
}

/// Split a script base into (base, sup, sub), so `x_i^2` and `x^2_i` attach to one base.
fn split_scripts(base: Option<M<'_>>) -> (Box<M<'_>>, Option<Box<M<'_>>>, Option<Box<M<'_>>>) {
    match base {
        Some(M::Scripts { base, sup, sub }) => (base, sup, sub),
        other => (bx(other.unwrap_or_else(empty)), None, None),
    }
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a [u8] {
        &self.s.as_bytes()[self.pos..]
    }
    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.pos).copied()
    }
    fn peek_char(&self) -> Option<char> {
        self.s[self.pos..].chars().next()
    }
    fn bump_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek_char()
            && c.is_whitespace()
        {
            self.pos += c.len_utf8();
        }
    }
    /// Command name after a backslash: a run of ASCII letters or one character.
    fn name(&mut self) -> &'a str {
        let start = self.pos;
        if self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                self.pos += 1;
            }
        } else {
            self.bump_char();
        }
        &self.s[start..self.pos]
    }
    /// Raw `{...}` contents (balanced), for \text and environment names.
    fn raw_group(&mut self) -> &'a str {
        self.skip_ws();
        let start = self.pos;
        if self.peek() != Some(b'{') {
            self.bump_char();
            return &self.s[start..self.pos];
        }
        self.pos += 1;
        let mut depth = 1u32;
        while let Some(c) = self.peek() {
            self.pos += 1;
            match c {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &self.s[start + 1..self.pos - 1];
                    }
                }
                b'\\' => {
                    self.bump_char();
                }
                _ => {}
            }
        }
        &self.s[start + 1..]
    }
    fn optional_arg(&mut self) -> Option<M<'a>> {
        self.skip_ws();
        if self.peek() != Some(b'[') {
            return None;
        }
        self.pos += 1;
        Some(M::Row(self.row(b']')))
    }

    /// One argument: a `{group}` or a single token.
    fn arg(&mut self) -> M<'a> {
        self.skip_ws();
        // A bare argument is a single token: `\frac12` is ½.
        if let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
            return M::Text(&self.s[self.pos - 1..self.pos], Class::Ord, Font::Upright);
        }
        self.atom().unwrap_or_else(empty)
    }

    fn with_font(&mut self, font: Font) -> M<'a> {
        let saved = std::mem::replace(&mut self.font, font);
        let m = self.arg();
        self.font = saved;
        m
    }

    /// Parse until `end` (consumed) or end of input. Handles `\over`; `&`/`\\` are left to matrices.
    fn row(&mut self, end: u8) -> Vec<M<'a>> {
        let mut items: Vec<M<'a>> = Vec::new();
        loop {
            self.skip_ws();
            let Some(c) = self.peek() else { break };
            if c == end {
                self.pos += 1;
                break;
            }
            match c {
                b'}' if end == NONE => {
                    self.pos += 1;
                    continue;
                }
                b'^' | b'_' => {
                    self.pos += 1;
                    let script = bx(self.arg());
                    let (base, mut sup, mut sub) = split_scripts(items.pop());
                    if c == b'^' {
                        sup = Some(script);
                    } else {
                        sub = Some(script);
                    }
                    items.push(M::Scripts { base, sup, sub });
                    continue;
                }
                b'\'' => {
                    self.pos += 1;
                    let (base, sup, sub) = split_scripts(items.pop());
                    let prime = M::Sym('′', Class::Ord);
                    let sup = Some(bx(match sup {
                        Some(s) => M::Row(vec![*s, prime]),
                        None => prime,
                    }));
                    items.push(M::Scripts { base, sup, sub });
                    continue;
                }
                b'&' => break, // handled by matrix
                b'\\' => {
                    let r = &self.rest()[1..];
                    if r.starts_with(b"over") && !r.get(4).is_some_and(u8::is_ascii_alphabetic) {
                        self.pos += 5;
                        let num = M::Row(std::mem::take(&mut items));
                        let den = M::Row(self.row(end));
                        return vec![M::Frac { num: bx(num), den: bx(den), rule: true, delims: None }];
                    }
                    if r.starts_with(b"right") || (end == NONE && r.starts_with(b"end")) || r.first() == Some(&b'\\') {
                        break;
                    }
                }
                _ => {}
            }
            match self.atom() {
                Some(m) => items.push(m),
                None => break,
            }
        }
        items
    }

    fn atom(&mut self) -> Option<M<'a>> {
        self.skip_ws();
        let start = self.pos;
        let c = self.bump_char()?;
        Some(match c {
            '{' => M::Row(self.row(b'}')),
            '\\' => self.command(),
            '0'..='9' | '.' if self.font == Font::Italic => {
                // Numbers stay together and upright.
                while let Some(b'0'..=b'9' | b'.') = self.peek() {
                    self.pos += 1;
                }
                let text = &self.s[start..self.pos];
                if text == "." { M::Sym('.', Class::Punct) } else { M::Text(text, Class::Ord, Font::Upright) }
            }
            '-' => M::Sym('−', Class::Bin),
            '*' => M::Sym('∗', Class::Bin),
            c => {
                let mapped = if c.is_ascii_alphanumeric() { symbols::styled(c, self.font) } else { c };
                M::Sym(mapped, symbols::class_of(c))
            }
        })
    }

    fn command(&mut self) -> M<'a> {
        let start = self.pos;
        let n = self.name();
        let (kind, [a, b, _]) = symbols::lookup(n);
        let digit = (a as usize).wrapping_sub('0' as usize);
        if let Some(class) = symbols::class(kind) {
            return M::Sym(a, class);
        }
        match kind {
            b'S' => M::Space(SPACES[digit & 7]),
            b'F' => {
                let num = bx(self.arg());
                let den = bx(self.arg());
                let binom = a == '1';
                M::Frac { num, den, rule: !binom, delims: binom.then_some(('(', ')')) }
            }
            b'R' => {
                let degree = self.optional_arg().map(bx);
                M::Sqrt { degree, body: bx(self.arg()) }
            }
            b'T' => M::Text(self.raw_group(), symbols::class(b as u8).unwrap_or(Class::Op), FONTS[digit]),
            b'M' => self.with_font(FONTS[digit]),
            b'L' => {
                let left = self.delimiter();
                let body = bx(M::Row(self.row(NONE)));
                let mut right = None;
                if self.rest().starts_with(b"\\right") {
                    self.pos += 6;
                    right = self.delimiter();
                }
                M::Delimited { left, right, body }
            }
            b'G' => {
                let d = self.delimiter().unwrap_or('.');
                M::BigDelim(d, BIG[digit & 3], symbols::class(b as u8).unwrap_or(Class::Ord))
            }
            b'H' => {
                let body = bx(self.arg());
                match a {
                    '0' => M::OverLine(body),
                    '1' => M::UnderLine(body),
                    _ => M::Brace { over: a == '2', body },
                }
            }
            b'E' => self.environment(),
            b'N' => empty(),
            b'C' => {
                let rgb = symbols::color(self.raw_group());
                M::Color(rgb, bx(self.arg()))
            }
            b'X' => match self.arg() {
                M::Sym(c, class) => M::Sym(symbols::negate(c), class),
                other => other,
            },
            b'B' => {
                self.skip_ws();
                let r = self.rest();
                let limits = if r.starts_with(b"\\limits") {
                    self.pos += 7;
                    Some(true)
                } else if r.starts_with(b"\\nolimits") {
                    self.pos += 9;
                    Some(false)
                } else {
                    None
                };
                M::BigOp { sym: a, limits, integral: b == '1' }
            }
            b'Z' => M::LimitFunc(n),
            b'Y' => M::Text(n, Class::Op, Font::Upright),
            b'A' => M::Accent { accent: a, body: bx(self.arg()), stretchy: b == '1' },
            _ => M::Error(&self.s[start - 1..self.pos]),
        }
    }

    /// A delimiter after `\left`, `\right`, `\big`: a character or a delimiter command; `.` is none.
    fn delimiter(&mut self) -> Option<char> {
        self.skip_ws();
        let c = match self.bump_char()? {
            '\\' => {
                let (kind, [c, flag, _]) = symbols::lookup(self.name());
                (symbols::class(kind).is_some() && flag == 'd').then_some(c)?
            }
            c => c,
        };
        (c != '.').then_some(c)
    }

    fn environment(&mut self) -> M<'a> {
        let env = self.raw_group();
        let (kind, [l, r, flag]) = symbols::lookup(env.trim_end_matches('*'));
        let (mut left, mut right, mut cases) = (None, None, false);
        if kind == b'V' {
            left = (l != '.').then_some(l);
            right = (r != '.').then_some(r);
            cases = flag == '1';
            if flag == '2' {
                self.raw_group(); // array column spec
            }
        }
        let mut rows: Vec<Vec<M<'a>>> = Vec::new();
        let mut cells: Vec<M<'a>> = Vec::new();
        loop {
            cells.push(M::Row(self.row(NONE)));
            self.skip_ws();
            let rest = self.rest();
            if rest.first() == Some(&b'&') {
                self.pos += 1;
            } else if rest.starts_with(b"\\\\") {
                self.pos += 2;
                self.optional_arg(); // \\[2pt]
                rows.push(std::mem::take(&mut cells));
            } else if rest.starts_with(b"\\end") {
                self.pos += 4;
                self.raw_group();
                break;
            } else if rest.is_empty() {
                break;
            } else {
                // Stray closing token; skip it to make progress.
                self.bump_char();
            }
        }
        // A trailing `\\` leaves an empty last row: drop it.
        if rows.is_empty() || !cells.iter().all(|c| matches!(c, M::Row(v) if v.is_empty())) {
            rows.push(cells);
        }
        // `aligned`: right-align odd columns, left-align even ones (approximated as "relation centred").
        let align_left = cases || env.starts_with("align") || env == "split";
        M::Matrix { rows, left, right, align_left, cases }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_and_fracs() {
        let m = parse(r"x^2 + \frac{a}{b}");
        let M::Row(items) = m else { panic!() };
        assert!(matches!(items[0], M::Scripts { .. }));
        assert!(matches!(items[1], M::Sym('+', Class::Bin)));
        assert!(matches!(items[2], M::Frac { .. }));
    }

    #[test]
    fn italic_letters() {
        let M::Row(items) = parse("ab") else { panic!() };
        assert_eq!(items[0], M::Sym('𝑎', Class::Ord));
    }

    #[test]
    fn matrix_env() {
        let M::Row(items) = parse(r"\begin{pmatrix} 1 & 2 \\ 3 & 4 \end{pmatrix}") else { panic!() };
        let M::Matrix { rows, left, .. } = &items[0] else { panic!("{items:?}") };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].len(), 2);
        assert_eq!(*left, Some('('));
    }

    #[test]
    fn left_right() {
        let M::Row(items) = parse(r"\left( \frac12 \right]") else { panic!() };
        assert!(matches!(&items[0], M::Delimited { left: Some('('), right: Some(']'), .. }));
    }

    #[test]
    fn integral_with_limits() {
        let M::Row(items) = parse(r"\int_0^1 x\,dx") else { panic!() };
        let M::Scripts { base, sup, sub } = &items[0] else { panic!() };
        assert!(matches!(**base, M::BigOp { sym: '∫', .. }));
        assert!(sup.is_some() && sub.is_some());
    }

    /// Compact, format-stable dump of a tree (independent of `M`'s Debug derive).
    fn dump(m: &M, o: &mut String) {
        use std::fmt::Write;
        let opt = |m: &Option<Box<M>>, o: &mut String| match m {
            Some(m) => dump(m, o),
            None => o.push('-'),
        };
        match m {
            M::Row(v) => {
                o.push('[');
                for (i, x) in v.iter().enumerate() {
                    if i > 0 {
                        o.push(' ');
                    }
                    dump(x, o);
                }
                o.push(']');
            }
            M::Sym(c, k) => write!(o, "{c:?}:{k:?}").unwrap(),
            M::Text(s, k, f) => {
                let s: String = s.chars().map(|c| symbols::styled(c, *f)).collect();
                write!(o, "{s:?}:{k:?}").unwrap()
            }
            M::Frac { num, den, rule, delims } => {
                write!(o, "Frac({rule},{delims:?} ").unwrap();
                dump(num, o);
                o.push(' ');
                dump(den, o);
                o.push(')');
            }
            M::Sqrt { degree, body } => {
                o.push_str("Sqrt(");
                opt(degree, o);
                o.push(' ');
                dump(body, o);
                o.push(')');
            }
            M::Scripts { base, sup, sub } => {
                o.push_str("Scripts(");
                dump(base, o);
                o.push_str(" ^");
                opt(sup, o);
                o.push_str(" _");
                opt(sub, o);
                o.push(')');
            }
            M::BigOp { sym, limits, integral } => write!(o, "BigOp({sym:?},{limits:?},{integral})").unwrap(),
            M::LimitFunc(s) => write!(o, "LimitFunc({s:?})").unwrap(),
            M::Delimited { left, right, body } => {
                write!(o, "Delimited({left:?},{right:?} ").unwrap();
                dump(body, o);
                o.push(')');
            }
            M::BigDelim(c, f, k) => write!(o, "BigDelim({c:?},{f:?},{k:?})").unwrap(),
            M::Accent { accent, body, stretchy } => {
                write!(o, "Accent({accent:?},{stretchy} ").unwrap();
                dump(body, o);
                o.push(')');
            }
            M::OverLine(b) | M::UnderLine(b) | M::Brace { body: b, .. } | M::Color(_, b) => {
                match m {
                    M::OverLine(_) => o.push_str("OverLine("),
                    M::UnderLine(_) => o.push_str("UnderLine("),
                    M::Brace { over, .. } => write!(o, "Brace({over} ").unwrap(),
                    M::Color(c, _) => write!(o, "Color({c:?} ").unwrap(),
                    _ => unreachable!(),
                }
                dump(b, o);
                o.push(')');
            }
            M::Matrix { rows, left, right, align_left, cases } => {
                write!(o, "Matrix({left:?},{right:?},{align_left},{cases}").unwrap();
                for r in rows {
                    o.push_str(" {");
                    for c in r {
                        o.push(' ');
                        dump(c, o);
                    }
                    o.push_str(" }");
                }
                o.push(')');
            }
            M::Space(em) => write!(o, "Space({em:?})").unwrap(),
            M::Error(s) => write!(o, "Error({s:?})").unwrap(),
        }
    }

    const CORPUS: &str = include_str!("../../assets/math-tests/corpus.txt");

    /// Every corpus formula must produce exactly the recorded tree
    /// (`UPDATE_GOLDEN=1 cargo test golden` rewrites the file).
    #[test]
    fn golden_trees() {
        let mut out = String::new();
        for line in CORPUS.lines() {
            out.push_str(line);
            out.push_str("\n=> ");
            dump(&parse(line), &mut out);
            out.push('\n');
        }
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/math-tests/golden.txt");
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(path, &out).unwrap();
        }
        let want = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        for (i, (a, b)) in want.lines().zip(out.lines()).enumerate() {
            assert_eq!(a, b, "golden line {}", i + 1);
        }
        assert_eq!(want.lines().count(), out.lines().count());
    }

    /// `cargo test --release bench_parse -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_parse() {
        let lines: Vec<&str> = CORPUS.lines().collect();
        let rounds = 5000 / lines.len() + 1;
        let mut best = f64::MAX;
        for _ in 0..20 {
            let t = std::time::Instant::now();
            for _ in 0..rounds {
                for l in &lines {
                    std::hint::black_box(parse(std::hint::black_box(l)));
                }
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        let n = rounds * lines.len();
        println!("parsed {n} formulas: best {:.3} ms ({:.2} us/formula)", best * 1e3, best * 1e6 / n as f64);
    }

    #[test]
    fn no_errors_for_common_input() {
        for src in [
            r"e^{i\pi} + 1 = 0",
            r"\sum_{k=1}^{n} k = \frac{n(n+1)}{2}",
            r"\lim_{x \to 0} \frac{\sin x}{x} = 1",
            r"f(x) = \begin{cases} 1 & x > 0 \\ 0 & \text{otherwise} \end{cases}",
            r"\sqrt[3]{\alpha^2 + \beta_i'}",
            r"\mathbb{R}^n \subseteq \mathcal{H}",
            r"\hat{x} \cdot \vec{v} \leq \|v\|",
        ] {
            let s = format!("{:?}", parse(src));
            assert!(!s.contains("Error"), "{src}: {s}");
        }
    }
}
