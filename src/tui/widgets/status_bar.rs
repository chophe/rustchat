use crate::tui::state::AppState;
use ratatui::{
    layout::Alignment,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Bottom status bar: current working dir on the left, dim keybinding hint on
/// the right — opencode-style minimal chrome.
pub fn render_status_bar(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let line = Line::from(vec![
        Span::styled(
            cwd,
            Style::default().fg(state.theme.status_fg),
        ),
        Span::styled(
            "  Ctrl+P commands",
            Style::default().fg(state.theme.muted),
        ),
    ]);

    let widget = Paragraph::new(line)
        .alignment(Alignment::Left)
        .style(Style::default().bg(state.theme.status_bg));
    f.render_widget(widget, area);
}
