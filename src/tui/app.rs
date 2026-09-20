use std::path::PathBuf;
use std::process::Command;

use ratatui::layout::Rect;
use ratatui::widgets::ListState;

use crate::cli;
use crate::herdr::PluginEnv;
use crate::model::Finding;

pub enum Action {
    Edit,
    Send,
}

pub struct App {
    pub root: PathBuf,
    pub findings: Vec<Finding>,
    pub env: Option<PluginEnv>,
    pub list: ListState,
    /// Where the list was drawn last frame, for mouse hits.
    pub list_area: Rect,
    pub status: Option<String>,
    pub quit: bool,
}

impl App {
    pub fn new(root: PathBuf, findings: Vec<Finding>, env: Option<PluginEnv>) -> Self {
        let mut list = ListState::default();
        if !findings.is_empty() {
            list.select(Some(0));
        }
        Self {
            root,
            findings,
            env,
            list,
            list_area: Rect::default(),
            status: None,
            quit: false,
        }
    }

    pub fn selected(&self) -> Option<&Finding> {
        self.list.selected().and_then(|i| self.findings.get(i))
    }

    pub fn select(&mut self, index: usize) {
        if self.findings.is_empty() {
            return;
        }
        self.list.select(Some(index.min(self.findings.len() - 1)));
    }

    pub fn move_selection(&mut self, delta: isize) {
        let cur = self.list.selected().unwrap_or(0) as isize;
        self.select((cur + delta).max(0) as usize);
    }

    pub fn perform(&mut self, action: Action, terminal: &mut ratatui::DefaultTerminal) {
        self.status = Some(match action {
            Action::Edit => self.edit(terminal),
            Action::Send => self.send(),
        });
    }

    fn edit(&self, terminal: &mut ratatui::DefaultTerminal) -> String {
        let Some(f) = self.selected() else {
            return "nothing selected".into();
        };
        let editor = std::env::var("EDITOR")
            .ok()
            .filter(|e| !e.trim().is_empty())
            .unwrap_or_else(|| "vi".to_string());
        let mut parts = editor.split_whitespace();
        let program = parts.next().unwrap_or("vi");
        let mut cmd = Command::new(program);
        cmd.args(parts);
        if let Some(line) = f.line {
            cmd.arg(format!("+{line}"));
        }
        cmd.arg(self.root.join(&f.file));
        ratatui::restore();
        let result = cmd.status();
        let _ = terminal.clear();
        match result {
            Ok(s) if s.success() => format!("edited {}", f.file.display()),
            Ok(s) => format!("{program} exited with {s}"),
            Err(e) => format!("{program}: {e}"),
        }
    }

    fn send(&self) -> String {
        if self.findings.is_empty() {
            return "no findings to send".into();
        }
        match &self.env {
            Some(env) => cli::send_findings(env, &self.findings).unwrap_or_else(|e| e),
            None => "not running under Herdr; nothing sent".into(),
        }
    }
}
