use crate::tui::state::AppState;
use ratatui::{
    layout::{Alignment, Rect},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn render_toast(f: &mut Frame, area: Rect, state: &AppState) {
    let Some(toast) = &state.toast else {
        return;
    };
    let width = toast.message.len().min(area.width as usize).max(20) as u16 + 4;
    let height = 3;
    let x = area.width.saturating_sub(width) / 2;
    let y = 1;
    let toast_area = Rect::new(x, y, width.min(area.width), height.min(area.height));

    f.render_widget(Clear, toast_area);
    let widget = Paragraph::new(toast.message.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(state.theme.dialog_border),
        )
        .alignment(Alignment::Center)
        .style(state.theme.toast);
    f.render_widget(widget, toast_area);
}
