//! The report popup: a list of findings, `enter` opens one in `$EDITOR`,
//! `a` sends them all to the workspace's agent, `q` quits.

mod app;
mod keys;
mod ui;

use std::path::PathBuf;

use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::herdr::PluginEnv;
use crate::model::Finding;
use app::App;

pub fn run(root: PathBuf, findings: Vec<Finding>, env: Option<PluginEnv>) -> Result<(), String> {
    let mut app = App::new(root, findings, env);
    let mut terminal = ratatui::try_init().map_err(|e| format!("terminal: {e}"))?;
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<(), String> {
    while !app.quit {
        terminal
            .draw(|frame| ui::draw(frame, app))
            .map_err(|e| format!("draw: {e}"))?;
        match event::read().map_err(|e| format!("input: {e}"))? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if let Some(action) = keys::key(app, key) {
                    app.perform(action, terminal);
                }
            }
            Event::Mouse(m) => keys::mouse(app, m),
            _ => {}
        }
    }
    Ok(())
}
