use crate::function::ToolCall;
use crossterm::event::{KeyEvent, MouseEvent};

#[derive(Debug, Clone)]
pub enum AppEvent {
    Input(KeyEvent),
    Mouse(MouseEvent),
    Resize,
}

#[derive(Debug, Clone)]
pub enum BackendEvent {
    StreamDelta(String),
    ToolCall(ToolCall),
    Done { text: String },
    Error(String),
    Info(String),
    /// A backend turn needs an explicit approval before continuing.
    /// No backend emits this yet; the plumbing is in place for future ones.
    #[allow(dead_code)]
    PermissionRequest { prompt: String },
    /// A backend turn needs the user to pick one of several options.
    #[allow(dead_code)]
    Question { prompt: String, options: Vec<String> },
    SessionUpdated,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Prompt,
    Messages,
    Dialog,
    Palette,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Quit,
    Submit,
    NewLine,
    Abort,
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollTop,
    ScrollBottom,
    ToggleSidebar,
    OpenPalette,
    OpenSessionList,
    OpenModelPicker,
    OpenRolePicker,
    OpenHelp,
    GoHome,
    None,
}
