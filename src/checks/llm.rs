//! Model-assisted checks, off unless `[llm] enabled = true`. They send the
//! file and a repo summary to the workspace agent through `herdr agent
//! prompt` and parse a fixed reply format. Planned: `llm-conflict`,
//! `llm-unclear`, `llm-missing`.
//!
//! Each check is a `fn(&Lint) -> Vec<Finding>` registered here; move its id
//! out of `PLANNED` in `mod.rs` when it lands.

use super::Check;

pub fn checks() -> Vec<Check> {
    Vec::new()
}
