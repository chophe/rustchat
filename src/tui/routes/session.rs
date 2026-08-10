use crate::tui::backend::ChatBackend;
use crate::tui::state::{AppState, Route};

pub fn sync_from_backend(state: &mut AppState, backend: &dyn ChatBackend) {
    state.route = Route::Session;
    let (left, right) = backend.sync_header();
    state.header_left = left;
    state.header_right = right;
    state.sidebar_lines = backend.sync_sidebar();
    if !state.streaming {
        state.sync_messages(backend.sync_messages());
    }
}
