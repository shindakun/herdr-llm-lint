use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::model::Finding;
use crate::scan::Doc;
use crate::Lint;

/// Applies the safe fixes for `findings` and returns one line per action.
/// Near-duplicates, headings, and anything else that needs judgement are
/// left alone.
pub fn apply(lint: &Lint, findings: &[Finding]) -> Result<Vec<String>, String> {
    let mut actions = Vec::new();
    let limit = lint.config.line_chars;
    for doc in &lint.docs {
        let mine: Vec<&Finding> = findings.iter().filter(|f| f.file == doc.rel).collect();
        let drop: BTreeSet<usize> = mine
            .iter()
            .filter(|f| f.check == "content-dup")
            .filter_map(|f| f.line)
            .filter(|&n| exact_earlier_duplicate(doc, n))
            .collect();
        let wrap: BTreeSet<usize> = mine
            .iter()
            .filter(|f| f.check == "shape-lines" || f.check == "shape-body")
            .filter_map(|f| f.line)
            .filter(|&n| !doc.lines[n - 1].trim_start().starts_with('#'))
            .collect();
        if drop.is_empty() && wrap.is_empty() {
            continue;
        }
        let mut out: Vec<String> = Vec::new();
        for (i, line) in doc.lines.iter().enumerate() {
            let n = i + 1;
            if drop.contains(&n) {
                actions.push(format!("{}:{n}: dropped duplicate line", doc.rel.display()));
                continue;
            }
            if wrap.contains(&n) {
                let wrapped = wrap_line(line, limit);
                if wrapped.len() > 1 {
                    actions.push(format!(
                        "{}:{n}: wrapped into {} lines",
                        doc.rel.display(),
                        wrapped.len()
                    ));
                }
                out.extend(wrapped);
                continue;
            }
            out.push(line.clone());
        }
        let mut text = out.join("\n");
        if doc.text.ends_with('\n') {
            text.push('\n');
        }
        if text != doc.text {
            std::fs::write(&doc.path, text).map_err(|e| format!("{}: {e}", doc.path.display()))?;
        }
    }
    if lint.config.enabled("drift-copies", true) {
        actions.extend(symlink_identical(&lint.docs)?);
    }
    Ok(actions)
}

fn exact_earlier_duplicate(doc: &Doc, n: usize) -> bool {
    let line = doc.lines[n - 1].trim();
    doc.lines[..n - 1].iter().any(|l| l.trim() == line)
}

/// Greedy fill at `limit` chars. A list marker's width becomes the
/// continuation indent; backtick spans stay whole.
pub fn wrap_line(line: &str, limit: usize) -> Vec<String> {
    let indent_len = line.len() - line.trim_start().len();
    let indent = &line[..indent_len];
    let body = &line[indent_len..];
    let marker_len = list_marker_len(body);
    let (marker, rest) = body.split_at(marker_len);
    let words = split_words(rest);
    let cont = format!("{indent}{}", " ".repeat(marker.chars().count()));
    let mut lines: Vec<String> = Vec::new();
    let mut cur = format!("{indent}{marker}");
    let mut cur_empty = true;
    for w in words {
        let candidate_len = cur.chars().count() + usize::from(!cur_empty) + w.chars().count();
        if !cur_empty && candidate_len > limit {
            lines.push(cur);
            cur = cont.clone();
            cur_empty = true;
        }
        if !cur_empty {
            cur.push(' ');
        }
        cur.push_str(w);
        cur_empty = false;
    }
    lines.push(cur);
    lines
}

fn list_marker_len(body: &str) -> usize {
    for m in ["- ", "* ", "+ ", "> "] {
        if body.starts_with(m) {
            return m.len();
        }
    }
    let digits = body.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0 && (body[digits..].starts_with(". ") || body[digits..].starts_with(") ")) {
        return digits + 2;
    }
    0
}

fn split_words(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    let mut in_code = false;
    for (i, c) in s.char_indices() {
        if c == '`' {
            in_code = !in_code;
        }
        if c.is_whitespace() && !in_code {
            if let Some(st) = start.take() {
                out.push(&s[st..i]);
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        out.push(&s[st..]);
    }
    out
}

/// In each directory, files identical to the reference (`CLAUDE.md` when
/// present, else the first by name) become relative symlinks to it.
fn symlink_identical(docs: &[Doc]) -> Result<Vec<String>, String> {
    let mut by_dir: BTreeMap<&Path, Vec<&Doc>> = BTreeMap::new();
    for d in docs.iter().filter(|d| !d.is_local()) {
        by_dir.entry(d.dir()).or_default().push(d);
    }
    let mut actions = Vec::new();
    for docs in by_dir.values_mut() {
        docs.sort_by_key(|d| d.path.file_name().is_none_or(|n| n != "CLAUDE.md"));
        let (reference, rest) = docs.split_first().unwrap();
        let ref_name = reference.path.file_name().unwrap_or_default();
        for d in rest {
            let is_link = d
                .path
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink());
            if is_link || d.text != reference.text {
                continue;
            }
            std::fs::remove_file(&d.path).map_err(|e| format!("{}: {e}", d.path.display()))?;
            symlink(ref_name, &d.path)?;
            actions.push(format!(
                "{}: symlinked to {}",
                d.rel.display(),
                ref_name.to_string_lossy()
            ));
        }
    }
    Ok(actions)
}

#[cfg(unix)]
fn symlink(target: &std::ffi::OsStr, link: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target, link).map_err(|e| format!("{}: {e}", link.display()))
}

#[cfg(not(unix))]
fn symlink(_target: &std::ffi::OsStr, link: &Path) -> Result<(), String> {
    Err(format!(
        "{}: symlinks are not supported here",
        link.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_words_with_list_indent() {
        let w = wrap_line("- one two three four five six", 14);
        assert_eq!(w, vec!["- one two", "  three four", "  five six"]);
        let w = wrap_line("  12. alpha beta gamma", 12);
        assert_eq!(w, vec!["  12. alpha", "      beta", "      gamma"]);
        assert_eq!(wrap_line("short", 80), vec!["short"]);
    }

    #[test]
    fn keeps_backtick_spans_whole() {
        let w = wrap_line("run `make check` now and `cargo test` too", 16);
        assert_eq!(w, vec!["run `make check`", "now and", "`cargo test` too"]);
    }

    #[test]
    fn a_word_over_the_limit_stays() {
        let w = wrap_line("- averyveryverylongword x", 8);
        assert_eq!(w, vec!["- averyveryverylongword", "  x"]);
    }
}
