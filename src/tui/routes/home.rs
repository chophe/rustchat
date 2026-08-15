use crate::tui::state::{AppState, Route};

pub fn render_home_hint(state: &mut AppState) {
    state.route = Route::Home;
    state.scrollback.clear();
    state.suppress_rebuild = false;
}
