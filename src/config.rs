use std::path::Path;

use serde::Deserialize;

use crate::model::Severity;
use crate::scan::DEFAULT_FILES;

pub const FILE_NAME: &str = ".herdr-llm-lint.toml";
pub const DEFAULT_SIZE_BYTES: usize = 8192;
pub const DEFAULT_LINE_CHARS: usize = 120;
pub const DEFAULT_AGE_COMMITS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub files: Vec<String>,
    pub size_bytes: usize,
    pub line_chars: usize,
    pub age_commits: usize,
    pub disable: Vec<String>,
    pub enable: Vec<String>,
    pub denylist_file: Option<String>,
    pub denylist: Vec<String>,
    pub fail_on: FailOn,
    pub llm: Llm,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Llm {
    pub enabled: bool,
    pub timeout_secs: u64,
}

impl Default for Llm {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout_secs: 180,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailOn {
    Info,
    Warn,
    Error,
}

impl From<FailOn> for Severity {
    fn from(f: FailOn) -> Self {
        match f {
            FailOn::Info => Severity::Info,
            FailOn::Warn => Severity::Warn,
            FailOn::Error => Severity::Error,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            files: DEFAULT_FILES.iter().map(|s| s.to_string()).collect(),
            size_bytes: DEFAULT_SIZE_BYTES,
            line_chars: DEFAULT_LINE_CHARS,
            age_commits: DEFAULT_AGE_COMMITS,
            disable: Vec::new(),
            enable: Vec::new(),
            denylist_file: None,
            denylist: Vec::new(),
            fail_on: FailOn::Warn,
            llm: Llm::default(),
        }
    }
}

impl Config {
    pub fn load(root: &Path, config_dir: Option<&Path>) -> Result<Self, String> {
        let mut candidates = vec![root.join(FILE_NAME)];
        if let Some(dir) = config_dir {
            candidates.push(dir.join("config.toml"));
        }
        for path in candidates {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    return Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
        Ok(Self::default())
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let c: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if c.files.is_empty() {
            return Err("files must name at least one pattern".into());
        }
        let known = crate::checks::ids();
        for (key, list) in [("disable", &c.disable), ("enable", &c.enable)] {
            for id in list {
                if !known.contains(&id.as_str()) {
                    return Err(format!("{key}: unknown check `{id}`"));
                }
            }
        }
        Ok(c)
    }

    /// Inline `denylist` plus the lines of `denylist_file` (`~` expands;
    /// blank lines and `#` comments skipped). A missing file is empty.
    pub fn denylist_phrases(&self) -> Vec<String> {
        let mut out = self.denylist.clone();
        if let Some(file) = &self.denylist_file {
            let path = match file.strip_prefix("~/") {
                Some(rest) => std::env::var_os("HOME")
                    .map(|h| std::path::PathBuf::from(h).join(rest))
                    .unwrap_or_else(|| std::path::PathBuf::from(file)),
                None => std::path::PathBuf::from(file),
            };
            if let Ok(text) = std::fs::read_to_string(path) {
                out.extend(
                    text.lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty() && !l.starts_with('#'))
                        .map(str::to_string),
                );
            }
        }
        out
    }

    pub fn enabled(&self, id: &str, default_on: bool) -> bool {
        if self.disable.iter().any(|d| d == id) {
            false
        } else if self.enable.iter().any(|e| e == id) {
            true
        } else {
            default_on
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_default() {
        let c = Config::parse("").unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.size_bytes, 8192);
        assert_eq!(c.files.len(), DEFAULT_FILES.len());
        assert!(!c.llm.enabled);
    }

    #[test]
    fn reads_fields() {
        let c = Config::parse(
            r#"
files = ["CLAUDE.md"]
size_bytes = 512
disable = ["ref-command"]
enable = ["shape-body"]
fail_on = "error"
[llm]
enabled = true
"#,
        )
        .unwrap();
        assert_eq!(c.files, vec!["CLAUDE.md"]);
        assert_eq!(c.size_bytes, 512);
        assert!(!c.enabled("ref-command", true));
        assert!(c.enabled("ref-path", true));
        assert!(c.enabled("shape-body", false));
        assert!(!c.enabled("shape-lines", false));
        assert!(!Config::default().enabled("shape-body", false));
        assert_eq!(Severity::from(c.fail_on), Severity::Error);
        assert!(c.llm.enabled);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Config::parse("files = []\n").is_err());
        assert!(Config::parse("disable = [\"no-such-check\"]\n").is_err());
        assert!(Config::parse("enable = [\"no-such-check\"]\n").is_err());
        assert!(Config::parse("colour = 1\n").is_err());
        assert!(Config::parse("fail_on = \"loud\"\n").is_err());
    }

    #[test]
    fn denylist_merges_inline_and_file() {
        let dir = crate::testutil::tempdir("denylist");
        let file = dir.join("deny.txt");
        std::fs::write(&file, "# personal\nmy rule\n\n  another one  \n").unwrap();
        let c = Config::parse(&format!(
            "denylist = [\"inline\"]\ndenylist_file = \"{}\"\n",
            file.display()
        ))
        .unwrap();
        assert_eq!(
            c.denylist_phrases(),
            vec!["inline", "my rule", "another one"]
        );
        let c = Config::parse("denylist_file = \"/nonexistent/deny.txt\"\n").unwrap();
        assert!(c.denylist_phrases().is_empty());
    }

    #[test]
    fn load_prefers_the_project_file() {
        let root = crate::testutil::tempdir("config-root");
        let cfg = crate::testutil::tempdir("config-dir");
        assert_eq!(Config::load(&root, Some(&cfg)).unwrap(), Config::default());
        std::fs::write(cfg.join("config.toml"), "size_bytes = 5\n").unwrap();
        assert_eq!(Config::load(&root, Some(&cfg)).unwrap().size_bytes, 5);
        std::fs::write(root.join(FILE_NAME), "size_bytes = 7\n").unwrap();
        assert_eq!(Config::load(&root, Some(&cfg)).unwrap().size_bytes, 7);
    }
}
