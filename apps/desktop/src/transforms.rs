//! AST transforms, the Rust counterpart of the iOS app's `astTransforms.ts`.
//!
//! Each transform is a plain `fn(&mut Node)`; `compose` chains them. Syntax that
//! pulldown-cmark does not know (`==mark==`, `++ins++`, `:emoji:`) is recognised on
//! text nodes after parsing and turned into first-class node kinds.

use crate::ast::{Kind, Node};
use crate::util::VecMap;

pub type Transform = fn(&mut Node);

pub fn compose(node: &mut Node, transforms: &[Transform]) {
    for t in transforms {
        t(node);
    }
}

/// The default pipeline used by the viewer.
pub fn default_pipeline() -> Vec<Transform> {
    vec![quote_cycle_transform, footnote_transform, mark_transform, ins_transform, typographic_transform, emoji_transform, abbreviation_transform, sup_sub_transform, unescape_transform, heading_ids]
}

/// Backslash-escaped characters back from the parser's marks ([`crate::ast::ESCAPE_MARK`]
/// + ASCII code), after the text transforms, which leave the marks alone: `\--`, `\...`,
/// `\:)` and `\==` stay as typed.
pub fn unescape_transform(node: &mut Node) {
    // Every mark is a 4-byte UTF-8 sequence starting with 0xF4.
    if node.text.as_bytes().contains(&0xF4) {
        let unmark = |c: char| match (c as u32).checked_sub(crate::ast::ESCAPE_MARK) {
            Some(b @ 0..=0x7F) => b as u8 as char,
            _ => c,
        };
        node.text = node.text.chars().map(unmark).collect();
    }
    if let Kind::Abbr { title } = &mut node.kind
        && title.as_bytes().contains(&0xF4)
    {
        let mut t = Node::leaf(Kind::Text, std::mem::take(title));
        unescape_transform(&mut t);
        *title = t.text;
    }
    node.children.iter_mut().for_each(unescape_transform);
}

/// Intra-word `x^2^` → `Kind::Sup`, `H~2~O` → `Kind::Sub` (pulldown-cmark only
/// recognises them at word boundaries).
pub fn sup_sub_transform(node: &mut Node) {
    rewrite_text_nodes(node, &|t| split_delimited(t, "^", &Kind::Sup));
    rewrite_text_nodes(node, &|t| split_delimited(t, "~", &Kind::Sub));
}

/// Apply `f` to every text node's parent, letting it replace the children list.
fn rewrite_text_nodes(node: &mut Node, f: &dyn Fn(&str) -> Option<Vec<Node>>) {
    // Code-like containers keep their text verbatim.
    if matches!(node.kind, Kind::CodeBlock { .. } | Kind::Code | Kind::InlineMath | Kind::DisplayMath) {
        return;
    }
    // In place: untouched containers keep their children vector (no reallocation).
    let mut i = 0;
    while i < node.children.len() {
        let child = &mut node.children[i];
        if child.kind == Kind::Text {
            if let Some(mut replacement) = f(&child.text) {
                let n = replacement.len();
                match replacement.pop() {
                    Some(only) if n == 1 => *child = only,
                    last => {
                        replacement.extend(last);
                        node.children.splice(i..=i, replacement);
                    }
                }
                i += n; // replacements are not visited again
                continue;
            }
        } else {
            rewrite_text_nodes(child, f);
        }
        i += 1;
    }
}

/// Split `text` at `delim ... delim` pairs into Text / `kind` nodes.
fn split_delimited(text: &str, delim: &str, kind: &Kind) -> Option<Vec<Node>> {
    let mut parts = Vec::new();
    let mut rest = text;
    let mut found = false;
    while let Some(open) = rest.find(delim) {
        let after = &rest[open + delim.len()..];
        let Some(close) = after.find(delim) else { break };
        let inner = &after[..close];
        // Like CommonMark emphasis: no whitespace just inside the delimiters.
        if inner.is_empty() || inner.starts_with(char::is_whitespace) || inner.ends_with(char::is_whitespace) {
            parts.push(Node::text(&rest[..open + delim.len()]));
            rest = after;
            continue;
        }
        found = true;
        if open > 0 {
            parts.push(Node::text(&rest[..open]));
        }
        parts.push(Node::with_children(kind.clone(), vec![Node::text(inner)]));
        rest = &after[close + delim.len()..];
    }
    if !found {
        return None;
    }
    if !rest.is_empty() {
        parts.push(Node::text(rest));
    }
    Some(merge_text(parts))
}

fn merge_text(nodes: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(nodes.len());
    for n in nodes {
        if let (Some(last), Kind::Text) = (out.last_mut(), &n.kind) {
            if last.kind == Kind::Text {
                last.text.push_str(&n.text);
                continue;
            }
        }
        out.push(n);
    }
    out
}

