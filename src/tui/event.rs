use crate::function::ToolCall;
use crossterm::event::{KeyEvent, MouseEvent};

#[derive(Debug, Clone)]
pub enum AppEvent {
    Input(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Backend(BackendEvent),
    Tick,
    Quit,
}

#[derive(Debug, Clone)]
pub enum BackendEvent {
    StreamDelta(String),
    ToolCall(ToolCall),
    Done { text: String },
    Error(String),
    Info(String),
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
    CloseDialog,
    None,
}
