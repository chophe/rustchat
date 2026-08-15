use crate::tui::footer::FooterMode;
use crate::tui::state::AppState;
use ratatui::{
    layout::Alignment,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Row 3 of the footer: context-sensitive secondary hints.
pub fn render_hints(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let text = match &state.footer_mode {
        FooterMode::Streaming => {
            if state.textarea.is_empty() {
                "streaming...  Esc Esc to stop".to_string()
            } else {
                format!(
                    "streaming...  queued: {}",
                    truncate(&state.textarea.lines().join("\n"), area.width as usize)
                )
            }
        }
        FooterMode::Permission { .. } => "y approve / n deny / Esc cancel".to_string(),
        FooterMode::Question { options, .. } => {
            if options.is_empty() {
                "Esc to cancel".to_string()
            } else {
                "choose a number / Esc to cancel".to_string()
            }
        }
        FooterMode::Prompt => {
            if state.autocomplete.is_open() {
                "Tab cycle / Shift+Tab prev / Enter accept (after Tab) / Esc close".to_string()
            } else {
                "Enter submit / Shift+Enter newline".to_string()
            }
        }
    };

    let widget = Paragraph::new(Line::from(Span::styled(
        text,
        Style::default().bg(state.theme.status_bg).fg(state.theme.status_fg),
    )))
    .alignment(Alignment::Left);
    f.render_widget(widget, area);
}

/// Row 4 of the footer: static keybinding bar.
pub fn render_keybar(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let text = "Ctrl+P palette  Ctrl+B sidebar  Ctrl+S sessions  Ctrl+M model  Ctrl+R role  Ctrl+H help  Ctrl+N home  Esc Esc quit/stop";
    let widget = Paragraph::new(Line::from(Span::styled(
        text,
        Style::default().bg(state.theme.status_bg).fg(state.theme.dim),
    )))
    .alignment(Alignment::Left);
    f.render_widget(widget, area);
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let mut out: String = text.chars().take(max.saturating_sub(3)).collect();
        out.push_str("...");
        out
    }
}
