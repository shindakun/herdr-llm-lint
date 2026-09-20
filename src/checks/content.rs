use std::collections::BTreeSet;

use super::Check;
use crate::model::{Finding, Severity};
use crate::scan::{is_fence, Doc};
use crate::Lint;

pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "content-dup",
            severity: Severity::Warn,
            default_on: true,
            run: content_dup,
        },
        Check {
            id: "content-conflict",
            severity: Severity::Warn,
            default_on: true,
            run: content_conflict,
        },
        Check {
            id: "content-enforced",
            severity: Severity::Info,
            default_on: true,
            run: content_enforced,
        },
        Check {
            id: "content-secret",
            severity: Severity::Error,
            default_on: true,
            run: content_secret,
        },
        Check {
            id: "content-denylist",
            severity: Severity::Warn,
            default_on: true,
            run: content_denylist,
        },
        Check {
            id: "content-vague",
            severity: Severity::Warn,
            default_on: true,
            run: content_vague,
        },
    ]
}

/// Prose lines with their 1-based numbers, outside fenced code blocks.
fn prose_lines(doc: &Doc) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (i, line) in doc.lines.iter().enumerate() {
        if is_fence(line) {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            out.push((i + 1, line.as_str()));
        }
    }
    out
}

/// Lowercase words with list markers, backticks, and punctuation removed.
fn tokens(line: &str) -> Vec<String> {
    let body = line
        .trim_start()
        .trim_start_matches(['-', '*', '+', '>', '#'])
        .trim_start();
    let body = body
        .split_once(". ")
        .filter(|(n, _)| n.bytes().all(|b| b.is_ascii_digit()))
        .map_or(body, |(_, rest)| rest);
    body.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

const DUP_MIN_TOKENS: usize = 5;

fn similarity(a: &[String], b: &[String]) -> f64 {
    let sa: BTreeSet<&String> = a.iter().collect();
    let sb: BTreeSet<&String> = b.iter().collect();
    let shared = sa.intersection(&sb).count();
    let union = sa.union(&sb).count();
    if union == 0 {
        0.0
    } else {
        shared as f64 / union as f64
    }
}

fn content_dup(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        let lines: Vec<(usize, Vec<String>)> = prose_lines(doc)
            .into_iter()
            .map(|(n, l)| (n, tokens(l)))
            .filter(|(_, t)| t.len() >= DUP_MIN_TOKENS)
            .collect();
        let mut reported = BTreeSet::new();
        for (i, (n, t)) in lines.iter().enumerate() {
            if reported.contains(n) {
                continue;
            }
            for (m, u) in &lines[i + 1..] {
                let opposite = matches!((polarity(t), polarity(u)), (Some(a), Some(b)) if a != b);
                if !reported.contains(m) && !opposite && (t == u || similarity(t, u) >= 0.8) {
                    reported.insert(*m);
                    out.push(Finding::new(
                        &doc.rel,
                        Some(*m),
                        "content-dup",
                        Severity::Warn,
                        format!("repeats line {n}"),
                    ));
                }
            }
        }
    }
    out
}

const POSITIVE: &[&str] = &["always", "must", "prefer"];
const NEGATIVE: &[&str] = &["never", "not", "avoid", "dont"];
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "to", "of", "in", "on", "for", "and", "or", "with", "before", "after",
    "when", "it", "is", "are", "be", "this", "that", "any", "all", "you", "your", "use", "using",
    "always", "never", "must", "not", "do", "dont", "avoid", "prefer", "should",
];

fn polarity(t: &[String]) -> Option<bool> {
    let neg = t.iter().any(|w| NEGATIVE.contains(&w.as_str()));
    let pos = t.iter().any(|w| POSITIVE.contains(&w.as_str()));
    match (pos, neg) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

fn object(t: &[String]) -> BTreeSet<&str> {
    t.iter()
        .map(String::as_str)
        .filter(|w| !STOPWORDS.contains(w) && w.len() > 2)
        .collect()
}

const CONFLICT_MIN_SHARED: usize = 3;

fn content_conflict(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        let rules: Vec<(usize, bool, Vec<String>)> = prose_lines(doc)
            .into_iter()
            .map(|(n, l)| (n, tokens(l)))
            .filter_map(|(n, t)| polarity(&t).map(|p| (n, p, t)))
            .collect();
        for (i, (n, p, t)) in rules.iter().enumerate() {
            let obj = object(t);
            for (m, q, u) in &rules[i + 1..] {
                if p == q {
                    continue;
                }
                let other = object(u);
                let shared = obj.intersection(&other).count();
                let smaller = obj.len().min(other.len()).max(1);
                if shared >= CONFLICT_MIN_SHARED && shared * 10 >= smaller * 6 {
                    out.push(Finding::new(
                        &doc.rel,
                        Some(*m),
                        "content-conflict",
                        Severity::Warn,
                        format!("may contradict line {n}"),
                    ));
                }
            }
        }
    }
    out
}

