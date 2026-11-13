//! Small, dependency-free syntax highlighter for code blocks.
//!
//! A table-driven lexer: per language a set of keywords, literal words, comment
//! and string delimiters. Good enough for reading; not a parser.

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tok {
    Keyword,
    Literal,
    Type,
    String,
    Number,
    Comment,
    Function,
    Tag,
    Attr,
    Punct,
}

struct Lang {
    keywords: &'static [&'static str],
    literals: &'static [&'static str],
    line_comments: &'static [&'static str],
    block_comment: Option<(&'static str, &'static str)>,
    /// Quote characters that start strings.
    quotes: &'static str,
    /// Identifiers starting with an uppercase letter are types.
    caps_types: bool,
    /// `#` starts a comment only at line start or after whitespace (shell, yaml…).
    hash_comment: bool,
}

const C_LIKE_LIT: &[&str] = &["true", "false", "null", "NULL", "nullptr", "None", "Some", "Ok", "Err", "self", "Self", "this", "super", "undefined", "nil"];

const RUST: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
    "move", "mut", "pub", "ref", "return", "static", "struct", "trait", "type", "unsafe", "use", "where", "while", "yield", "i8", "i16", "i32", "i64", "i128", "isize",
    "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64", "bool", "char", "str",
];
const JS: &[&str] = &[
    "async", "await", "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete", "do", "else", "export", "extends", "finally", "for",
    "from", "function", "if", "import", "in", "instanceof", "let", "new", "of", "return", "static", "switch", "throw", "try", "typeof", "var", "void", "while", "with",
    "yield", "interface", "type", "enum", "implements", "private", "public", "protected", "readonly", "as", "declare", "namespace", "keyof", "string", "number",
    "boolean", "any", "unknown", "never",
];
const PY: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while", "with", "yield", "match", "case",
];
const PY_LIT: &[&str] = &["True", "False", "None", "self", "cls"];
const C: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else", "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long",
    "register", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef", "union", "unsigned", "void", "volatile", "while", "class", "namespace",
    "template", "typename", "public", "private", "protected", "virtual", "override", "new", "delete", "using", "try", "catch", "throw", "bool", "constexpr", "noexcept",
    "#include", "#define", "#if", "#ifdef", "#ifndef", "#endif", "#else", "#pragma",
];
const JAVA: &[&str] = &[
    "abstract", "boolean", "break", "byte", "case", "catch", "char", "class", "const", "continue", "default", "do", "double", "else", "enum", "extends", "final",
    "finally", "float", "for", "if", "implements", "import", "instanceof", "int", "interface", "long", "native", "new", "package", "private", "protected", "public",
    "return", "short", "static", "super", "switch", "synchronized", "throw", "throws", "try", "void", "volatile", "while", "var", "val", "fun", "when", "object",
    "string", "using", "namespace", "async", "await", "override", "virtual", "readonly", "get", "set", "let", "func", "guard", "struct", "protocol", "extension",
    "init", "deinit", "where", "in", "is", "as",
];
const GO: &[&str] = &[
    "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func", "go", "goto", "if", "import", "interface", "map",
    "package", "range", "return", "select", "struct", "switch", "type", "var", "string", "int", "int64", "uint", "byte", "rune", "bool", "error", "float64",
];
const SH: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "in", "function", "return", "local", "export", "echo", "cd", "exit", "set",
    "unset", "source", "alias", "sudo", "cargo", "git", "npm", "npx", "yarn", "pnpm", "brew", "apt", "rustup", "make",
];
const SQL: &[&str] = &[
    "select", "from", "where", "insert", "into", "values", "update", "set", "delete", "create", "table", "drop", "alter", "add", "index", "join", "left", "right",
    "inner", "outer", "on", "as", "and", "or", "not", "null", "is", "in", "group", "by", "order", "having", "limit", "offset", "primary", "key", "foreign",
    "references", "distinct", "union", "all", "case", "when", "then", "else", "end", "integer", "text", "varchar", "default",
];
const PS: &[&str] = &["function", "param", "if", "else", "elseif", "foreach", "for", "while", "return", "switch", "try", "catch", "finally", "throw", "begin", "process", "end"];
const CSS: &[&str] = &["important", "media", "import", "keyframes", "from", "to"];
const NONE: &[&str] = &[];

