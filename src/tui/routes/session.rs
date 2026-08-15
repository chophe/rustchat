use crate::tui::backend::ChatBackend;
use crate::tui::state::{AppState, Route};

pub fn sync_from_backend(state: &mut AppState, backend: &dyn ChatBackend) {
    state.route = Route::Session;
    let (left, right) = backend.sync_header();
    state.header_left = left;
    state.header_right = right;
    state.sidebar_lines = backend.sync_sidebar();
    if !state.streaming && !state.suppress_rebuild {
        let messages = backend.sync_messages();
        // Fast path: when the local log already matches the backend (the common
        // case after a streamed `Done`), skip the O(history) line/markdown
        // rebuild so per-turn cost stays flat as the conversation grows.
        if !state.scrollback.same_as(&messages) {
            state.sync_messages(messages);
        }
    }
}
