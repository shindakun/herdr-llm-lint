use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn toplevel(dir: &Path) -> Option<PathBuf> {
    git(dir, &["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

pub fn is_tracked(dir: &Path, path: &Path) -> bool {
    git(
        dir,
        &["ls-files", "--error-unmatch", "--", &path.to_string_lossy()],
    )
    .is_some()
}

pub fn has_uncommitted_changes(dir: &Path, path: &Path) -> bool {
    git(
        dir,
        &["status", "--porcelain", "--", &path.to_string_lossy()],
    )
    .is_some_and(|s| !s.is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub hash: String,
    pub unix: i64,
    /// `YYYY-MM-DD`.
    pub date: String,
}

pub fn last_change(dir: &Path, path: &Path) -> Option<Change> {
    let out = git(
        dir,
        &[
            "log",
            "-1",
            "--format=%H%n%ct%n%cs",
            "--",
            &path.to_string_lossy(),
        ],
    )?;
    let mut lines = out.lines();
    Some(Change {
        hash: lines.next()?.to_string(),
        unix: lines.next()?.parse().ok()?,
        date: lines.next()?.to_string(),
    })
}

pub fn commits_after(dir: &Path, hash: &str) -> Option<usize> {
    git(dir, &["rev-list", "--count", &format!("{hash}..HEAD")])?
        .parse()
        .ok()
}
