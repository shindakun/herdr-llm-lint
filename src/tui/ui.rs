use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

use super::app::App;
use super::keys::HINT;
use crate::model::Severity;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [header, list, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    frame.render_widget(Paragraph::new(header_line(app)), header);
    draw_list(frame, app, list);
    draw_footer(frame, app, footer);
}

fn header_line(app: &App) -> Line<'static> {
    let name = app
        .root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| app.root.display().to_string());
    let count = app.findings.len();
    let summary = if count == 0 {
        Span::styled("✓ no findings", Style::default().fg(Color::Green))
    } else {
        let errors = app
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count();
        Span::styled(
            format!("{count} findings, {errors} errors"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    };
    Line::from(vec![
        Span::styled(
            " Instruction lint ",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" {name}  ")),
        summary,
    ])
}

fn location(f: &crate::model::Finding) -> String {
    match f.line {
        Some(l) => format!("{}:{l}", f.file.display()),
        None => f.file.display().to_string(),
    }
}

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect) {
    app.list_area = area;
    let loc_width = app
        .findings
        .iter()
        .map(|f| location(f).chars().count())
        .max()
        .unwrap_or(0);
    let check_width = app
        .findings
        .iter()
        .map(|f| f.check.len())
        .max()
        .unwrap_or(0);
    let items: Vec<ListItem> = app
        .findings
        .iter()
        .map(|f| {
            let color = match f.severity {
                Severity::Error => Color::Red,
                Severity::Warn => Color::Yellow,
                Severity::Info => Color::Blue,
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {:<5} ", f.severity), Style::default().fg(color)),
                Span::styled(
                    format!("{:<loc_width$}  ", location(f)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<check_width$}  ", f.check),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(f.message.clone()),
            ]))
        })
        .collect();
    let list = List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, area, &mut app.list);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let line = match &app.status {
        Some(s) => Line::from(vec![
            Span::styled(format!(" {s}"), Style::default().fg(Color::Cyan)),
            Span::styled(format!("  {HINT}"), Style::default().fg(Color::DarkGray)),
        ]),
        None => Line::from(Span::styled(
            format!(" {HINT}"),
            Style::default().fg(Color::DarkGray),
        )),
    };
    frame.render_widget(Paragraph::new(line), area);
}
