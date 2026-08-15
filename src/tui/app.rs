use crate::config::GlobalConfig;
use crate::tui::backend::{create_backend, Backend, ChatBackend};
use crate::tui::event::{AppEvent, BackendEvent, Focus, KeyAction};
use crate::tui::footer::FooterMode;
use crate::tui::kv::TuiPreferences;
use crate::tui::routes::{home, session};
use crate::tui::state::AppState;
use crate::tui::theme::Theme;
use crate::tui::widgets::{DialogKind, PaletteCommand};
use crate::tui::widgets::render;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub async fn run(config: GlobalConfig, tui_url: Option<String>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let prefs = TuiPreferences::load();
    let theme = Theme::detect();
    let mut state = AppState::new(theme, prefs.clone());
    let mut backend = create_backend(config, tui_url)?;
    let mut backend_rx = backend.subscribe();

    session::sync_from_backend(&mut state, &backend);

    let (input_tx, mut input_rx) = unbounded_channel();
    spawn_input_reader(input_tx);

    let result = run_loop(
        &mut terminal,
        &mut state,
        &mut backend,
        &mut backend_rx,
        &mut input_rx,
    )
    .await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    state.prefs.sidebar_open = state.sidebar_open;
    let _ = state.prefs.save();
    backend.exit()?;
    result
}

