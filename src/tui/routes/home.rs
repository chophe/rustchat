use crate::tui::state::{AppState, Route};

pub fn render_home_hint(state: &mut AppState) {
    state.route = Route::Home;
    state.messages.clear();
}
