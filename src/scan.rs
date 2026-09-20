//! Finds the instruction files under a root and parses each one into lines
//! and sections.

use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;

/// The patterns a run scans when the config does not set `files`. Root
/// files for each agent, plus nested `CLAUDE.md` and `AGENTS.md` for
/// monorepos.
pub const DEFAULT_FILES: &[&str] = &[
    "CLAUDE.md",
    "CLAUDE.local.md",
    "AGENTS.md",
    "GEMINI.md",
    ".cursorrules",
    ".cursor/rules/*.mdc",
    ".github/copilot-instructions.md",
    "**/CLAUDE.md",
    "**/AGENTS.md",
];

/// One instruction file, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    /// Absolute path.
    pub path: PathBuf,
    /// Relative to the lint root; what findings print.
    pub rel: PathBuf,
    pub text: String,
    pub lines: Vec<String>,
    pub sections: Vec<Section>,
}

/// A heading and the lines under it. A file with text before its first
/// heading gets a section with no heading for that text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub heading: Option<String>,
    /// `#` count; 0 for the headingless preamble.
    pub level: usize,
    /// 1-based, inclusive.
    pub start: usize,
    pub end: usize,
}

impl Doc {
    pub fn load(root: &Path, path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rel = path.strip_prefix(root).unwrap_or(path).to_path_buf();
        Ok(Self::parse(path.to_path_buf(), rel, text))
    }

    pub fn parse(path: PathBuf, rel: PathBuf, text: String) -> Self {
        let lines: Vec<String> = text.lines().map(str::to_string).collect();
        let sections = sections(&lines);
        Self {
            path,
            rel,
            text,
            lines,
            sections,
        }
    }

    /// The directory the file lives in, for resolving relative paths.
    pub fn dir(&self) -> &Path {
        self.path.parent().unwrap_or(&self.path)
    }

    pub fn bytes(&self) -> usize {
        self.text.len()
    }
}

/// Splits `lines` at markdown ATX headings. Headings inside fenced code
/// blocks do not count.
pub fn sections(lines: &[String]) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        if is_fence(line) {
            in_fence = !in_fence;
        }
        let heading = if in_fence { None } else { heading(line) };
        match heading {
            Some((level, title)) => {
                if let Some(last) = out.last_mut() {
                    last.end = n - 1;
                }
                out.push(Section {
                    heading: Some(title),
                    level,
                    start: n,
                    end: n,
                });
            }
            None => {
                if out.is_empty() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    out.push(Section {
                        heading: None,
                        level: 0,
                        start: n,
                        end: n,
                    });
                }
            }
        }
    }
    if let Some(last) = out.last_mut() {
        last.end = lines.len();
    }
    out
}

/// Whether a line opens or closes a fenced code block.
pub fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

fn heading(line: &str) -> Option<(usize, String)> {
    let level = line.bytes().take_while(|b| *b == b'#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with(' ') && !rest.is_empty() {
        return None;
    }
    Some((level, rest.trim().trim_end_matches('#').trim().to_string()))
}

/// The instruction files under `root` matching `patterns`, sorted. Walks
/// with gitignore rules whether or not `root` is a git checkout, includes
/// dotfiles, and never enters `.git`.
pub fn find(root: &Path, patterns: &[String]) -> Result<Vec<PathBuf>, String> {
    let set = glob_set(patterns)?;
    let mut out = Vec::new();
    let walk = WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    for entry in walk {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        if set.is_match(rel) {
            out.push(entry.into_path());
        }
    }
    out.sort();
    Ok(out)
}

fn glob_set(patterns: &[String]) -> Result<GlobSet, String> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        b.add(Glob::new(p).map_err(|e| format!("files pattern `{p}`: {e}"))?);
    }
    b.build().map_err(|e| e.to_string())
}

/// `find` then `Doc::load` for each hit.
pub fn load_all(root: &Path, patterns: &[String]) -> Result<Vec<Doc>, String> {
    find(root, patterns)?
        .iter()
        .map(|p| Doc::load(root, p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        s.lines().map(str::to_string).collect()
    }

    #[test]
    fn splits_sections_at_headings() {
        let s = sections(&lines(
            "intro\n\n# One\na\n## Two\n```\n# not a heading\n```\nb\n",
        ));
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].heading, None);
        assert_eq!((s[0].start, s[0].end), (1, 2));
        assert_eq!(s[1].heading.as_deref(), Some("One"));
        assert_eq!((s[1].start, s[1].end), (3, 4));
        assert_eq!(s[2].heading.as_deref(), Some("Two"));
        assert_eq!(s[2].level, 2);
        assert_eq!((s[2].start, s[2].end), (5, 9));
    }

    #[test]
    fn no_sections_for_an_empty_file() {
        assert!(sections(&lines("\n\n")).is_empty());
    }

    #[test]
    fn heading_needs_a_space() {
        assert_eq!(heading("#tag"), None);
        assert_eq!(heading("# Title #"), Some((1, "Title".into())));
        assert_eq!(heading("### x"), Some((3, "x".into())));
    }

    #[test]
    fn finds_default_files_including_hidden_and_nested() {
        let root = crate::testutil::tempdir("scan");
        for f in [
            "CLAUDE.md",
            ".cursor/rules/a.mdc",
            ".github/copilot-instructions.md",
            "web/AGENTS.md",
            "node_modules/x/CLAUDE.md",
            ".git/CLAUDE.md",
            "README.md",
        ] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "x\n").unwrap();
        }
        std::fs::write(root.join(".gitignore"), "node_modules/\n").unwrap();
        let patterns: Vec<String> = DEFAULT_FILES.iter().map(|s| s.to_string()).collect();
        let found: Vec<String> = find(&root, &patterns)
            .unwrap()
            .iter()
            .map(|p| {
                p.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            found,
            vec![
                ".cursor/rules/a.mdc",
                ".github/copilot-instructions.md",
                "CLAUDE.md",
                "web/AGENTS.md",
            ]
        );
    }
}
