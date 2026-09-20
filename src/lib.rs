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

pub struct Lint {
    pub root: PathBuf,
    pub config: Config,
    pub docs: Vec<Doc>,
    pub repo: Repo,
}

impl Lint {
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

    pub fn run(&self) -> Vec<Finding> {
        checks::run(self)
    }
}
