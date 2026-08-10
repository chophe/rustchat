use crate::tui::state::{AppState, MessageView};
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render_messages(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let mut lines: Vec<Line> = Vec::new();

    if state.route == crate::tui::state::Route::Home {
        lines.push(Line::from(Span::styled(
            "RustChat TUI — press Ctrl+N for new session, Ctrl+P for command palette",
            Style::default().fg(state.theme.accent),
        )));
    }

    for message in &state.messages {
        lines.extend(render_message(message, state));
        lines.push(Line::from(""));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No messages yet. Type below to start chatting.",
            Style::default().fg(state.theme.dim),
        )));
    }

    let height = lines.len().max(1) as u16;
    let max_scroll = height.saturating_sub(area.height.saturating_sub(2));
    let scroll = if state.scroll == u16::MAX {
        max_scroll
    } else {
        state.scroll.min(max_scroll)
    };

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Messages ")
                .border_style(state.theme.prompt_border),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .style(Style::default().bg(state.theme.bg).fg(state.theme.fg));

    f.render_widget(widget, area);
}

fn render_message(message: &MessageView, state: &AppState) -> Vec<Line<'static>> {
    let label_style = match message.role {
        crate::client::MessageRole::User => state.theme.user_label,
        crate::client::MessageRole::Assistant => state.theme.assistant_label,
        crate::client::MessageRole::System => state.theme.system_label,
        crate::client::MessageRole::Tool => state.theme.system_label,
    };

    let mut lines = vec![Line::from(vec![
        Span::styled(format!("[{}]", message.label()), label_style),
        if message.streaming {
            Span::styled(" ●", Style::default().fg(state.theme.accent))
        } else {
            Span::raw("")
        },
    ])];

    for line in message.text.lines() {
        lines.push(Line::from(line.to_string()));
    }

    for tool in &message.tool_calls {
        lines.push(Line::from(Span::styled(
            format!("  tool: {}({})", tool.name, tool.arguments),
            Style::default().fg(state.theme.dim),
        )));
    }

    lines
}
