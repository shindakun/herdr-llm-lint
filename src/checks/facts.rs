use std::collections::BTreeSet;

use super::Check;
use crate::facts::{self, is_prose_tool, lang_key, lang_label, versions_agree};
use crate::model::{Finding, Severity};
use crate::scan::is_fence;
use crate::Lint;

pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "fact-version",
            severity: Severity::Error,
            default_on: true,
            run: fact_version,
        },
        Check {
            id: "fact-layout",
            severity: Severity::Error,
            default_on: true,
            run: fact_layout,
        },
        Check {
            id: "fact-tool",
            severity: Severity::Warn,
            default_on: true,
            run: fact_tool,
        },
    ]
}

fn fact_version(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        for (i, line) in doc.lines.iter().enumerate() {
            for (lang, stated) in version_claims(line) {
                let Some(repo) = lint.repo(doc).versions.get(lang) else {
                    continue;
                };
                if versions_agree(&stated, &repo.value) {
                    continue;
                }
                out.push(Finding::new(
                    &doc.rel,
                    Some(i + 1),
                    "fact-version",
                    Severity::Error,
                    format!(
                        "says {} {stated}, {} says {}",
                        lang_label(lang),
                        repo.source,
                        repo.value
                    ),
                ));
            }
        }
    }
    out
}

// "Go 1.21", "Node.js v20", "python 3.12". A dotless version counts only
// for Node, where majors are the norm; "go 2 weeks" must not match.
fn version_claims(line: &str) -> Vec<(&'static str, String)> {
    let words: Vec<String> = facts::words(line).collect();
    let mut out = Vec::new();
    for pair in words.windows(2) {
        let Some(lang) = lang_key(&pair[0]) else {
            continue;
        };
        let v = pair[1].trim_start_matches('v').trim_end_matches('+');
        if !facts::is_numeric(v) || (!v.contains('.') && lang != "node") {
            continue;
        }
        out.push((lang, v.to_string()));
    }
    out
}

const PREPOSITIONS: &[&str] = &["in", "under", "at", "into", "inside", "from", "to"];
const NOT_PATHS: &[&str] = &["and/or", "tcp/ip", "i/o", "a/b", "w/", "w/o", "24/7"];

fn fact_layout(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        let mut in_fence = false;
        for (i, line) in doc.lines.iter().enumerate() {
            if is_fence(line) {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                continue;
            }
            for path in bare_path_claims(line) {
                if lint.repo(doc).is_git_ref(path) || lint.repo(doc).path_exists(doc.dir(), path) {
                    continue;
                }
                out.push(Finding::new(
                    &doc.rel,
                    Some(i + 1),
                    "fact-layout",
                    Severity::Error,
                    format!("{path} does not exist"),
                ));
            }
        }
    }
    out
}

// A path-shaped word after a preposition, outside backticks; backticked
// paths are ref-path's.
fn bare_path_claims(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut after_prep = false;
    let mut in_code = false;
    for raw in line.split_whitespace() {
        let ticks = raw.matches('`').count();
        if ticks % 2 == 1 {
            in_code = !in_code;
        }
        if in_code || ticks > 0 {
            after_prep = false;
            continue;
        }
        let word = raw
            .trim_start_matches(['(', '"', '\''])
            .trim_end_matches([',', '.', ';', ':', ')', '"', '\'']);
        if after_prep && looks_like_path(word) {
            out.push(word);
        }
        after_prep = PREPOSITIONS.contains(&word.to_ascii_lowercase().as_str());
    }
    out
}

// `Cargo.toml/Cargo.lock` is an either/or, not a path.
fn looks_like_path(w: &str) -> bool {
    let mut segments = w.split('/').peekable();
    while let Some(seg) = segments.next() {
        if segments.peek().is_some() && crate::refs::has_path_ext(seg) {
            return false;
        }
    }
    w.contains('/')
        && !w.contains("://")
        && !w.starts_with(['/', '~', '$', '-', '<', '*'])
        && !w.contains(['*', '{', '<', '>', '|', '=', '(', '[', '!', '?'])
        && !NOT_PATHS.contains(&w.to_ascii_lowercase().as_str())
        && w.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

fn fact_tool(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (i, line) in doc.lines.iter().enumerate() {
            for w in facts::words(line) {
                if !is_prose_tool(&w) || lint.repo(doc).has_tool(&w) || !seen.insert(w.clone()) {
                    continue;
                }
                out.push(Finding::new(
                    &doc.rel,
                    Some(i + 1),
                    "fact-tool",
                    Severity::Warn,
                    format!("{w} is named but the repo has no config or dependency for it"),
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_version_claims() {
        let c = |s: &str| version_claims(s);
        assert_eq!(
            c("Requires Go 1.21 and Node 20."),
            vec![("go", "1.21".into()), ("node", "20".into())]
        );
        assert_eq!(c("Python 3.12+ only"), vec![("python", "3.12".into())]);
        assert_eq!(c("Node.js v18.2"), vec![("node", "18.2".into())]);
        assert!(c("go 2 weeks without a release").is_empty());
        assert!(c("rust stable").is_empty());
    }

    #[test]
    fn finds_bare_path_claims() {
        assert_eq!(
            bare_path_claims("Tests live in tests/ and handlers under internal/handlers/."),
            vec!["tests/", "internal/handlers/"]
        );
        assert_eq!(
            bare_path_claims("Entry point is in cmd/serve/main.go, see docs."),
            vec!["cmd/serve/main.go"]
        );
        assert!(bare_path_claims("Paths in `src/` are checked by ref-path").is_empty());
        assert!(bare_path_claims("Fetch from https://example.com/x").is_empty());
        assert!(bare_path_claims("read/write from disk").is_empty());
        assert!(bare_path_claims("works in TCP/IP mode").is_empty());
        assert!(bare_path_claims("lives in /usr/local").is_empty());
        assert!(bare_path_claims("the version in Cargo.toml/Cargo.lock").is_empty());
        assert_eq!(
            bare_path_claims("Put it in (docs/notes/)"),
            vec!["docs/notes/"]
        );
    }
}
