use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

pub const PLUGIN_ID: &str = "shindakun.llm-lint";

pub const REPORT_PANE: &str = "report";

// Set by `herdr-action` on the popup so the pane lints the same root.
pub const ROOT_VAR: &str = "HERDR_LLM_LINT_ROOT";

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

pub fn plugin_id() -> String {
    var("HERDR_PLUGIN_ID").unwrap_or_else(|| PLUGIN_ID.to_string())
}

pub fn root_override() -> Option<PathBuf> {
    var(ROOT_VAR).map(PathBuf::from)
}

#[derive(Debug, Clone)]
pub struct PluginEnv {
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub bin_path: PathBuf,
    pub context: Option<Context>,
}

impl PluginEnv {
    pub fn from_env() -> Result<Self, String> {
        let context = match var("HERDR_PLUGIN_CONTEXT_JSON") {
            Some(json) => Some(Context::parse(&json)?),
            None => None,
        };
        Ok(Self {
            config_dir: var("HERDR_PLUGIN_CONFIG_DIR")
                .map(PathBuf::from)
                .ok_or("HERDR_PLUGIN_CONFIG_DIR is not set; run under herdr")?,
            state_dir: var("HERDR_PLUGIN_STATE_DIR")
                .map(PathBuf::from)
                .ok_or("HERDR_PLUGIN_STATE_DIR is not set; run under herdr")?,
            bin_path: var("HERDR_BIN_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("herdr")),
            context,
        })
    }

    pub fn present() -> bool {
        var("HERDR_PLUGIN_STATE_DIR").is_some()
    }

    pub fn run(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new(&self.bin_path)
            .args(args)
            .output()
            .map_err(|e| format!("spawn {}: {e}", self.bin_path.display()))?;
        if !out.status.success() {
            return Err(format!(
                "herdr {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn agents(&self) -> Result<Vec<Agent>, String> {
        parse_agent_list(&self.run(&["agent", "list"])?)
    }

    // Returns once Herdr has typed the text and Enter; it does not wait
    // for the agent's turn.
    pub fn prompt(&self, target: &str, text: &str) -> Result<(), String> {
        self.run(&["agent", "prompt", target, text]).map(drop)
    }

    pub fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        self.run(&["notification", "show", title, "--body", body])
            .map(drop)
    }

    pub fn open_report(&self, root: &Path) -> Result<(), String> {
        let root_env = format!("{ROOT_VAR}={}", root.display());
        let cwd = root.display().to_string();
        self.run(&[
            "plugin",
            "pane",
            "open",
            "--plugin",
            &plugin_id(),
            "--entrypoint",
            REPORT_PANE,
            "--cwd",
            &cwd,
            "--env",
            &root_env,
            "--focus",
        ])
        .map(drop)
    }

    /// Herdr's plugins directory (`~/.config/herdr/plugins`), derived from
    /// the config dir Herdr hands out under `plugins/config/<id>`.
    pub fn plugins_dir(&self) -> Option<PathBuf> {
        self.config_dir
            .ancestors()
            .find(|a| a.file_name().is_some_and(|n| n == "plugins"))
            .map(Path::to_path_buf)
    }

    /// A plugin pane (a file viewer, this popup) runs with a cwd inside a
    /// plugin checkout; that is never the project to lint.
    pub fn is_plugin_dir(&self, path: &Path) -> bool {
        self.plugins_dir().is_some_and(|d| path.starts_with(d))
    }

    /// The directory to lint from: the workspace cwd, else the workspace
    /// agent's cwd, else the focused pane's cwd, skipping plugin dirs.
    pub fn lint_start(&self) -> Result<Option<PathBuf>, String> {
        let ctx = self.context.as_ref();
        let usable = |p: Option<&str>| {
            p.map(PathBuf::from)
                .filter(|p| p.is_dir() && !self.is_plugin_dir(p))
        };
        if let Some(p) = usable(ctx.and_then(|c| c.workspace_cwd.as_deref())) {
            return Ok(Some(p));
        }
        if let Some(workspace) = ctx.and_then(|c| c.workspace_id.as_deref()) {
            let agents = self.agents()?;
            let agent = pick_agent(
                &agents,
                workspace,
                ctx.and_then(|c| c.focused_pane_id.as_deref()),
            );
            if let Some(p) = usable(agent.and_then(|a| a.cwd.as_deref())) {
                return Ok(Some(p));
            }
        }
        Ok(usable(ctx.and_then(|c| c.focused_pane_cwd.as_deref())))
    }

    pub fn workspace_agent(&self) -> Result<Agent, String> {
        let ctx = self.context.as_ref();
        let workspace = ctx
            .and_then(|c| c.workspace_id.as_deref())
            .ok_or("no workspace in HERDR_PLUGIN_CONTEXT_JSON")?;
        let agents = self.agents()?;
        pick_agent(
            &agents,
            workspace,
            ctx.and_then(|c| c.focused_pane_id.as_deref()),
        )
        .cloned()
        .ok_or_else(|| format!("no agent in workspace {workspace}"))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Context {
    pub workspace_id: Option<String>,
    pub workspace_cwd: Option<String>,
    pub focused_pane_id: Option<String>,
    pub focused_pane_cwd: Option<String>,
    pub invocation_source: Option<String>,
}

impl Context {
    pub fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("HERDR_PLUGIN_CONTEXT_JSON: {e}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Agent {
    pub pane_id: String,
    pub workspace_id: String,
    pub agent_status: String,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentListing {
    result: AgentList,
}

#[derive(Debug, Deserialize)]
struct AgentList {
    agents: Vec<Agent>,
}

pub fn parse_agent_list(json: &str) -> Result<Vec<Agent>, String> {
    let l: AgentListing = serde_json::from_str(json).map_err(|e| format!("agent list: {e}"))?;
    Ok(l.result.agents)
}

pub fn pick_agent<'a>(
    agents: &'a [Agent],
    workspace_id: &str,
    focused_pane: Option<&str>,
) -> Option<&'a Agent> {
    let mine: Vec<&Agent> = agents
        .iter()
        .filter(|a| a.workspace_id == workspace_id)
        .collect();
    match mine.as_slice() {
        [] => None,
        [one] => Some(one),
        many => many
            .iter()
            .find(|a| Some(a.pane_id.as_str()) == focused_pane)
            .or_else(|| many.iter().find(|a| a.focused))
            .copied()
            .or(Some(many[0])),
    }
}

// `worktree.created` per herdr 0.9 `src/api/schema/events.rs`: EventKind
// is snake_case, EventData is tagged by `type`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeCreated {
    pub workspace_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    event: String,
    data: EventData,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct EventData {
    workspace: Option<WorkspaceInfo>,
    worktree: Option<WorktreeInfo>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceInfo {
    workspace_id: String,
}

#[derive(Debug, Deserialize)]
struct WorktreeInfo {
    path: String,
}

impl WorktreeCreated {
    pub fn parse(json: &str) -> Result<Option<Self>, String> {
        let e: Envelope =
            serde_json::from_str(json).map_err(|e| format!("HERDR_PLUGIN_EVENT_JSON: {e}"))?;
        if e.event != "worktree_created" {
            return Ok(None);
        }
        let missing = |f: &str| format!("HERDR_PLUGIN_EVENT_JSON: missing {f}");
        Ok(Some(Self {
            workspace_id: e
                .data
                .workspace
                .map(|w| w.workspace_id)
                .ok_or_else(|| missing("workspace"))?,
            path: e
                .data
                .worktree
                .map(|w| PathBuf::from(w.path))
                .ok_or_else(|| missing("worktree"))?,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AGENTS: &str = include_str!("../tests/fixtures/agent_list.json");
    const EVENT: &str = include_str!("../tests/fixtures/worktree_created_event.json");

    #[test]
    fn parses_agent_list() {
        let agents = parse_agent_list(AGENTS).unwrap();
        assert_eq!(agents.len(), 3);
        assert_eq!(agents[0].pane_id, "w3:p1");
        assert_eq!(agents[0].agent_status, "idle");
        assert!(agents[1].focused);
    }

    #[test]
    fn picks_the_workspace_agent() {
        let agents = parse_agent_list(AGENTS).unwrap();
        assert_eq!(pick_agent(&agents, "w5", None).unwrap().pane_id, "w5:p1");
        assert!(pick_agent(&agents, "w9", None).is_none());
        let mut two = agents.clone();
        two[0].workspace_id = "w5".into();
        assert_eq!(
            pick_agent(&two, "w5", Some("w3:p1")).unwrap().pane_id,
            "w3:p1"
        );
        assert_eq!(pick_agent(&two, "w5", None).unwrap().pane_id, "w5:p1");
    }

    #[test]
    fn parses_worktree_created() {
        let ev = WorktreeCreated::parse(EVENT).unwrap().unwrap();
        assert_eq!(ev.workspace_id, "w4");
        assert_eq!(ev.path, PathBuf::from("/home/u/Code/app-wt/feature"));
        let other = r#"{"event":"pane_focused","data":{"type":"pane_focused","pane_id":"w1:p1","workspace_id":"w1"}}"#;
        assert_eq!(WorktreeCreated::parse(other).unwrap(), None);
        assert!(WorktreeCreated::parse("{").is_err());
        let partial = r#"{"event":"worktree_created","data":{"type":"worktree_created"}}"#;
        assert!(WorktreeCreated::parse(partial).is_err());
    }

    #[test]
    fn plugin_dirs_are_skipped_as_lint_starts() {
        let cfg = Path::new("/home/u/.config/herdr/plugins/config/shindakun.llm-lint");
        let env = PluginEnv {
            config_dir: cfg.to_path_buf(),
            state_dir: PathBuf::new(),
            bin_path: PathBuf::from("/nonexistent/herdr"),
            context: Some(Context {
                workspace_id: None,
                workspace_cwd: Some("/home/u/.config/herdr/plugins/github/viewer/src".into()),
                focused_pane_id: None,
                focused_pane_cwd: Some(std::env::temp_dir().display().to_string()),
                invocation_source: None,
            }),
        };
        assert_eq!(
            env.plugins_dir(),
            Some(PathBuf::from("/home/u/.config/herdr/plugins"))
        );
        assert!(env.is_plugin_dir(Path::new("/home/u/.config/herdr/plugins/github/viewer/src")));
        assert_eq!(env.lint_start().unwrap(), Some(std::env::temp_dir()));
        assert_eq!(Context::parse("{}").unwrap().workspace_cwd, None);
    }
}