fn spawn_input_reader(tx: UnboundedSender<AppEvent>) {
    std::thread::spawn(move || {
        loop {
            match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    if tx.send(AppEvent::Input(key)).is_err() {
                        break;
                    }
                }
                Ok(Event::Mouse(mouse)) => {
                    if tx.send(AppEvent::Mouse(mouse)).is_err() {
                        break;
                    }
                }
                Ok(Event::Resize(w, h)) => {
                    if tx.send(AppEvent::Resize(w, h)).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    backend: &mut Backend,
    backend_rx: &mut UnboundedReceiver<crate::tui::event::BackendEvent>,
    input_rx: &mut UnboundedReceiver<AppEvent>,
) -> Result<()> {
    // ~30fps tick drives the spinner, toasts and the interrupt window.
    let mut tick = tokio::time::interval(Duration::from_millis(33));

    loop {
        terminal.draw(|f| render(f, &mut *state))?;

        tokio::select! {
            _ = tick.tick() => {
                state.tick();
            }
            Some(app_event) = input_rx.recv() => {
                if handle_app_event(state, backend, app_event).await? {
                    break;
                }
            }
            Some(backend_event) = backend_rx.recv() => {
                handle_backend_event(state, backend, backend_event);
            }
        }

        if state.should_quit {
            break;
        }
    }
    Ok(())
}

async fn handle_app_event(
    state: &mut AppState,
    backend: &mut Backend,
    event: AppEvent,
) -> Result<bool> {
    match event {
        AppEvent::Input(key) => {
            if !state.dialog_stack.is_empty() {
                return handle_dialog_input(state, backend, key).await;
            }

            // Footer modal modes consume all keys until answered.
            let permission = matches!(state.footer_mode, FooterMode::Permission { .. });
            let question_len = match &state.footer_mode {
                FooterMode::Question { options, .. } => Some(options.len()),
                _ => None,
            };
            if permission {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => state.resolve_permission(true),
                    KeyCode::Char('n') | KeyCode::Char('N') => state.resolve_permission(false),
                    KeyCode::Esc => state.resolve_permission(false),
                    _ => {}
                }
                return Ok(false);
            }
            if let Some(len) = question_len {
                use crossterm::event::KeyCode;
                match key.code {
                    KeyCode::Char(c) if c.is_ascii_digit() => {
                        let n = c.to_digit(10).unwrap_or(0) as usize;
                        if n >= 1 && n <= len {
                            state.resolve_question(Some(n - 1));
                        }
                    }
                    KeyCode::Esc => state.resolve_question(None),
                    _ => {}
                }
                return Ok(false);
            }

            if state.focus == Focus::Prompt && should_pass_to_textarea(state, key) {
                state.textarea.input(tui_textarea::Input::from(key));
                state.autocomplete.on_input(&state.textarea);
            }

            match state.map_key(key) {
                KeyAction::Quit => return Ok(true),
                KeyAction::Submit => {
                    let text = state.textarea.lines().join("\n");
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        let is_command = trimmed.starts_with('.');
                        state.suppress_rebuild = false;
                        if !is_command {
                            state.push_user_message(text.clone());
                            state.begin_streaming();
                        }
                        reset_textarea(state);
                        backend.send_message(&text).await?;
                    }
                }
                KeyAction::NewLine => {
                    state.textarea.insert_newline();
                    state.autocomplete.on_input(&state.textarea);
                }
                KeyAction::Abort => {
                    backend.abort();
                    state.begin_abort();
                }
                KeyAction::ScrollUp => {
                    state.auto_scroll = false;
                    state.scroll = state.scroll.saturating_sub(1);
                }
                KeyAction::ScrollDown => {
                    state.scroll = state.scroll.saturating_add(1);
                }
                KeyAction::ScrollPageUp => {
                    state.auto_scroll = false;
                    state.scroll = state.scroll.saturating_sub(10);
                }
                KeyAction::ScrollPageDown => {
                    state.scroll = state.scroll.saturating_add(10);
                }
                KeyAction::ScrollTop => {
                    state.auto_scroll = false;
                    state.scroll = 0;
                }
                KeyAction::ScrollBottom => {
                    state.auto_scroll = true;
                    state.scroll_to_bottom();
                }
                KeyAction::ToggleSidebar => {
                    state.sidebar_open = !state.sidebar_open;
                    state.sidebar_lines = backend.sync_sidebar();
                }
                KeyAction::OpenPalette => state.open_palette(),
                KeyAction::OpenSessionList => {
                    let sessions = backend.list_sessions()?;
                    state.sidebar_lines = sessions.iter().map(|s| s.name.clone()).collect();
                    state.open_dialog(DialogKind::SessionList);
                }
                KeyAction::OpenModelPicker => {
                    state.sidebar_lines = backend.list_models();
                    state.open_dialog(DialogKind::ModelPicker);
                }
                KeyAction::OpenRolePicker => {
                    state.sidebar_lines = backend.list_roles();
                    state.open_dialog(DialogKind::RolePicker);
                }
                KeyAction::OpenHelp => state.open_dialog(DialogKind::Help),
                KeyAction::GoHome => home::render_home_hint(state),
                KeyAction::None => {}
            }
        }
        AppEvent::Resize(_, _) => {}
        AppEvent::Mouse(event) => handle_mouse(state, event),
        AppEvent::Backend(event) => handle_backend_event(state, backend, event),
        AppEvent::Tick => state.tick(),
        AppEvent::Quit => return Ok(true),
    }
    Ok(false)
}

fn handle_mouse(state: &mut AppState, event: crossterm::event::MouseEvent) {
    use crossterm::event::MouseEventKind;
    // Wheel scroll with a small per-notch multiplier for a more natural feel.
    const WHEEL_STEP: u16 = 3;
    match event.kind {
        MouseEventKind::ScrollUp => {
            state.auto_scroll = false;
            state.scroll = state.scroll.saturating_sub(WHEEL_STEP);
        }
        MouseEventKind::ScrollDown => {
            state.scroll = state.scroll.saturating_add(WHEEL_STEP);
        }
        _ => {}
    }
}

async fn handle_dialog_input(
    state: &mut AppState,
    backend: &mut Backend,
    key: crossterm::event::KeyEvent,
) -> Result<bool> {
    use crossterm::event::{KeyCode, KeyModifiers};

    let top = state.dialog_stack.top().cloned();
    match top {
        Some(DialogKind::Palette) => match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => state.close_dialog(),
            (KeyCode::Down, _) => {
                let len = state.filtered_palette_commands().len();
                if len > 0 {
                    state.palette_selected = (state.palette_selected + 1) % len;
                }
            }
            (KeyCode::Up, _) => {
                let len = state.filtered_palette_commands().len();
                if len > 0 {
                    state.palette_selected = (state.palette_selected + len - 1) % len;
                }
            }
            (KeyCode::Enter, _) => {
                if let Some(cmd) = state
                    .filtered_palette_commands()
                    .into_iter()
                    .nth(state.palette_selected)
                {
                    state.close_dialog();
                    execute_palette_command(state, backend, &cmd).await?;
                }
            }
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                state.palette_filter.push(c);
                state.palette_selected = 0;
            }
            (KeyCode::Backspace, _) => {
                state.palette_filter.pop();
                state.palette_selected = 0;
            }
            _ => {}
        },
        Some(DialogKind::ShellConfirm { command }) => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                state.show_toast(format!("Would execute: {command}"));
                state.close_dialog();
            }
            _ => state.close_dialog(),
        },
        Some(DialogKind::SessionList)
        | Some(DialogKind::ModelPicker)
        | Some(DialogKind::RolePicker) => {
            let kind = state.dialog_stack.top().cloned();
            match key.code {
                KeyCode::Esc => state.close_dialog(),
                KeyCode::Down => {
                    if !state.sidebar_lines.is_empty() {
                        state.palette_selected =
                            (state.palette_selected + 1) % state.sidebar_lines.len();
                    }
                }
                KeyCode::Up => {
                    if !state.sidebar_lines.is_empty() {
                        state.palette_selected = (state.palette_selected
                            + state.sidebar_lines.len()
                            - 1)
                            % state.sidebar_lines.len();
                    }
                }
                KeyCode::Enter => {
                    if let Some(item) = state.sidebar_lines.get(state.palette_selected) {
                        match kind {
                            Some(DialogKind::SessionList) => {
                                backend.use_session(Some(item))?;
                                state.show_toast(format!("Session: {item}"));
                            }
                            Some(DialogKind::ModelPicker) => {
                                backend.set_model(item)?;
                                state.show_toast(format!("Model: {item}"));
                            }
                            Some(DialogKind::RolePicker) => {
                                backend.set_role(item)?;
                                state.show_toast(format!("Role: {item}"));
                            }
                            _ => {}
                        }
                        state.close_dialog();
                        state.suppress_rebuild = false;
                        session::sync_from_backend(state, backend);
                    }
                }
                _ => {}
            }
        }
        Some(DialogKind::Help) => {
            if matches!(key.code, KeyCode::Esc) {
                state.close_dialog();
            }
        }
        None => {}
    }
    Ok(false)
}

