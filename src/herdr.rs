//! The environment Herdr injects into plugin commands, the context and
//! event JSON it passes, and calls back into Herdr through `HERDR_BIN_PATH`.
//! Names follow herdr 0.9.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// The manifest id, used when Herdr did not pass `HERDR_PLUGIN_ID`.
pub const PLUGIN_ID: &str = "shindakun.llm-lint";

/// The pane entrypoint id in `herdr-plugin.toml`.
pub const REPORT_PANE: &str = "report";

/// Set by `herdr-action` when it opens the popup, so the pane lints the
/// same root without re-deriving it.
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

    /// Whether this process was started by Herdr.
    pub fn present() -> bool {
        var("HERDR_PLUGIN_STATE_DIR").is_some()
    }

    /// Runs `herdr <args>` and returns stdout.
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

    /// `herdr agent list`, parsed.
    pub fn agents(&self) -> Result<Vec<Agent>, String> {
        parse_agent_list(&self.run(&["agent", "list"])?)
    }

    /// `herdr agent prompt TARGET TEXT`. Returns once Herdr has written the
    /// text and Enter; it does not wait for the agent's turn.
    pub fn prompt(&self, target: &str, text: &str) -> Result<(), String> {
        self.run(&["agent", "prompt", target, text]).map(drop)
    }

    /// `herdr notification show TITLE --body BODY`.
    pub fn notify(&self, title: &str, body: &str) -> Result<(), String> {
        self.run(&["notification", "show", title, "--body", body])
            .map(drop)
    }

    /// Opens the report popup for `root`.
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

    /// The workspace agent to prompt, from the context and `agent list`.
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

/// The parts of `HERDR_PLUGIN_CONTEXT_JSON` this plugin reads.
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

    /// The directory to lint: the workspace cwd. Instruction files live at
    /// the worktree root, so the focused pane's cwd is not used.
    pub fn root(&self) -> Option<PathBuf> {
        self.workspace_cwd.as_deref().map(PathBuf::from)
    }
}

/// One row of `herdr agent list`.
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

/// The workspace's only agent, else the focused one, else the first.
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

/// `HERDR_PLUGIN_EVENT_JSON` for `worktree.created`: the new worktree's
/// path and the workspace Herdr opened for it.
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
    /// `None` when the JSON is some other event.
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
    fn context_root_is_the_workspace_cwd() {
        let c = Context::parse(
            r#"{"workspace_id":"w1","workspace_cwd":"/ws","focused_pane_cwd":"/ws/sub","correlation_id":"x"}"#,
        )
        .unwrap();
        assert_eq!(c.root(), Some(PathBuf::from("/ws")));
        assert_eq!(Context::parse("{}").unwrap().root(), None);
    }
}
