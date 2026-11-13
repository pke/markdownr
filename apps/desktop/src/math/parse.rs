//! LaTeX math (the common subset used in Markdown documents) → math tree.

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

#[derive(Debug, Clone, PartialEq)]
pub enum M {
    Row(Vec<M>),
    /// A single symbol (already mapped to its final code point).
    Sym(char, Class),
    /// Upright text run (`\text{}`, function names like `sin`).
    Text(String, Class),
    Frac { num: Box<M>, den: Box<M>, rule: bool, delims: Option<(char, char)> },
    Sqrt { degree: Option<Box<M>>, body: Box<M> },
    Scripts { base: Box<M>, sup: Option<Box<M>>, sub: Option<Box<M>> },
    /// Large operator; `limits`: Some(true) = \limits, Some(false) = \nolimits, None = default.
    BigOp { sym: char, limits: Option<bool>, integral: bool },
    /// Function name with display-style limits (`\lim`, `\max`).
    LimitFunc(String),
    Delimited { left: Option<char>, right: Option<char>, body: Box<M> },
    /// `\big(` etc.: a delimiter at a fixed size (multiple of the font size).
    BigDelim(char, f32, Class),
    Accent { accent: char, body: Box<M>, stretchy: bool },
    OverLine(Box<M>),
    UnderLine(Box<M>),
    /// `\overbrace`, `\underbrace` (stretched horizontal glyph).
    Brace { over: bool, body: Box<M> },
    Matrix { rows: Vec<Vec<M>>, left: Option<char>, right: Option<char>, align_left: bool, cases: bool },
    /// Horizontal space in em.
    Space(f32),
    /// Text color (`\color{red}{...}`)
    Color([u8; 3], Box<M>),
    Error(String),
}

struct Parser<'a> {
    s: &'a str,
    pos: usize,
    font: Font,
}