/// `==text==` → `Kind::Mark`
pub fn mark_transform(node: &mut Node) {
    rewrite_text_nodes(node, &|t| split_delimited(t, "==", &Kind::Mark));
}

/// `++text++` → `Kind::Ins`
pub fn ins_transform(node: &mut Node) {
    rewrite_text_nodes(node, &|t| split_delimited(t, "++", &Kind::Ins));
}

// ---------------------------------------------------------------------------
// Emoji   :grin: → 😁   :-) → 😃   (full markdown-it-emoji dataset)
// ---------------------------------------------------------------------------


pub fn emoji(name: &str) -> Option<&'static str> {
    if name.is_empty() || name.contains(['\t', '\n']) {
        return None;
    }
    let e = crate::tables::find(crate::tables::emoji(), name);
    (!e.is_empty()).then_some(e)
}

/// Emoticon shortcuts (markdown-it-emoji `shortcuts.mjs`), longest first: `face\tname`
/// lines (one string instead of 70 pairs of `&str`).
const EMOTICONS: &str = ":,'-(\tsob\n>:-(\tangry\n:-\")\tblush\n:'-(\tcry\n:,-(\tcry\n]:-(\timp\no:-)\tinnocent\nO:-)\tinnocent\n0:-)\tinnocent\n:'-)\tjoy\n:,-)\tjoy\n:'-D\tjoy\n:,-D\tjoy\n]:-)\tsmiling_imp\n:,'(\tsob\n,:-)\tsweat_smile\n>:(\tangry\n:\")\tblush\n</3\tbroken_heart\n<\\3\tbroken_heart\n:-/\tconfused\n:'(\tcry\n:,(\tcry\n:-(\tfrowning\n]:(\timp\no:)\tinnocent\nO:)\tinnocent\n0:)\tinnocent\n:')\tjoy\n:,)\tjoy\n:'D\tjoy\n:,D\tjoy\n:-*\tkissing\nx-)\tlaughing\nX-)\tlaughing\n:-|\tneutral_face\n:-o\topen_mouth\n:-O\topen_mouth\n:-@\trage\n:-D\tsmile\n:-)\tsmiley\n]:)\tsmiling_imp\n;-(\tsob\n:-P\tstuck_out_tongue\n8-)\tsunglasses\nB-)\tsunglasses\n,:(\tsweat\n,:)\tsweat_smile\n:-S\tunamused\n:-Z\tunamused\n:-$\tunamused\n;-)\twink\n:/\tconfused\n:(\tfrowning\n<3\theart\n:*\tkissing\n:|\tneutral_face\n:o\topen_mouth\n:O\topen_mouth\n:@\trage\n:D\tsmile\n:)\tsmiley\n;(\tsob\n:P\tstuck_out_tongue\n:s\tunamused\n:z\tunamused\n:$\tunamused\n;)\twink";

fn emoticons() -> impl Iterator<Item = (&'static str, &'static str)> {
    EMOTICONS.split('\n').filter_map(|l| l.split_once('\t'))
}

pub fn emoji_transform(node: &mut Node) {
    rewrite_text_nodes(node, &|t| replace_emoji(t).map(|r| vec![Node::text(r)]));
}

/// Bit set of the first bytes of all emoticons (all ASCII).
const EMOTICON_STARTS: u128 = {
    let b = EMOTICONS.as_bytes();
    let mut m = 1u128 << b[0];
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'\n' {
            m |= 1 << b[i + 1];
        }
        i += 1;
    }
    m
};

/// Replace `:name:` codes and stand-alone emoticons; None if nothing changed.
/// Allocates only once a replacement is found.
fn replace_emoji(text: &str) -> Option<String> {
    let b = text.as_bytes();
    let mut out: Option<String> = None;
    let mut copied = 0; // text[..copied] is already in `out`
    let mut i = 0;
    // Emoticons only as separate words (keeps `http://` and `a:b` intact): the
    // previous output char is none, whitespace, `(` or `"`.
    let mut word_start = true;
    while i < b.len() {
        let mut hit: Option<(&str, usize)> = None; // (emoji, bytes consumed)
        if b[i] == b':' {
            let name_len = b[i + 1..].iter().position(|&c| !(c.is_ascii_alphanumeric() || c == b'_' || c == b'+' || c == b'-'));
            if let Some(n) = name_len.filter(|&n| n > 0 && b[i + 1 + n] == b':') {
                hit = emoji(&text[i + 1..i + 1 + n]).map(|e| (e, n + 2));
            }
        }
        if hit.is_none() && word_start && b[i] < 128 && EMOTICON_STARTS & (1 << b[i]) != 0 {
            let rest = &text[i..];
            for (face, name) in emoticons() {
                if let Some(after) = rest.strip_prefix(face) {
                    if after.chars().next().is_none_or(|c| c.is_whitespace() || ".,!?)\"".contains(c)) {
                        if let Some(e) = emoji(name) {
                            hit = Some((e, face.len()));
                            break;
                        }
                    }
                }
            }
        }
        let last = match hit {
            Some((e, n)) => {
                let o = out.get_or_insert_with(|| String::with_capacity(text.len()));
                o.push_str(&text[copied..i]);
                o.push_str(e);
                i += n;
                copied = i;
                e.chars().next_back()
            }
            None => {
                let c = text[i..].chars().next();
                i += c.map_or(1, char::len_utf8);
                c
            }
        };
        word_start = last.is_some_and(|c| c.is_whitespace() || c == '(' || c == '"');
    }
    out.map(|mut o| {
        o.push_str(&text[copied..]);
        o
    })
}

