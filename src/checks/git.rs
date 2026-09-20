use super::Check;
use crate::git;
use crate::model::{Finding, Severity};
use crate::Lint;

pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "git-untracked",
            severity: Severity::Warn,
            default_on: true,
            run: git_untracked,
        },
        Check {
            id: "git-local-tracked",
            severity: Severity::Warn,
            default_on: true,
            run: git_local_tracked,
        },
    ]
}

fn git_untracked(lint: &Lint) -> Vec<Finding> {
    lint.docs
        .iter()
        .filter(|d| !d.is_local())
        .filter(|d| git::toplevel(d.dir()).is_some() && !git::is_tracked(d.dir(), &d.path))
        .map(|d| {
            Finding::new(
                &d.rel,
                None,
                "git-untracked",
                Severity::Warn,
                "not tracked by git",
            )
        })
        .collect()
}

fn git_local_tracked(lint: &Lint) -> Vec<Finding> {
    lint.docs
        .iter()
        .filter(|d| d.is_local() && git::is_tracked(d.dir(), &d.path))
        .map(|d| {
            Finding::new(
                &d.rel,
                None,
                "git-local-tracked",
                Severity::Warn,
                "tracked by git; a .local.md file is for one machine",
            )
        })
        .collect()
}
