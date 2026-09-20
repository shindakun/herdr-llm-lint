//! Git: instruction files that should be tracked and are not, and local
//! files that should not be tracked and are. Planned: `git-untracked`,
//! `git-local-tracked`.
//!
//! Each check is a `fn(&Lint) -> Vec<Finding>` registered here; move its id
//! out of `PLANNED` in `mod.rs` when it lands.

use super::Check;

pub fn checks() -> Vec<Check> {
    Vec::new()
}
