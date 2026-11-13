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
    vec![mark_transform, ins_transform, emoji_transform, heading_ids]
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

// A starter set; the iOS app uses the full markdown-it-emoji dataset, which can be
// generated into a static table later.
const EMOJI: &[(&str, &str)] = &[
    ("smile", "😄"), ("grin", "😁"), ("joy", "😂"), ("wink", "😉"), ("heart", "❤️"),
    ("thumbsup", "👍"), ("+1", "👍"), ("thumbsdown", "👎"), ("-1", "👎"), ("tada", "🎉"),
    ("rocket", "🚀"), ("fire", "🔥"), ("star", "⭐"), ("warning", "⚠️"), ("bulb", "💡"),
    ("check", "✔️"), ("white_check_mark", "✅"), ("x", "❌"), ("eyes", "👀"), ("book", "📖"),
    ("memo", "📝"), ("bug", "🐛"), ("sparkles", "✨"), ("coffee", "☕"), ("thinking", "🤔"),
];
const EMOTICONS: &[(&str, &str)] = &[(":-)", "😃"), (":)", "😃"), (";-)", "😉"), (":-(", "😦"), (":D", "😄"), ("<3", "❤️")];

pub fn emoji_transform(node: &mut Node) {
    let map: HashMap<&str, &str> = EMOJI.iter().copied().collect();
    rewrite_text_nodes(node, &|t| {
        let replaced = replace_emoji(t, &map);
        (replaced != t).then(|| vec![Node::text(replaced)])
    });
}

fn replace_emoji(text: &str, map: &HashMap<&str, &str>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    'outer: while !rest.is_empty() {
        if let Some(stripped) = rest.strip_prefix(':') {
            if let Some(end) = stripped.find(':') {
                let name = &stripped[..end];
                if let Some(e) = map.get(name) {
                    out.push_str(e);
                    rest = &stripped[end + 1..];
                    continue;
                }
            }
        }
        // Emoticons only at word boundaries.
        let at_boundary = out.is_empty() || out.ends_with(char::is_whitespace);
        if at_boundary {
            for (face, e) in EMOTICONS {
                if let Some(after) = rest.strip_prefix(face) {
                    if after.is_empty() || after.starts_with(char::is_whitespace) {
                        out.push_str(e);
                        rest = after;
                        continue 'outer;
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
