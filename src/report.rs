use crate::model::{Finding, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
}

impl std::str::FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "text" => Ok(Format::Text),
            "json" => Ok(Format::Json),
            other => Err(format!("unknown format `{other}`; use text or json")),
        }
    }
}

pub fn text(findings: &[Finding]) -> String {
    let mut s = String::new();
    for f in findings {
        s.push_str(&f.to_string());
        s.push('\n');
    }
    s
}

pub fn json(findings: &[Finding]) -> Result<String, String> {
    serde_json::to_string_pretty(findings).map_err(|e| e.to_string())
}

pub fn render(findings: &[Finding], format: Format) -> Result<String, String> {
    match format {
        Format::Text => Ok(text(findings)),
        Format::Json => json(findings),
    }
}

pub fn fails(findings: &[Finding], fail_on: Severity) -> bool {
    findings.iter().any(|f| f.severity >= fail_on)
}

pub const MAX_PROMPT_FINDINGS: usize = 40;

pub fn prompt(findings: &[Finding]) -> String {
    let mut s = String::from(
        "herdr-llm-lint found problems in this repo's agent instruction files. \
Fix the instruction files so each finding goes away. Do not change code, \
build targets, or paths to match a stale rule unless the rule is right and \
the repo is what drifted. Findings, one per line as file:line: check: message:\n\n",
    );
    for f in findings.iter().take(MAX_PROMPT_FINDINGS) {
        s.push_str(&f.to_string());
        s.push('\n');
    }
    if findings.len() > MAX_PROMPT_FINDINGS {
        s.push_str(&format!(
            "\n...and {} more; run `herdr-llm-lint lint` for the full list.\n",
            findings.len() - MAX_PROMPT_FINDINGS
        ));
    }
    s
}

pub fn count(findings: &[Finding]) -> String {
    match findings.len() {
        1 => "1 finding".to_string(),
        n => format!("{n} findings"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Finding> {
        vec![
            Finding::new(
                "CLAUDE.md",
                Some(3),
                "ref-path",
                Severity::Error,
                "`x` does not exist",
            ),
            Finding::new("CLAUDE.md", None, "size-bytes", Severity::Warn, "big"),
        ]
    }

    #[test]
    fn text_is_one_line_per_finding() {
        assert_eq!(
            text(&sample()),
            "CLAUDE.md:3: ref-path: `x` does not exist\nCLAUDE.md: size-bytes: big\n"
        );
        assert_eq!(text(&[]), "");
    }

    #[test]
    fn json_round_trips_fields() {
        let j = json(&sample()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v[0]["file"], "CLAUDE.md");
        assert_eq!(v[0]["line"], 3);
        assert_eq!(v[0]["check"], "ref-path");
        assert_eq!(v[0]["severity"], "error");
        assert!(v[1]["line"].is_null());
    }

    #[test]
    fn fail_on_compares_severity() {
        assert!(fails(&sample(), Severity::Warn));
        assert!(fails(&sample(), Severity::Error));
        assert!(!fails(&sample()[1..], Severity::Error));
        assert!(!fails(&[], Severity::Info));
    }

    #[test]
    fn prompt_lists_findings_and_caps() {
        let p = prompt(&sample());
        assert!(p.contains("CLAUDE.md:3: ref-path"));
        assert!(!p.contains("more;"));
        let many: Vec<Finding> = (0..50)
            .map(|i| Finding::new("A.md", Some(i), "ref-path", Severity::Error, "m"))
            .collect();
        assert!(prompt(&many).contains("and 10 more"));
    }

    #[test]
    fn counts() {
        assert_eq!(count(&sample()), "2 findings");
        assert_eq!(count(&sample()[..1]), "1 finding");
    }
}
