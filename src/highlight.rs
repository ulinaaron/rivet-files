//! Light syntax highlighting for the Peek panel's text previews, colored from the
//! theme (primary, secondary and tertiary palette colors). A small tokenizer that
//! understands comments, strings, numbers, keywords, function calls and type names
//! for common languages, plus Markdown headings/code and JSON keys.

use eframe::egui::{text::LayoutJob, Color32, FontId, TextFormat};

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub struct Colors {
    pub text: Color32,
    pub keyword: Color32,
    pub string: Color32,
    pub number: Color32,
    pub comment: Color32,
    pub function: Color32,
    pub type_: Color32,
    pub punct: Color32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Lang {
    /// Code with these comment markers.
    Code { line: &'static [&'static str], block: Option<(&'static str, &'static str)> },
    Markdown,
    Json,
    Plain,
}

fn lang_for(name: &str) -> Lang {
    let lower = name.to_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or(lower.as_str());
    const C: Option<(&str, &str)> = Some(("/*", "*/"));
    match ext {
        "rs" | "c" | "h" | "cc" | "cpp" | "hpp" | "cxx" | "js" | "mjs" | "cjs" | "ts" | "tsx" | "jsx" | "go" | "java" | "kt" | "kts" | "swift" | "cs"
        | "scala" | "dart" | "zig" | "v" | "glsl" | "wgsl" | "qml" | "groovy" | "gradle" | "php" | "jsonc" | "proto" => Lang::Code { line: &["//"], block: C },
        "css" | "scss" | "less" => Lang::Code { line: &["//"], block: C },
        "py" | "pyw" | "sh" | "bash" | "zsh" | "fish" | "rb" | "pl" | "pm" | "yaml" | "yml" | "toml" | "conf" | "cfg" | "r" | "nix" | "cmake" | "mk" | "ps1"
        | "desktop" | "service" | "env" | "gitignore" | "dockerfile" | "makefile" | "properties" | "tf" | "hcl" | "jl" | "ex" | "exs" | "nim" | "cr" => {
            Lang::Code { line: &["#"], block: None }
        }
        "ini" | "reg" => Lang::Code { line: &[";", "#"], block: None },
        "lua" | "sql" | "hs" | "elm" | "ada" => Lang::Code { line: &["--"], block: None },
        "lisp" | "el" | "clj" | "cljs" | "scm" | "rkt" | "asm" | "s" => Lang::Code { line: &[";"], block: None },
        "vim" => Lang::Code { line: &["\""], block: None },
        "html" | "htm" | "xml" | "svg" | "vue" | "svelte" | "xhtml" | "plist" | "ui" => Lang::Code { line: &[], block: Some(("<!--", "-->")) },
        "md" | "markdown" | "mdx" => Lang::Markdown,
        "json" | "geojson" | "lock" => Lang::Json,
        _ => Lang::Plain,
    }
}

const KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "pub", "use", "mod", "struct", "enum", "impl", "trait", "const", "static", "if", "else", "match", "for", "while", "loop", "return",
    "break", "continue", "in", "as", "where", "self", "Self", "true", "false", "null", "None", "nil", "def", "class", "import", "from", "lambda", "yield", "async",
    "await", "function", "var", "new", "this", "typeof", "instanceof", "try", "catch", "finally", "throw", "throws", "package", "func", "go", "defer",
    "interface", "type", "extends", "implements", "public", "private", "protected", "void", "int", "float", "double", "bool", "boolean", "string", "char",
    "final", "switch", "case", "default", "do", "elif", "fi", "then", "esac", "done", "local", "export", "echo", "end", "and", "or", "not", "is", "with",
    "unsafe", "crate", "super", "dyn", "ref", "move", "extern", "include", "define", "ifdef", "ifndef", "endif", "namespace", "using", "template", "typename",
    "auto", "struct", "union", "sizeof", "goto", "volatile", "inline", "virtual", "override", "delete", "operator", "friend", "elsif", "unless", "until",
    "begin", "rescue", "ensure", "module", "require", "raise", "pass", "global", "nonlocal", "assert", "del", "except", "print", "select", "insert", "update",
    "create", "table", "values", "into", "set", "delete", "drop", "alter", "join", "on", "order", "by", "group", "having", "limit", "val", "object", "data",
    "when", "fun", "it", "readonly", "declare", "enum", "abstract", "sealed", "record", "yield", "undefined", "let", "of", "keyof", "satisfies",
];

