use crate::tui::state::AppState;
use ratatui::{
    widgets::{Block, Borders},
    Frame,
};

pub fn render_prompt(f: &mut Frame, area: ratatui::layout::Rect, state: &AppState) {
    let mut textarea = state.textarea.clone();
    textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Prompt ")
            .border_style(state.theme.prompt_border),
    );
    f.render_widget(&textarea, area);
}
