pub mod checks;
pub mod cli;
pub mod config;
pub mod diff;
pub mod facts;
pub mod fix;
pub mod git;
pub mod herdr;
pub mod model;
pub mod refs;
pub mod repo;
pub mod report;
pub mod scan;
#[cfg(test)]
pub mod testutil;
pub mod tui;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use config::Config;
use model::Finding;
use repo::Repo;
use scan::Doc;

pub struct Lint {
    pub root: PathBuf,
    pub config: Config,
    pub docs: Vec<Doc>,
    /// One per project directory the docs live under, keyed by that dir.
    pub repos: BTreeMap<PathBuf, Repo>,
}

const PROJECT_MARKERS: &[&str] = &[
    ".git",
    "go.mod",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "build.zig",
    "Makefile",
    "justfile",
];

impl Lint {
    pub fn load(root: &Path, config_dir: Option<&Path>) -> Result<Self, String> {
        let root = std::fs::canonicalize(root).map_err(|e| format!("{}: {e}", root.display()))?;
        let config = Config::load(&root, config_dir)?;
        let docs = scan::load_all(&root, &config.files)?;
        let mut repos = BTreeMap::new();
        repos.insert(root.clone(), Repo::load(&root));
        for doc in &docs {
            let dir = project_dir(&root, doc.dir());
            repos.entry(dir.clone()).or_insert_with(|| Repo::load(&dir));
        }
        Ok(Self {
            root,
            config,
            docs,
            repos,
        })
    }

    /// The repo facts for the project a doc belongs to: the nearest
    /// ancestor with a project marker, else the lint root. A vendored
    /// subtree with its own `build.zig` is checked against itself.
    pub fn repo(&self, doc: &Doc) -> &Repo {
        let dir = project_dir(&self.root, doc.dir());
        self.repos.get(&dir).unwrap_or(&self.repos[&self.root])
    }

    pub fn run(&self) -> Vec<Finding> {
        checks::run(self)
    }
}

fn project_dir(root: &Path, dir: &Path) -> PathBuf {
    dir.ancestors()
        .take_while(|d| d.starts_with(root))
        .find(|d| PROJECT_MARKERS.iter().any(|m| d.join(m).exists()))
        .unwrap_or(root)
        .to_path_buf()
}
