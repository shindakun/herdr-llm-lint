//! Facts about the repo: versions, layout claims, and named tools that the
//! repo files contradict. Planned: `fact-version`, `fact-layout`,
//! `fact-tool`.
//!
//! Each check is a `fn(&Lint) -> Vec<Finding>` registered here; move its id
//! out of `PLANNED` in `mod.rs` when it lands.

use super::Check;

pub fn checks() -> Vec<Check> {
    Vec::new()
}
