use crate::tui::state::AppState;
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render_sidebar(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let lines: Vec<Line> = state
        .sidebar_lines
        .iter()
        .map(|line| Line::from(Span::raw(line.as_str())))
        .collect();

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::LEFT)
                .title(" Sidebar ")
                .border_style(state.theme.sidebar_border),
        )
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(state.theme.bg).fg(state.theme.fg));

    f.render_widget(widget, area);
}