pub fn parse(src: &str) -> M {
    let mut p = Parser { s: src, pos: 0, font: Font::Italic };
    let row = p.row(None);
    M::Row(row)
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.s[self.pos..].chars().next()
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }
    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }
    fn command_name(&mut self) -> String {
        let start = self.pos;
        if self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                self.bump();
            }
        } else {
            self.bump();
        }
        self.s[start..self.pos].to_string()
    }
    /// Raw `{...}` contents (balanced), for \text and environment names.
    fn raw_group(&mut self) -> String {
        self.skip_ws();
        if self.peek() != Some('{') {
            return self.bump().map(String::from).unwrap_or_default();
        }
        self.bump();
        let start = self.pos;
        let mut depth = 1;
        while let Some(c) = self.bump() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return self.s[start..self.pos - 1].to_string();
                    }
                }
                '\\' => {
                    self.bump();
                }
                _ => {}
            }
        }
        self.s[start..].to_string()
    }
    fn optional_arg(&mut self) -> Option<M> {
        self.skip_ws();
        if self.peek() != Some('[') {
            return None;
        }
        self.bump();
        let row = self.row(Some(']'));
        Some(M::Row(row))
    }

    /// One argument: a `{group}` or a single token.
    fn arg(&mut self) -> M {
        self.skip_ws();
        match self.peek() {
            Some('{') => {
                self.bump();
                M::Row(self.row(Some('}')))
            }
            // A bare argument is a single token: `\frac12` is ½.
            Some(c) if c.is_ascii_digit() => {
                self.bump();
                M::Text(c.to_string(), Class::Ord)
            }
            Some(_) => self.atom().unwrap_or(M::Row(vec![])),
            None => M::Row(vec![]),
        }
    }

    fn with_font(&mut self, font: Font) -> M {
        let saved = self.font;
        self.font = font;
        let m = self.arg();
        self.font = saved;
        m
    }

    /// Parse until `end` (consumed) or end of input. Handles `\over`, `&`/`\\` are left to matrices.
    fn row(&mut self, end: Option<char>) -> Vec<M> {
        let mut items: Vec<M> = Vec::new();
        loop {
            self.skip_ws();
            let Some(c) = self.peek() else { break };
            if Some(c) == end {
                self.bump();
                break;
            }
            if c == '}' && end.is_none() {
                self.bump();
                continue;
            }
            if c == '^' || c == '_' {
                self.bump();
                let script = self.arg();
                let base = items.pop().unwrap_or(M::Row(vec![]));
                let (base, mut sup, mut sub) = match base {
                    M::Scripts { base, sup, sub } => (base, sup, sub),
                    other => (Box::new(other), None, None),
                };
                if c == '^' {
                    sup = Some(Box::new(script));
                } else {
                    sub = Some(Box::new(script));
                }
                items.push(M::Scripts { base, sup, sub });
                continue;
            }
            if c == '\'' {
                self.bump();
                let base = items.pop().unwrap_or(M::Row(vec![]));
                let prime = M::Sym('′', Class::Ord);
                let (base, sup, sub) = match base {
                    M::Scripts { base, sup, sub } => (base, sup, sub),
                    other => (Box::new(other), None, None),
                };
                let sup = Some(Box::new(match sup {
                    Some(s) => M::Row(vec![*s, prime]),
                    None => prime,
                }));
                items.push(M::Scripts { base, sup, sub });
                continue;
            }
            if self.s[self.pos..].starts_with("\\over") && !self.s[self.pos + 5..].starts_with(|ch: char| ch.is_ascii_alphabetic()) {
                self.pos += 5;
                let num = M::Row(std::mem::take(&mut items));
                let den = M::Row(self.row(end));
                return vec![M::Frac { num: Box::new(num), den: Box::new(den), rule: true, delims: None }];
            }
            if self.s[self.pos..].starts_with("\\right") || (end.is_none() && self.s[self.pos..].starts_with("\\end")) {
                break;
            }
            if c == '&' || self.s[self.pos..].starts_with("\\\\") {
                break; // handled by matrix
            }
            match self.atom() {
                Some(m) => items.push(m),
                None => break,
            }
        }
        items
    }

    fn atom(&mut self) -> Option<M> {
        self.skip_ws();
        let c = self.bump()?;
        Some(match c {
            '{' => M::Row(self.row(Some('}'))),
            '\\' => self.command(),
            '0'..='9' | '.' if self.font == Font::Italic => {
                // Numbers stay together and upright.
                let start = self.pos - 1;
                while self.peek().is_some_and(|d| d.is_ascii_digit() || d == '.') {
                    self.bump();
                }
                let text = &self.s[start..self.pos];
                if text == "." { M::Sym('.', Class::Punct) } else { M::Text(text.to_string(), Class::Ord) }
            }
            c => self.symbol(c),
        })
    }

    fn symbol(&self, c: char) -> M {
        let class = symbols::class_of(c);
        let mapped = if c.is_ascii_alphanumeric() { symbols::styled(c, self.font) } else { c };
        let mapped = match c {
            '-' => '−',
            '*' => '∗',
            _ => mapped,
        };
        M::Sym(mapped, class)
    }

    fn command(&mut self) -> M {
        let name = self.command_name();
        let n = name.as_str();
        // Spacing
        match n {
            "," => return M::Space(3.0 / 18.0),
            ":" | ">" => return M::Space(4.0 / 18.0),
            ";" => return M::Space(5.0 / 18.0),
            "!" => return M::Space(-3.0 / 18.0),
            " " => return M::Space(0.25),
            "quad" => return M::Space(1.0),
            "qquad" => return M::Space(2.0),
            "{" => return M::Sym('{', Class::Open),
            "}" => return M::Sym('}', Class::Close),
            "|" => return M::Sym('‖', Class::Ord),
            "\\" => return M::Space(0.0),
            "%" | "$" | "#" | "&" | "_" => return M::Sym(n.chars().next().unwrap(), Class::Ord),
            _ => {}
        }
        match n {
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let num = self.arg();
                let den = self.arg();
                M::Frac { num: Box::new(num), den: Box::new(den), rule: true, delims: None }
            }
            "binom" | "dbinom" | "tbinom" => {
                let num = self.arg();
                let den = self.arg();
                M::Frac { num: Box::new(num), den: Box::new(den), rule: false, delims: Some(('(', ')')) }
            }
            "sqrt" => {
                let degree = self.optional_arg().map(Box::new);
                let body = Box::new(self.arg());
                M::Sqrt { degree, body }
            }
            "text" | "textrm" | "mbox" | "textnormal" | "textup" => M::Text(self.raw_group(), Class::Ord),
            "textbf" => M::Text(self.raw_group().chars().map(|c| symbols::styled(c, Font::Bold)).collect(), Class::Ord),
            "textit" => M::Text(self.raw_group().chars().map(|c| symbols::styled(c, Font::Italic)).collect(), Class::Ord),
            "operatorname" => M::Text(self.raw_group(), Class::Op),
            "mathrm" | "rm" => self.with_font(Font::Upright),
            "mathbf" | "bf" | "boldsymbol" | "bm" => self.with_font(if n == "mathbf" || n == "bf" { Font::Bold } else { Font::BoldItalic }),
            "mathit" | "it" => self.with_font(Font::Italic),
            "mathbb" => self.with_font(Font::DoubleStruck),
            "mathcal" | "mathscr" => self.with_font(Font::Script),
            "mathfrak" => self.with_font(Font::Fraktur),
            "mathsf" => self.with_font(Font::SansSerif),
            "mathtt" => self.with_font(Font::Mono),
            "left" => self.delimited(),
            "big" | "Big" | "bigg" | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr" | "biggl" | "biggr" | "Biggl" | "Biggr" => {
                let size = match n.trim_end_matches(['l', 'r']) {
                    "big" => 1.2,
                    "Big" => 1.8,
                    "bigg" => 2.4,
                    _ => 3.0,
                };
                let class = if n.ends_with('l') { Class::Open } else if n.ends_with('r') { Class::Close } else { Class::Ord };
                let d = self.delimiter().unwrap_or('.');
                M::BigDelim(d, size, class)
            }
            "overline" => M::OverLine(Box::new(self.arg())),
            "underline" => M::UnderLine(Box::new(self.arg())),
            "overbrace" => M::Brace { over: true, body: Box::new(self.arg()) },
            "underbrace" => M::Brace { over: false, body: Box::new(self.arg()) },
            "begin" => self.environment(),
            "limits" | "nolimits" | "displaystyle" | "textstyle" | "scriptstyle" | "nonumber" | "notag" => M::Row(vec![]),
            "color" | "textcolor" => {
                let name = self.raw_group();
                let body = self.arg();
                M::Color(symbols::color(&name), Box::new(body))
            }
            "not" => {
                let next = self.arg();
                match next {
                    M::Sym(c, class) => M::Sym(symbols::negate(c), class),
                    other => other,
                }
            }
            _ => {
                if let Some((sym, integral)) = symbols::big_operator(n) {
                    self.skip_ws();
                    let limits = if self.s[self.pos..].starts_with("\\limits") {
                        self.pos += 7;
                        Some(true)
                    } else if self.s[self.pos..].starts_with("\\nolimits") {
                        self.pos += 9;
                        Some(false)
                    } else {
                        None
                    };
                    return M::BigOp { sym, limits, integral };
                }
                if symbols::LIMIT_FUNCS.contains(&n) {
                    return M::LimitFunc(n.to_string());
                }
                if symbols::FUNCS.contains(&n) {
                    return M::Text(n.to_string(), Class::Op);
                }
                if let Some((accent, stretchy)) = symbols::accent(n) {
                    let body = self.arg();
                    return M::Accent { accent, body: Box::new(body), stretchy };
                }
                if let Some((c, class)) = symbols::command(n) {
                    let c = if self.font == Font::Bold && c.is_alphabetic() { symbols::styled(c, Font::Bold) } else { c };
                    return M::Sym(c, class);
                }
                M::Error(["\\", n].concat())
            }
        }
    }

    fn delimiter(&mut self) -> Option<char> {
        self.skip_ws();
        let c = self.bump()?;
        if c != '\\' {
            return Some(c);
        }
        let name = self.command_name();
        symbols::delimiter(&name)
    }

    fn delimited(&mut self) -> M {
        let left = self.delimiter().filter(|&c| c != '.');
        let body = self.row(None);
        let mut right = None;
        if self.s[self.pos..].starts_with("\\right") {
            self.pos += 6;
            right = self.delimiter().filter(|&c| c != '.');
        }
        M::Delimited { left, right, body: Box::new(M::Row(body)) }
    }

    fn environment(&mut self) -> M {
        let env = self.raw_group();
        let (left, right, align_left, cases) = match env.trim_end_matches('*') {
            "pmatrix" => (Some('('), Some(')'), false, false),
            "bmatrix" => (Some('['), Some(']'), false, false),
            "Bmatrix" => (Some('{'), Some('}'), false, false),
            "vmatrix" => (Some('|'), Some('|'), false, false),
            "Vmatrix" => (Some('‖'), Some('‖'), false, false),
            "cases" => (Some('{'), None, true, true),
            "aligned" | "align" | "split" | "gathered" | "alignat" => (None, None, false, false),
            "array" => {
                self.raw_group(); // column spec
                (None, None, false, false)
            }
            _ => (None, None, false, false),
        };
        let mut rows: Vec<Vec<M>> = vec![vec![]];
        loop {
            let cell = self.row(None);
            rows.last_mut().unwrap().push(M::Row(cell));
            self.skip_ws();
            if self.peek() == Some('&') {
                self.bump();
            } else if self.s[self.pos..].starts_with("\\\\") {
                self.pos += 2;
                self.optional_arg(); // \\[2pt]
                rows.push(vec![]);
            } else if self.s[self.pos..].starts_with("\\end") {
                self.pos += 4;
                self.raw_group();
                break;
            } else if self.peek().is_none() {
                break;
            } else {
                // Stray closing token; skip it to make progress.
                self.bump();
            }
        }
        if rows.last().is_some_and(|r| r.iter().all(|c| matches!(c, M::Row(v) if v.is_empty()))) && rows.len() > 1 {
            rows.pop();
        }
        // `aligned`: right-align odd columns, left-align even ones (approximated as "relation centred").
        let align_left = align_left || env.starts_with("align") || env == "split";
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
