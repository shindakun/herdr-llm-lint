//! Findings and severities. One finding is one line of output.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Error => "error",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "info" => Ok(Severity::Info),
            "warn" | "warning" => Ok(Severity::Warn),
            "error" => Ok(Severity::Error),
            other => Err(format!(
                "unknown severity `{other}`; use info, warn, or error"
            )),
        }
    }
}

/// One problem in one file. `file` is relative to the lint root. `line` is
/// 1-based and absent for whole-file findings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Finding {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub check: &'static str,
    pub severity: Severity,
    pub message: String,
}

impl Finding {
    pub fn new(
        file: impl Into<PathBuf>,
        line: Option<usize>,
        check: &'static str,
        severity: Severity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            file: file.into(),
            line,
            check,
            severity,
            message: message.into(),
        }
    }
}

/// Compiler style: `file:line: check: message`, or `file: check: message`
/// when the finding has no line.
impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(
                f,
                "{}:{line}: {}: {}",
                self.file.display(),
                self.check,
                self.message
            ),
            None => write!(
                f,
                "{}: {}: {}",
                self.file.display(),
                self.check,
                self.message
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severities_order_and_parse() {
        assert!(Severity::Info < Severity::Warn);
        assert!(Severity::Warn < Severity::Error);
        assert_eq!("warn".parse::<Severity>().unwrap(), Severity::Warn);
        assert_eq!("warning".parse::<Severity>().unwrap(), Severity::Warn);
        assert!("loud".parse::<Severity>().is_err());
    }

    #[test]
    fn displays_like_a_compiler() {
        let f = Finding::new("CLAUDE.md", Some(14), "ref-path", Severity::Error, "x");
        assert_eq!(f.to_string(), "CLAUDE.md:14: ref-path: x");
        let f = Finding::new("CLAUDE.md", None, "drift-stale", Severity::Warn, "y");
        assert_eq!(f.to_string(), "CLAUDE.md: drift-stale: y");
    }
}
