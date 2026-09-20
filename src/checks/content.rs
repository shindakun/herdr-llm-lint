//! Content: duplicate lines, contradictions, rules tooling already enforces,
//! secrets, denylisted phrases, vague imperatives. Planned: `content-dup`,
//! `content-conflict`, `content-enforced`, `content-secret`,
//! `content-denylist`, `content-vague`.
//!
//! Each check is a `fn(&Lint) -> Vec<Finding>` registered here; move its id
//! out of `PLANNED` in `mod.rs` when it lands.

use super::Check;

pub fn checks() -> Vec<Check> {
    Vec::new()
}
