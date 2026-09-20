use super::Check;
use crate::model::{Finding, Severity};
use crate::refs::{self, RefKind};
use crate::repo;
use crate::Lint;

pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "ref-path",
            severity: Severity::Error,
            default_on: true,
            run: ref_path,
        },
        Check {
            id: "ref-command",
            severity: Severity::Error,
            default_on: true,
            run: ref_command,
        },
        Check {
            id: "ref-target",
            severity: Severity::Error,
            default_on: true,
            run: ref_target,
        },
    ]
}

// `~` and absolute paths describe another machine as often as this one,
// so they are not checked. Git refs such as `origin/main` are not paths.
fn ref_path(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        for r in refs::extract(doc) {
            let path = match r.kind {
                RefKind::Path => r.path(),
                RefKind::Import => r.path(),
                _ => continue,
            };
            if path.starts_with('/') || path.starts_with('~') || lint.repo(doc).is_git_ref(path) {
                continue;
            }
            if !lint.repo(doc).path_exists(doc.dir(), path) {
                out.push(Finding::new(
                    &doc.rel,
                    Some(r.line),
                    "ref-path",
                    Severity::Error,
                    format!("`{path}` does not exist"),
                ));
            }
        }
    }
    out
}

fn ref_command(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        for r in refs::extract(doc) {
            if r.kind != RefKind::Command {
                continue;
            }
            let words = r.words();
            let program = words[0];
            if repo::on_path(program)
                || lint.repo(doc).has_tool(program)
                || lint.repo(doc).has_make_target(program)
                || lint.repo(doc).has_package_script(program)
            {
                continue;
            }
            out.push(Finding::new(
                &doc.rel,
                Some(r.line),
                "ref-command",
                Severity::Error,
                format!("`{program}` is not on PATH and the repo does not declare it"),
            ));
        }
    }
    out
}

fn ref_target(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        for r in refs::extract(doc) {
            if r.kind != RefKind::Command {
                continue;
            }
            let words = r.words();
            let message = match words.as_slice() {
                ["make", target, ..] if !target.starts_with('-') && !target.contains('=') => {
                    if lint.repo(doc).has_make_target(target) {
                        continue;
                    }
                    match &lint.repo(doc).make_targets {
                        None => format!("`make {target}`: no Makefile"),
                        Some(t) => format!(
                            "`make {target}` is not a Makefile target (have: {})",
                            join(t.iter().map(String::as_str))
                        ),
                    }
                }
                [pm @ ("npm" | "pnpm" | "yarn" | "bun"), "run", script, ..] => {
                    if lint.repo(doc).has_package_script(script) {
                        continue;
                    }
                    match &lint.repo(doc).package_scripts {
                        None => format!("`{pm} run {script}`: no package.json"),
                        Some(s) => format!(
                            "`{pm} run {script}` is not a package.json script (have: {})",
                            join(s.iter().map(String::as_str))
                        ),
                    }
                }
                _ => continue,
            };
            out.push(Finding::new(
                &doc.rel,
                Some(r.line),
                "ref-target",
                Severity::Error,
                message,
            ));
        }
    }
    out
}

fn join<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let v: Vec<&str> = names.collect();
    if v.is_empty() {
        "none".to_string()
    } else {
        v.join(" ")
    }
}
