use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_REMOTES: &[&str] = &["origin", "upstream"];

#[derive(Debug, Clone, Default)]
pub struct Repo {
    pub root: PathBuf,
    pub make_targets: Option<BTreeSet<String>>,
    pub package_scripts: Option<BTreeSet<String>>,
    pub remotes: BTreeSet<String>,
}

impl Repo {
    pub fn load(root: &Path) -> Self {
        let make_targets = ["Makefile", "makefile", "GNUmakefile"]
            .iter()
            .find_map(|n| std::fs::read_to_string(root.join(n)).ok())
            .map(|t| make_targets(&t));
        let package_scripts = std::fs::read_to_string(root.join("package.json"))
            .ok()
            .map(|t| package_scripts(&t));
        Self {
            root: root.to_path_buf(),
            make_targets,
            package_scripts,
            remotes: git_remotes(root),
        }
    }

    pub fn is_git_ref(&self, s: &str) -> bool {
        if s == "HEAD" || s.starts_with("refs/") {
            return true;
        }
        match s.split_once('/') {
            Some((remote, branch)) => !branch.is_empty() && self.remotes.contains(remote),
            None => false,
        }
    }

    pub fn path_exists(&self, base: &Path, rel: &str) -> bool {
        base.join(rel).exists() || self.root.join(rel).exists()
    }

    pub fn has_make_target(&self, name: &str) -> bool {
        self.make_targets.as_ref().is_some_and(|t| t.contains(name))
    }

    pub fn has_package_script(&self, name: &str) -> bool {
        self.package_scripts
            .as_ref()
            .is_some_and(|s| s.contains(name))
    }
}

pub fn make_targets(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in text.lines() {
        if line.starts_with(['\t', '#', ' ']) {
            continue;
        }
        let Some((lhs, rhs)) = line.split_once(':') else {
            continue;
        };
        // `VAR := 1` and `VAR ::= 1` are assignments, not rules.
        if lhs.contains('=') || rhs.starts_with('=') || rhs.starts_with(":=") {
            continue;
        }
        for name in lhs.split_whitespace() {
            if name.starts_with('.') || name.contains(['%', '$', '(']) {
                continue;
            }
            out.insert(name.to_string());
        }
    }
    out
}

pub fn package_scripts(text: &str) -> BTreeSet<String> {
    let v: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return BTreeSet::new(),
    };
    v.get("scripts")
        .and_then(|s| s.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

fn git_remotes(root: &Path) -> BTreeSet<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("remote")
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let names: BTreeSet<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect();
            if names.is_empty() {
                default_remotes()
            } else {
                names
            }
        }
        _ => default_remotes(),
    }
}

fn default_remotes() -> BTreeSet<String> {
    DEFAULT_REMOTES.iter().map(|s| s.to_string()).collect()
}

pub fn on_path(program: &str) -> bool {
    if program.contains('/') {
        return is_executable(Path::new(program));
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| is_executable(&dir.join(program)))
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        p.metadata()
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_make_targets() {
        let t = make_targets(
            ".PHONY: build test\nVAR := 1\nbuild: ## Build\n\tcargo build\ntest check: build\n%.o: %.c\n$(BIN): x\n# comment: no\n",
        );
        let names: Vec<&str> = t.iter().map(String::as_str).collect();
        assert_eq!(names, vec!["build", "check", "test"]);
    }

    #[test]
    fn reads_package_scripts() {
        let s = package_scripts(r#"{"name":"x","scripts":{"lint":"eslint .","test":"vitest"}}"#);
        assert_eq!(s.len(), 2);
        assert!(s.contains("lint"));
        assert!(package_scripts("{").is_empty());
        assert!(package_scripts(r#"{"name":"x"}"#).is_empty());
    }

    #[test]
    fn finds_programs_on_path() {
        assert!(on_path("sh"));
        assert!(!on_path("no-such-program-herdr-llm-lint"));
        assert!(on_path("/bin/sh"));
    }

    #[test]
    fn loads_a_root() {
        let root = crate::testutil::tempdir("repo");
        std::fs::write(root.join("Makefile"), "check:\n\ttrue\n").unwrap();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/a.txt"), "").unwrap();
        let r = Repo::load(&root);
        assert!(r.has_make_target("check"));
        assert!(!r.has_make_target("build"));
        assert!(!r.has_package_script("lint"));
        assert!(r.path_exists(&root, "sub/a.txt"));
        assert!(r.path_exists(&root.join("sub"), "a.txt"));
        assert!(!r.path_exists(&root, "nope"));
    }

    #[test]
    fn git_refs_outside_a_checkout_use_the_default_remotes() {
        let root = crate::testutil::tempdir("remotes");
        let r = Repo::load(&root);
        assert!(r.is_git_ref("origin/main"));
        assert!(r.is_git_ref("upstream/master"));
        assert!(r.is_git_ref("HEAD"));
        assert!(r.is_git_ref("refs/heads/main"));
        assert!(!r.is_git_ref("origin/"));
        assert!(!r.is_git_ref("src/main.rs"));
        assert!(!r.is_git_ref("fork/main"));
    }

    #[test]
    fn git_refs_inside_a_checkout_use_its_remotes() {
        let root = crate::testutil::tempdir("remotes-git");
        let git = |args: &[&str]| {
            let ok = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["remote", "add", "fork", "https://example.invalid/x.git"]);
        let r = Repo::load(&root);
        assert!(r.is_git_ref("fork/main"));
        assert!(!r.is_git_ref("origin/main"));
    }
}
