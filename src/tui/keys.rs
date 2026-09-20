use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use super::app::{Action, App};

pub const HINT: &str = "enter edit  a agent  j/k move  q quit";

pub fn key(app: &mut App, key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q') | KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.quit = true
        }
        (KeyCode::Enter, _) => return Some(Action::Edit),
        (KeyCode::Char('a'), _) => return Some(Action::Send),
        (KeyCode::Char('j') | KeyCode::Down, _) => app.move_selection(1),
        (KeyCode::Char('k') | KeyCode::Up, _) => app.move_selection(-1),
        (KeyCode::Char('g') | KeyCode::Home, _) => app.select(0),
        (KeyCode::Char('G') | KeyCode::End, _) => app.select(usize::MAX),
        (KeyCode::PageDown, _) => app.move_selection(10),
        (KeyCode::PageUp, _) => app.move_selection(-10),
        _ => {}
    }
    None
}

pub fn mouse(app: &mut App, m: MouseEvent) {
    match m.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let a = app.list_area;
            if m.column >= a.x && m.column < a.x + a.width && m.row >= a.y && m.row < a.y + a.height
            {
                let index = app.list.offset() + (m.row - a.y) as usize;
                if index < app.findings.len() {
                    app.select(index);
                }
            }
        }
        MouseEventKind::ScrollDown => app.move_selection(1),
        MouseEventKind::ScrollUp => app.move_selection(-1),
        _ => {}
    }
}