fn is_keyword(w: &str) -> bool {
    KEYWORDS.contains(&w) || (w.len() > 1 && KEYWORDS.contains(&w.to_lowercase().as_str()) && w.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
}

struct Out<'a> {
    job: LayoutJob,
    font: FontId,
    c: &'a Colors,
}

impl Out<'_> {
    fn push(&mut self, s: &str, color: Color32, italic: bool) {
        if s.is_empty() {
            return;
        }
        let fmt = TextFormat { font_id: self.font.clone(), color, italics: italic, ..Default::default() };
        self.job.append(s, 0.0, fmt);
    }
}

/// Build a colored layout for `text` (the file name picks the language).
pub fn layout(text: &str, name: &str, colors: &Colors, font: FontId) -> LayoutJob {
    let mut out = Out { job: LayoutJob::default(), font, c: colors };
    out.job.wrap.max_width = f32::INFINITY;
    match lang_for(name) {
        Lang::Markdown => markdown(text, &mut out),
        Lang::Json => code(text, &mut out, &[], None, true, false),
        Lang::Code { line, block } => {
            let rust = name.to_lowercase().ends_with(".rs");
            code(text, &mut out, line, block, false, rust)
        }
        Lang::Plain => {
            let c = out.c.text;
            out.push(text, c, false);
        }
    }
    out.job
}

fn code(text: &str, out: &mut Out, line: &[&str], block: Option<(&str, &str)>, json: bool, rust: bool) {
    let c = *out.c;
    let b = text.as_bytes();
    let mut i = 0;
    let mut plain_start = 0;
    let flush = |out: &mut Out, from: usize, to: usize| {
        if to > from {
            out.push(&text[from..to], c.text, false);
        }
    };
    while i < b.len() {
        let rest = &text[i..];
        // block comment
        if let Some((open, close)) = block {
            if rest.starts_with(open) {
                flush(out, plain_start, i);
                let end = rest[open.len()..].find(close).map(|p| i + open.len() + p + close.len()).unwrap_or(b.len());
                out.push(&text[i..end], c.comment, true);
                i = end;
                plain_start = i;
                continue;
            }
        }
        // line comment (for "#", only at a word boundary so "a#b" or "#[attr]" in Rust aren't comments)
        if let Some(m) = line.iter().find(|m| rest.starts_with(**m)) {
            let ok = *m != "#" || i == 0 || b[i - 1].is_ascii_whitespace();
            if ok {
                flush(out, plain_start, i);
                let end = rest.find('\n').map(|p| i + p).unwrap_or(b.len());
                out.push(&text[i..end], c.comment, true);
                i = end;
                plain_start = i;
                continue;
            }
        }
        let ch = b[i];
        // strings
        if ch == b'"' || ch == b'`' || (ch == b'\'' && !json && looks_like_char_or_string(b, i, rust)) {
            flush(out, plain_start, i);
            let mut j = i + 1;
            while j < b.len() && b[j] != ch {
                if b[j] == b'\\' {
                    j += 1;
                }
                if b[j.min(b.len() - 1)] == b'\n' && ch != b'`' && ch != b'"' {
                    break;
                }
                j += 1;
            }
            let end = (j + 1).min(b.len());
            let end = floor_char(text, end);
            // JSON keys: a string followed by ':'
            let is_key = json && text[end..].trim_start().starts_with(':');
            out.push(&text[i..end], if is_key { c.function } else { c.string }, false);
            i = end;
            plain_start = i;
            continue;
        }
        // numbers
        if ch.is_ascii_digit() && (i == 0 || !is_ident(b[i - 1])) {
            flush(out, plain_start, i);
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'.' || b[j] == b'_') {
                j += 1;
            }
            out.push(&text[i..j], c.number, false);
            i = j;
            plain_start = i;
            continue;
        }
        // words
        if is_ident(ch) && (i == 0 || !is_ident(b[i - 1])) && !ch.is_ascii_digit() {
            let mut j = i;
            while j < b.len() && is_ident(b[j]) {
                j += 1;
            }
            let w = &text[i..j];
            let after = text[j..].trim_start_matches([' ', '\t']);
            let color = if is_keyword(w) {
                Some(c.keyword)
            } else if after.starts_with('(') || after.starts_with("!(") {
                Some(c.function)
            } else if w.chars().next().is_some_and(|f| f.is_ascii_uppercase()) && w.chars().any(|f| f.is_ascii_lowercase()) {
                Some(c.type_)
            } else {
                None
            };
            if let Some(color) = color {
                flush(out, plain_start, i);
                out.push(w, color, false);
                plain_start = j;
            }
            i = j;
            continue;
        }
        // punctuation
        if b"{}[]()<>;:,.=+-*/%&|!?^~@".contains(&ch) {
            flush(out, plain_start, i);
            out.push(&text[i..i + 1], c.punct, false);
            i += 1;
            plain_start = i;
            continue;
        }
        i += utf8_len(ch);
    }
    flush(out, plain_start, b.len());
}

