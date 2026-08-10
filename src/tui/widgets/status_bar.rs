use crate::tui::state::AppState;
use ratatui::{
    style::Style,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_status_bar(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let status = if state.status.is_empty() {
        "Ready".to_string()
    } else {
        state.status.clone()
    };
    let widget = Paragraph::new(status)
        .style(
            Style::default()
                .bg(state.theme.status_bg)
                .fg(state.theme.status_fg),
        )
        .block(Block::default().borders(Borders::TOP));
    f.render_widget(widget, area);
}
