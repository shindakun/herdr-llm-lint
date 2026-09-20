//! Size and shape: the file is over budget, has no structure, or has lines
//! too long to read.

use super::Check;
use crate::model::{Finding, Severity};
use crate::Lint;

pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "size-bytes",
            severity: Severity::Warn,
            default_on: true,
            run: size_bytes,
        },
        Check {
            id: "shape-headings",
            severity: Severity::Warn,
            default_on: true,
            run: shape_headings,
        },
        Check {
            id: "shape-lines",
            severity: Severity::Warn,
            default_on: true,
            run: shape_lines,
        },
        Check {
            id: "shape-body",
            severity: Severity::Warn,
            default_on: false,
            run: shape_body,
        },
    ]
}

/// Over `size_bytes` from the config.
fn size_bytes(lint: &Lint) -> Vec<Finding> {
    let budget = lint.config.size_bytes;
    lint.docs
        .iter()
        .filter(|d| d.bytes() > budget)
        .map(|d| {
            Finding::new(
                &d.rel,
                None,
                "size-bytes",
                Severity::Warn,
                format!("{} bytes, budget is {budget}", d.bytes()),
            )
        })
        .collect()
}

/// Minimum non-blank lines before a file with no headings is a wall of
/// text.
const WALL_LINES: usize = 20;

/// No headings over a long file, or one heading over everything.
fn shape_headings(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for d in &lint.docs {
        let body = d.lines.iter().filter(|l| !l.trim().is_empty()).count();
        if body < WALL_LINES {
            continue;
        }
        let headings = d.sections.iter().filter(|s| s.heading.is_some()).count();
        let message = match headings {
            0 => format!("{body} lines and no headings"),
            1 => format!("{body} lines under a single heading"),
            _ => continue,
        };
        out.push(Finding::new(
            &d.rel,
            None,
            "shape-headings",
            Severity::Warn,
            message,
        ));
    }
    out
}

/// Headings and list items over `line_chars` from the config.
fn shape_lines(lint: &Lint) -> Vec<Finding> {
    long_lines(lint, "shape-lines", is_heading_or_item)
}

/// Every other line over `line_chars`: paragraphs, list continuations,
/// quotes. Off by default; files written one paragraph per line trip it
/// on every line.
fn shape_body(lint: &Lint) -> Vec<Finding> {
    long_lines(lint, "shape-body", |line| !is_heading_or_item(line))
}

fn is_heading_or_item(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with('#') || t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") {
        return true;
    }
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && (t[digits..].starts_with(". ") || t[digits..].starts_with(") "))
}

/// Lines over `line_chars` that `pick` accepts, outside fenced code
/// blocks and tables.
fn long_lines(lint: &Lint, id: &'static str, pick: fn(&str) -> bool) -> Vec<Finding> {
    let limit = lint.config.line_chars;
    let mut out = Vec::new();
    for d in &lint.docs {
        let mut in_fence = false;
        for (i, line) in d.lines.iter().enumerate() {
            if crate::scan::is_fence(line) {
                in_fence = !in_fence;
                continue;
            }
            if in_fence || line.trim_start().starts_with('|') || !pick(line) {
                continue;
            }
            let n = line.chars().count();
            if n > limit {
                out.push(Finding::new(
                    &d.rel,
                    Some(i + 1),
                    id,
                    Severity::Warn,
                    format!("{n} chars, limit is {limit}"),
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
    fn headings_and_items_are_told_from_body() {
        assert!(is_heading_or_item("# Title"));
        assert!(is_heading_or_item("- item"));
        assert!(is_heading_or_item("  * nested"));
        assert!(is_heading_or_item("12. step"));
        assert!(is_heading_or_item("3) step"));
        assert!(!is_heading_or_item("plain prose"));
        assert!(!is_heading_or_item("  continuation of an item"));
        assert!(!is_heading_or_item("> quote"));
        assert!(!is_heading_or_item("2024 was a year"));
    }
}
