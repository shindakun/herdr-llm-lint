use std::path::{Path, PathBuf};

use crate::herdr::{self, PluginEnv, WorktreeCreated};
use crate::model::{Finding, Severity};
use crate::report::{self, Format};
use crate::{fix, tui, Lint};

pub const USAGE: &str =
    "usage: herdr-llm-lint lint [PATH] [--format text|json] [--fail-on info|warn|error] [--fix] \
| herdr-action | herdr-send [--print] | herdr-event | herdr-pane [PATH]";

struct Args {
    path: Option<PathBuf>,
    opts: Vec<(String, String)>,
    flags: Vec<String>,
}

impl Args {
    fn parse(cmd: &str, args: &[String], opts: &[&str], flags: &[&str]) -> Result<Self, String> {
        let mut a = Args {
            path: None,
            opts: Vec::new(),
            flags: Vec::new(),
        };
        let mut it = args.iter();
        while let Some(arg) = it.next() {
            match arg.as_str() {
                o if opts.contains(&o) => {
                    let v = it.next().ok_or_else(|| format!("{o} needs a value"))?;
                    a.opts.push((o.to_string(), v.clone()));
                }
                f if flags.contains(&f) => a.flags.push(f.to_string()),
                other if other.starts_with('-') => {
                    return Err(format!("{cmd}: unknown argument {other}\n{USAGE}"))
                }
                p if a.path.is_none() => a.path = Some(PathBuf::from(p)),
                other => return Err(format!("{cmd}: unexpected argument {other}\n{USAGE}")),
            }
        }
        Ok(a)
    }

    fn opt(&self, name: &str) -> Option<&str> {
        self.opts
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
}

fn plugin_env() -> Result<Option<PluginEnv>, String> {
    if PluginEnv::present() {
        PluginEnv::from_env().map(Some)
    } else {
        Ok(None)
    }
}

fn lint_root(path: Option<PathBuf>, env: Option<&PluginEnv>) -> Result<PathBuf, String> {
    if let Some(p) = path.or_else(herdr::root_override) {
        return Ok(p);
    }
    if let Some(root) = env.and_then(|e| e.context.as_ref()).and_then(|c| c.root()) {
        return Ok(root);
    }
    let cwd = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
    Ok(find_root(&cwd))
}

pub fn find_root(start: &Path) -> PathBuf {
    start
        .ancestors()
        .find(|d| d.join(".git").exists() || d.join(crate::config::FILE_NAME).is_file())
        .unwrap_or(start)
        .to_path_buf()
}

fn load(root: &Path, env: Option<&PluginEnv>) -> Result<(Lint, Vec<Finding>), String> {
    let lint = Lint::load(root, env.map(|e| e.config_dir.as_path()))?;
    let findings = lint.run();
    Ok((lint, findings))
}

pub fn lint(args: &[String]) -> Result<(), String> {
    let a = Args::parse("lint", args, &["--format", "--fail-on"], &["--fix"])?;
    let env = plugin_env()?;
    let root = lint_root(a.path.clone(), env.as_ref())?;
    let (lint, mut findings) = load(&root, env.as_ref())?;
    let format: Format = a.opt("--format").unwrap_or("text").parse()?;
    let fail_on: Severity = match a.opt("--fail-on") {
        Some(s) => s.parse()?,
        None => lint.config.fail_on.into(),
    };
    if a.flag("--fix") {
        let actions = fix::apply(&lint, &findings)?;
        for action in &actions {
            eprintln!("{action}");
        }
        if !actions.is_empty() {
            findings = load(&root, env.as_ref())?.1;
        }
    }
    print!("{}", report::render(&findings, format)?);
    if report::fails(&findings, fail_on) {
        std::process::exit(1);
    }
    Ok(())
}

pub fn herdr_action() -> Result<(), String> {
    let env = PluginEnv::from_env()?;
    let root = lint_root(None, Some(&env))?;
    let (_, findings) = load(&root, Some(&env))?;
    if findings.is_empty() {
        return env.notify("Instruction lint", "no findings");
    }
    env.open_report(&root)
}

pub fn herdr_send(args: &[String]) -> Result<(), String> {
    let a = Args::parse("herdr-send", args, &[], &["--print"])?;
    let env = plugin_env()?;
    let root = lint_root(a.path.clone(), env.as_ref())?;
    let (_, findings) = load(&root, env.as_ref())?;
    if findings.is_empty() {
        return Err("no findings; nothing to send".into());
    }
    if a.flag("--print") {
        print!("{}", report::prompt(&findings));
        return Ok(());
    }
    let env = env.ok_or("herdr-send needs the Herdr environment; use --print outside Herdr")?;
    println!("{}", send_findings(&env, &findings)?);
    Ok(())
}

pub fn send_findings(env: &PluginEnv, findings: &[Finding]) -> Result<String, String> {
    let agent = env.workspace_agent()?;
    env.prompt(&agent.pane_id, &report::prompt(findings))?;
    Ok(format!(
        "sent {} to {}",
        report::count(findings),
        agent.pane_id
    ))
}

pub fn herdr_event() -> Result<(), String> {
    let json = std::env::var("HERDR_PLUGIN_EVENT_JSON")
        .map_err(|_| "HERDR_PLUGIN_EVENT_JSON is not set; run under herdr")?;
    let Some(event) = WorktreeCreated::parse(&json)? else {
        return Ok(());
    };
    let env = PluginEnv::from_env()?;
    let (_, findings) = load(&event.path, Some(&env))?;
    if findings.is_empty() {
        return Ok(());
    }
    let body = format!(
        "{} in {}; run the lint action for the report",
        report::count(&findings),
        event.path.display()
    );
    env.notify("Instruction lint", &body)
}

pub fn herdr_pane(args: &[String]) -> Result<(), String> {
    let a = Args::parse("herdr-pane", args, &[], &[])?;
    let env = plugin_env()?;
    let root = lint_root(a.path, env.as_ref())?;
    let (lint, findings) = load(&root, env.as_ref())?;
    tui::run(lint.root, findings, env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_args() {
        let args: Vec<String> = ["--format", "json", "x", "--fix"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let a = Args::parse("lint", &args, &["--format"], &["--fix"]).unwrap();
        assert_eq!(a.path, Some(PathBuf::from("x")));
        assert_eq!(a.opt("--format"), Some("json"));
        assert!(a.flag("--fix"));
        assert!(Args::parse("lint", &["--nope".to_string()], &[], &[]).is_err());
        assert!(Args::parse("lint", &["a".to_string(), "b".to_string()], &[], &[]).is_err());
        assert!(Args::parse("lint", &["--format".to_string()], &["--format"], &[]).is_err());
    }

    #[test]
    fn find_root_stops_at_git_or_config() {
        let root = crate::testutil::tempdir("root");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        assert_eq!(find_root(&root.join("a/b")), root);
        let loose = crate::testutil::tempdir("loose");
        assert_eq!(find_root(&loose), loose);
    }
}