async fn execute_palette_command(
    state: &mut AppState,
    backend: &mut Backend,
    cmd: &PaletteCommand,
) -> Result<()> {
    match cmd.id.as_str() {
        "session.list" => {
            let sessions = backend.list_sessions()?;
            state.sidebar_lines = sessions.iter().map(|s| s.name.clone()).collect();
            state.open_dialog(DialogKind::SessionList);
        }
        "session.new" => home::render_home_hint(state),
        "model.list" => {
            state.sidebar_lines = backend.list_models();
            state.open_dialog(DialogKind::ModelPicker);
        }
        "role.list" => {
            state.sidebar_lines = backend.list_roles();
            state.open_dialog(DialogKind::RolePicker);
        }
        "view.sidebar" => {
            state.sidebar_open = !state.sidebar_open;
            state.sidebar_lines = backend.sync_sidebar();
        }
        "help" => state.open_dialog(DialogKind::Help),
        "quit" => state.should_quit = true,
        _ => {}
    }
    Ok(())
}

fn handle_backend_event(
    state: &mut AppState,
    backend: &Backend,
    event: BackendEvent,
) {
    if matches!(event, BackendEvent::Exit) {
        state.should_quit = true;
        return;
    }
    state.handle_backend(event);
    session::sync_from_backend(state, backend);
}

fn reset_textarea(state: &mut AppState) {
    state.textarea = tui_textarea::TextArea::default();
    state.textarea.set_placeholder_text(
        "Type a message... (Enter submit, Shift+Enter newline, Esc abort, Ctrl+P palette)",
    );
}

fn should_pass_to_textarea(state: &AppState, key: crossterm::event::KeyEvent) -> bool {
    use crossterm::event::{KeyCode, KeyModifiers};

    // While the autocomplete popup is open, Tab / Shift-Tab are reserved for
    // cycling candidates instead of inserting literal tab characters.
    if state.autocomplete.is_open()
        && matches!((key.code, key.modifiers), (KeyCode::Tab, _) | (KeyCode::BackTab, _))
    {
        return false;
    }

    !matches!(
        (key.code, key.modifiers),
        (KeyCode::Char('c'), KeyModifiers::CONTROL)
            | (KeyCode::Char('d'), KeyModifiers::CONTROL)
            | (KeyCode::Char('p'), KeyModifiers::CONTROL)
            | (KeyCode::Char('b'), KeyModifiers::CONTROL)
            | (KeyCode::Char('s'), KeyModifiers::CONTROL)
            | (KeyCode::Char('m'), KeyModifiers::CONTROL)
            | (KeyCode::Char('r'), KeyModifiers::CONTROL)
            | (KeyCode::Char('h'), KeyModifiers::CONTROL)
            | (KeyCode::Char('n'), KeyModifiers::CONTROL)
            | (KeyCode::Enter, KeyModifiers::NONE)
            | (KeyCode::Enter, KeyModifiers::SHIFT)
            | (KeyCode::Esc, _)
    )
}