const ENFORCED_TRIGGERS: &[&str] = &[
    "run", "use", "with", "via", "using", "apply", "execute", "invoke", "keep", "pass", "through",
];

// "run gofmt", "format with prettier", "keep clippy clean": the tool within
// three tokens of a trigger. "make check runs clippy" describes, not rules.
fn enforced_rule_tools<'a>(line: &str, enforced: &'a [&'static str]) -> Vec<&'a &'static str> {
    let toks: Vec<String> = crate::facts::words(&crate::facts::unalias(line)).collect();
    let mut out = Vec::new();
    for (i, tok) in toks.iter().enumerate() {
        let Some(tool) = enforced.iter().find(|t| **t == tok.as_str()) else {
            continue;
        };
        let from = i.saturating_sub(3);
        if toks[from..i]
            .iter()
            .any(|w| ENFORCED_TRIGGERS.contains(&w.as_str()))
            && !out.contains(&tool)
        {
            out.push(tool);
        }
    }
    out
}

fn content_enforced(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        let enforced = &lint.repo(doc).enforced;
        if enforced.is_empty() {
            continue;
        }
        let names: Vec<&'static str> = enforced.keys().copied().collect();
        for (n, line) in prose_lines(doc) {
            for tool in enforced_rule_tools(line, &names) {
                out.push(Finding::new(
                    &doc.rel,
                    Some(n),
                    "content-enforced",
                    Severity::Info,
                    format!("{tool} already runs in {}", enforced[tool]),
                ));
            }
        }
    }
    out
}

const PLACEHOLDERS: &[&str] = &[
    "<",
    ">",
    "${",
    "xxx",
    "...",
    "your",
    "changeme",
    "example",
    "placeholder",
    "redacted",
    "dummy",
    "todo",
    "fixme",
    "$",
];

fn is_placeholder(v: &str) -> bool {
    let l = v.to_ascii_lowercase();
    PLACEHOLDERS.iter().any(|p| l.contains(p))
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

/// A run of token characters of at least `min` after `prefix`, anywhere
/// in `line`.
fn has_prefixed_token(line: &str, prefix: &str, min: usize) -> bool {
    line.match_indices(prefix).any(|(i, _)| {
        let rest = &line[i + prefix.len()..];
        rest.chars().take_while(|c| is_token_char(*c)).count() >= min
    })
}

fn has_assignment_secret(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    for key in [
        "password", "passwd", "secret", "token", "api_key", "apikey", "api-key",
    ] {
        for (i, _) in lower.match_indices(key) {
            let rest = &line[i + key.len()..];
            let rest = rest.trim_start_matches(|c: char| is_token_char(c));
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix([':', '=']).map(str::trim_start) else {
                continue;
            };
            let value: String = rest
                .trim_start_matches(['"', '\''])
                .chars()
                .take_while(|c| !c.is_whitespace() && !"\"'`,;".contains(*c))
                .collect();
            if value.len() >= 8 && !is_placeholder(&value) {
                return true;
            }
        }
    }
    false
}

fn secret_kind(line: &str) -> Option<&'static str> {
    if line.contains("PRIVATE KEY-----") {
        return Some("a private key");
    }
    if has_prefixed_token(line, "AKIA", 16) {
        return Some("an AWS access key");
    }
    if has_prefixed_token(line, "ghp_", 36) || has_prefixed_token(line, "github_pat_", 22) {
        return Some("a GitHub token");
    }
    if ["xoxb-", "xoxp-", "xoxa-", "xoxr-", "xoxs-"]
        .iter()
        .any(|p| has_prefixed_token(line, p, 10))
    {
        return Some("a Slack token");
    }
    if has_prefixed_token(line, "sk-", 20) {
        return Some("an API key");
    }
    if has_prefixed_token(line, "AIza", 35) {
        return Some("a Google API key");
    }
    if has_assignment_secret(line) {
        return Some("a credential assignment");
    }
    None
}

fn content_secret(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        for (i, line) in doc.lines.iter().enumerate() {
            if let Some(kind) = secret_kind(line) {
                out.push(Finding::new(
                    &doc.rel,
                    Some(i + 1),
                    "content-secret",
                    Severity::Error,
                    format!("looks like {kind}"),
                ));
            }
        }
    }
    out
}