fn lang(name: &str) -> Option<Lang> {
    let l = |keywords, literals, line_comments, block_comment, quotes, caps_types| Lang {
        keywords,
        literals,
        line_comments,
        block_comment,
        quotes,
        caps_types,
        hash_comment: false,
    };
    let slash = Some(("/*", "*/"));
    Some(match name.to_ascii_lowercase().as_str() {
        "rust" | "rs" => l(RUST, C_LIKE_LIT, &["//"], slash, "\"", true),
        "js" | "javascript" | "jsx" | "ts" | "typescript" | "tsx" | "mjs" | "cjs" => l(JS, C_LIKE_LIT, &["//"], slash, "\"'`", true),
        "json" | "jsonc" | "json5" => l(NONE, &["true", "false", "null"], &["//"], slash, "\"", false),
        "py" | "python" => Lang { hash_comment: true, ..l(PY, PY_LIT, &["#"], None, "\"'", true) },
        "c" | "h" | "cpp" | "c++" | "cc" | "hpp" | "cxx" | "objc" | "objective-c" => l(C, C_LIKE_LIT, &["//"], slash, "\"'", true),
        "java" | "kotlin" | "kt" | "cs" | "csharp" | "c#" | "swift" | "scala" | "dart" => l(JAVA, C_LIKE_LIT, &["//"], slash, "\"'", true),
        "go" | "golang" => l(GO, C_LIKE_LIT, &["//"], slash, "\"'`", true),
        "sh" | "bash" | "shell" | "zsh" | "console" | "fish" => Lang { hash_comment: true, ..l(SH, NONE, &["#"], None, "\"'", false) },
        "ps" | "ps1" | "powershell" | "pwsh" => Lang { hash_comment: true, ..l(PS, &["$true", "$false", "$null"], &["#"], Some(("<#", "#>")), "\"'", false) },
        "toml" | "ini" | "cfg" | "conf" => Lang { hash_comment: true, ..l(NONE, &["true", "false"], &["#", ";"], None, "\"'", false) },
        "yaml" | "yml" => Lang { hash_comment: true, ..l(NONE, &["true", "false", "null", "yes", "no", "~"], &["#"], None, "\"'", false) },
        "sql" => l(SQL, &["true", "false"], &["--"], slash, "'\"", false),
        "css" | "scss" | "less" => l(CSS, NONE, &["//"], slash, "\"'", false),
        "lua" => l(&["and", "break", "do", "else", "elseif", "end", "for", "function", "if", "in", "local", "not", "or", "repeat", "return", "then", "until", "while"], &["true", "false", "nil"], &["--"], Some(("--[[", "]]")), "\"'", false),
        "rb" | "ruby" => Lang {
            hash_comment: true,
            ..l(&["def", "end", "class", "module", "if", "elsif", "else", "unless", "while", "until", "for", "in", "do", "return", "yield", "begin", "rescue", "ensure", "require", "attr_accessor", "then", "case", "when"], &["true", "false", "nil", "self"], &["#"], None, "\"'", true)
        },
        "html" | "xml" | "svg" | "xhtml" | "vue" | "plist" => return None, // handled by `markup`
        "diff" | "patch" => return None,
        _ => return None,
    })
}

fn is_markup(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "html" | "xml" | "svg" | "xhtml" | "vue" | "plist")
}

