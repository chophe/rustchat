use crate::client::{Message, MessageRole};
use crate::function::ToolCall;
use crate::tui::event::{BackendEvent, Focus, KeyAction};
use crate::tui::footer::FooterMode;
use crate::tui::kv::TuiPreferences;
use crate::tui::scrollback::Scrollback;
use crate::tui::theme::Theme;
use crate::tui::widgets::{AutocompleteState, DialogKind, DialogStack, PaletteCommand};
use std::time::{Duration, Instant};
use tui_textarea::TextArea;

/// Window for the two-press interrupt / quit pattern.
const INTERRUPT_WINDOW: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Home,
    Session,
}

#[derive(Debug, Clone)]
pub struct MessageView {
    pub role: MessageRole,
    pub text: String,
    pub streaming: bool,
    pub tool_calls: Vec<ToolCall>,
}

impl MessageView {
    pub fn from_message(message: &Message) -> Self {
        Self {
            role: message.role,
            text: message.content.to_text(),
            streaming: false,
            tool_calls: vec![],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub ttl: u8,
}

pub struct AppState {
    pub route: Route,
    pub theme: Theme,
    pub prefs: TuiPreferences,
    pub focus: Focus,
    pub scrollback: Scrollback,
    pub scroll: u16,
    pub auto_scroll: bool,
    pub streaming: bool,
    pub footer_mode: FooterMode,
    pub status: String,
    pub header_left: String,
    pub header_right: String,
    pub sidebar_open: bool,
    pub sidebar_lines: Vec<String>,
    /// Last-modified timestamps parallel to `sidebar_lines` while the session
    /// list dialog is open (for date grouping).
    pub session_times: Vec<Option<std::time::SystemTime>>,
    pub dialog_stack: DialogStack,
    pub palette_filter: String,
    pub palette_selected: usize,
    pub palette_commands: Vec<PaletteCommand>,
    pub toast: Option<Toast>,
    pub should_quit: bool,
    pub textarea: TextArea<'static>,
    pub autocomplete: AutocompleteState,
    pub interrupt_armed: bool,
    pub interrupt_deadline: Option<Instant>,
    /// True while an abort is being processed by the backend; suppresses the
    /// error toast so the interrupt reads cleanly.
    pub aborting: bool,
    /// After an abort, do not let `sync_from_backend` clobber the locally
    /// preserved partial response until the user takes a fresh action.
    pub suppress_rebuild: bool,
    /// When the current turn started (for the duration meta line).
    stream_started: Option<Instant>,
    /// Tick counter used for animations / spinners.
    pub frame: u64,
}

impl AppState {
    pub fn new(theme: Theme, prefs: TuiPreferences) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("Type a message... (Enter submit, Shift+Enter newline, Esc abort, Ctrl+P palette)");
        Self {
            route: Route::Session,
            theme: theme.clone(),
            sidebar_open: prefs.sidebar_open,
            prefs,
            focus: Focus::Prompt,
            scrollback: Scrollback::new(theme.mode, true),
            scroll: 0,
            auto_scroll: true,
            streaming: false,
            footer_mode: FooterMode::Prompt,
            status: String::new(),
            header_left: String::new(),
            header_right: String::new(),
            sidebar_lines: vec![],
            session_times: vec![],
            dialog_stack: DialogStack::default(),
            palette_filter: String::new(),
            palette_selected: 0,
            palette_commands: PaletteCommand::defaults(),
            toast: None,
            should_quit: false,
            textarea,
            autocomplete: AutocompleteState::default(),
            interrupt_armed: false,
            interrupt_deadline: None,
            aborting: false,
            suppress_rebuild: false,
            stream_started: None,
            frame: 0,
        }
    }

    pub fn sync_messages(&mut self, messages: Vec<MessageView>) {
        self.scrollback.load_messages(&self.theme, messages);
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn append_stream_delta(&mut self, delta: &str) {
        self.scrollback
            .append_stream_delta(&self.theme, delta);
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn finish_stream(&mut self, text: String) {
        let meta = self
            .stream_started
            .take()
            .map(|t| format!("✦ {:.1}s", t.elapsed().as_secs_f32()));
        self.scrollback.finish_stream(&self.theme, text);
        if let Some(meta) = meta {
            self.scrollback.append_meta(&self.theme, &meta);
        }
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn push_user_message(&mut self, text: String) {
        self.scrollback.push_user(&self.theme, text);
        self.auto_scroll = true;
        self.scroll_to_bottom();
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll = u16::MAX;
    }

    pub fn handle_backend(&mut self, event: BackendEvent) {
        match event {
            BackendEvent::StreamDelta(delta) => {
                self.streaming = true;
                self.footer_mode = FooterMode::Streaming;
                self.status = "streaming...".into();
                self.aborting = false;
                self.append_stream_delta(&delta);
            }
            BackendEvent::ToolCall(call) => {
                self.scrollback.append_tool_call(&self.theme, call);
            }
            BackendEvent::Done { text } => {
                self.streaming = false;
                self.status.clear();
                self.footer_mode = FooterMode::Prompt;
                self.aborting = false;
                self.finish_stream(text);
            }
            BackendEvent::Error(err) => {
                if self.aborting {
                    self.aborting = false;
                    self.streaming = false;
                    self.status = "interrupted".into();
                    self.footer_mode = FooterMode::Prompt;
                    self.scrollback.finalize_interrupt(&self.theme);
                } else {
                    self.streaming = false;
                    self.status = format!("error: {err}");
                    self.footer_mode = FooterMode::Prompt;
                    self.show_toast(err);
                }
            }
            BackendEvent::Info(msg) => {
                self.show_toast(msg);
            }
            BackendEvent::PermissionRequest { prompt } => {
                self.footer_mode = FooterMode::Permission { prompt };
                self.status = "permission required".into();
            }
            BackendEvent::Question { prompt, options } => {
                self.footer_mode = FooterMode::Question { prompt, options };
                self.status = "question".into();
            }
            BackendEvent::Exit => {
                self.should_quit = true;
            }
            BackendEvent::SessionUpdated => {}
        }
    }

    /// y/n answer for the current permission request (if any).
    pub fn resolve_permission(&mut self, approved: bool) {
        if matches!(self.footer_mode, FooterMode::Permission { .. }) {
            self.footer_mode = FooterMode::Prompt;
            self.status.clear();
            self.show_toast(if approved {
                "Permission granted"
            } else {
                "Permission denied"
            });
        }
    }

    /// Numbered answer for the current question (if any).
    pub fn resolve_question(&mut self, choice: Option<usize>) {
        if matches!(self.footer_mode, FooterMode::Question { .. }) {
            self.footer_mode = FooterMode::Prompt;
            self.status.clear();
            match choice {
                Some(i) => self.show_toast(format!("Chose option {}", i + 1)),
                None => self.show_toast("Question cancelled"),
            }
        }
    }

    /// Enter the streaming state after submitting a prompt. The textarea keeps
    /// accepting input as a queued draft for the next turn.
    pub fn begin_streaming(&mut self) {
        self.streaming = true;
        self.footer_mode = FooterMode::Streaming;
        self.status = "thinking...".into();
        self.suppress_rebuild = false;
        self.aborting = false;
        self.stream_started = Some(Instant::now());
    }

    /// Called when the user confirms an abort. The partial response is
    /// preserved until the backend reports the turn end.
    pub fn begin_abort(&mut self) {
        self.aborting = true;
        self.suppress_rebuild = true;
        self.status = "stopping...".into();
        self.disarm_interrupt();
    }

    pub fn show_toast(&mut self, message: impl Into<String>) {
        self.toast = Some(Toast {
            message: message.into(),
            ttl: 60,
        });
    }

    pub fn tick_toast(&mut self) {
        if let Some(toast) = self.toast.as_mut() {
            toast.ttl = toast.ttl.saturating_sub(1);
            if toast.ttl == 0 {
                self.toast = None;
            }
        }
    }

    /// Per-frame tick: advance animations, expire the interrupt window.
    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
        self.tick_toast();
        if self.interrupt_armed
            && self
                .interrupt_deadline
                .is_some_and(|d| Instant::now() >= d)
        {
            self.disarm_interrupt();
        }
    }

    pub fn open_palette(&mut self) {
        self.dialog_stack.push(DialogKind::Palette);
        self.palette_filter.clear();
        self.palette_selected = 0;
        self.focus = Focus::Palette;
    }

    pub fn open_dialog(&mut self, kind: DialogKind) {
        self.dialog_stack.push(kind);
        self.focus = Focus::Dialog;
    }

    pub fn close_dialog(&mut self) {
        self.dialog_stack.pop();
        if self.dialog_stack.is_empty() {
            self.focus = Focus::Prompt;
        }
    }

    pub fn filtered_palette_commands(&self) -> Vec<PaletteCommand> {
        let filter = self.palette_filter.to_lowercase();
        self.palette_commands
            .iter()
            .filter(|cmd| {
                filter.is_empty()
                    || cmd.title.to_lowercase().contains(&filter)
                    || cmd.id.to_lowercase().contains(&filter)
            })
            .cloned()
            .collect()
    }

    /// Map a key to an action, mutating transient state along the way
    /// (autocomplete cycling, two-press interrupt arming).
    pub fn map_key(&mut self, key: crossterm::event::KeyEvent) -> KeyAction {
        use crossterm::event::{KeyCode, KeyModifiers};

        if !self.dialog_stack.is_empty() {
            return KeyAction::None;
        }

        // --- Autocomplete surface (prompt focus only) ---
        if self.focus == Focus::Prompt {
            match (key.code, key.modifiers) {
                (KeyCode::Tab, _) => {
                    if self.autocomplete.cycle_next() {
                        return KeyAction::None;
                    }
                }
                (KeyCode::BackTab, _) => {
                    if self.autocomplete.cycle_prev() {
                        return KeyAction::None;
                    }
                }
                (KeyCode::Esc, _) if self.autocomplete.is_open() => {
                    self.autocomplete.close();
                    return KeyAction::None;
                }
                (KeyCode::Enter, KeyModifiers::NONE) if self.autocomplete.active() => {
                    let mut textarea = std::mem::take(&mut self.textarea);
                    self.autocomplete.accept(&mut textarea);
                    self.textarea = textarea;
                    return KeyAction::None;
                }
                _ => {}
            }
        }

        // --- Two-press interrupt / quit ---
        let is_escape_like = matches!(
            (key.code, key.modifiers),
            (KeyCode::Esc, _)
                | (KeyCode::Char('c'), KeyModifiers::CONTROL)
                | (KeyCode::Char('d'), KeyModifiers::CONTROL)
        );
        if is_escape_like {
            if self.streaming {
                return self.press_escape_during_stream();
            }
            if self.focus == Focus::Prompt || self.focus == Focus::Messages {
                return self.press_escape_idle();
            }
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) if self.focus == Focus::Messages => {
                KeyAction::Quit
            }
            (KeyCode::Enter, KeyModifiers::NONE)
                if self.focus == Focus::Prompt && !self.streaming =>
            {
                KeyAction::Submit
            }
            (KeyCode::Enter, KeyModifiers::SHIFT) if self.focus == Focus::Prompt => {
                KeyAction::NewLine
            }
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => KeyAction::OpenPalette,
            (KeyCode::Char('b'), KeyModifiers::CONTROL) => KeyAction::ToggleSidebar,
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => KeyAction::OpenSessionList,
            (KeyCode::Char('m'), KeyModifiers::CONTROL) => KeyAction::OpenModelPicker,
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => KeyAction::OpenRolePicker,
            (KeyCode::Char('h'), KeyModifiers::CONTROL) => KeyAction::OpenHelp,
            (KeyCode::Char('n'), KeyModifiers::CONTROL) => KeyAction::GoHome,
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => KeyAction::ScrollUp,
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => KeyAction::ScrollDown,
            (KeyCode::PageUp, _) => KeyAction::ScrollPageUp,
            (KeyCode::PageDown, _) => KeyAction::ScrollPageDown,
            (KeyCode::Home, _) => KeyAction::ScrollTop,
            (KeyCode::End, _) => KeyAction::ScrollBottom,
            _ => KeyAction::None,
        }
    }

    fn press_escape_during_stream(&mut self) -> KeyAction {
        if self.interrupt_armed
            && self
                .interrupt_deadline
                .is_some_and(|d| Instant::now() < d)
        {
            self.disarm_interrupt();
            KeyAction::Abort
        } else {
            self.arm_interrupt("Press Esc again to stop");
            KeyAction::None
        }
    }

    fn press_escape_idle(&mut self) -> KeyAction {
        if self.interrupt_armed
            && self
                .interrupt_deadline
                .is_some_and(|d| Instant::now() < d)
        {
            self.disarm_interrupt();
            KeyAction::Quit
        } else {
            self.arm_interrupt("Press Esc again to quit");
            KeyAction::None
        }
    }

    fn arm_interrupt(&mut self, hint: &str) {
        self.interrupt_armed = true;
        self.interrupt_deadline = Some(Instant::now() + INTERRUPT_WINDOW);
        self.show_toast(hint);
    }

    fn disarm_interrupt(&mut self) {
        self.interrupt_armed = false;
        self.interrupt_deadline = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn state() -> AppState {
        AppState::new(Theme::dark(), TuiPreferences::default())
    }

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn footer_transitions_prompt_stream_prompt() {
        let mut s = state();
        assert!(s.footer_mode.is_prompt());
        s.begin_streaming();
        assert!(s.footer_mode.is_streaming());
        assert!(s.streaming);
        s.handle_backend(BackendEvent::Done {
            text: "hello".into(),
        });
        assert!(s.footer_mode.is_prompt());
        assert!(!s.streaming);
    }

    #[test]
    fn typing_during_stream_is_queued_draft() {
        let mut s = state();
        s.begin_streaming();
        // The textarea keeps accepting input while streaming (the draft is not
        // cleared by Done).
        s.textarea.insert_str("next prompt");
        s.handle_backend(BackendEvent::Done {
            text: "response".into(),
        });
        assert_eq!(s.textarea.lines().join("\n"), "next prompt");
    }

    #[test]
    fn two_press_interrupt_aborts() {
        let mut s = state();
        s.begin_streaming();
        // First Esc arms and shows a hint, does not abort.
        let first = s.map_key(key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(first, KeyAction::None);
        assert!(s.interrupt_armed);
        // Second Esc within the window aborts.
        let second = s.map_key(key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(second, KeyAction::Abort);
        assert!(!s.interrupt_armed);
    }

    #[test]
    fn two_press_quit_when_idle() {
        let mut s = state();
        let first = s.map_key(key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(first, KeyAction::None);
        let second = s.map_key(key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(second, KeyAction::Quit);
    }

    #[test]
    fn interrupt_window_expires_on_tick() {
        let mut s = state();
        s.begin_streaming();
        let _ = s.map_key(key(KeyCode::Esc, KeyModifiers::NONE));
        assert!(s.interrupt_armed);
        // Force expiry by backdating the deadline.
        s.interrupt_deadline = Some(Instant::now() - Duration::from_secs(10));
        s.tick();
        assert!(!s.interrupt_armed);
        // After expiry, Esc re-arms instead of aborting.
        let first = s.map_key(key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(first, KeyAction::None);
    }

    #[test]
    fn ctrl_c_is_two_press() {
        let mut s = state();
        s.begin_streaming();
        let first = s.map_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(first, KeyAction::None);
        let second = s.map_key(key(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(second, KeyAction::Abort);
    }

    #[test]
    fn enter_submit_blocked_while_streaming() {
        let mut s = state();
        s.begin_streaming();
        let action = s.map_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(action, KeyAction::None);
        s.handle_backend(BackendEvent::Done {
            text: "done".into(),
        });
        let action = s.map_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(action, KeyAction::Submit);
    }

    #[test]
    fn dialog_stack_routes_keys() {
        let mut s = state();
        s.open_dialog(DialogKind::Help);
        // Any key while a dialog is open maps to None (the dialog handles it
        // first in app.rs).
        assert_eq!(
            s.map_key(key(KeyCode::Char('p'), KeyModifiers::NONE)),
            KeyAction::None
        );
        s.close_dialog();
        assert!(s.dialog_stack.is_empty());
        assert_eq!(s.focus, Focus::Prompt);
    }

    #[test]
    fn abort_preserves_partial_response() {
        let mut s = state();
        s.begin_streaming();
        s.append_stream_delta("partial text ");
        s.begin_abort();
        s.handle_backend(BackendEvent::Error("aborted".into()));
        assert!(s.footer_mode.is_prompt());
        assert!(!s.streaming);
        assert!(s.suppress_rebuild);
        s.scrollback.prepare(&s.theme, 40);
        assert_eq!(s.scrollback.line_count(), 2); // header + partial line
    }

    #[test]
    fn permission_mode_resolves() {
        let mut s = state();
        s.handle_backend(BackendEvent::PermissionRequest {
            prompt: "run command?".into(),
        });
        assert!(matches!(s.footer_mode, FooterMode::Permission { .. }));
        s.resolve_permission(true);
        assert!(s.footer_mode.is_prompt());
    }

    #[test]
    fn question_mode_resolves() {
        let mut s = state();
        s.handle_backend(BackendEvent::Question {
            prompt: "pick?".into(),
            options: vec!["a".into(), "b".into()],
        });
        assert!(matches!(s.footer_mode, FooterMode::Question { .. }));
        s.resolve_question(Some(1));
        assert!(s.footer_mode.is_prompt());
    }
}
