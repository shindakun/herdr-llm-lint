//! Drift between and across instruction files: diverged copies, nested files
//! repeating the root, paths changed since the file was edited, age.
//! Planned: `drift-copies`, `drift-nested`, `drift-stale`, `drift-age`.
//!
//! Each check is a `fn(&Lint) -> Vec<Finding>` registered here; move its id
//! out of `PLANNED` in `mod.rs` when it lands.

use super::Check;

pub fn checks() -> Vec<Check> {
    Vec::new()
}
