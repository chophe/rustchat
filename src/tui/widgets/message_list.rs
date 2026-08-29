use crate::tui::state::{AppState, Route};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Renders the messages zone from the append-only `Scrollback` line cache.
///
/// The transcript is a bare reading surface (no chrome/box), matching opencode.
/// Only the lines visible in the viewport are written into the frame buffer,
/// so the per-frame cost is bounded by the terminal height — not by the length
/// of the transcript.
pub fn render_messages(f: &mut Frame, area: Rect, state: &mut AppState) {
    if state.route == Route::Home {
        render_home(f, area, state);
        return;
    }

    let width = area.width.max(1) as usize;
    state.scrollback.prepare(&state.theme, width);

    let buf = f.buffer_mut();
    let total = state.scrollback.line_count();
    if total == 0 {
        let hint = Line::from(Span::styled(
            "No messages yet. Type below to start chatting.",
            Style::default().fg(state.theme.muted).bg(state.theme.bg),
        ));
        buf.set_line(area.x, area.y, &hint, area.width);
        return;
    }

    let viewport = area.height.max(1) as usize;
    let max_scroll = total.saturating_sub(viewport);
    let scroll = if state.scroll == u16::MAX {
        max_scroll
    } else {
        (state.scroll as usize).min(max_scroll)
    };

    for (i, line) in state.scrollback.visible_lines(scroll, viewport).into_iter().enumerate() {
        buf.set_line(area.x, area.y + i as u16, line, area.width);
    }
}

/// Home route: a centered ASCII rustchat splash with a hint line.
fn render_home(f: &mut Frame, area: Rect, state: &mut AppState) {
    use ratatui::layout::Alignment;

    let logo = r#"
  ____            _      _   _         _   
 |  _ \ _   _ ___| |_ __| |_| |__  ___| |_ 
 | |_) | | | / __| __/ _` | __| '_ \/ __| __|
 |  _ <| |_| \__ \ || (_| | |_| |_) \__ \ |_ 
 |_| \_\\__,_|___/\__\__,_|\__|_.__/|___/\__|
"#;
    let mut lines: Vec<Line> = logo
        .lines()
        .map(|l| {
            Line::from(Span::styled(
                l.to_string(),
                Style::default().fg(state.theme.primary),
            ))
        })
        .collect();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "type a message below,  Ctrl+P command palette,  Ctrl+N new session",
        Style::default().fg(state.theme.muted),
    )));

    let widget = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .style(Style::default().bg(state.theme.bg).fg(state.theme.fg));
    f.render_widget(widget, area);
}