fn content_denylist(lint: &Lint) -> Vec<Finding> {
    let phrases: Vec<String> = lint
        .config
        .denylist_phrases()
        .into_iter()
        .map(|p| p.to_ascii_lowercase())
        .collect();
    if phrases.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for doc in &lint.docs {
        for (i, line) in doc.lines.iter().enumerate() {
            let lower = line.to_ascii_lowercase();
            for p in phrases.iter().filter(|p| lower.contains(p.as_str())) {
                out.push(Finding::new(
                    &doc.rel,
                    Some(i + 1),
                    "content-denylist",
                    Severity::Warn,
                    format!("contains denylisted phrase `{p}`"),
                ));
            }
        }
    }
    out
}

const VAGUE: &[&str] = &[
    "be careful",
    "be cautious",
    "be thoughtful",
    "be mindful",
    "be thorough",
    "be consistent",
    "be sensible",
    "be reasonable",
    "use best practices",
    "follow best practices",
    "use common sense",
    "use good judgment",
    "use good judgement",
    "write clean code",
    "write good code",
    "keep it simple",
    "do your best",
    "think carefully",
    "think step by step",
    "as appropriate",
    "when appropriate",
    "where appropriate",
    "as needed",
    "as necessary",
    "if necessary",
    "when necessary",
    "where necessary",
];

fn content_vague(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        for (n, line) in prose_lines(doc) {
            let lower = line.to_ascii_lowercase();
            for phrase in VAGUE.iter().filter(|p| lower.contains(*p)) {
                out.push(Finding::new(
                    &doc.rel,
                    Some(n),
                    "content-vague",
                    Severity::Warn,
                    format!("`{phrase}` has no object or checkable condition"),
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Vec<String> {
        tokens(s)
    }

    #[test]
    fn tokens_strip_markers() {
        assert_eq!(
            t("- Run `make check` first."),
            vec!["run", "make", "check", "first"]
        );
        assert_eq!(t("3. Do it"), vec!["do", "it"]);
        assert_eq!(t("## Heading"), vec!["heading"]);
    }

    #[test]
    fn near_duplicates_by_token_overlap() {
        let a = t("Always run make check before you commit anything");
        let b = t("Always run make check before you commit anything.");
        let c = t("Always run make check before you commit");
        let d = t("Never push directly to the main branch please");
        assert!(similarity(&a, &b) >= 0.8);
        assert!(similarity(&a, &c) >= 0.8);
        assert!(similarity(&a, &d) < 0.8);
    }

    #[test]
    fn polarity_and_objects() {
        assert_eq!(polarity(&t("Always run the tests")), Some(true));
        assert_eq!(polarity(&t("Never skip the tests")), Some(false));
        assert_eq!(polarity(&t("Do not push")), Some(false));
        assert_eq!(polarity(&t("Always do this, never that")), None);
        assert_eq!(polarity(&t("Tests live in tests/")), None);
        let toks = t("Always run the integration tests before merging");
        let o = object(&toks);
        assert!(o.contains("integration") && o.contains("tests") && o.contains("merging"));
        assert!(!o.contains("the"));
    }

    #[test]
    fn enforced_needs_a_trigger_near_the_tool() {
        let e = ["rustfmt", "clippy", "gofmt"];
        assert_eq!(
            enforced_rule_tools(
                "Format with `cargo fmt` and keep `cargo clippy` warning-free.",
                &e
            ),
            vec![&"rustfmt", &"clippy"]
        );
        assert_eq!(
            enforced_rule_tools("Run gofmt before committing.", &e),
            vec![&"gofmt"]
        );
        assert!(
            enforced_rule_tools("`make check` runs `make clippy` and the tests.", &e).is_empty()
        );
        assert!(enforced_rule_tools("clippy is strict about this crate.", &e).is_empty());
    }

    #[test]
    fn secret_patterns() {
        let aws = format!("key = AKIA{}", "A".repeat(16));
        assert_eq!(secret_kind(&aws), Some("an AWS access key"));
        let gh = format!("export GH=ghp_{}", "a".repeat(36));
        assert_eq!(secret_kind(&gh), Some("a GitHub token"));
        assert_eq!(
            secret_kind("-----BEGIN RSA PRIVATE KEY-----"),
            Some("a private key")
        );
        assert_eq!(
            secret_kind("password = \"hunter2hunter2\""),
            Some("a credential assignment")
        );
        assert_eq!(
            secret_kind("DB_PASSWORD: s3cr3tpassw0rd"),
            Some("a credential assignment")
        );
        assert_eq!(secret_kind("password = \"<your password>\""), None);
        assert_eq!(secret_kind("token = ${GITHUB_TOKEN}"), None);
        assert_eq!(secret_kind("Set `API_KEY` in the environment."), None);
        assert_eq!(secret_kind("The token is short: token=abc"), None);
        assert_eq!(secret_kind("skip sk-ip this"), None);
    }
}
