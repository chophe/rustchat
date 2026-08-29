use crate::tui::state::AppState;
use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_toast(f: &mut Frame, area: Rect, state: &AppState) {
    let Some(toast) = &state.toast else {
        return;
    };
    // Single-line toast: render the first line of the message.
    let line = toast.message.lines().next().unwrap_or("");
    let width = line
        .len()
        .min(area.width.saturating_sub(2) as usize)
        .max(10) as u16
        + 2;
    let height = 1;
    // Top-right, rail-bordered (left/right only) like opencode toasts.
    let x = area.width.saturating_sub(width).saturating_sub(1);
    let y = 1;
    let toast_area = Rect::new(x, y, width.min(area.width), height.min(area.height));

    f.render_widget(Clear, toast_area);
    let widget = Paragraph::new(line)
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT)
                .border_style(state.theme.dialog_border)
                .style(Style::default().bg(state.theme.panel_bg)),
        )
        .alignment(Alignment::Center)
        .style(state.theme.toast);
    f.render_widget(widget, toast_area);
}
