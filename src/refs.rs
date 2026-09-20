use crate::scan::{is_fence, Doc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Path,
    Command,
    EnvVar,
    Import,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    /// 1-based.
    pub line: usize,
    pub kind: RefKind,
    pub text: String,
}

impl Ref {
    pub fn words(&self) -> Vec<&str> {
        self.text.split_whitespace().collect()
    }

    pub fn name(&self) -> &str {
        self.text
            .trim_start_matches('$')
            .split('=')
            .next()
            .unwrap_or("")
    }

    pub fn path(&self) -> &str {
        self.text.trim_start_matches('@')
    }
}

const PATH_EXTENSIONS: &[&str] = &[
    "md", "mdc", "toml", "json", "jsonc", "yaml", "yml", "rs", "go", "py", "js", "ts", "tsx",
    "jsx", "mjs", "cjs", "lock", "txt", "sh", "mod", "sum", "cfg", "ini", "env", "sql", "html",
    "css", "proto", "lua", "rb", "java", "kt", "swift", "c", "h", "cpp", "hpp",
];

pub fn extract(doc: &Doc) -> Vec<Ref> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (i, line) in doc.lines.iter().enumerate() {
        let n = i + 1;
        if is_fence(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(path) = import_line(line) {
            out.push(Ref {
                line: n,
                kind: RefKind::Import,
                text: format!("@{path}"),
            });
            continue;
        }
        for span in backtick_spans(line) {
            if let Some(kind) = classify(span) {
                out.push(Ref {
                    line: n,
                    kind,
                    text: span.to_string(),
                });
            }
        }
    }
    out
}

fn import_line(line: &str) -> Option<&str> {
    let t = line.trim();
    let rest = t.strip_prefix('@')?;
    if rest.is_empty() || rest.contains(char::is_whitespace) {
        return None;
    }
    // `@example.com` in prose is not an import; require a path shape.
    if !(rest.contains('/') || rest.starts_with('~') || rest.starts_with('.') || has_path_ext(rest))
    {
        return None;
    }
    Some(rest)
}

pub fn backtick_spans(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        let after = &rest[start + 1..];
        // Double backticks open a literal span; skip them.
        if after.starts_with('`') {
            rest = after.trim_start_matches('`');
            continue;
        }
        let Some(end) = after.find('`') else {
            break;
        };
        let span = after[..end].trim();
        if !span.is_empty() {
            out.push(span);
        }
        rest = &after[end + 1..];
    }
    out
}

pub fn classify(span: &str) -> Option<RefKind> {
    if is_env_var(span) {
        return Some(RefKind::EnvVar);
    }
    if is_path(span) {
        return Some(RefKind::Path);
    }
    if is_command(span) {
        return Some(RefKind::Command);
    }
    None
}

fn is_env_var(s: &str) -> bool {
    let name = s.trim_start_matches('$');
    let name = if s.starts_with('$') {
        name
    } else if let Some((n, _)) = s.split_once('=') {
        n
    } else {
        return false;
    };
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_uppercase() || c == '_')
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn is_path(s: &str) -> bool {
    if s.contains(char::is_whitespace)
        || s.starts_with('/')
        || s.starts_with('~')
        || s.starts_with('$')
        || s.starts_with('-')
        || s.contains("://")
        || s.contains(|c: char| "*{}<>|=()[]\"'!?:".contains(c))
    {
        return false;
    }
    if s.contains('/') {
        return true;
    }
    has_path_ext(s)
}

pub fn has_path_ext(s: &str) -> bool {
    let Some((stem, ext)) = s.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty() && !stem.ends_with('.') && PATH_EXTENSIONS.contains(&ext)
}

// Code fragments in backticks start with these, not commands.
const KEYWORDS: &[&str] = &[
    "async", "await", "class", "const", "def", "enum", "export", "extern", "fn", "for", "from",
    "func", "if", "impl", "import", "let", "new", "package", "pub", "return", "self", "struct",
    "this", "type", "use", "var", "while",
];

// `closes #12` is an issue reference, not a command.
fn is_command(s: &str) -> bool {
    let mut words = s.split_whitespace();
    let Some(first) = words.next() else {
        return false;
    };
    let Some(second) = words.next() else {
        return false;
    };
    if second.starts_with('#') || KEYWORDS.contains(&first) {
        return false;
    }
    let mut chars = first.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-.+".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn doc(text: &str) -> Doc {
        Doc::parse(
            PathBuf::from("/x/CLAUDE.md"),
            PathBuf::from("CLAUDE.md"),
            text.into(),
        )
    }

    #[test]
    fn splits_backtick_spans() {
        assert_eq!(backtick_spans("a `b` c `d e` f"), vec!["b", "d e"]);
        assert_eq!(backtick_spans("no spans"), Vec::<&str>::new());
        assert_eq!(backtick_spans("odd `one"), Vec::<&str>::new());
        assert_eq!(backtick_spans("``lit`` and `x`"), vec!["x"]);
    }

    #[test]
    fn classifies_by_shape() {
        assert_eq!(classify("src/main.rs"), Some(RefKind::Path));
        assert_eq!(classify("internal/httpx/"), Some(RefKind::Path));
        assert_eq!(classify("go.mod"), Some(RefKind::Path));
        assert_eq!(classify(".cursorrules"), None);
        assert_eq!(classify("make check"), Some(RefKind::Command));
        assert_eq!(classify("npm run lint"), Some(RefKind::Command));
        assert_eq!(classify("$DATABASE_URL"), Some(RefKind::EnvVar));
        assert_eq!(classify("PORT=8080"), Some(RefKind::EnvVar));
        assert_eq!(classify("Config"), None);
        assert_eq!(classify("foo()"), None);
        assert_eq!(classify("https://example.com/x"), None);
        assert_eq!(classify("/usr/bin/env"), None);
        assert_eq!(classify("~/.claude/CLAUDE.md"), None);
        assert_eq!(classify("**/*.go"), None);
        assert_eq!(classify("--fix"), None);
        assert_eq!(classify("Do not"), None);
        assert_eq!(classify("foo.bar"), None);
        assert_eq!(classify("closes #12"), None);
        assert_eq!(classify("pub fn run()"), None);
        assert_eq!(classify("extern crate x"), None);
    }

    #[test]
    fn extracts_in_line_order_and_skips_fences() {
        let d = doc("See `src/lib.rs` and run `make test`.\n```sh\nmake nope\n```\n@docs/PLAN.md\n`$HOME`\n");
        let refs = extract(&d);
        let got: Vec<(usize, RefKind, &str)> = refs
            .iter()
            .map(|r| (r.line, r.kind, r.text.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (1, RefKind::Path, "src/lib.rs"),
                (1, RefKind::Command, "make test"),
                (5, RefKind::Import, "@docs/PLAN.md"),
                (6, RefKind::EnvVar, "$HOME"),
            ]
        );
        assert_eq!(refs[2].path(), "docs/PLAN.md");
        assert_eq!(refs[3].name(), "HOME");
        assert_eq!(refs[1].words(), vec!["make", "test"]);
    }

    #[test]
    fn import_needs_a_path_shape() {
        assert_eq!(import_line("@docs/x.md"), Some("docs/x.md"));
        assert_eq!(
            import_line("@~/.claude/rules.md"),
            Some("~/.claude/rules.md")
        );
        assert_eq!(import_line("@someone said"), None);
        assert_eq!(import_line("@handle"), None);
    }
}
