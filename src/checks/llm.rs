//! Opt-in: `[llm] enabled = true`. One prompt per instruction file goes to
//! the workspace agent through `herdr agent prompt --wait`; the agent
//! writes its reply to a file under the plugin state dir in a fixed line
//! format, which is parsed here. Needs the Herdr environment and never
//! runs when `CI` is set.

use std::path::Path;
use std::sync::OnceLock;

use super::Check;
use crate::herdr::PluginEnv;
use crate::model::{Finding, Severity};
use crate::scan::Doc;
use crate::Lint;

pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "llm-conflict",
            severity: Severity::Info,
            default_on: true,
            run: |l| only(l, "llm-conflict"),
        },
        Check {
            id: "llm-unclear",
            severity: Severity::Info,
            default_on: true,
            run: |l| only(l, "llm-unclear"),
        },
        Check {
            id: "llm-missing",
            severity: Severity::Info,
            default_on: true,
            run: |l| only(l, "llm-missing"),
        },
    ]
}

static REPLIES: OnceLock<Vec<Finding>> = OnceLock::new();

fn only(lint: &Lint, id: &'static str) -> Vec<Finding> {
    REPLIES
        .get_or_init(|| all(lint))
        .iter()
        .filter(|f| f.check == id)
        .cloned()
        .collect()
}

fn all(lint: &Lint) -> Vec<Finding> {
    if std::env::var_os("CI").is_some() {
        return Vec::new();
    }
    if !PluginEnv::present() {
        eprintln!("herdr-llm-lint: llm checks need the Herdr environment; skipped");
        return Vec::new();
    }
    match ask_all(lint) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("herdr-llm-lint: llm checks skipped: {e}");
            Vec::new()
        }
    }
}

fn ask_all(lint: &Lint) -> Result<Vec<Finding>, String> {
    let env = PluginEnv::from_env()?;
    let agent = env.workspace_agent()?;
    let dir = env.state_dir.join("llm");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let timeout_ms = (lint.config.llm.timeout_secs * 1000).to_string();
    let mut out = Vec::new();
    for doc in &lint.docs {
        let reply = dir.join(format!("{}.txt", reply_name(&doc.path)));
        let _ = std::fs::remove_file(&reply);
        let summary = repo_summary(lint, doc);
        env.run(&[
            "agent",
            "prompt",
            &agent.pane_id,
            &prompt(doc, &summary, &reply),
            "--wait",
            "--timeout",
            &timeout_ms,
        ])?;
        let text = std::fs::read_to_string(&reply)
            .map_err(|_| format!("agent wrote no reply at {}", reply.display()))?;
        out.extend(parse_reply(&text, doc));
    }
    Ok(out)
}

fn reply_name(path: &Path) -> String {
    let s = path.to_string_lossy();
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn repo_summary(lint: &Lint, doc: &Doc) -> String {
    let repo = lint.repo(doc);
    let mut s = String::new();
    let mut entries: Vec<String> = std::fs::read_dir(&repo.root)
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n != ".git")
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    s.push_str(&format!("top level: {}\n", entries.join(" ")));
    if let Some(t) = &repo.make_targets {
        let t: Vec<&str> = t.iter().map(String::as_str).collect();
        s.push_str(&format!("make targets: {}\n", t.join(" ")));
    }
    if let Some(p) = &repo.package_scripts {
        let p: Vec<&str> = p.iter().map(String::as_str).collect();
        s.push_str(&format!("package scripts: {}\n", p.join(" ")));
    }
    if !repo.tools.is_empty() {
        let t: Vec<&str> = repo.tools.iter().copied().collect();
        s.push_str(&format!("tools declared: {}\n", t.join(" ")));
    }
    for (lang, v) in &repo.versions {
        s.push_str(&format!("{lang} version: {} ({})\n", v.value, v.source));
    }
    if !repo.enforced.is_empty() {
        let e: Vec<String> = repo
            .enforced
            .iter()
            .map(|(t, f)| format!("{t} ({f})"))
            .collect();
        s.push_str(&format!("enforced by hooks or CI: {}\n", e.join(", ")));
    }
    s
}

fn prompt(doc: &Doc, summary: &str, reply: &Path) -> String {
    format!(
        "herdr-llm-lint review of {file}. Read that file. Repo summary:\n{summary}\n\
Find (1) rules that contradict each other in meaning, (2) rules an agent could read two ways, \
(3) things the repo does that no rule covers: an unusual build step, a generated directory, a required env var. \
Write your answer ONLY to {reply}, one finding per line, nothing else in the file, and do not edit any other file:\n\
conflict <line> <one sentence>\nunclear <line> <one sentence>\nmissing 0 <one sentence>\n\
<line> is the 1-based line in {file}, or 0 when it is about the whole file. \
Write the single word `none` if there is nothing. Then stop.",
        file = doc.path.display(),
        reply = reply.display(),
    )
}

/// `conflict 14 message`, `unclear 22 message`, `missing 0 message`;
/// other lines are ignored.
pub fn parse_reply(text: &str, doc: &Doc) -> Vec<Finding> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut parts = line.trim().splitn(3, char::is_whitespace);
        let (Some(kind), Some(n), Some(msg)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let id = match kind.to_ascii_lowercase().as_str() {
            "conflict" => "llm-conflict",
            "unclear" => "llm-unclear",
            "missing" => "llm-missing",
            _ => continue,
        };
        let Ok(n) = n
            .trim_matches(|c: char| !c.is_ascii_digit())
            .parse::<usize>()
        else {
            continue;
        };
        let line = (n >= 1 && n <= doc.lines.len()).then_some(n);
        out.push(Finding::new(
            &doc.rel,
            line,
            id,
            Severity::Info,
            msg.trim().to_string(),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_reply_format() {
        let doc = Doc::parse(
            std::path::PathBuf::from("/x/CLAUDE.md"),
            std::path::PathBuf::from("CLAUDE.md"),
            "a\nb\nc\n".into(),
        );
        let f = parse_reply(
            "conflict 2 Says both always and never for tests.\nUNCLEAR 3: two readings\nmissing 0 No rule covers the generated dir.\nnone\nchatter here\nconflict x bad\nconflict 99 out of range\n",
            &doc,
        );
        let got: Vec<(&str, Option<usize>, &str)> = f
            .iter()
            .map(|f| (f.check, f.line, f.message.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (
                    "llm-conflict",
                    Some(2),
                    "Says both always and never for tests."
                ),
                ("llm-unclear", Some(3), "two readings"),
                ("llm-missing", None, "No rule covers the generated dir."),
                ("llm-conflict", None, "out of range"),
            ]
        );
        assert!(parse_reply("none\n", &doc).is_empty());
    }

    #[test]
    fn reply_names_are_stable_hex() {
        let a = reply_name(Path::new("/a/CLAUDE.md"));
        assert_eq!(a, reply_name(Path::new("/a/CLAUDE.md")));
        assert_ne!(a, reply_name(Path::new("/b/CLAUDE.md")));
        assert_eq!(a.len(), 16);
    }
}
