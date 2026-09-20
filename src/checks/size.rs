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
            run: size_bytes,
        },
        Check {
            id: "shape-headings",
            severity: Severity::Warn,
            run: shape_headings,
        },
        Check {
            id: "shape-lines",
            severity: Severity::Warn,
            run: shape_lines,
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

/// Lines over `line_chars` from the config, outside fenced code blocks and
/// tables.
fn shape_lines(lint: &Lint) -> Vec<Finding> {
    let limit = lint.config.line_chars;
    let mut out = Vec::new();
    for d in &lint.docs {
        let mut in_fence = false;
        for (i, line) in d.lines.iter().enumerate() {
            if crate::scan::is_fence(line) {
                in_fence = !in_fence;
                continue;
            }
            if in_fence || line.trim_start().starts_with('|') {
                continue;
            }
            let n = line.chars().count();
            if n > limit {
                out.push(Finding::new(
                    &d.rel,
                    Some(i + 1),
                    "shape-lines",
                    Severity::Warn,
                    format!("{n} chars, limit is {limit}"),
                ));
            }
        }
    }
    out
}
