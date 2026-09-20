//! The history checks against a throwaway git repo.

use std::path::{Path, PathBuf};
use std::process::Command;

use herdr_llm_lint::report;
use herdr_llm_lint::Lint;

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn commit(root: &Path, msg: &str, date: &str) {
    git(root, &["add", "-A"]);
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["commit", "-q", "--allow-empty", "-m", msg])
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn temp_repo(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("herdr-llm-lint-git-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    dir
}

fn lint(root: &Path) -> String {
    report::text(&Lint::load(root, None).unwrap().run())
}

#[test]
fn stale_and_age_and_tracking() {
    let root = temp_repo("history");
    std::fs::write(root.join("src/a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(root.join("src/b.rs"), "fn b() {}\n").unwrap();
    std::fs::write(
        root.join("CLAUDE.md"),
        "# t\n\n- Code is in `src/a.rs` and `src/b.rs`.\n",
    )
    .unwrap();
    std::fs::write(root.join("CLAUDE.local.md"), "# local\n").unwrap();
    std::fs::write(root.join(".herdr-llm-lint.toml"), "age_commits = 2\n").unwrap();
    commit(&root, "one", "2026-01-01T00:00:00Z");
    assert_eq!(
        lint(&root),
        "CLAUDE.local.md: git-local-tracked: tracked by git; a .local.md file is for one machine\n"
    );

    git(&root, &["rm", "-q", "--cached", "CLAUDE.local.md"]);
    std::fs::write(root.join(".gitignore"), "CLAUDE.local.md\n").unwrap();
    std::fs::write(root.join("src/a.rs"), "fn a() { 1 }\n").unwrap();
    commit(&root, "two", "2026-02-01T00:00:00Z");
    assert_eq!(
        lint(&root),
        "CLAUDE.md: drift-stale: 1 referenced path changed since 2026-01-01 (newest: src/a.rs)\n"
    );

    std::fs::write(root.join("src/b.rs"), "fn b() { 2 }\n").unwrap();
    commit(&root, "three", "2026-03-01T00:00:00Z");
    commit(&root, "four", "2026-03-02T00:00:00Z");
    std::fs::copy(root.join("CLAUDE.md"), root.join("AGENTS.md")).unwrap();
    let text = lint(&root);
    assert_eq!(
        text,
        "AGENTS.md: git-untracked: not tracked by git\n\
CLAUDE.md: drift-age: 3 commits since it last changed (2026-01-01), limit is 2\n\
CLAUDE.md: drift-stale: 2 referenced paths changed since 2026-01-01 (newest: src/b.rs)\n"
    );

    std::fs::write(root.join("CLAUDE.md"), "# t\n\n- Code is in `src/a.rs`.\n").unwrap();
    assert!(
        !lint(&root).contains("CLAUDE.md: drift"),
        "an edited file is not stale"
    );
}
