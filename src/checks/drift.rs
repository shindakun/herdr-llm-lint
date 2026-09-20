use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use super::Check;
use crate::model::{Finding, Severity};
use crate::refs::{self, RefKind};
use crate::scan::Doc;
use crate::{diff, git, Lint};

pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "drift-copies",
            severity: Severity::Warn,
            default_on: true,
            run: drift_copies,
        },
        Check {
            id: "drift-nested",
            severity: Severity::Warn,
            default_on: true,
            run: drift_nested,
        },
        Check {
            id: "drift-stale",
            severity: Severity::Info,
            default_on: true,
            run: drift_stale,
        },
        Check {
            id: "drift-age",
            severity: Severity::Info,
            default_on: true,
            run: drift_age,
        },
    ]
}

fn drift_copies(lint: &Lint) -> Vec<Finding> {
    let mut by_dir: BTreeMap<&Path, Vec<&Doc>> = BTreeMap::new();
    for d in lint.docs.iter().filter(|d| !d.is_local()) {
        by_dir.entry(d.dir()).or_default().push(d);
    }
    let mut out = Vec::new();
    for docs in by_dir.values_mut() {
        // CLAUDE.md is the reference when present.
        docs.sort_by_key(|d| d.path.file_name().is_none_or(|n| n != "CLAUDE.md"));
        for (i, a) in docs.iter().enumerate() {
            for b in &docs[i + 1..] {
                if a.text == b.text || symlinked(&a.path, &b.path) {
                    continue;
                }
                let n = diff::hunks(&a.lines, &b.lines);
                out.push(Finding::new(
                    &b.rel,
                    Some(1),
                    "drift-copies",
                    Severity::Warn,
                    format!(
                        "differs from {} in {n} hunk{}",
                        a.rel.file_name().unwrap_or_default().to_string_lossy(),
                        if n == 1 { "" } else { "s" }
                    ),
                ));
            }
        }
    }
    out
}

fn symlinked(a: &Path, b: &Path) -> bool {
    let resolves = |from: &Path, to: &Path| {
        std::fs::canonicalize(from).ok() == std::fs::canonicalize(to).ok()
            && from
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
    };
    resolves(a, b) || resolves(b, a)
}

const NESTED_MIN_SHARED: usize = 3;
const NESTED_MIN_CHARS: usize = 20;

fn drift_nested(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for nested in lint.docs.iter().filter(|d| !d.is_local()) {
        let own: Vec<&str> = content_lines(nested).collect();
        if own.len() < NESTED_MIN_SHARED {
            continue;
        }
        let best = lint
            .docs
            .iter()
            .filter(|root| root.dir() != nested.dir() && nested.dir().starts_with(root.dir()))
            .map(|root| {
                let set: HashSet<&str> = content_lines(root).collect();
                (own.iter().filter(|l| set.contains(*l)).count(), root)
            })
            .filter(|(shared, _)| *shared >= NESTED_MIN_SHARED)
            .max_by_key(|(shared, _)| *shared);
        if let Some((shared, root)) = best {
            out.push(Finding::new(
                &nested.rel,
                None,
                "drift-nested",
                Severity::Warn,
                format!(
                    "{shared} of {} lines repeat {} ({}%)",
                    own.len(),
                    root.rel.display(),
                    shared * 100 / own.len()
                ),
            ));
        }
    }
    out
}

fn content_lines(doc: &Doc) -> impl Iterator<Item = &str> {
    doc.lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| l.len() >= NESTED_MIN_CHARS && !l.starts_with('#'))
}

fn drift_stale(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        let dir = doc.dir();
        if git::has_uncommitted_changes(dir, &doc.path) {
            continue;
        }
        let Some(edited) = git::last_change(dir, &doc.path) else {
            continue;
        };
        let repo = lint.repo(doc);
        let mut changed: Vec<(String, git::Change)> = Vec::new();
        let mut seen = HashSet::new();
        for r in refs::extract(doc) {
            if !matches!(r.kind, RefKind::Path | RefKind::Import)
                || !seen.insert(r.path().to_string())
            {
                continue;
            }
            let rel = r.path();
            let full = if dir.join(rel).exists() {
                dir.join(rel)
            } else if repo.root.join(rel).exists() {
                repo.root.join(rel)
            } else {
                continue;
            };
            // Outside the project, or the whole project (`../herdr` from
            // inside a checkout named herdr), is not a specific path.
            let root = std::fs::canonicalize(&repo.root).unwrap_or_else(|_| repo.root.clone());
            if !std::fs::canonicalize(&full).is_ok_and(|f| f.starts_with(&root) && f != root) {
                continue;
            }
            if let Some(c) = git::last_change(dir, &full) {
                if c.unix > edited.unix {
                    changed.push((rel.to_string(), c));
                }
            }
        }
        if changed.is_empty() {
            continue;
        }
        let newest = changed.iter().max_by_key(|(_, c)| c.unix).unwrap();
        out.push(Finding::new(
            &doc.rel,
            None,
            "drift-stale",
            Severity::Info,
            format!(
                "{} referenced path{} changed since {} (newest: {})",
                changed.len(),
                if changed.len() == 1 { "" } else { "s" },
                edited.date,
                newest.0
            ),
        ));
    }
    out
}

fn drift_age(lint: &Lint) -> Vec<Finding> {
    let limit = lint.config.age_commits;
    let mut out = Vec::new();
    for doc in &lint.docs {
        let dir = doc.dir();
        if git::has_uncommitted_changes(dir, &doc.path) {
            continue;
        }
        let Some(edited) = git::last_change(dir, &doc.path) else {
            continue;
        };
        let Some(n) = git::commits_after(dir, &edited.hash) else {
            continue;
        };
        if n > limit {
            out.push(Finding::new(
                &doc.rel,
                None,
                "drift-age",
                Severity::Info,
                format!(
                    "{n} commits since it last changed ({}), limit is {limit}",
                    edited.date
                ),
            ));
        }
    }
    out
}