// ---------------------------------------------------------------------------
// Typographic replacements   (c) → ©   -- → –   (quotes: the Markdown parser, see set_locale)
// ---------------------------------------------------------------------------

/// Set once by the platform shell (e.g. `de-DE`), before the first parse: the quote style.
/// The Markdown parser pairs the quotes (smart punctuation, also across emphasis and
/// links) and puts in this locale's; escaped ones (`\"`) stay as typed.
pub fn set_locale(locale: &str) {
    let [od, cd, os, cs] = quotes(locale);
    pulldown_cmark::set_smart_quotes([od, cd, os, cs, "\u{2019}"]);
}

fn quotes(locale: &str) -> [&'static str; 4] {
    match locale.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase().as_str() {
        "de" | "da" | "cs" => ["\u{201E}", "\u{201C}", "\u{201A}", "\u{2018}"],
        "fr" => ["\u{00AB}\u{00A0}", "\u{00A0}\u{00BB}", "\u{2039}\u{00A0}", "\u{00A0}\u{203A}"],
        "ru" => ["\u{00AB}", "\u{00BB}", "\u{201E}", "\u{201C}"],
        "ja" => ["\u{300C}", "\u{300D}", "\u{300E}", "\u{300F}"],
        "pl" | "hu" => ["\u{201E}", "\u{201D}", "\u{201A}", "\u{2019}"],
        "es" | "it" => ["\u{00AB}", "\u{00BB}", "\u{201C}", "\u{201D}"],
        "sv" => ["\u{201D}", "\u{201D}", "\u{2019}", "\u{2019}"],
        _ => ["\u{201C}", "\u{201D}", "\u{2018}", "\u{2019}"],
    }
}

