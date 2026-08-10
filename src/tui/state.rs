use crate::client::{Message, MessageRole};
use crate::function::ToolCall;
use crate::tui::event::{BackendEvent, Focus, KeyAction};
use crate::tui::kv::TuiPreferences;
use crate::tui::theme::Theme;
use crate::tui::widgets::{DialogKind, PaletteCommand};
use tui_textarea::TextArea;

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

    pub fn label(&self) -> &'static str {
        match self.role {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Tool => "tool",
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
    pub messages: Vec<MessageView>,
    pub scroll: u16,
    pub auto_scroll: bool,
    pub streaming: bool,
    pub status: String,
    pub header_left: String,
    pub header_right: String,
    pub sidebar_open: bool,
    pub sidebar_lines: Vec<String>,
    pub dialog: Option<DialogKind>,
    pub palette_filter: String,
    pub palette_selected: usize,
    pub palette_commands: Vec<PaletteCommand>,
    pub toast: Option<Toast>,
    pub should_quit: bool,
    pub textarea: TextArea<'static>,
}

impl AppState {
    pub fn new(theme: Theme, prefs: TuiPreferences) -> Self {
        let mut textarea = TextArea::default();
        textarea.set_placeholder_text("Type a message... (Enter submit, Shift+Enter newline, Esc abort, Ctrl+P palette)");
        Self {
            route: Route::Session,
            theme,
            sidebar_open: prefs.sidebar_open,
            prefs,
            focus: Focus::Prompt,
            messages: vec![],
            scroll: 0,
            auto_scroll: true,
            streaming: false,
            status: String::new(),
            header_left: String::new(),
            header_right: String::new(),
            sidebar_lines: vec![],
            dialog: None,
            palette_filter: String::new(),
            palette_selected: 0,
            palette_commands: PaletteCommand::defaults(),
            toast: None,
            should_quit: false,
            textarea,
        }
    }

    pub fn sync_messages(&mut self, messages: Vec<MessageView>) {
        self.messages = messages;
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn append_stream_delta(&mut self, delta: &str) {
        if let Some(last) = self.messages.last_mut() {
            if last.streaming {
                last.text.push_str(delta);
                if self.auto_scroll {
                    self.scroll_to_bottom();
                }
                return;
            }
        }
        self.messages.push(MessageView {
            role: MessageRole::Assistant,
            text: delta.to_string(),
            streaming: true,
            tool_calls: vec![],
        });
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn finish_stream(&mut self, text: String) {
        if let Some(last) = self.messages.last_mut() {
            if last.streaming {
                last.text = text;
                last.streaming = false;
                return;
            }
        }
        if !text.is_empty() {
            self.messages.push(MessageView {
                role: MessageRole::Assistant,
                text,
                streaming: false,
                tool_calls: vec![],
            });
        }
    }

    pub fn push_user_message(&mut self, text: String) {
        self.messages.push(MessageView {
            role: MessageRole::User,
            text,
            streaming: false,
            tool_calls: vec![],
        });
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
                self.status = "streaming...".into();
                self.append_stream_delta(&delta);
            }
            BackendEvent::ToolCall(call) => {
                if let Some(last) = self.messages.last_mut() {
                    last.tool_calls.push(call);
                }
            }
            BackendEvent::Done { text } => {
                self.streaming = false;
                self.status.clear();
                self.finish_stream(text);
            }
            BackendEvent::Error(err) => {
                self.streaming = false;
                self.status = format!("error: {err}");
                self.show_toast(err);
            }
            BackendEvent::Info(msg) => {
                self.show_toast(msg);
            }
            BackendEvent::Exit => {
                self.should_quit = true;
            }
            BackendEvent::SessionUpdated => {}
        }
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

    pub fn open_palette(&mut self) {
        self.dialog = Some(DialogKind::Palette);
        self.palette_filter.clear();
        self.palette_selected = 0;
        self.focus = Focus::Palette;
    }

    pub fn open_dialog(&mut self, kind: DialogKind) {
        self.dialog = Some(kind);
        self.focus = Focus::Dialog;
    }

    pub fn close_dialog(&mut self) {
        self.dialog = None;
        self.focus = Focus::Prompt;
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

    pub fn map_key(&self, key: crossterm::event::KeyEvent) -> KeyAction {
        use crossterm::event::{KeyCode, KeyModifiers};

        if self.dialog.is_some() {
            return match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => KeyAction::None,
                _ => KeyAction::None,
            };
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => KeyAction::Quit,
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => KeyAction::Quit,
            (KeyCode::Char('q'), KeyModifiers::NONE) if self.focus == Focus::Messages => {
                KeyAction::Quit
            }
            (KeyCode::Esc, _) if self.streaming => KeyAction::Abort,
            (KeyCode::Esc, _) => KeyAction::CloseDialog,
            (KeyCode::Enter, KeyModifiers::NONE) if self.focus == Focus::Prompt => KeyAction::Submit,
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
}
