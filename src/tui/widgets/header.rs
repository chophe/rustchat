use crate::tui::state::AppState;
use ratatui::{
    layout::Alignment,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn render_header(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let left = strip_ansi(&state.header_left);
    let right = strip_ansi(&state.header_right);
    let line = Line::from(vec![
        Span::styled(left, Style::default().fg(state.theme.header_fg)),
        Span::raw(" "),
        Span::styled(
            right,
            Style::default().fg(state.theme.dim),
        ),
    ]);
    let widget = Paragraph::new(line)
        .style(Style::default().bg(state.theme.header_bg).fg(state.theme.fg))
        .alignment(Alignment::Left);
    f.render_widget(widget, area);
}

fn strip_ansi(text: &str) -> String {
    text.chars()
        .fold(
            (String::new(), false),
            |(mut out, in_esc), c| match (in_esc, c) {
                (true, 'm') => (out, false),
                (true, _) => (out, true),
                (false, '\u{1b}') => (out, true),
                (false, _) => {
                    out.push(c);
                    (out, false)
                }
            },
        )
        .0
}
