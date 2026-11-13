//! Markdownr's own document tree.
//!
//! pulldown-cmark only emits a flat event stream. We fold it into a real tree so
//! transforms (see `transforms.rs`) can add node kinds of their own (`Mark`, `Ins`,
//! ...) as first-class enum variants, and so the layout can work block by block.

use pulldown_cmark::{
    Alignment, BlockQuoteKind, CodeBlockKind, Event, MetadataBlockKind, Options, Parser, Tag,
    TagEnd,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    // Blocks
    Document,
    FrontMatter,
    Paragraph,
    Heading { level: u8, id: String },
    BlockQuote(Option<AlertKind>),
    CodeBlock { lang: String },
    HtmlBlock,
    List { start: Option<u64> },
    Item { task: Option<bool> },
    FootnoteDef { label: String },
    DefList,
    DefTitle,
    DefDetails,
    Table { aligns: Vec<Align> },
    TableHead,
    TableRow,
    TableCell,
    Rule,
    DisplayMath,
    // Inlines
    Text,
    Code,
    InlineMath,
    InlineHtml,
    Emph,
    Strong,
    Strike,
    Sup,
    Sub,
    /// `==marked==` (added by a transform)
    Mark,
    /// `++inserted++` (added by a transform)
    Ins,
    Link { url: String },
    Image { url: String },
    SoftBreak,
    HardBreak,
    FootnoteRef { label: String },
}