/// The iOS `createTypographicTransform` rules, applied per text node.
pub fn typographic(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let run = |ch: char| chars[i..].iter().take_while(|&&x| x == ch).count();
        let before = out.chars().next_back();
        if before.is_some_and(|b| b as u32 == crate::ast::ESCAPE_MARK + c as u32) {
            // The rest of a run whose first character was escaped (`\...`, `\--`) stays.
            let n = run(c);
            out.extend(std::iter::repeat_n(c, n));
            i += n;
            continue;
        }
        match c {
            '-' => {
                let n = run('-');
                let after = chars.get(i + n).copied();
                let spaced = before.is_none_or(char::is_whitespace) && after.is_none_or(char::is_whitespace);
                let joined = before.is_some_and(|b| !b.is_whitespace()) && after.is_some_and(|a| !a.is_whitespace());
                if n == 3 {
                    out.push('\u{2014}');
                } else if n == 2 && (spaced || joined) {
                    out.push('\u{2013}');
                } else {
                    out.extend(std::iter::repeat_n('-', n));
                }
                i += n;
            }
            '.' if run('.') >= 2 => {
                let n = run('.');
                if matches!(before, Some('?' | '!')) {
                    out.push_str("..");
                } else {
                    out.push('\u{2026}');
                }
                i += n;
            }
            '+' if chars.get(i + 1) == Some(&'-') => {
                out.push('\u{00B1}');
                i += 2;
            }
            '(' if i + 2 < chars.len() && chars[i + 2] == ')' && "cCrRpP".contains(chars[i + 1]) => {
                out.push(match chars[i + 1].to_ascii_lowercase() {
                    'c' => '\u{00A9}',
                    'r' => '\u{00AE}',
                    _ => '\u{00A7}',
                });
                i += 3;
            }
            '(' if i + 3 < chars.len() && chars[i + 3] == ')' && chars[i + 1].eq_ignore_ascii_case(&'t') && chars[i + 2].eq_ignore_ascii_case(&'m') => {
                out.push('\u{2122}');
                i += 4;
            }
            '?' | '!' if run(c) >= 4 => {
                let n = run(c);
                out.extend(std::iter::repeat_n(c, 3));
                i += n;
            }
            ',' if run(',') >= 2 => {
                let n = run(',');
                out.push(',');
                i += n;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

pub fn typographic_transform(node: &mut Node) {
    rewrite_text_nodes(node, &|t| {
        // Only these characters can change; most text nodes have none of them.
        if !t.bytes().any(|b| matches!(b, b'-' | b'.' | b'+' | b'(' | b'?' | b'!' | b',')) {
            return None;
        }
        let r = typographic(t);
        (r != t).then(|| vec![Node::text(r)])
    });
}

// ---------------------------------------------------------------------------
// Abbreviations   *[HTML]: Hyper Text Markup Language
// ---------------------------------------------------------------------------

pub fn abbreviation_transform(doc: &mut Node) {
    let mut defs: Vec<(String, String)> = Vec::new();
    fn collect(node: &mut Node, defs: &mut Vec<(String, String)>) {
        node.children.retain_mut(|c| {
            if c.kind == Kind::Paragraph {
                let text = c.plain_text();
                let mut all = true;
                let mut found = Vec::new();
                // Soft breaks may flatten to nothing, so split on the `*[` markers.
                let text = text.trim();
                let all_defs = text.starts_with("*[");
                for line in text.split("*[").map(str::trim).filter(|l| !l.is_empty()) {
                    match Some(line).filter(|_| all_defs).and_then(|r| r.split_once("]:")) {
                        Some((term, full)) if !term.is_empty() => found.push((term.to_string(), full.trim().to_string())),
                        _ => all = false,
                    }
                }
                if all && !found.is_empty() {
                    defs.extend(found);
                    return false;
                }
            }
            collect(c, defs);
            true
        });
    }
    collect(doc, &mut defs);
    if defs.is_empty() {
        return;
    }
    defs.sort_by_key(|(t, _)| std::cmp::Reverse(t.len()));
    // Terms are only tried where they can match: at a word start, on their first byte.
    let mut first = [false; 256];
    for (t, _) in &defs {
        first[t.as_bytes()[0] as usize] = true;
    }
    rewrite_text_nodes(doc, &|text| abbreviate(text, &defs, &first));
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// Wrap whole-word occurrences of the `defs` terms (longest first) in `Kind::Abbr`.
fn abbreviate(text: &str, defs: &[(String, String)], first: &[bool; 256]) -> Option<Vec<Node>> {
    let b = text.as_bytes();
    let mut parts = Vec::new();
    let mut last = 0;
    let mut i = 0;
    let mut after_word = false; // is_word(text[..i].chars().next_back())
    while i < text.len() {
        let hit = (!after_word && first[b[i] as usize])
            .then(|| defs.iter().find(|(t, _)| text[i..].starts_with(t.as_str()) && !is_word(text[i + t.len()..].chars().next())))
            .flatten();
        if let Some((term, full)) = hit {
            if last < i {
                parts.push(Node::text(&text[last..i]));
            }
            parts.push(Node::with_children(Kind::Abbr { title: full.clone() }, vec![Node::text(term.as_str())]));
            i += term.len();
            last = i;
            after_word = is_word(term.chars().next_back());
        } else {
            let c = text[i..].chars().next();
            i += c.map_or(1, char::len_utf8);
            after_word = is_word(c);
        }
    }
    if parts.is_empty() {
        return None;
    }
    if last < text.len() {
        parts.push(Node::text(&text[last..]));
    }
    Some(parts)
}

// ---------------------------------------------------------------------------
// Footnotes: numbered by first reference, inline `^[text]` notes, all
// definitions collected at the end (back-links are added by the layout).
// ---------------------------------------------------------------------------

pub fn footnote_transform(doc: &mut Node) {
    // Inline notes first: `^[text]` in (merged) text nodes → ref + new definition.
    merge_adjacent_text(doc);
    let counter = std::cell::Cell::new(0usize);
    let inline_defs = std::cell::RefCell::new(Vec::new());
    rewrite_text_nodes(doc, &|text| {
        if !text.contains("^[") {
            return None;
        }
        let mut parts = Vec::new();
        let mut rest = text;
        while let Some(p) = rest.find("^[") {
            let Some(end) = rest[p + 2..].find(']') else { break };
            if p > 0 {
                parts.push(Node::text(&rest[..p]));
            }
            let label = ["__inline_", &crate::util::num(counter.get() as u64)].concat();
            counter.set(counter.get() + 1);
            inline_defs.borrow_mut().push((label.clone(), rest[p + 2..p + 2 + end].to_string()));
            parts.push(Node::leaf(Kind::FootnoteRef { label }, ""));
            rest = &rest[p + 2 + end + 1..];
        }
        if parts.is_empty() {
            return None;
        }
        if !rest.is_empty() {
            parts.push(Node::text(rest));
        }
        Some(parts)
    });

    // Pull all definitions out of the tree.
    let mut defs: Vec<Node> = Vec::new();
    fn take_defs(node: &mut Node, defs: &mut Vec<Node>) {
        let mut i = 0;
        while i < node.children.len() {
            if matches!(node.children[i].kind, Kind::FootnoteDef { .. }) {
                defs.push(node.children.remove(i));
            } else {
                take_defs(&mut node.children[i], defs);
                i += 1;
            }
        }
    }
    take_defs(doc, &mut defs);
    for (label, text) in inline_defs.into_inner() {
        defs.push(Node::with_children(Kind::FootnoteDef { label }, vec![Node::with_children(Kind::Paragraph, vec![Node::text(text)])]));
    }
    if defs.is_empty() {
        return;
    }
    // Number in order of first reference.
    let mut order: Vec<String> = Vec::new();
    fn refs(node: &mut Node, order: &mut Vec<String>) {
        if let Kind::FootnoteRef { label } = &mut node.kind {
            let n = match order.iter().position(|l| l == label) {
                Some(p) => p + 1,
                None => {
                    order.push(label.clone());
                    order.len()
                }
            };
            *label = crate::util::num(n as u64);
        }
        for c in &mut node.children {
            refs(c, order);
        }
    }
    refs(doc, &mut order);
    let mut numbered: Vec<(usize, Node)> = defs
        .into_iter()
        .filter_map(|mut d| {
            let Kind::FootnoteDef { label } = &d.kind else { return None };
            let n = order.iter().position(|l| l == label)? + 1;
            d.kind = Kind::FootnoteDef { label: crate::util::num(n as u64) };
            // Back-link to the (first) reference.
            let back = Node::with_children(Kind::Link { url: ["#fnref-", &crate::util::num(n as u64)].concat() }, vec![Node::text("\u{21A9}")]);
            match d.children.iter_mut().rev().find(|c| c.kind == Kind::Paragraph) {
                Some(p) => p.children.extend([Node::text(" "), back]),
                None => d.children.push(Node::with_children(Kind::Paragraph, vec![back])),
            }
            Some((n, d))
        })
        .collect();
    numbered.sort_by_key(|(n, _)| *n);
    if numbered.is_empty() {
        return;
    }
    doc.children.push(Node::new(Kind::Rule));
    doc.children.extend(numbered.into_iter().map(|(_, d)| d));
}

fn merge_adjacent_text(node: &mut Node) {
    if !node.children.is_empty() {
        node.children = merge_text(std::mem::take(&mut node.children));
        for c in &mut node.children {
            merge_adjacent_text(c);
        }
    }
}

// ---------------------------------------------------------------------------
// Quote cycle   :::quotes … :::  → Kind::QuoteCycle (shuffled blockquotes)
// ---------------------------------------------------------------------------

pub fn quote_cycle_transform(doc: &mut Node) {
    let is = |n: &Node, s: &str| n.kind == Kind::Paragraph && n.plain_text().trim() == s;
    let Some(start) = doc.children.iter().position(|n| is(n, ":::quotes")) else { return };
    let Some(len) = doc.children[start + 1..].iter().position(|n| is(n, ":::")) else { return };
    let end = start + 1 + len;
    let mut quotes: Vec<Node> = doc.children[start + 1..end].iter().filter(|n| matches!(n.kind, Kind::BlockQuote(_))).cloned().collect();
    if quotes.is_empty() {
        return;
    }
    // Shuffle (Fisher–Yates, time-seeded) so the order varies per open.
    let mut seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos()) | 1;
    for i in (1..quotes.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        quotes.swap(i, seed as usize % (i + 1));
    }
    doc.children.splice(start..=end, [Node::with_children(Kind::QuoteCycle, quotes)]);
}

// ---------------------------------------------------------------------------
// HTML preprocessing   <b>x</b> → **x**  (outside code)
// ---------------------------------------------------------------------------

/// Convert common inline HTML tags to Markdown before parsing, like the iOS
/// `preprocessMarkdownHtml`. Code spans and fenced blocks are left alone; `<br>`
/// stays HTML (the layout turns it into a line break, also inside tables).
pub fn preprocess_html(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut in_fence = false;
    for line in src.split_inclusive('\n') {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if in_fence || line.starts_with("    ") || line.starts_with('\t') {
            out.push_str(line);
            continue;
        }
        // Split on inline code spans; odd segments are code.
        for (i, seg) in line.split('`').enumerate() {
            if i > 0 {
                out.push('`');
            }
            if i % 2 == 1 {
                out.push_str(seg);
            } else {
                out.push_str(&html_tags(seg));
            }
        }
    }
    out
}

fn html_tags(s: &str) -> String {
    if !s.contains('<') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('<') {
        out.push_str(&rest[..p]);
        let tag = &rest[p..];
        let Some(end) = tag.find('>') else {
            out.push_str(tag);
            return out;
        };
        let inner = &tag[1..end];
        let name = inner.trim_start_matches('/').split(char::is_whitespace).next().unwrap_or("").to_ascii_lowercase();
        let md = match name.as_str() {
            "b" | "strong" => Some("**"),
            "i" | "em" => Some("*"),
            "s" | "del" | "strike" => Some("~~"),
            "u" | "ins" => Some("++"),
            "mark" => Some("=="),
            "sup" => Some("^"),
            "sub" => Some("~"),
            "code" => Some("`"),
            _ => None,
        };
        match md {
            Some(m) => out.push_str(m),
            None => out.push_str(&tag[..=end]),
        }
        rest = &tag[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod ios_parity_tests {
    use super::*;

    #[test]
    fn typography_de() {
        assert_eq!(typographic("Hallo -- (c) 2026..."), "Hallo \u{2013} \u{00A9} 2026\u{2026}");
        assert_eq!(typographic("a---b +-1 (tm)"), "a\u{2014}b \u{00B1}1 \u{2122}");
    }

    /// Quotes come from the parser (English here: tests do not set a locale), paired
    /// across emphasis; a lone single quote is an apostrophe, escaped quotes stay.
    #[test]
    fn quotes_from_the_parser() {
        let mut doc = crate::ast::parse("\"**Fett** am Anfang\" und 'nein', geht's \\\"wörtlich\\\"");
        compose(&mut doc, &default_pipeline());
        assert_eq!(doc.plain_text(), "\u{201C}Fett am Anfang\u{201D} und \u{2018}nein\u{2019}, geht\u{2019}s \"wörtlich\"");
    }

    /// Escaped characters are not transformed, and come out as typed.
    #[test]
    fn escapes_stay() {
        let mut doc = crate::ast::parse("a \\-- b \\... c \\:) d \\==x== e \\(c) f -- g...");
        compose(&mut doc, &default_pipeline());
        assert_eq!(doc.plain_text(), "a -- b ... c :) d ==x== e (c) f \u{2013} g\u{2026}");
    }

    #[test]
    fn full_emoji_set() {
        assert_eq!(emoji("smiley"), Some("😃"));
        assert_eq!(replace_emoji("hi :) and :heart_eyes: http://x").as_deref(), Some("hi 😃 and 😍 http://x"));
        assert_eq!(replace_emoji("no codes here"), None);
    }

    #[test]
    fn html_preprocess() {
        assert_eq!(preprocess_html("<b>x</b> `<b>`\n"), "**x** `<b>`\n");
    }
}

/// GitHub-style slug ids for headings (`## Hello World` → `hello-world`), unique per document.
pub fn heading_ids(node: &mut Node) {
    let mut seen: VecMap<String, usize> = VecMap::new();
    fn walk(node: &mut Node, seen: &mut VecMap<String, usize>) {
        if let Kind::Heading { id, .. } = &mut node.kind {
            if id.is_empty() {
                let base = slug(&node_plain(&node.children));
                let n = seen.or_insert_with(base.clone(), || 0);
                *id = if *n == 0 { base } else { [&base, "-", &crate::util::num(*n as u64)].concat() };
                *n += 1;
            }
        }
        for c in &mut node.children {
            walk(c, seen);
        }
    }
    walk(node, &mut seen);
}

fn node_plain(children: &[Node]) -> String {
    children.iter().map(Node::plain_text).collect()
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().chars() {
        if c.is_alphanumeric() || c == '-' || c == '_' {
            out.extend(c.to_lowercase());
        } else if c.is_whitespace() {
            out.push('-');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::parse;

    fn run(src: &str) -> Node {
        let mut d = parse(src);
        compose(&mut d, &default_pipeline());
        d
    }

    #[test]
    fn mark_and_ins() {
        let d = run("a ==hi== b ++new++ c");
        let p = &d.children[0];
        let kinds: Vec<_> = p.children.iter().map(|n| n.kind.clone()).collect();
        assert_eq!(kinds, vec![Kind::Text, Kind::Mark, Kind::Text, Kind::Ins, Kind::Text]);
        assert_eq!(p.children[1].plain_text(), "hi");
    }

    #[test]
    fn mark_needs_tight_delims() {
        let d = run("a == b == c");
        assert_eq!(d.children[0].children.len(), 1);
    }

    #[test]
    fn intraword_sup_sub() {
        let d = run("x^2^ and H~2~O, not ~/a ~/b");
        let kinds: Vec<_> = d.children[0].children.iter().map(|n| n.kind.clone()).collect();
        assert_eq!(kinds, vec![Kind::Text, Kind::Sup, Kind::Text, Kind::Sub, Kind::Text]);
        assert!(d.children[0].plain_text().ends_with("O, not ~/a ~/b"));
    }

    #[test]
    fn emoji() {
        let d = run("Ship it :rocket: :)");
        assert_eq!(d.children[0].plain_text(), "Ship it 🚀 😃");
    }

    #[test]
    fn emoji_not_in_code() {
        let d = run("`:rocket:`");
        assert_eq!(d.children[0].children[0].text, ":rocket:");
    }

    #[test]
    fn unique_heading_ids() {
        let d = run("# Hello World\n## Hello World\n");
        let ids: Vec<_> = d.children.iter().map(|h| match &h.kind {
            Kind::Heading { id, .. } => id.clone(),
            _ => unreachable!(),
        }).collect();
        assert_eq!(ids, vec!["hello-world", "hello-world-1"]);
    }

    /// The whole pipeline over the sample and test documents must produce exactly
    /// the recorded trees (hash per top-level block; `UPDATE_GOLDEN=1` rewrites).
    #[test]
    fn golden_documents() {
        let docs: [(&str, &str); 9] = [
            ("sample", include_str!("../assets/sample.md")),
            ("extensions", include_str!("../assets/extensions.md")),
            ("transforms", include_str!("../assets/transform-tests/transforms.md")),
            ("math1", include_str!("../assets/math-tests/1-basics.md")),
            ("math2", include_str!("../assets/math-tests/2-delims.md")),
            ("math3", include_str!("../assets/math-tests/3-envs.md")),
            ("math4", include_str!("../assets/math-tests/4-text-fonts.md")),
            ("math5", include_str!("../assets/math-tests/5-greek-symbols.md")),
            ("math6", include_str!("../assets/math-tests/6-errors.md")),
        ];
        let mut out = String::new();
        for (name, text) in docs {
            let mut d = parse(&preprocess_html(text));
            compose(&mut d, &default_pipeline());
            // quote_cycle shuffles its quotes by time: compare them as a sorted set.
            fn normalize(n: &mut Node) {
                n.children.iter_mut().for_each(normalize);
                if n.kind == Kind::QuoteCycle {
                    n.children.sort_by_cached_key(|c| format!("{c:?}"));
                }
            }
            normalize(&mut d);
            for (i, block) in d.children.iter().enumerate() {
                let dump = format!("{block:?}");
                let hash = dump.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
                let kind = format!("{:?}", block.kind);
                out.push_str(&format!("{name} {i} {hash:016x} {}
", kind.split([' ', '(', '{']).next().unwrap_or("")));
            }
        }
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/transform-tests/golden.txt");
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(path, &out).unwrap();
        }
        let want = std::fs::read_to_string(path).unwrap().replace("
", "
");
        for (i, (a, b)) in want.lines().zip(out.lines()).enumerate() {
            assert_eq!(a, b, "transforms golden line {}", i + 1);
        }
        assert_eq!(want.lines().count(), out.lines().count());
    }
}

#[cfg(test)]
mod perf_rewrite_tests {
    use super::*;

    /// The former emoji replacement, as the reference for the allocation-free one.
    fn old_replace_emoji(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        'outer: while !rest.is_empty() {
            if let Some(stripped) = rest.strip_prefix(':') {
                let end = stripped.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '+' || c == '-'));
                if let Some(end) = end.filter(|&e| e > 0 && stripped[e..].starts_with(':')) {
                    if let Some(e) = emoji(&stripped[..end]) {
                        out.push_str(e);
                        rest = &stripped[end + 1..];
                        continue;
                    }
                }
            }
            let at_start = out.is_empty() || out.ends_with(char::is_whitespace) || out.ends_with(['(', '"']);
            if at_start {
                for (face, name) in emoticons() {
                    if let Some(after) = rest.strip_prefix(face) {
                        if after.is_empty() || after.starts_with(|c: char| c.is_whitespace() || ".,!?)\"".contains(c)) {
                            if let Some(e) = emoji(name) {
                                out.push_str(e);
                                rest = after;
                                continue 'outer;
                            }
                        }
                    }
                }
            }
            let c = rest.chars().next().unwrap();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
        out
    }

    /// Pseudo-random texts from emoji/emoticon/typography fragments.
    fn texts() -> impl Iterator<Item = String> {
        const PARTS: [&str; 32] = [
            ":", ")", "(", "rocket", " ", "\u{a0}", "x", "-", "'", ",", "D", "<", "3", ";", "\"", "a", "é", "\t", "8", "B", "O", "0",
            ":heart_eyes:", ":+1:", "]", ">", "/", "\\", ".", "!", "?", "_",
        ];
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        (0..300_000).map(move |_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut s = String::new();
            let mut r = seed;
            for _ in 0..(r % 9) {
                r = r.rotate_right(5);
                s.push_str(PARTS[(r % 32) as usize]);
            }
            s
        })
    }

    #[test]
    fn emoji_matches_old_implementation() {
        for t in texts() {
            let old = old_replace_emoji(&t);
            let new = replace_emoji(&t);
            assert_eq!(new.as_deref().unwrap_or(&t), old, "{t:?}");
            assert_eq!(new.is_some(), old != t, "{t:?}");
        }
    }

    #[test]
    fn typographic_skip_is_exact() {
        // Text without trigger bytes is returned unchanged by `typographic`.
        for t in texts() {
            if !t.bytes().any(|b| matches!(b, b'-' | b'.' | b'+' | b'(' | b'?' | b'!' | b',')) {
                assert_eq!(typographic(&t), t);
            }
        }
    }

    #[test]
    fn every_emoji_name_is_found() {
        let t = crate::tables::emoji();
        let mut n = 0;
        for line in t.lines().filter(|l| !l.is_empty()) {
            let (name, value) = line.split_once('\t').unwrap();
            assert_eq!(emoji(name), Some(value), "{name}");
            n += 1;
        }
        assert!(n > 1800);
        let names: Vec<&str> = t.lines().filter(|l| !l.is_empty()).map(|l| l.split('\t').next().unwrap()).collect();
        assert!(names.windows(2).all(|w| w[0].as_bytes() < w[1].as_bytes()), "emoji.txt must be sorted");
        assert_eq!(emoji("rocketx"), None);
        assert_eq!(emoji("rocke"), None);
    }
}

#[cfg(test)]
mod abbr_rewrite_tests {
    use super::*;

    /// The former matcher, as the reference.
    fn old_abbreviate(text: &str, defs: &[(String, String)]) -> Option<Vec<Node>> {
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        let mut parts = Vec::new();
        let mut last = 0;
        let mut i = 0;
        while i < text.len() {
            let hit = defs.iter().find(|(t, _)| text[i..].starts_with(t.as_str()) && !word(text[..i].chars().next_back()) && !word(text[i + t.len()..].chars().next()));
            if let Some((term, full)) = hit {
                if last < i {
                    parts.push(Node::text(&text[last..i]));
                }
                parts.push(Node::with_children(Kind::Abbr { title: full.clone() }, vec![Node::text(term.as_str())]));
                i += term.len();
                last = i;
            } else {
                i += text[i..].chars().next().map_or(1, char::len_utf8);
            }
        }
        if parts.is_empty() {
            return None;
        }
        if last < text.len() {
            parts.push(Node::text(&text[last..]));
        }
        Some(parts)
    }

    #[test]
    fn abbreviate_matches_old_implementation() {
        let mut defs: Vec<(String, String)> =
            ["HTML", "W3C", "HTML5", "é", "_x", "a b", "CSS"].iter().map(|t| (t.to_string(), [*t, "!"].concat())).collect();
        defs.sort_by_key(|(t, _)| std::cmp::Reverse(t.len()));
        let mut first = [false; 256];
        for (t, _) in &defs {
            first[t.as_bytes()[0] as usize] = true;
        }
        const PARTS: [&str; 16] = ["HTML", "W3C", "5", "x", " ", "-", "_", "é", "CSS", "a", "b", "\u{a0}", "1", ".", "ü", "_x"];
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..200_000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut s = String::new();
            let mut r = seed;
            for _ in 0..(r % 8) {
                r = r.rotate_right(4);
                s.push_str(PARTS[(r % 16) as usize]);
            }
            assert_eq!(abbreviate(&s, &defs, &first), old_abbreviate(&s, &defs), "{s:?}");
        }
    }
}

#[cfg(test)]
mod emoticon_table_tests {
    /// All 68 shortcuts survive the packing, each with a known emoji, longest first.
    #[test]
    fn emoticons_intact() {
        let all: Vec<_> = super::emoticons().collect();
        assert_eq!(all.len(), 68);
        assert!(all.iter().all(|(f, n)| !f.is_empty() && super::emoji(n).is_some()));
        assert!(all.windows(2).all(|w| w[0].0.len() >= w[1].0.len()));
        assert!(all.contains(&(":-\")", "blush")) && all.contains(&("<\\3", "broken_heart")));
    }
}