/// Highlight `code` written in `lang_name` (the info string of a fence).
pub fn highlight(lang_name: &str, code: &str) -> Vec<(Range<usize>, Tok)> {
    let name = lang_name.split([' ', ',', '{']).next().unwrap_or("");
    if is_markup(name) {
        return markup(code);
    }
    if matches!(name.to_ascii_lowercase().as_str(), "md" | "markdown") {
        return markdown(code);
    }
    if name.eq_ignore_ascii_case("diff") || name.eq_ignore_ascii_case("patch") {
        return diff(code);
    }
    let Some(lang) = lang(name) else { return Vec::new() };
    let is_css = matches!(name, "css" | "scss" | "less");
    let b = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let line_start = |i: usize| i == 0 || b[i - 1] == b'\n' || b[..i].iter().rev().take_while(|&&c| c != b'\n').all(|c| c.is_ascii_whitespace());
    while i < b.len() {
        let rest = &code[i..];
        let c = b[i];
        // Comments
        if let Some((open, close)) = lang.block_comment {
            if rest.starts_with(open) {
                let end = rest[open.len()..].find(close).map_or(b.len(), |p| i + open.len() + p + close.len());
                out.push((i..end, Tok::Comment));
                i = end;
                continue;
            }
        }
        if let Some(lc) = lang.line_comments.iter().find(|lc| rest.starts_with(**lc)) {
            let ok = !lang.hash_comment || *lc != "#" || i == 0 || b[i - 1].is_ascii_whitespace();
            // `#include` etc. are keywords in C, `#` colour in CSS — never comments there.
            if ok {
                let end = rest.find('\n').map_or(b.len(), |p| i + p);
                out.push((i..end, Tok::Comment));
                i = end;
                continue;
            }
            let _ = lc;
        }
        // Strings
        if c < 128 && lang.quotes.as_bytes().contains(&c) {
            // Rust lifetimes / chars: 'a without closing quote soon -> not a string.
            let mut j = i + 1;
            let triple = rest.len() >= 3 && rest.as_bytes()[1] == c && rest.as_bytes()[2] == c && name.starts_with("py");
            if triple {
                let q = &rest[..3];
                let end = rest[3..].find(q).map_or(b.len(), |p| i + 3 + p + 3);
                out.push((i..end, Tok::String));
                i = end;
                continue;
            }
            while j < b.len() && b[j] != c && (b[j] != b'\n' || c == b'`') {
                if b[j] == b'\\' {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(b.len());
            out.push((i..end, Tok::String));
            i = end;
            continue;
        }
        // Numbers
        if c.is_ascii_digit() && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'.' || b[j] == b'_') {
                if b[j] == b'.' && !b.get(j + 1).is_some_and(u8::is_ascii_digit) {
                    break;
                }
                j += 1;
            }
            out.push((i..j, Tok::Number));
            i = j;
            continue;
        }
        // Words
        if c.is_ascii_alphabetic() || c == b'_' || c == b'$' || (c == b'#' && line_start(i)) || (c == b'@' && is_css) {
            let mut j = i + 1;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || (b[j] == b'-' && is_css)) {
                j += 1;
            }
            let word = &code[i..j];
            let lower = word.to_ascii_lowercase();
            let w = if name == "sql" { lower.as_str() } else { word };
            let tok = if lang.keywords.contains(&w) || (is_css && word.starts_with('@')) {
                Some(Tok::Keyword)
            } else if lang.literals.contains(&w) {
                Some(Tok::Literal)
            } else if b.get(j) == Some(&b'!') && name.starts_with("r") {
                Some(Tok::Function) // macro
            } else if b.get(j) == Some(&b'(') {
                Some(Tok::Function)
            } else if lang.caps_types && word.as_bytes()[0].is_ascii_uppercase() {
                Some(Tok::Type)
            } else if is_css && b.get(j) == Some(&b':') {
                Some(Tok::Attr)
            } else if (name == "toml" || name == "yaml" || name == "yml" || name == "ini") && line_start(i) {
                Some(Tok::Attr)
            } else {
                None
            };
            if let Some(t) = tok {
                out.push((i..j, t));
            }
            i = j;
            continue;
        }
        if name == "json" && c == b'"' {
            i += 1;
            continue;
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    // JSON keys: strings followed by ':' are attributes.
    if name.starts_with("json") {
        for (r, t) in out.iter_mut() {
            if *t == Tok::String && code[r.end..].trim_start().starts_with(':') {
                *t = Tok::Attr;
            }
        }
    }
    out
}

fn markup(code: &str) -> Vec<(Range<usize>, Tok)> {
    let b = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if code[i..].starts_with("<!--") {
            let end = code[i..].find("-->").map_or(b.len(), |p| i + p + 3);
            out.push((i..end, Tok::Comment));
            i = end;
        } else if b[i] == b'<' {
            let mut j = i + 1;
            while j < b.len() && (b[j] == b'/' || b[j] == b'!' || b[j] == b'?') {
                j += 1;
            }
            let ns = j;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'-' || b[j] == b':') {
                j += 1;
            }
            out.push((i..j, Tok::Tag));
            let _ = ns;
            // attributes until '>'
            while j < b.len() && b[j] != b'>' {
                if b[j] == b'"' || b[j] == b'\'' {
                    let q = b[j];
                    let s = j;
                    j += 1;
                    while j < b.len() && b[j] != q {
                        j += 1;
                    }
                    j = (j + 1).min(b.len());
                    out.push((s..j, Tok::String));
                } else if b[j].is_ascii_alphabetic() {
                    let s = j;
                    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'-' || b[j] == b':') {
                        j += 1;
                    }
                    out.push((s..j, Tok::Attr));
                } else {
                    j += 1;
                }
            }
            if j < b.len() {
                out.push((j..j + 1, Tok::Tag));
                j += 1;
            }
            i = j;
        } else if b[i] == b'&' {
            let end = code[i..].find(';').filter(|&p| p < 10).map_or(i + 1, |p| i + p + 1);
            out.push((i..end, Tok::Literal));
            i = end;
        } else {
            i += code[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

fn markdown(code: &str) -> Vec<(Range<usize>, Tok)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut fence = false;
    for line in code.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let r = start..start + body.len();
        let t = body.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            out.push((r, Tok::Comment));
        } else if fence {
            out.push((r, Tok::String));
        } else if t.starts_with('#') {
            out.push((r, Tok::Keyword));
        } else if t.starts_with('>') {
            out.push((r, Tok::Comment));
        } else {
            let b = body.as_bytes();
            let lead = body.len() - t.len();
            if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
                out.push((start + lead..start + lead + 1, Tok::Number));
            }
            let mut i = 0;
            while i < b.len() {
                match b[i] {
                    b'`' => {
                        let end = body[i + 1..].find('`').map_or(b.len(), |p| i + p + 2);
                        out.push((start + i..start + end, Tok::String));
                        i = end;
                    }
                    b'[' => {
                        if let Some(close) = body[i..].find("](") {
                            let end = body[i + close..].find(')').map_or(b.len(), |p| i + close + p + 1);
                            out.push((start + i..start + end, Tok::Function));
                            i = end;
                        } else {
                            i += 1;
                        }
                    }
                    b'*' | b'_' if i + 1 < b.len() && b[i + 1] == b[i] => {
                        let d = &body[i..i + 2];
                        let end = body[i + 2..].find(d).map_or(i + 2, |p| i + 2 + p + 2);
                        out.push((start + i..start + end, Tok::Type));
                        i = end;
                    }
                    _ => i += 1,
                }
            }
        }
        start += line.len();
    }
    out
}

