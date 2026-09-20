use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::Check;
use crate::model::{Finding, Severity};
use crate::refs::{self, RefKind};
use crate::repo;
use crate::scan::Doc;
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
        Check {
            id: "ref-env",
            severity: Severity::Warn,
            default_on: true,
            run: ref_env,
        },
        Check {
            id: "ref-skill",
            severity: Severity::Error,
            default_on: true,
            run: ref_skill,
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

// Variables any shell or CI provides; a doc may name them without the
// repo defining them.
const COMMON_ENV: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "SHELL",
    "EDITOR",
    "VISUAL",
    "PAGER",
    "TERM",
    "LANG",
    "LC_ALL",
    "PWD",
    "TMPDIR",
    "TMP",
    "TEMP",
    "CI",
    "DEBUG",
    "PORT",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "GOPATH",
    "GOROOT",
    "GOFLAGS",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTFLAGS",
    "NODE_ENV",
    "NODE_OPTIONS",
    "PYTHONPATH",
    "VIRTUAL_ENV",
    "GITHUB_TOKEN",
    "GITHUB_ACTIONS",
    "GITHUB_WORKSPACE",
    "GITHUB_SHA",
    "GITHUB_REF",
    "GIT_DIR",
    "SSH_AUTH_SOCK",
    "DISPLAY",
];

const ENV_MAX_FILE_BYTES: u64 = 1 << 20;

fn ref_env(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut corpus: HashMap<PathBuf, Vec<(PathBuf, String)>> = HashMap::new();
    for doc in &lint.docs {
        let wanted: Vec<(usize, String)> = refs::extract(doc)
            .into_iter()
            .filter(|r| r.kind == RefKind::EnvVar)
            .map(|r| (r.line, r.name().to_string()))
            .filter(|(_, n)| !n.is_empty() && !COMMON_ENV.contains(&n.as_str()))
            .collect();
        if wanted.is_empty() {
            continue;
        }
        let root = lint.repo(doc).root.clone();
        let files = corpus
            .entry(root.clone())
            .or_insert_with(|| text_files(&root, &lint.docs));
        let mut seen = HashSet::new();
        for (line, name) in wanted {
            if !seen.insert(name.clone()) {
                continue;
            }
            if files.iter().any(|(_, text)| text.contains(&name)) {
                continue;
            }
            out.push(Finding::new(
                &doc.rel,
                Some(line),
                "ref-env",
                Severity::Warn,
                format!("`{name}` is not mentioned by any file in the repo"),
            ));
        }
    }
    out
}

/// Every text file under `root` except the instruction files themselves.
fn text_files(root: &Path, docs: &[Doc]) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let walk = ignore::WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    for entry in walk.filter_map(Result::ok) {
        let path = entry.path();
        if !entry.file_type().is_some_and(|t| t.is_file())
            || docs.iter().any(|d| d.path == path)
            || entry
                .metadata()
                .map_or(true, |m| m.len() > ENV_MAX_FILE_BYTES)
        {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(path) {
            out.push((path.to_path_buf(), text));
        }
    }
    out
}

// `/deploy` as a slash command, or a backticked word next to "skill",
// must exist in .claude/commands, .claude/skills, .agents/skills, or
// skills/.
fn ref_skill(lint: &Lint) -> Vec<Finding> {
    let mut out = Vec::new();
    for doc in &lint.docs {
        let root = &lint.repo(doc).root;
        let has_dir = root.join(".claude/commands").is_dir()
            || SKILL_DIRS.iter().any(|d| root.join(d).is_dir());
        let mut in_fence = false;
        let mut seen = HashSet::new();
        for (i, line) in doc.lines.iter().enumerate() {
            if crate::scan::is_fence(line) {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                continue;
            }
            for name in skill_mentions(line, has_dir) {
                if !seen.insert(name.clone()) || skill_exists(root, &name) {
                    continue;
                }
                out.push(Finding::new(
                    &doc.rel,
                    Some(i + 1),
                    "ref-skill",
                    Severity::Error,
                    format!("skill or command `{name}` is not in .claude/commands or a skills directory"),
                ));
            }
        }
    }
    out
}

const SKILL_DIRS: &[&str] = &[".claude/skills", ".agents/skills", "skills"];

fn skill_exists(root: &Path, name: &str) -> bool {
    root.join(".claude/commands")
        .join(format!("{name}.md"))
        .is_file()
        || SKILL_DIRS.iter().any(|d| {
            let dir = root.join(d);
            dir.join(name).is_dir() || dir.join(format!("{name}.md")).is_file()
        })
}

fn is_skill_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

fn skill_mentions(line: &str, slash_commands: bool) -> Vec<String> {
    let mut out = Vec::new();
    let words: Vec<&str> = line.split_whitespace().collect();
    for (i, w) in words.iter().enumerate() {
        let w = w.trim_end_matches(|c: char| ",.;:)\"'".contains(c));
        let bare = w.trim_matches(|c: char| "`(".contains(c));
        if slash_commands {
            if let Some(name) = bare.strip_prefix('/') {
                if is_skill_name(name) && w.starts_with(['/', '`']) {
                    out.push(name.to_string());
                    continue;
                }
            }
        }
        if !(w.starts_with('`') && w.ends_with('`') && is_skill_name(bare)) {
            continue;
        }
        let neighbour = |j: Option<usize>| {
            j.and_then(|j| words.get(j)).map(|n| {
                n.trim_matches(|c: char| !c.is_ascii_alphabetic())
                    .to_ascii_lowercase()
            })
        };
        let before = neighbour(i.checked_sub(1));
        let after = neighbour(Some(i + 1));
        let tag = |s: &Option<String>| matches!(s.as_deref(), Some("skill" | "skills"));
        if tag(&before) || tag(&after) {
            out.push(bare.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_skill_mentions() {
        assert_eq!(
            skill_mentions("Run `/deploy` before `/release`.", true),
            vec!["deploy", "release"]
        );
        assert_eq!(
            skill_mentions("Use the `review` skill, then the skill `ship`.", false),
            vec!["review", "ship"]
        );
        assert!(skill_mentions("Run /deploy now", false).is_empty());
        assert!(skill_mentions("Paths like /usr/bin are absolute", true).is_empty());
        assert!(skill_mentions("The `Config` type", true).is_empty());
        assert!(skill_mentions("the `ghostty-bench` command line", true).is_empty());
        assert!(skill_mentions("and/or something", true).is_empty());
    }
}
