//! herdr-llm-lint: a linter for agent instruction files (`CLAUDE.md`,
//! `AGENTS.md`, and friends) that runs as a CLI and as a Herdr plugin. The
//! library holds everything; `main.rs` only dispatches argv.
//!
//! The pipeline: `scan` finds and parses the instruction files under a root,
//! `repo` reads the facts the checks compare against (Makefile targets,
//! package scripts, PATH), `checks` runs every enabled check over the lot,
//! and `report` prints the findings. The design is in docs/PLAN.md.

pub mod checks;
pub mod cli;
pub mod config;
pub mod herdr;
pub mod model;
pub mod refs;
pub mod repo;
pub mod report;
pub mod scan;
#[cfg(test)]
pub mod testutil;
pub mod tui;

use std::path::{Path, PathBuf};

use config::Config;
use model::Finding;
use repo::Repo;
use scan::Doc;

/// Everything one lint run has loaded. Checks read from this.
pub struct Lint {
    pub root: PathBuf,
    pub config: Config,
    pub docs: Vec<Doc>,
    pub repo: Repo,
}

impl Lint {
    /// Loads the config, the instruction files, and the repo facts under
    /// `root`. `config_dir` is the Herdr plugin config dir, when present.
    pub fn load(root: &Path, config_dir: Option<&Path>) -> Result<Self, String> {
        let root = std::fs::canonicalize(root).map_err(|e| format!("{}: {e}", root.display()))?;
        let config = Config::load(&root, config_dir)?;
        let docs = scan::load_all(&root, &config.files)?;
        let repo = Repo::load(&root);
        Ok(Self {
            root,
            config,
            docs,
            repo,
        })
    }

    /// Runs every enabled check and returns the findings sorted by file,
    /// line, and check id.
    pub fn run(&self) -> Vec<Finding> {
        checks::run(self)
    }
}