impl Kind {
    pub fn is_inline(&self) -> bool {
        matches!(
            self,
            Kind::Text
                | Kind::Code
                | Kind::InlineMath
                | Kind::InlineHtml
                | Kind::Emph
                | Kind::Strong
                | Kind::Strike
                | Kind::Sup
                | Kind::Sub
                | Kind::Mark
                | Kind::Ins
                | Kind::Link { .. }
                | Kind::Image { .. }
                | Kind::SoftBreak
                | Kind::HardBreak
                | Kind::FootnoteRef { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: Kind,
    /// Literal content for leaf nodes (text, code, math, html, front matter).
    pub text: String,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(kind: Kind) -> Self {
        Node { kind, text: String::new(), children: Vec::new() }
    }
    pub fn leaf(kind: Kind, text: impl Into<String>) -> Self {
        Node { kind, text: text.into(), children: Vec::new() }
    }
    pub fn text(text: impl Into<String>) -> Self {
        Self::leaf(Kind::Text, text)
    }
    pub fn with_children(kind: Kind, children: Vec<Node>) -> Self {
        Node { kind, text: String::new(), children }
    }

    /// Concatenated plain text of this subtree.
    pub fn plain_text(&self) -> String {
        let mut s = String::new();
        self.collect_text(&mut s);
        s
    }
    fn collect_text(&self, out: &mut String) {
        match self.kind {
            Kind::SoftBreak | Kind::HardBreak => out.push(' '),
            _ => out.push_str(&self.text),
        }
        for c in &self.children {
            c.collect_text(out);
        }
    }
}

pub fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
        | Options::ENABLE_MATH
        | Options::ENABLE_GFM
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_SUPERSCRIPT
        | Options::ENABLE_SUBSCRIPT
}

/// Parse Markdown into a `Kind::Document` tree.
pub fn parse(src: &str) -> Node {
    let mut stack: Vec<Node> = vec![Node::new(Kind::Document)];

    fn push_child(stack: &mut [Node], node: Node) {
        let parent = stack.last_mut().expect("document root");
        // Merge adjacent text so transforms can match across event boundaries
        // (pulldown splits text at e.g. smart-punctuation replacements).
        if node.kind == Kind::Text {
            if let Some(last) = parent.children.last_mut() {
                if last.kind == Kind::Text {
                    last.text.push_str(&node.text);
                    return;
                }
            }
        }
        parent.children.push(node);
    }

    for event in Parser::new_ext(src, options()) {
        match event {
            Event::Start(tag) => {
                let kind = match tag {
                    Tag::Paragraph => Kind::Paragraph,
                    Tag::Heading { level, id, .. } => Kind::Heading {
                        level: level as u8,
                        id: id.map(|s| s.to_string()).unwrap_or_default(),
                    },
                    Tag::BlockQuote(kind) => Kind::BlockQuote(kind.map(|k| match k {
                        BlockQuoteKind::Note => AlertKind::Note,
                        BlockQuoteKind::Tip => AlertKind::Tip,
                        BlockQuoteKind::Important => AlertKind::Important,
                        BlockQuoteKind::Warning => AlertKind::Warning,
                        BlockQuoteKind::Caution => AlertKind::Caution,
                    })),
                    Tag::CodeBlock(CodeBlockKind::Fenced(lang)) => Kind::CodeBlock {
                        lang: lang.split_whitespace().next().unwrap_or("").to_string(),
                    },
                    Tag::CodeBlock(CodeBlockKind::Indented) => Kind::CodeBlock { lang: String::new() },
                    Tag::HtmlBlock => Kind::HtmlBlock,
                    Tag::List(start) => Kind::List { start },
                    Tag::Item => Kind::Item { task: None },
                    Tag::FootnoteDefinition(label) => Kind::FootnoteDef { label: label.to_string() },
                    Tag::DefinitionList => Kind::DefList,
                    Tag::DefinitionListTitle => Kind::DefTitle,
                    Tag::DefinitionListDefinition => Kind::DefDetails,
                    Tag::Table(aligns) => Kind::Table {
                        aligns: aligns
                            .into_iter()
                            .map(|a| match a {
                                Alignment::None => Align::None,
                                Alignment::Left => Align::Left,
                                Alignment::Center => Align::Center,
                                Alignment::Right => Align::Right,
                            })
                            .collect(),
                    },
                    Tag::TableHead => Kind::TableHead,
                    Tag::TableRow => Kind::TableRow,
                    Tag::TableCell => Kind::TableCell,
                    Tag::Emphasis => Kind::Emph,
                    Tag::Strong => Kind::Strong,
                    Tag::Strikethrough => Kind::Strike,
                    Tag::Superscript => Kind::Sup,
                    Tag::Subscript => Kind::Sub,
                    Tag::Link { dest_url, .. } => Kind::Link { url: dest_url.to_string() },
                    Tag::Image { dest_url, .. } => Kind::Image { url: dest_url.to_string() },
                    Tag::MetadataBlock(MetadataBlockKind::YamlStyle | MetadataBlockKind::PlusesStyle) => {
                        Kind::FrontMatter
                    }
                };
                stack.push(Node::new(kind));
            }
            Event::End(end) => {
                let node = stack.pop().expect("balanced events");
                // A table's first row lives directly in TableHead (no TableRow); normalise.
                let node = if matches!(end, TagEnd::TableHead) {
                    Node::with_children(Kind::TableHead, vec![Node::with_children(Kind::TableRow, node.children)])
                } else {
                    node
                };
                // Front matter and code blocks keep their content as one string.
                let node = match node.kind {
                    Kind::FrontMatter | Kind::CodeBlock { .. } | Kind::HtmlBlock => {
                        let text = node.plain_text();
                        Node { kind: node.kind, text, children: Vec::new() }
                    }
                    _ => node,
                };
                push_child(&mut stack, node);
            }
            Event::Text(t) => push_child(&mut stack, Node::text(t.to_string())),
            Event::Code(t) => push_child(&mut stack, Node::leaf(Kind::Code, t.to_string())),
            Event::InlineMath(t) => push_child(&mut stack, Node::leaf(Kind::InlineMath, t.to_string())),
            Event::DisplayMath(t) => push_child(&mut stack, Node::leaf(Kind::DisplayMath, t.to_string())),
            Event::Html(t) => push_child(&mut stack, Node::text(t.to_string())),
            Event::InlineHtml(t) => push_child(&mut stack, Node::leaf(Kind::InlineHtml, t.to_string())),
            Event::FootnoteReference(l) => {
                push_child(&mut stack, Node::new(Kind::FootnoteRef { label: l.to_string() }))
            }
            Event::SoftBreak => push_child(&mut stack, Node::new(Kind::SoftBreak)),
            Event::HardBreak => push_child(&mut stack, Node::new(Kind::HardBreak)),
            Event::Rule => push_child(&mut stack, Node::new(Kind::Rule)),
            Event::TaskListMarker(done) => {
                if let Some(Node { kind: Kind::Item { task }, .. }) = stack.last_mut() {
                    *task = Some(done);
                }
            }
        }
    }
    debug_assert_eq!(stack.len(), 1);
    stack.pop().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_tree() {
        let doc = parse("# Hi\n\nSome *em* text\n\n- [x] done\n- open\n");
        assert_eq!(doc.children.len(), 3);
        assert!(matches!(doc.children[0].kind, Kind::Heading { level: 1, .. }));
        let list = &doc.children[2];
        assert!(matches!(list.children[0].kind, Kind::Item { task: Some(true) }));
    }

    #[test]
    fn front_matter_is_a_node() {
        let doc = parse("---\ntitle: X\n---\n# A\n");
        assert_eq!(doc.children[0].kind, Kind::FrontMatter);
        assert!(doc.children[0].text.contains("title: X"));
    }

    #[test]
    fn table_head_normalised() {
        let doc = parse("| a | b |\n|---|:-:|\n| 1 | 2 |\n");
        let t = &doc.children[0];
        assert_eq!(t.children[0].kind, Kind::TableHead);
        assert_eq!(t.children[0].children[0].kind, Kind::TableRow);
        assert_eq!(t.children[0].children[0].children.len(), 2);
    }
}
