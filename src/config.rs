use std::path::Path;

use serde::Deserialize;

use crate::model::Severity;
use crate::scan::DEFAULT_FILES;

pub const FILE_NAME: &str = ".herdr-llm-lint.toml";
pub const DEFAULT_SIZE_BYTES: usize = 8192;
pub const DEFAULT_LINE_CHARS: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub files: Vec<String>,
    pub size_bytes: usize,
    pub line_chars: usize,
    pub disable: Vec<String>,
    pub enable: Vec<String>,
    pub denylist_file: Option<String>,
    pub fail_on: FailOn,
    pub llm: Llm,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Llm {
    pub enabled: bool,
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
            disable: Vec::new(),
            enable: Vec::new(),
            denylist_file: None,
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
