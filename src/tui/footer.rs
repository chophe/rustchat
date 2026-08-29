use crate::tui::state::AppState;
use crate::tui::widgets::{render_autocomplete, render_hints, render_status_bar};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// The footer has exactly one mode at a time. Rendering is a simple match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FooterMode {
    /// Idle: the prompt textarea is visible and interactive.
    Prompt,
    /// A turn is running: the prompt row shows a progress line. Typed text is
    /// buffered into the textarea as a queued draft and flushed when idle.
    Streaming,
    /// A backend permission request is awaiting y/n (future backends).
    Permission { prompt: String },
    /// A backend question is awaiting a numbered choice (future backends).
    Question { prompt: String, options: Vec<String> },
}

/// Scanner-style spinner: diamond glyphs with an alpha-decay feel.
const SPINNER: [char; 4] = ['◇', '◈', '◆', '◈'];

/// Rows inside the fixed footer region: prompt / hints / status.
pub fn render_footer(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    match &state.footer_mode {
        FooterMode::Prompt => render_prompt(f, rows[0], state),
        FooterMode::Streaming => render_streaming(f, rows[0], state),
        FooterMode::Permission { prompt } => render_permission(f, rows[0], state, prompt),
        FooterMode::Question { prompt, options } => {
            render_question(f, rows[0], state, prompt, options)
        }
    }

    render_hints(f, rows[1], state);
    render_status_bar(f, rows[2], state);

    if state.autocomplete.is_open() {
        render_autocomplete(f, area, state);
    }
}

/// SplitBorder: vertical rails only (no top/bottom) for the opencode look.
fn rail_block(state: &AppState, title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_type(ratatui::widgets::BorderType::Plain)
        .title(Line::from(format!(" {title} ")).style(Style::default().fg(state.theme.muted)))
        .border_style(state.theme.prompt_border)
        .style(Style::default().bg(state.theme.panel_bg))
}

fn render_prompt(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let mut textarea = state.textarea.clone();
    textarea.set_block(rail_block(state, "Prompt"));
    f.render_widget(&textarea, area);
}

fn render_streaming(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let spinner = SPINNER[(state.frame % SPINNER.len() as u64) as usize];
    let status = if state.status.is_empty() {
        "streaming..."
    } else {
        &state.status
    };
    let line = Line::from(vec![
        Span::styled(
            format!("{spinner} "),
            Style::default().fg(state.theme.primary),
        ),
        Span::styled(status, Style::default().fg(state.theme.fg)),
    ]);
    let widget = Paragraph::new(line)
        .block(rail_block(state, "Response"))
        .style(Style::default().bg(state.theme.panel_bg).fg(state.theme.fg));
    f.render_widget(widget, area);
}

fn render_permission(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState, prompt: &str) {
    render_prompt_block(
        f,
        area,
        state,
        "Permission",
        &[Span::raw(prompt.to_string()), Span::raw("  (y/n)")],
    );
}

fn render_question(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    state: &AppState,
    prompt: &str,
    options: &[String],
) {
    let mut spans = vec![Span::raw(prompt.to_string())];
    for (i, option) in options.iter().enumerate() {
        spans.push(Span::raw(format!("  {} {}", i + 1, option)));
    }
    render_prompt_block(f, area, state, "Question", &spans);
}

fn render_prompt_block(
    f: &mut Frame,
    area: ratatui::layout::Rect,
    state: &AppState,
    title: &str,
    spans: &[Span<'static>],
) {
    let widget = Paragraph::new(Line::from(spans.to_vec()))
        .block(rail_block(state, title))
        .style(Style::default().bg(state.theme.panel_bg).fg(state.theme.fg));
    f.render_widget(widget, area);
}

impl FooterMode {
    /// Transition helpers used by the state machine tests.
    #[cfg(test)]
    pub fn is_prompt(&self) -> bool {
        matches!(self, FooterMode::Prompt)
    }

    #[cfg(test)]
    pub fn is_streaming(&self) -> bool {
        matches!(self, FooterMode::Streaming)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_is_prompt_and_streaming() {
        assert!(FooterMode::Prompt.is_prompt());
        assert!(!FooterMode::Streaming.is_prompt());
        assert!(FooterMode::Streaming.is_streaming());
        assert!(!FooterMode::Permission { prompt: "x".into() }.is_streaming());
    }
}
