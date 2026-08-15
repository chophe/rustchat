use crate::tui::state::{AppState, Route};
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
    Frame,
};

/// Renders the messages zone from the append-only `Scrollback` line cache.
///
/// Only the lines visible in the viewport are written into the frame buffer,
/// so the per-frame cost is bounded by the terminal height — not by the length
/// of the transcript.
pub fn render_messages(f: &mut Frame, area: ratatui::layout::Rect, state: &mut AppState) {
    if state.route == Route::Home {
        let widget = Paragraph::new(Line::from(Span::styled(
            "RustChat TUI — press Ctrl+N for new session, Ctrl+P for command palette",
            Style::default().fg(state.theme.accent),
        )))
        .style(Style::default().bg(state.theme.bg).fg(state.theme.fg));
        f.render_widget(widget, area);
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Messages ")
        .border_style(state.theme.prompt_border)
        .style(Style::default().bg(state.theme.bg));
    let inner = block.inner(area);

    let width = inner.width.max(1) as usize;
    state.scrollback.prepare(&state.theme, width);

    {
        let buf = f.buffer_mut();
        block.render(area, buf);

        let total = state.scrollback.line_count();
        if total == 0 {
            let hint = Line::from(Span::styled(
                "No messages yet. Type below to start chatting.",
                Style::default().fg(state.theme.dim).bg(state.theme.bg),
            ));
            buf.set_line(inner.x, inner.y, &hint, inner.width);
            return;
        }

        let viewport = inner.height.max(1) as usize;
        let max_scroll = total.saturating_sub(viewport);
        let scroll = if state.scroll == u16::MAX {
            max_scroll
        } else {
            (state.scroll as usize).min(max_scroll)
        };

        for (i, line) in state.scrollback.visible_lines(scroll, viewport).into_iter().enumerate() {
            buf.set_line(inner.x, inner.y + i as u16, line, inner.width);
        }
    }
}
