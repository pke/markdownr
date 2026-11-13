//! AST transforms, the Rust counterpart of the iOS app's `astTransforms.ts`.
//!
//! Each transform is a plain `fn(&mut Node)`; `compose` chains them. Syntax that
//! pulldown-cmark does not know (`==mark==`, `++ins++`, `:emoji:`) is recognised on
//! text nodes after parsing and turned into first-class node kinds.

use crate::ast::{Kind, Node};
use std::collections::HashMap;

pub type Transform = fn(&mut Node);

pub fn compose(node: &mut Node, transforms: &[Transform]) {
    for t in transforms {
        t(node);
    }
}

/// The default pipeline used by the viewer.
pub fn default_pipeline() -> Vec<Transform> {
    vec![quote_cycle_transform, footnote_transform, mark_transform, ins_transform, typographic_transform, emoji_transform, abbreviation_transform, sup_sub_transform, heading_ids]
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
    let mut out = Vec::with_capacity(node.children.len());
    for mut child in std::mem::take(&mut node.children) {
        if child.kind == Kind::Text {
            if let Some(replacement) = f(&child.text) {
                out.extend(replacement);
                continue;
            }
        } else {
            rewrite_text_nodes(&mut child, f);
        }
        out.push(child);
    }
    node.children = out;
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

#[path = "emoji_data.rs"]
mod emoji_data;

pub fn emoji(name: &str) -> Option<&'static str> {
    if name.is_empty() || name.contains(['\t', '\n']) {
        return None;
    }
    let data = emoji_data::EMOJI;
    let key = format!("\n{name}\t");
    let start = data.find(&key)? + key.len();
    let end = data[start..].find('\n')? + start;
    Some(&data[start..end])
}

/// Emoticon shortcuts (markdown-it-emoji `shortcuts.mjs`), longest first.
const EMOTICONS: &[(&str, &str)] = &[
    (":,'-(", "sob"), (">:-(", "angry"), (":-\")", "blush"), (":'-(", "cry"), (":,-(", "cry"), ("]:-(", "imp"), ("o:-)", "innocent"),
    ("O:-)", "innocent"), ("0:-)", "innocent"), (":'-)", "joy"), (":,-)", "joy"), (":'-D", "joy"), (":,-D", "joy"), ("]:-)", "smiling_imp"),
    (":,'(", "sob"), (",:-)", "sweat_smile"), (">:(", "angry"), (":\")", "blush"), ("</3", "broken_heart"), ("<\\3", "broken_heart"),
    (":-/", "confused"), (":'(", "cry"), (":,(", "cry"), (":-(", "frowning"), ("]:(", "imp"), ("o:)", "innocent"), ("O:)", "innocent"),
    ("0:)", "innocent"), (":')", "joy"), (":,)", "joy"), (":'D", "joy"), (":,D", "joy"), (":-*", "kissing"), ("x-)", "laughing"),
    ("X-)", "laughing"), (":-|", "neutral_face"), (":-o", "open_mouth"), (":-O", "open_mouth"), (":-@", "rage"), (":-D", "smile"),
    (":-)", "smiley"), ("]:)", "smiling_imp"), (";-(", "sob"), (":-P", "stuck_out_tongue"), ("8-)", "sunglasses"), ("B-)", "sunglasses"),
    (",:(", "sweat"), (",:)", "sweat_smile"), (":-S", "unamused"), (":-Z", "unamused"), (":-$", "unamused"), (";-)", "wink"),
    (":/", "confused"), (":(", "frowning"), ("<3", "heart"), (":*", "kissing"), (":|", "neutral_face"), (":o", "open_mouth"),
    (":O", "open_mouth"), (":@", "rage"), (":D", "smile"), (":)", "smiley"), (";(", "sob"), (":P", "stuck_out_tongue"), (":s", "unamused"),
    (":z", "unamused"), (":$", "unamused"), (";)", "wink"),
];

pub fn emoji_transform(node: &mut Node) {
    rewrite_text_nodes(node, &|t| {
        let replaced = replace_emoji(t);
        (replaced != t).then(|| vec![Node::text(replaced)])
    });
}

fn replace_emoji(text: &str) -> String {
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
        // Emoticons only as separate words (keeps `http://` and `a:b` intact).
        let at_start = out.is_empty() || out.ends_with(char::is_whitespace) || out.ends_with(['(', '"']);
        if at_start {
            for (face, name) in EMOTICONS {
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

// ---------------------------------------------------------------------------
// Typographic replacements   (c) → ©   "quotes" → „quotes"   -- → –
// ---------------------------------------------------------------------------

static LOCALE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Set once by the platform shell (e.g. `de-DE`); controls the quote style.
pub fn set_locale(locale: &str) {
    let _ = LOCALE.set(locale.to_string());
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
pub fn typographic(text: &str, locale: &str) -> String {
    let [od, cd, os, cs] = quotes(locale);
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let run = |ch: char| chars[i..].iter().take_while(|&&x| x == ch).count();
        let before = out.chars().next_back();
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
            '"' | '\'' => {
                let after = chars.get(i + 1).copied();
                let opening = before.is_none_or(|b| b.is_whitespace() || "([{".contains(b)) && after.is_some_and(|a| !a.is_whitespace());
                out.push_str(match (c, opening) {
                    ('"', true) => od,
                    ('"', false) => cd,
                    (_, true) => os,
                    _ => cs,
                });
                i += 1;
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
    let locale = LOCALE.get().map(String::as_str).unwrap_or("en");
    rewrite_text_nodes(node, &|t| {
        let r = typographic(t, locale);
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
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    rewrite_text_nodes(doc, &|text| {
        let mut parts = Vec::new();
        let mut last = 0;
        let mut i = 0;
        while i < text.len() {
            let hit = defs.iter().find(|(t, _)| {
                text[i..].starts_with(t.as_str()) && !word(text[..i].chars().next_back()) && !word(text[i + t.len()..].chars().next())
            });
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
    });
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
            let label = format!("__inline_{}", counter.get());
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
            *label = n.to_string();
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
            d.kind = Kind::FootnoteDef { label: n.to_string() };
            // Back-link to the (first) reference.
            let back = Node::with_children(Kind::Link { url: format!("#fnref-{n}") }, vec![Node::text("\u{21A9}")]);
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
        assert_eq!(typographic("\"Hallo\" -- (c) 2026...", "de-DE"), "\u{201E}Hallo\u{201C} \u{2013} \u{00A9} 2026\u{2026}");
        assert_eq!(typographic("a---b +-1 (tm) it's", "en"), "a\u{2014}b \u{00B1}1 \u{2122} it\u{2019}s");
    }

    #[test]
    fn full_emoji_set() {
        assert_eq!(emoji("smiley"), Some("😃"));
        assert_eq!(replace_emoji("hi :) and :heart_eyes: http://x"), "hi 😃 and 😍 http://x");
    }

    #[test]
    fn html_preprocess() {
        assert_eq!(preprocess_html("<b>x</b> `<b>`\n"), "**x** `<b>`\n");
    }
}

/// GitHub-style slug ids for headings (`## Hello World` → `hello-world`), unique per document.
pub fn heading_ids(node: &mut Node) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    fn walk(node: &mut Node, seen: &mut HashMap<String, usize>) {
        if let Kind::Heading { id, .. } = &mut node.kind {
            if id.is_empty() {
                let base = slug(&node_plain(&node.children));
                let n = seen.entry(base.clone()).or_insert(0);
                *id = if *n == 0 { base } else { format!("{base}-{n}") };
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
}
