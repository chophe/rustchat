mod dialog;
mod header;
mod message_list;
mod prompt;
mod sidebar;
mod status_bar;
mod toast;

pub use dialog::{render_dialog, DialogKind, PaletteCommand};
pub use header::render_header;
pub use message_list::render_messages;
pub use prompt::render_prompt;
pub use sidebar::render_sidebar;
pub use status_bar::render_status_bar;
pub use toast::render_toast;

use crate::tui::state::AppState;
use ratatui::Frame;

pub fn render(f: &mut Frame, state: &AppState) {
    let area = f.area();
    let chunks = if state.sidebar_open {
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Min(20),
                ratatui::layout::Constraint::Length(28),
            ])
            .split(area)
    } else {
        ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([ratatui::layout::Constraint::Percentage(100)])
            .split(area)
    };

    let main = chunks[0];
    let inner = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(3),
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Length(4),
        ])
        .split(main);

    render_header(f, inner[0], state);
    render_messages(f, inner[1], state);
    render_status_bar(f, inner[2], state);
    render_prompt(f, inner[3], state);

    if state.sidebar_open {
        if let Some(sidebar_area) = chunks.get(1) {
            render_sidebar(f, *sidebar_area, state);
        }
    }

    if let Some(dialog) = &state.dialog {
        render_dialog(f, area, state, dialog);
    }

    if state.toast.is_some() {
        render_toast(f, area, state);
    }
}
