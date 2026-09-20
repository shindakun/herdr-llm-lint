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
            id: "size-section",
            severity: Severity::Warn,
            default_on: true,
            run: size_section,
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

const SECTION_MIN_COUNT: usize = 4;

// Half the file, and only with four or more sections; a third fires on
// ordinary four-section files.
fn size_section(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for d in &lint.docs {
        let headed: Vec<_> = d.sections.iter().filter(|s| s.heading.is_some()).collect();
        if headed.len() < SECTION_MIN_COUNT {
            continue;
        }
        let total = d.bytes().max(1);
        for s in headed {
            let bytes: usize = d.lines[s.start - 1..s.end]
                .iter()
                .map(|l| l.len() + 1)
                .sum();
            if bytes * 2 > total {
                out.push(Finding::new(
                    &d.rel,
                    Some(s.start),
                    "size-section",
                    Severity::Warn,
                    format!(
                        "section `{}` is {bytes} of {total} bytes ({}%)",
                        s.heading.as_deref().unwrap_or(""),
                        bytes * 100 / total
                    ),
                ));
            }
        }
    }
    out
}

const WALL_LINES: usize = 20;

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

fn shape_lines(lint: &Lint) -> Vec<Finding> {
    long_lines(lint, "shape-lines", is_heading_or_item)
}

// Off by default: files written one paragraph per line trip it everywhere.
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
