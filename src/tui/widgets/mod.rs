mod autocomplete;
mod dialog;
mod header;
mod hints;
mod message_list;
mod sidebar;
mod status_bar;
mod toast;

pub use autocomplete::{render_autocomplete, AutocompleteState};
pub use dialog::{render_dialog, DialogKind, DialogStack, PaletteCommand};
pub use header::render_header;
pub use hints::{render_hints, render_keybar};
pub use message_list::render_messages;
pub use sidebar::render_sidebar;
pub use status_bar::render_status_bar;
pub use toast::render_toast;

use crate::tui::footer::render_footer;
use crate::tui::state::AppState;
use ratatui::Frame;

/// Footer region height: status(1) + prompt(3) + hints(1) + keybar(1).
const FOOTER_ROWS: u16 = 6;

pub fn render(f: &mut Frame, state: &mut AppState) {
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
            ratatui::layout::Constraint::Length(FOOTER_ROWS),
        ])
        .split(main);

    render_header(f, inner[0], &*state);
    render_messages(f, inner[1], state);
    render_footer(f, inner[2], &*state);

    if state.sidebar_open {
        if let Some(sidebar_area) = chunks.get(1) {
            render_sidebar(f, *sidebar_area, &*state);
        }
    }

    if let Some(dialog) = state.dialog_stack.top() {
        render_dialog(f, area, &*state, dialog);
    }

    if state.toast.is_some() {
        render_toast(f, area, &*state);
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::tui::event::BackendEvent;
    use crate::tui::kv::TuiPreferences;
    use crate::tui::theme::Theme;
    use ratatui::backend::TestBackend;

    fn state() -> AppState {
        AppState::new(Theme::dark(), TuiPreferences::default())
    }

    #[test]
    fn render_full_layout_does_not_panic() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();

        let mut s = state();
        s.push_user_message("hello there".into());
        s.begin_streaming();
        s.handle_backend(BackendEvent::StreamDelta("response token one ".into()));
        s.handle_backend(BackendEvent::StreamDelta("response token two\n".into()));
        s.handle_backend(BackendEvent::StreamDelta("more text".into()));

        terminal.draw(|f| render(f, &mut s)).unwrap();
        terminal.draw(|f| render(f, &mut s)).unwrap();

        // Long transcript scrolls without panicking.
        for i in 0..200 {
            s.handle_backend(BackendEvent::StreamDelta(format!("line {i}\n")));
        }
        s.handle_backend(BackendEvent::Done {
            text: "final answer".into(),
        });
        terminal.draw(|f| render(f, &mut s)).unwrap();
        terminal.draw(|f| render(f, &mut s)).unwrap();

        // Dialog + sidebar + autocomplete surfaces render.
        s.open_dialog(DialogKind::Help);
        terminal.draw(|f| render(f, &mut s)).unwrap();
        s.close_dialog();
        s.sidebar_open = true;
        s.sidebar_lines = vec!["model: test".into()];
        terminal.draw(|f| render(f, &mut s)).unwrap();
    }

    #[test]
    fn autocomplete_popup_renders() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut s = state();
        s.textarea.insert_str("/mod");
        s.autocomplete.on_input(&s.textarea);
        assert!(s.autocomplete.is_open());
        terminal.draw(|f| render(f, &mut s)).unwrap();
    }

    #[test]
    fn renders_on_degenerate_terminal_size() {
        // A pty can report a tiny size; rendering must not panic.
        let backend = TestBackend::new(10, 6);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut s = state();
        s.push_user_message("hello".into());
        s.textarea.insert_str("/mod");
        s.autocomplete.on_input(&s.textarea);
        s.open_dialog(DialogKind::Help);
        terminal.draw(|f| render(f, &mut s)).unwrap();
        s.close_dialog();
        terminal.draw(|f| render(f, &mut s)).unwrap();
    }
}