fn diff(code: &str) -> Vec<(Range<usize>, Tok)> {
    let mut out = Vec::new();
    let mut start = 0;
    for line in code.split_inclusive('\n') {
        let r = start..start + line.trim_end_matches('\n').len();
        match line.as_bytes().first() {
            Some(b'+') => out.push((r, Tok::String)),
            Some(b'-') => out.push((r, Tok::Keyword)),
            Some(b'@') => out.push((r, Tok::Function)),
            _ => {}
        }
        start += line.len();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(lang: &str, code: &str) -> Vec<(String, Tok)> {
        highlight(lang, code).into_iter().map(|(r, t)| (code[r].to_string(), t)).collect()
    }

    #[test]
    fn rust_basics() {
        let t = toks("rust", "fn main() { let x = \"hi\"; // c\n println!(\"{x}\", 42); }");
        assert!(t.contains(&("fn".into(), Tok::Keyword)));
        assert!(t.contains(&("main".into(), Tok::Function)));
        assert!(t.contains(&("\"hi\"".into(), Tok::String)));
        assert!(t.contains(&("// c".into(), Tok::Comment)));
        assert!(t.contains(&("println".into(), Tok::Function)));
        assert!(t.contains(&("42".into(), Tok::Number)));
    }

    #[test]
    fn json_keys() {
        let t = toks("json", "{\"a\": 1, \"b\": \"x\"}");
        assert!(t.contains(&("\"a\"".into(), Tok::Attr)));
        assert!(t.contains(&("\"x\"".into(), Tok::String)));
    }

    #[test]
    fn html_tags() {
        let t = toks("html", "<a href=\"x\">y</a>");
        assert!(t.contains(&("<a".into(), Tok::Tag)));
        assert!(t.contains(&("href".into(), Tok::Attr)));
        assert!(t.contains(&("</a".into(), Tok::Tag)));
    }

    #[test]
    fn unknown_language_is_plain() {
        assert!(highlight("klingon", "qapla'").is_empty());
    }
}
