//! The check registry. One file per group; each group exports `checks()`.
//! A check is a plain function over the loaded `Lint`, so cross-file checks
//! (drift between copies) and single-file checks share one signature.
//!
//! Groups whose checks are not written yet still register their ids in
//! `PLANNED`, so `disable = [...]` in a config validates against the full
//! list from docs/PLAN.md and a check landing later does not change what a
//! config accepts.

pub mod content;
pub mod drift;
pub mod facts;
pub mod git;
pub mod llm;
pub mod refs;
pub mod size;

use crate::model::{Finding, Severity};
use crate::Lint;

pub struct Check {
    pub id: &'static str,
    pub severity: Severity,
    pub run: fn(&Lint) -> Vec<Finding>,
}

/// Ids from the plan with no implementation yet.
pub const PLANNED: &[&str] = &[
    "ref-env",
    "ref-skill",
    "fact-version",
    "fact-layout",
    "fact-tool",
    "drift-copies",
    "drift-nested",
    "drift-stale",
    "drift-age",
    "content-dup",
    "content-conflict",
    "content-enforced",
    "content-secret",
    "content-denylist",
    "content-vague",
    "size-section",
    "git-untracked",
    "git-local-tracked",
    "llm-conflict",
    "llm-unclear",
    "llm-missing",
];

/// Every implemented check, in the order the groups appear in the plan.
pub fn all() -> Vec<Check> {
    let mut out = Vec::new();
    out.extend(refs::checks());
    out.extend(facts::checks());
    out.extend(drift::checks());
    out.extend(content::checks());
    out.extend(size::checks());
    out.extend(git::checks());
    out.extend(llm::checks());
    out
}

/// Implemented and planned ids, for config validation.
pub fn ids() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = all().iter().map(|c| c.id).collect();
    out.extend(PLANNED);
    out
}

/// Runs the enabled checks and sorts the findings by file, line, and id.
pub fn run(lint: &Lint) -> Vec<Finding> {
    let mut out: Vec<Finding> = all()
        .iter()
        .filter(|c| lint.config.enabled(c.id))
        .filter(|c| !c.id.starts_with("llm-") || lint.config.llm.enabled)
        .flat_map(|c| (c.run)(lint))
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn ids_are_unique_and_planned_are_not_implemented() {
        let implemented: Vec<&str> = all().iter().map(|c| c.id).collect();
        let set: BTreeSet<&str> = implemented.iter().copied().collect();
        assert_eq!(set.len(), implemented.len(), "duplicate check id");
        for id in PLANNED {
            assert!(
                !set.contains(id),
                "{id} is implemented; drop it from PLANNED"
            );
        }
    }

    #[test]
    fn every_id_has_a_group_prefix() {
        for id in ids() {
            let group = id.split('-').next().unwrap();
            assert!(
                ["ref", "fact", "drift", "content", "size", "shape", "git", "llm"].contains(&group),
                "{id}"
            );
        }
    }
}