fn markdown(text: &str, out: &mut Out) {
    let c = *out.c;
    let mut in_fence = false;
    for (n, line) in text.split_inclusive('\n').enumerate() {
        let _ = n;
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            out.push(line, c.punct, false);
            continue;
        }
        if in_fence {
            out.push(line, c.string, false);
            continue;
        }
        if t.starts_with('#') {
            let fmt = TextFormat { font_id: out.font.clone(), color: c.keyword, ..Default::default() };
            out.job.append(line, 0.0, fmt);
            continue;
        }
        if t.starts_with('>') {
            out.push(line, c.comment, true);
            continue;
        }
        // list markers
        let indent = line.len() - t.len();
        let mut rest = line;
        if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
            out.push(&line[..indent + 2], c.number, false);
            rest = &line[indent + 2..];
        }
        inline_md(rest, out);
    }
}

fn inline_md(s: &str, out: &mut Out) {
    let c = *out.c;
    let b = s.as_bytes();
    let mut i = 0;
    let mut start = 0;
    while i < b.len() {
        match b[i] {
            b'`' => {
                if let Some(p) = s[i + 1..].find('`') {
                    out.push(&s[start..i], c.text, false);
                    let end = i + 1 + p + 1;
                    out.push(&s[i..end], c.string, false);
                    i = end;
                    start = i;
                    continue;
                }
            }
            b'[' => {
                if let Some(p) = s[i..].find("](") {
                    if let Some(q) = s[i + p..].find(')') {
                        out.push(&s[start..i], c.text, false);
                        let end = i + p + q + 1;
                        out.push(&s[i..end], c.function, false);
                        i = end;
                        start = i;
                        continue;
                    }
                }
            }
            b'*' | b'_' if i + 1 < b.len() && b[i + 1] == b[i] => {
                let mark = &s[i..i + 2];
                if let Some(p) = s[i + 2..].find(mark) {
                    out.push(&s[start..i], c.text, false);
                    let end = i + 2 + p + 2;
                    out.push(&s[i..end], c.type_, false);
                    i = end;
                    start = i;
                    continue;
                }
            }
            _ => {}
        }
        i += utf8_len(b[i]);
    }
    out.push(&s[start..], c.text, false);
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn utf8_len(first: u8) -> usize {
    match first {
        0..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xff => 4,
        _ => 1,
    }
}

fn floor_char(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// A ' that starts a string or char literal, not a Rust lifetime or an apostrophe in a word.
fn looks_like_char_or_string(b: &[u8], i: usize, rust: bool) -> bool {
    if i > 0 && is_ident(b[i - 1]) {
        return false;
    }
    if rust {
        // Only char literals: 'x', '\n', '\u{1F600}' (anything else is a lifetime).
        let close = b[i + 1..].iter().take(12).position(|&x| x == b'\'');
        return match close {
            Some(p) => p >= 1 && (b[i + 1] == b'\\' || p <= 4 && std::str::from_utf8(&b[i + 1..i + 1 + p]).is_ok_and(|s| s.chars().count() == 1)),
            None => false,
        };
    }
    // Rust lifetimes: 'a followed by an ident char and no closing quote soon.
    let close = b[i + 1..].iter().take(80).position(|&x| x == b'\'' || x == b'\n');
    matches!(close.map(|p| b[i + 1 + p]), Some(b'\''))
}

/// Pick readable highlight colors from the theme's three palette colors.
pub fn colors(text: Color32, bg: Color32, primary: Color32, secondary: Color32, tertiary: Color32) -> Colors {
    let lum = crate::palette::luminance;
    let dark = lum(bg) < 0.5;
    // Push a color away from the background until it reads well.
    let readable = |c: Color32| {
        let mut c = c;
        for _ in 0..4 {
            if (lum(c) - lum(bg)).abs() >= 0.38 {
                break;
            }
            c = crate::palette::mix(c, if dark { Color32::WHITE } else { Color32::BLACK }, 0.3);
        }
        c
    };
    let mix = crate::palette::mix;
    Colors {
        text,
        keyword: readable(primary),
        string: readable(secondary),
        number: readable(tertiary),
        comment: mix(text, bg, 0.45),
        function: readable(mix(tertiary, text, 0.35)),
        type_: readable(mix(primary, text, 0.4)),
        punct: mix(text, bg, 0.3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors_of(job: &LayoutJob, word: &str) -> Option<Color32> {
        job.sections.iter().find(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0] == word).map(|s| s.format.color)
    }

    #[test]
    fn highlights_rust() {
        let c = Colors {
            text: Color32::WHITE,
            keyword: Color32::RED,
            string: Color32::GREEN,
            number: Color32::BLUE,
            comment: Color32::GRAY,
            function: Color32::YELLOW,
            type_: Color32::LIGHT_RED,
            punct: Color32::DARK_GRAY,
        };
        let src = "fn main() {\n    let x: Vec<u8> = vec![1, 2]; // hi\n    println!(\"hé {}\", x.len());\n}\n";
        let job = layout(src, "main.rs", &c, FontId::monospace(11.0));
        assert_eq!(job.text, src);
        assert_eq!(colors_of(&job, "fn"), Some(Color32::RED));
        assert_eq!(colors_of(&job, "main"), Some(Color32::YELLOW));
        assert_eq!(colors_of(&job, "Vec"), Some(Color32::LIGHT_RED));
        assert_eq!(colors_of(&job, "1"), Some(Color32::BLUE));
        assert_eq!(colors_of(&job, "// hi"), Some(Color32::GRAY));
        assert_eq!(colors_of(&job, "\"hé {}\""), Some(Color32::GREEN));
        // JSON keys vs values, Python comments, Markdown headings
        let job = layout("{\"name\": \"rivet\", \"n\": 3}", "a.json", &c, FontId::monospace(11.0));
        assert_eq!(colors_of(&job, "\"name\""), Some(Color32::YELLOW));
        assert_eq!(colors_of(&job, "\"rivet\""), Some(Color32::GREEN));
        let job = layout("x = 1  # note\n", "a.py", &c, FontId::monospace(11.0));
        assert_eq!(colors_of(&job, "# note"), Some(Color32::GRAY));
        let job = layout("# Title\nSome `code` here\n", "README.md", &c, FontId::monospace(11.0));
        assert_eq!(colors_of(&job, "# Title\n"), Some(Color32::RED));
        assert_eq!(colors_of(&job, "`code`"), Some(Color32::GREEN));
        // Rust lifetimes are not strings
        let job = layout("fn f<'a>(x: &'a str) {}", "l.rs", &c, FontId::monospace(11.0));
        assert_eq!(job.text, "fn f<'a>(x: &'a str) {}");
        assert!(!job.sections.iter().any(|s| s.format.color == Color32::GREEN));
        // Never panics on odd input
        for s in ["\"unterminated", "'", "/* open", "é'ü\"", "`"] {
            let _ = layout(s, "x.js", &c, FontId::monospace(11.0));
        }
    }
}
